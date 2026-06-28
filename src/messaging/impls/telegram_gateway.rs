//! Telegram/`teloxide` setup & integration for message trafficking

use crate::messaging::contracts::messaging::{Dialog, DialogKind, Language, Messaging, Mo, Party};
use crate::models::config::{BotConfig, TelegramIntegrationMode};
use anyhow::{Result, anyhow};
use futures::{Stream, StreamExt};
use log::{debug, error, info};
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::future;
use std::pin::Pin;
use std::sync::Arc;
use teloxide::dispatching::{Dispatcher, UpdateFilterExt};
use teloxide::error_handlers::LoggingErrorHandler;
use teloxide::prelude::{CallbackQuery, Message, Request, ResponseResult, Update};
use teloxide::requests::Payload;
use teloxide::types::{ChatKind, Seconds, User};
use teloxide::{Bot, RequestError, dptree};
use tokio::sync::RwLock;

#[derive(Debug)]
pub enum TelegramMo {
    /// Usual text messages sent by the user
    Message(Box<Message>),
    /// Clicks on [teloxide::types::InlineKeyboardMarkup] buttons
    CallbackQuery(Box<CallbackQuery>),
}

pub struct TelegramGateway {
    all_users_mo_tx: async_channel::Sender<TelegramMo>,
    per_user_mo_tx: Arc<RwLock<HashMap<u64, async_channel::Sender<Mo<User, TelegramMo>>>>>,
    bot: Bot,
}

impl TelegramGateway {
    pub fn new<ProcessorType: UserMoProcessor + Send + Sync + 'static>(config: BotConfig, user_mo_processor: ProcessorType) -> (Arc<Self>, tokio::task::JoinHandle<()>) {
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

        let (all_users_mo_tx, all_users_mo_rx) = async_channel::bounded(64);
        let instance = Arc::new(Self {
            all_users_mo_tx,
            per_user_mo_tx: Arc::new(RwLock::new(HashMap::new())),
            bot: bot.clone(),
        });

        // spawn the Teloxide gateway
        tokio::spawn({
            #[cfg(debug_assertions)]
            debug!("Telegram: Starting the Teloxide task");
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
                    .read()
                    .await
                    .values()
                    .for_each(|user_mo_tx| {
                        user_mo_tx.close();
                    });
                info!("Shutting Down Telegram -- possibly due to operator's request via CTRL-C or SIGTERM");
            }
        });

        let all_users_mo_stream = Self::get_mo_stream(all_users_mo_rx);

        // process the stream to completion with the given concurrency
        let mo_routing_task_concurrency = 4;
        let user_mo_processor = Arc::new(user_mo_processor);
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
            #[cfg(debug_assertions)]
            debug!("Telegram: Starting the all-users-to-each-user MO routing task");
            all_users_mo_stream
                .for_each_concurrent(mo_routing_task_concurrency, move |mo| {
                    #[cfg(debug_assertions)]
                    debug!("Telegram: ALL-USERS-MO-TASK: {mo:?}");
                    let bot = bot.clone();
                    let user_mo_processor = user_mo_processor.clone();
                    let per_user_mo_tx = per_user_mo_tx.clone();
                    let all_users_mo_tx = all_users_mo_tx.clone();
                    let all_users_mt_tx = all_users_mt_tx.clone();
                    async move {
                        let user = mo
                            .sender()
                            .clone();
                        let user_id = user
                            .id
                            .0;
                        let route_mo = async |mo, user_mo_tx: &async_channel::Sender<Mo<User, TelegramMo>>| {
                            #[cfg(debug_assertions)]
                            debug!("Telegram: ALL-USERS-MO-TASK: routing MO");
                            let route_result = user_mo_tx
                                .send(mo)
                                .await;
                            if let Err(err) = route_result {
                                error!("Telegram: Bailing out of User's Dialog Processor task: Error routing MO message to user_id #{user_id}'s channel: {err}");
                                user_mo_tx.close();
                            }
                        };
                        let locked_per_user_mo_tx = per_user_mo_tx
                            .read()
                            .await;
                        match locked_per_user_mo_tx.get(&user_id) {
                            Some(user_mo_tx) => {
                                // A channel already exist for the user. Route the message.
                                route_mo(mo, user_mo_tx).await;
                            }
                            None => {
                                // No channel exists yet for the user. Create, Store & spawn the Dialog Processor task... and also send the message as above
                                let (user_mo_tx, user_mo_rx) = async_channel::unbounded();
                                route_mo(mo, &user_mo_tx).await;
                                drop(locked_per_user_mo_tx);
                                per_user_mo_tx
                                    .write()
                                    .await
                                    .insert(user_id, user_mo_tx);
                                let user_mo_processor = user_mo_processor.clone();
                                tokio::spawn(async move {
                                    #[cfg(debug_assertions)]
                                    debug!("Telegram: Starting Dialog Processor (and user-to-all-users-mt) tasks for user #{user_id}");
                                    let user_mt_stream = user_mo_processor
                                        .process(bot, user, user_mo_rx)
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
                                });
                            }
                        }
                    }
                })
                .await;
            info!("Telegram: MO routing task ended -- `all_users_mo_stream` must have finished.");
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
    fn process<MoStream: Stream<Item = Mo<User, TelegramMo>> + Send>(
        &self,
        bot: Bot,
        user: User,
        user_mo_stream: MoStream,
    ) -> impl Future<Output = impl Stream<Item = TelegramBoxSendFuture> + Send> + Send;
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
            debug!("TELEGRAM: Starting all-users-mt sending task");
            all_users_mt_stream
                .for_each_concurrent(concurrency, |mt_future_result| async {
                    _ = mt_future_result
                        .await
                        .inspect_err(|err| {
                            error!("!!!GOT YOU!!!");
                            error!("TELEGRAM: error processing or sending message #{{mt.id()}}: {err}")
                        })
                        .inspect(|response| debug!("WE HAVE A RESPONSE! {response}"));
                })
                .await;
            info!("Telegram: MT sending task ended -- `all_users_mt_stream` must have finished.");
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
            .map_err(|err| anyhow!("Telegram Gateway: error sending MTs: {err}"))
            .inspect_err(|err| error!("### ERROR: {err}"))
            .inspect_err(|err| log::error!("{err}"))
            .map(|answer| format!("{answer:?}"))
    })
}
