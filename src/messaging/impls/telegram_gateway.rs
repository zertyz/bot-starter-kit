//! Telegram/`teloxide` setup & integration for message trafficking

use crate::messaging::contracts::messaging::{Dialog, DialogKind, Language, Messaging, Mo, Party};
use crate::models::config::{BotConfig, TelegramIntegrationMode};
use anyhow::{Result, anyhow};
use futures::{Stream, StreamExt};
use log::{debug, error, info};
use ogre_stream_ext::StreamExtCloseOnItemTimeout;
use std::collections::{HashMap, hash_map::Entry};
use std::fmt::{Debug, Display};
use std::future;
use std::pin::Pin;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use teloxide::dispatching::{Dispatcher, UpdateFilterExt};
use teloxide::error_handlers::LoggingErrorHandler;
use teloxide::prelude::{CallbackQuery, Message, Request, ResponseResult, Update};
use teloxide::requests::Payload;
use teloxide::types::{ChatKind, Seconds, User};
use teloxide::{Bot, RequestError, dptree};
use tokio::sync::Mutex;

#[derive(Debug)]
pub enum TelegramMo {
    /// Usual text messages sent by the user
    Message(Box<Message>),
    /// Clicks on [teloxide::types::InlineKeyboardMarkup] buttons
    CallbackQuery(Box<CallbackQuery>),
}

/// Sender used to enqueue MOs for one active dialog processor.
type DialogMoSender<MoPayloadType> = async_channel::Sender<Mo<User, MoPayloadType>>;

/// Stream handed to a processor when a dialog is created.
type DialogMoReceiver<MoPayloadType> = async_channel::Receiver<Mo<User, MoPayloadType>>;

/// Maps every `user_id` to a dialog.
type DialogRoutingTable<MoPayloadType> = Arc<Mutex<HashMap<u64, DialogMoSender<MoPayloadType>>>>;

pub struct TelegramGateway {
    /// The channel that receives each and every Telegram Message -- for all users
    all_users_mo_tx: async_channel::Sender<TelegramMo>,
    /// The aggregator of per-user channels that will receive each user's MO
    per_user_mo_tx: DialogRoutingTable<TelegramMo>,
    bot: Bot,
}

/// One-shot teardown handle for removing a dialog route before closing its channel.
struct DialogCleanupContext<MoPayloadType> {
    dialog_mo_tx_by_user: DialogRoutingTable<MoPayloadType>,
    user_id: u64,
    mo_tx: DialogMoSender<MoPayloadType>,
    closed: Arc<AtomicBool>,
}
impl<MoPayloadType> Clone for DialogCleanupContext<MoPayloadType> {
    // note: using #[derive(Clone)] on this struct is not currently possible, as Rust requires `TelegramMo` to also be `Clone` and we don't want that.
    fn clone(&self) -> Self {
        Self {
            dialog_mo_tx_by_user: self
                .dialog_mo_tx_by_user
                .clone(),
            user_id: self.user_id,
            mo_tx: self
                .mo_tx
                .clone(),
            closed: self
                .closed
                .clone(),
        }
    }
}

/// Context needed to spawn a processor for a newly created dialog between this bot and a user.
struct NewDialogContext<MoPayloadType> {
    dialog_cleanup_context: DialogCleanupContext<MoPayloadType>,
    mo_rx: DialogMoReceiver<MoPayloadType>,
    user: User,
}

/// Routes an MO to the per-user processor, signaling the caller if a new dialog processor task should be created (return is `Some`) or one already exists (return is `None` `).
async fn route_mo<MoPayloadType>(dialog_mo_tx_by_user: &DialogRoutingTable<MoPayloadType>, mo: Mo<User, MoPayloadType>) -> Option<NewDialogContext<MoPayloadType>> {
    route_mo_with_before_new_dialog(dialog_mo_tx_by_user, mo, || future::ready(())).await
}

/// Test-hook variant of [route_mo]; only tests pass a non-no-op callback.
async fn route_mo_with_before_new_dialog<MoPayloadType, BeforeNewDialogFn, BeforeNewDialogFuture>(
    dialog_mo_tx_by_user: &DialogRoutingTable<MoPayloadType>,
    mo: Mo<User, MoPayloadType>,
    before_new_dialog: BeforeNewDialogFn,
) -> Option<NewDialogContext<MoPayloadType>>
where
    BeforeNewDialogFn: Fn() -> BeforeNewDialogFuture,
    BeforeNewDialogFuture: Future<Output = ()>,
{
    let user = mo.sender();
    let user_id = user
        .id
        .0;
    let send_mo = async |mo, mo_tx: &DialogMoSender<MoPayloadType>| {
        info!("Telegram: ALL-USERS-MO-TASK: routing MO");
        mo_tx
            .send(mo)
            .await
            .map_err(|err| {
                error!("Telegram: Bailing out of User's Dialog Processor task: Error routing MO message to user_id #{user_id}'s channel: {err}");
                mo_tx.close();
                err.into_inner()
            })
    };
    /// Routing decision after the dialog map has been updated.
    enum DialogRoute<MoPayloadType> {
        Existing(DialogMoSender<MoPayloadType>),
        New { mo_tx: DialogMoSender<MoPayloadType>, mo_rx: DialogMoReceiver<MoPayloadType> },
    }

    // Clone the sender while holding the map lock, then route the MO after releasing it.
    let route = {
        let mut locked_dialog_mo_tx_by_user = dialog_mo_tx_by_user
            .lock()
            .await;
        match locked_dialog_mo_tx_by_user.entry(user_id) {
            Entry::Occupied(entry) => {
                // A channel already exists for the user. Route the message.
                DialogRoute::Existing(
                    entry
                        .get()
                        .clone(),
                )
            }
            Entry::Vacant(entry) => {
                // No channel exists yet for the user. Create, Store & spawn the Dialog Processor task... and also send the message as above
                let (mo_tx, mo_rx) = async_channel::unbounded();
                entry.insert(mo_tx.clone());
                DialogRoute::New { mo_tx, mo_rx }
            }
        }
    };

    match route {
        DialogRoute::Existing(mo_tx) => {
            _ = send_mo(mo, &mo_tx).await;
            None
        }
        DialogRoute::New { mo_tx, mo_rx } => {
            let user = user.clone();
            before_new_dialog().await;
            _ = send_mo(mo, &mo_tx).await;
            Some(NewDialogContext {
                dialog_cleanup_context: DialogCleanupContext {
                    dialog_mo_tx_by_user: dialog_mo_tx_by_user.clone(),
                    user_id,
                    mo_tx,
                    closed: Arc::new(AtomicBool::new(false)),
                },
                mo_rx,
                user,
            })
        }
    }
}

/// Removes a dialog route before closing its channel.
async fn close_dialog<MoPayloadType>(dialog_cleanup_context: &DialogCleanupContext<MoPayloadType>) -> bool {
    close_dialog_with_before_channel_close(dialog_cleanup_context, || future::ready(())).await
}

/// Test-hook variant of [close_dialog]; only tests pass a non-no-op callback.
async fn close_dialog_with_before_channel_close<MoPayloadType, BeforeChannelCloseFn, BeforeChannelCloseFuture>(
    dialog_cleanup_context: &DialogCleanupContext<MoPayloadType>,
    before_channel_close: BeforeChannelCloseFn,
) -> bool
where
    BeforeChannelCloseFn: FnOnce() -> BeforeChannelCloseFuture,
    BeforeChannelCloseFuture: Future<Output = ()>,
{
    if dialog_cleanup_context
        .closed
        .swap(true, Ordering::SeqCst)
    {
        return false;
    }

    let had_dialog_route = dialog_cleanup_context
        .dialog_mo_tx_by_user
        .lock()
        .await
        .remove(&dialog_cleanup_context.user_id)
        .is_some();

    before_channel_close().await;
    dialog_cleanup_context
        .mo_tx
        .close();
    had_dialog_route
}

/// Upgrades the `mo_stream` to one that auto-closes after no MO arrives within `dialog_idle_timeout`.
fn set_close_on_idle<MoPayloadType: Send + 'static>(
    dialog_cleanup_context: DialogCleanupContext<MoPayloadType>,
    mo_stream: DialogMoReceiver<MoPayloadType>,
    dialog_idle_timeout: Duration,
) -> impl Stream<Item = Mo<User, MoPayloadType>> + Send {
    mo_stream
        .boxed()
        .close_stream_on_item_timeout(dialog_idle_timeout)
        .then(move |mo_timeout_result| {
            let dialog_cleanup = dialog_cleanup_context.clone();
            async move {
                match mo_timeout_result {
                    Ok(mo) => Some(mo),
                    Err(err) => {
                        let user_id = dialog_cleanup.user_id;
                        close_dialog(&dialog_cleanup).await;
                        info!("Telegram: Closing dialog processor for user #{user_id} after {dialog_idle_timeout:?} without receiving MOs: {err}");
                        None
                    }
                }
            }
        })
        .filter_map(future::ready)
}

impl TelegramGateway {
    pub fn new<ProcessorType: UserMoProcessor + Send + Sync + 'static>(config: BotConfig, per_user_mo_processor: ProcessorType) -> (Arc<Self>, tokio::task::JoinHandle<()>) {
        unsafe {
            std::env::set_var(
                "TELOXIDE_TOKEN",
                config
                    .telegram_config
                    .teloxide_token
                    .clone(),
            );
        }
        let bot = Bot::from_env(); // expects TELOXIDE_TOKEN from env -- set above so no external env setting is needed.
        let dialog_processor_idle_timeout = config
            .telegram_config
            .dialog_processor_idle_timeout;

        let (all_users_mo_tx, all_users_mo_rx) = async_channel::bounded(64);
        let instance = Arc::new(Self {
            all_users_mo_tx,
            per_user_mo_tx: Arc::new(Mutex::new(HashMap::new())),
            bot: bot.clone(),
        });

        // spawn the Teloxide gateway
        tokio::spawn({
            info!("Telegram: Starting the Teloxide event loop task");
            let instance_clone = instance.clone();
            async move {
                _ = match &config
                    .telegram_config
                    .integration_mode
                {
                    TelegramIntegrationMode::WebHook { url, secret } => {
                        instance_clone
                            .run_webhook(bot, url, secret)
                            .await
                    }
                    TelegramIntegrationMode::Polling => {
                        instance_clone
                            .run_polling(bot)
                            .await
                    }
                }
                .inspect_err(|err| error!("Telegram loop exited with error: {}", err));
                instance_clone
                    .all_users_mo_tx
                    .close();
                instance_clone
                    .per_user_mo_tx
                    .lock()
                    .await
                    .values()
                    .for_each(|mo_tx| {
                        mo_tx.close();
                    });
                info!("Shutting Down Telegram -- possibly due to operator's request via CTRL-C or SIGTERM");
            }
        });

        let all_users_mo_stream = Self::get_mo_stream(all_users_mo_rx);

        // process the stream to completion with the given concurrency
        let mo_routing_task_concurrency = 4;
        let per_user_mo_processor = Arc::new(per_user_mo_processor);
        let bot = instance
            .bot
            .clone();
        let all_users_mo_tx = instance
            .all_users_mo_tx
            .clone();
        let (all_users_mt_tx, all_users_mt_rx) = async_channel::bounded(64);
        let cloned_all_users_mt_rx = all_users_mt_rx.clone();
        let per_user_mo_tx = instance
            .per_user_mo_tx
            .clone();
        // spawn the MO routing task
        tokio::spawn(async move {
            info!("Telegram: Starting the all-users-to-each-user MO routing task");
            all_users_mo_stream
                .for_each_concurrent(mo_routing_task_concurrency, move |mo| {
                    debug!("Telegram: ALL-USERS-MO-TASK received a message: {mo:?}");
                    let bot = bot.clone();
                    let per_user_mo_processor = per_user_mo_processor.clone();
                    let per_user_mo_tx = per_user_mo_tx.clone();
                    let all_users_mo_tx = all_users_mo_tx.clone();
                    let all_users_mt_tx = all_users_mt_tx.clone();
                    async move {
                        if let Some(new_dialog_context) = route_mo(&per_user_mo_tx, mo).await {
                            let NewDialogContext { dialog_cleanup_context: dialog_cleanup, mo_rx, user } = new_dialog_context;
                            let user_id = dialog_cleanup.user_id;
                            let per_user_mo_processor = per_user_mo_processor.clone();
                            tokio::spawn(async move {
                                info!(
                                    "Telegram: Starting Dialog Processor (and user-to-all-users-mt) tasks for user #{user_id}, named '{}{}'",
                                    user.first_name,
                                    user.last_name
                                        .as_ref()
                                        .map(|last_name| format!(" {last_name}"))
                                        .unwrap_or_default()
                                );
                                let per_user_ttl_mo_stream = set_close_on_idle(dialog_cleanup.clone(), mo_rx, dialog_processor_idle_timeout);
                                let user_mt_stream = per_user_mo_processor
                                    .process(bot, per_user_ttl_mo_stream)
                                    .await;
                                // process 1 message at a time (per user)
                                user_mt_stream
                                    .for_each(|user_mt| async {
                                        let route_result = all_users_mt_tx
                                            .send(user_mt)
                                            .await;
                                        if let Err(err) = route_result {
                                            error!("Telegram: Bailing out of User's Dialog Processor task: Error routing user_id #{user_id}'s MT message to the all-users mt channel: {err}");
                                            all_users_mo_tx.close();
                                        }
                                    })
                                    .await;
                                info!(
                                    "Telegram: Closing Dialog Processor (and user-to-all-users-mt) tasks for user #{user_id}, named '{}{}'",
                                    user.first_name,
                                    user.last_name
                                        .as_ref()
                                        .map(|last_name| format!(" {last_name}"))
                                        .unwrap_or_default()
                                );
                                close_dialog(&dialog_cleanup).await;
                            });
                        }
                    }
                })
                .await;
            info!("Telegram: MO routing task ended -- `all_users_mo_stream` must have finished. Bot is likely shutting down...");
            cloned_all_users_mt_rx.close();
        });
        // spawn the MT sending task
        let all_users_mt_concurrency = 4;
        let task_join_handle = instance.consume_mt_stream(all_users_mt_concurrency, all_users_mt_rx);

        (instance, task_join_handle)
    }

    async fn run_polling(self: &Arc<Self>, bot: Bot) -> anyhow::Result<()> {
        let message_handler = {
            let self_clone = self.clone();
            move |bot: Bot, msg: Message| {
                let self_clone = self_clone.clone();
                async move {
                    self_clone
                        .handler(bot, msg)
                        .await
                }
            }
        };

        let callback_handler = {
            let self_clone = self.clone();
            move |bot: Bot, q: CallbackQuery| {
                let self_clone = self_clone.clone();
                async move {
                    self_clone
                        .on_callback(bot, q)
                        .await
                }
            }
        };

        info!("Telegram: Starting in LONG-POLLING mode");
        let handler = dptree::entry()
            .branch(Update::filter_message().endpoint(message_handler))
            .branch(Update::filter_callback_query().endpoint(callback_handler));
        // .branch(Update::filter_inline_query().endpoint(crate::logic::button_example::inline_query_handler));
        Dispatcher::builder(bot, handler)
            .enable_ctrlc_handler()
            .build()
            .dispatch()
            .await;
        Ok(())
    }

    /// TODO:
    /// 1) research if we have either better performance or better limits by using this MO receiving alternative
    /// 2) then complete this method
    async fn run_webhook(self: &Arc<Self>, bot: Bot, webhook_url: &str, webhook_secret: &str) -> anyhow::Result<()> {
        info!("Telegram: Starting in WEBHOOK mode");
        // WEBHOOK_URL must be public HTTPS: e.g. https://bot.yourdomain.com/webhook/abc123
        let url = if webhook_url
            .trim()
            .is_empty()
        {
            let err = "not present in configuration";
            return Err(anyhow!("WEBHOOK_URL is required in webhook mode: {err}"));
        } else {
            webhook_url
        };
        let addr = ([0, 0, 0, 0], 8443).into(); // local bind; reverse-proxy can front on :443

        // teloxide spins up an Axum server & calls setWebhook for you:
        let listener = teloxide::update_listeners::webhooks::axum(bot.clone(), teloxide::update_listeners::webhooks::Options::new(addr, url.parse()?).secret_token(webhook_secret.to_string()))
            .await
            .map_err(|err| anyhow!("webhook setup failed: {err}"))?;

        info!("Webhook listening; press Ctrl+C to stop");

        let handlers = Update::filter_message()
            .branch(dptree::endpoint({
                let self_clone = self.clone();
                move |bot: Bot, msg: Message| {
                    let self_clone = self_clone.clone();
                    async move {
                        self_clone
                            .handler(bot, msg)
                            .await
                    }
                }
            }))
            .branch(Update::filter_callback_query().endpoint({
                let self_clone = self.clone();
                move |bot: Bot, callback_query: CallbackQuery| {
                    let self_clone = self_clone.clone();
                    async move {
                        self_clone
                            .on_callback(bot, callback_query)
                            .await
                    }
                }
            }));
        Dispatcher::builder(bot, handlers)
            .enable_ctrlc_handler()
            .build()
            .dispatch_with_listener(listener, LoggingErrorHandler::new())
            .await;
        Ok(())
    }

    /// Telegram messages will be delivered by calling this function.
    /// On error -- meaning the channel is full -- we instruct Telegram to try again after some seconds.
    async fn handler(self: &Arc<Self>, _bot: Bot, msg: Message) -> ResponseResult<()> {
        self.all_users_mo_tx
            .send(TelegramMo::Message(Box::new(msg)))
            .await
            .map_err(|_err| RequestError::RetryAfter(Seconds::from_seconds(15)))
    }

    async fn on_callback(self: &Arc<Self>, _bot: Bot, callback_query: CallbackQuery) -> ResponseResult<()> {
        self.all_users_mo_tx
            .send(TelegramMo::CallbackQuery(Box::new(callback_query)))
            .await
            .map_err(|_err| RequestError::RetryAfter(Seconds::from_seconds(15)))
    }

    pub fn bot(self: &Arc<Self>) -> &Bot {
        &self.bot
    }
}

pub trait UserMoProcessor {
    /// per user Stream processor
    fn process<MoStream: Stream<Item = Mo<User, TelegramMo>> + Send>(&self, bot: Bot, user_mo_stream: MoStream) -> impl Future<Output = impl Stream<Item = TelegramBoxSendFuture> + Send> + Send;
}

impl Messaging<User, TelegramMo, TelegramBoxSendFuture> for TelegramGateway {
    fn get_mo_stream(mo_rx: async_channel::Receiver<TelegramMo>) -> impl Stream<Item = Mo<User, TelegramMo>> {
        fn map_kind(teloxide_kind: &ChatKind) -> DialogKind {
            match teloxide_kind {
                ChatKind::Public(_) => DialogKind::Group,
                ChatKind::Private(_) => DialogKind::Private,
            }
        }

        fn map_language(teloxide_language: Option<&String>) -> Language {
            teloxide_language
                .map(|teloxide_language| match teloxide_language.as_str() {
                    "en" => Language::English,
                    "pt" => Language::Portuguese,
                    "" => Language::Unspecified,
                    _ => Language::Unknown,
                })
                .unwrap_or(Language::Unspecified)
        }

        fn map_mo(telegram_mo: TelegramMo) -> Option<Mo<User, TelegramMo>> {
            match &telegram_mo {
                TelegramMo::Message(message) => {
                    let from = message
                        .from
                        .as_ref()?;
                    let id = message
                        .id
                        .0 as u64;
                    let sender = Party::new(from.clone());
                    let kind = map_kind(
                        &message
                            .chat
                            .kind,
                    );
                    let language = map_language(
                        message
                            .from
                            .as_ref()
                            .and_then(|from| {
                                from.language_code
                                    .as_ref()
                            }),
                    );
                    let dialog = Dialog::new(id, kind, language);
                    Some(Mo::new(id, sender, dialog, telegram_mo))
                }
                TelegramMo::CallbackQuery(callback_query) => {
                    let message = callback_query
                        .message
                        .as_ref()
                        .and_then(|message| message.regular_message())?;
                    let id = message
                        .id
                        .0 as u64;
                    let kind = map_kind(
                        &message
                            .chat
                            .kind,
                    );
                    let language = map_language(
                        message
                            .from
                            .as_ref()
                            .and_then(|from| {
                                from.language_code
                                    .as_ref()
                            }),
                    );
                    let dialog = Dialog::new(id, kind, language);
                    let sender = Party::new(
                        callback_query
                            .from
                            .clone(),
                    );
                    Some(Mo::new(id, sender, dialog, telegram_mo))
                }
            }
        }

        mo_rx.filter_map(|telegram_mo| future::ready(map_mo(telegram_mo)))
    }

    fn consume_mt_stream(&self, concurrency: usize, all_users_mt_stream: impl Stream<Item = TelegramBoxSendFuture> + Send + 'static) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            info!("TELEGRAM: Starting all-users-mt sending task");
            all_users_mt_stream
                .for_each_concurrent(concurrency, |mt_future_result| async {
                    _ = mt_future_result
                        .await
                        .inspect_err(|err| error!("TELEGRAM: error processing or sending message #{{mt.id()}}: {err}"))
                        .inspect(|response| debug!("MT! {response}"));
                })
                .await;
            info!("Telegram: MT sending task ended -- `all_users_mt_stream` must have finished. But is, likely, shutting down.");
        })
    }
}

pub type TelegramBoxSendFuture = Pin<Box<dyn Future<Output = Result<String>> + Send + 'static>>;

/// Since Teloxide doesn't provide a single type that would be the root of all MT messages,
/// we do a type erasure to allow that -- and store the `Future` of the sending operation
/// instead of the object to send.
/// The downside is that we loose the MT contents -- and are only able to fetch the result of the send operation.
/// The alternative would be to create such type ourselves, but a lot of duplicated work would be needed.
///
/// See [mts()] if you want to enqueue an async process or the sending of multiple messages
pub fn mt<TeloxideRequest>(request: TeloxideRequest) -> TelegramBoxSendFuture
where
    TeloxideRequest: Request<Err = RequestError> + Send + 'static,
    TeloxideRequest::Payload: Payload + Debug,
    <TeloxideRequest::Payload as Payload>::Output: Send + Debug + 'static,
{
    Box::pin(async move {
        #[cfg(debug_assertions)]
        let payload = format!("{:?}", request.payload_ref());
        request
            .send()
            .await
            .map_err(|err| {
                #[cfg(not(debug_assertions))]
                let msg = format!("Telegram Gateway: error sending MT '{{payload}}': {err}");
                #[cfg(debug_assertions)]
                let msg = format!("Telegram Gateway: error sending MT '{payload}': {err}");
                anyhow!("{msg}")
            })
            .inspect_err(|err| error!("### ERROR: {err}"))
            .inspect_err(|err| log::error!("{err}"))
            .map(|output| format!("{output:?}"))
    })
}

/// Similar to [mt()], but meant to enqueue convoluted async processes or the sending of multiple messages
pub fn mts<OkType: Debug, ErrorType: Into<anyhow::Error> + Display>(process: impl Future<Output = Result<OkType, ErrorType>> + Send + 'static) -> TelegramBoxSendFuture {
    Box::pin(async move {
        process
            .await
            .map_err(|err| anyhow!("Telegram Gateway: error processing or sending MTs: {err}"))
            .inspect_err(|err| error!("### ERROR: {err}"))
            .inspect_err(|err| log::error!("{err}"))
            .map(|answer| format!("{answer:?}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{StreamExt, future::join_all, pin_mut};
    use teloxide::types::UserId;
    use tokio::sync::Barrier;

    /// Makes sure our routing is rock-solid by simulating a surge of messages from the same user.
    /// We want to make sure a single dialog processor per user will be created.
    #[tokio::test]
    async fn concurrent_first_mos_for_same_user_create_one_user_channel() {
        const ROUTED_MESSAGES: u64 = 128;
        const USER_ID: u64 = 42;

        let dialog_mo_tx_by_user = Arc::new(Mutex::new(HashMap::new()));
        let lock_guard = dialog_mo_tx_by_user
            .lock()
            .await;
        let start = Arc::new(Barrier::new(ROUTED_MESSAGES as usize + 1));

        let route_tasks = (0..ROUTED_MESSAGES)
            .map(|message_id| {
                let dialog_mo_tx_by_user = dialog_mo_tx_by_user.clone();
                let start = start.clone();
                tokio::spawn(async move {
                    start
                        .wait()
                        .await;
                    route_mo_with_before_new_dialog(&dialog_mo_tx_by_user, test_mo(USER_ID, message_id), || async {
                        tokio::task::yield_now().await;
                    })
                    .await
                })
            })
            .collect::<Vec<_>>();

        start
            .wait()
            .await;
        for _ in 0..ROUTED_MESSAGES {
            tokio::task::yield_now().await;
        }
        drop(lock_guard);

        let new_dialog_contexts = join_all(route_tasks)
            .await
            .into_iter()
            .filter_map(|join_result| join_result.expect("routing task panicked"))
            .collect::<Vec<_>>();

        assert_eq!(new_dialog_contexts.len(), 1);
        assert_eq!(
            new_dialog_contexts[0]
                .dialog_cleanup_context
                .user_id,
            USER_ID
        );
        assert_eq!(
            new_dialog_contexts[0]
                .mo_rx
                .len(),
            ROUTED_MESSAGES as usize
        );
        assert_eq!(
            dialog_mo_tx_by_user
                .lock()
                .await
                .len(),
            1
        );
    }

    /// Makes sure idle dialog processors are removed from the routing table.
    #[tokio::test]
    async fn idle_dialog_timeout_removes_dialog_route() {
        const USER_ID: u64 = 42;

        let dialog_mo_tx_by_user = Arc::new(Mutex::new(HashMap::new()));
        let NewDialogContext { dialog_cleanup_context: dialog_cleanup, mo_rx, user: _ } = route_mo(&dialog_mo_tx_by_user, test_mo(USER_ID, 1))
            .await
            .expect("first MO should create a dialog context");
        let user_mo_stream = set_close_on_idle(dialog_cleanup, mo_rx, Duration::from_millis(10));
        pin_mut!(user_mo_stream);

        assert_eq!(
            user_mo_stream
                .next()
                .await
                .map(|mo| mo.id()),
            Some(2)
        );
        assert!(
            user_mo_stream
                .next()
                .await
                .is_none()
        );
        assert_eq!(
            dialog_mo_tx_by_user
                .lock()
                .await
                .len(),
            0
        );
    }

    /// Makes sure cleanup removes the route before closing the channel and does not remove a new dialog later.
    #[tokio::test]
    async fn dialog_cleanup_removes_route_before_closing_channel() {
        const USER_ID: u64 = 42;

        let dialog_mo_tx_by_user = Arc::new(Mutex::new(HashMap::new()));
        let NewDialogContext { dialog_cleanup_context: dialog_cleanup, .. } = route_mo(&dialog_mo_tx_by_user, test_mo(USER_ID, 1))
            .await
            .expect("first MO should create a dialog context");
        let new_dialog_context = Arc::new(Mutex::new(None));

        assert!(
            close_dialog_with_before_channel_close(&dialog_cleanup, {
                let dialog_mo_tx_by_user = dialog_mo_tx_by_user.clone();
                let new_dialog_context = new_dialog_context.clone();
                || async move {
                    assert_eq!(
                        dialog_mo_tx_by_user
                            .lock()
                            .await
                            .len(),
                        0
                    );
                    let mut new_dialog_context = new_dialog_context
                        .lock()
                        .await;
                    *new_dialog_context = route_mo(&dialog_mo_tx_by_user, test_mo(USER_ID, 2)).await;
                }
            })
            .await
        );

        assert!(
            dialog_cleanup
                .mo_tx
                .is_closed()
        );
        assert!(!close_dialog(&dialog_cleanup).await);
        let new_dialog_context = new_dialog_context
            .lock()
            .await
            .take()
            .expect("message between route removal and channel close should create a new dialog");
        assert_eq!(
            dialog_mo_tx_by_user
                .lock()
                .await
                .len(),
            1
        );
        assert_eq!(
            new_dialog_context
                .mo_rx
                .len(),
            1
        );
    }

    fn test_user(user_id: u64) -> User {
        User {
            id: UserId(user_id),
            is_bot: false,
            first_name: format!("test-user-{user_id}"),
            last_name: None,
            username: None,
            language_code: None,
            is_premium: false,
            added_to_attachment_menu: false,
        }
    }

    fn test_mo(user_id: u64, message_id: u64) -> Mo<User, ()> {
        Mo::new(message_id + 1, Party::new(test_user(user_id)), Dialog::new(777, DialogKind::Private, Language::English), ())
    }
}
