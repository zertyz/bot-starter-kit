//! Telegram/`teloxide` setup & integration for message trafficking

use crate::messaging::contracts::messaging::{Dialog, DialogKind, Language, Messaging, Mo, Party};
use crate::messaging::contracts::messaging_platform::MessagingPlatform;
use crate::messaging::user_router::{MessagingPlatformHandleSupplier, UserMoProcessor, UserRouter};
use crate::models::config::{BotConfig, TelegramIntegrationMode};
use anyhow::{Result, anyhow};
use futures::{Stream, StreamExt};
use log::{debug, error, info};
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
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

#[derive(Debug)]
pub enum TelegramMo {
    /// Usual text messages sent by the user
    Message(Box<Message>),
    /// Clicks on [teloxide::types::InlineKeyboardMarkup] buttons
    CallbackQuery(Box<CallbackQuery>),
}

pub struct TelegramGateway {
    bot: Bot,
    /// The channel that receives every raw Telegram Message -- for all users
    teloxide_mo_producer: async_channel::Sender<TelegramMo>,
    join_handle: Mutex<Option<JoinHandle<()>>>,
}

impl TelegramGateway {
    pub async fn new<ProcessorType: UserMoProcessor<User, Bot, TelegramMo, TelegramBoxSendFuture> + Send + Sync + 'static>(config: BotConfig, per_user_mo_processor: ProcessorType) -> Arc<Self> {
        unsafe {
            std::env::set_var(
                "TELOXIDE_TOKEN",
                config
                    .telegram
                    .teloxide_token
                    .clone(),
            );
        }
        let bot = Bot::from_env(); // expects TELOXIDE_TOKEN from env -- set above so no external env setting is needed.

        let (teloxide_mo_producer, teloxide_stream) = async_channel::bounded(64);
        let instance = Arc::new(Self { bot: bot.clone(), teloxide_mo_producer, join_handle: Mutex::new(None) });

        // The router to manage one Stream and one Dialog Processor task per user
        let user_router = UserRouter::new(&config, MessagingPlatform::Telegram);

        // spawn the Teloxide gateway
        tokio::spawn({
            info!("Telegram Gateway: Starting the Teloxide event loop task");
            let instance = instance.clone();
            let bot = bot.clone();
            async move {
                _ = match &config
                    .telegram
                    .integration_mode
                {
                    TelegramIntegrationMode::WebHook { url, secret } => {
                        instance
                            .run_webhook(bot, url, secret)
                            .await
                    }
                    TelegramIntegrationMode::Polling => {
                        instance
                            .run_polling(bot)
                            .await
                    }
                }
                .inspect_err(|err| error!("Teloxide event-loop task exited with error: {}", err));
                instance
                    .teloxide_mo_producer
                    .close();
                info!("Telegram Gateway: Shutting Down the Teloxide event loop task -- possibly due to operator's request via CTRL-C or SIGTERM");
            }
        });

        // our MO stream, mapped to our internal types
        let telegram_mo_stream = Self::get_mo_stream(teloxide_stream);
        // start the routing task
        let telegram_mt_stream = user_router
            .start(telegram_mo_stream, TelegramHandleSupplier(bot), per_user_mo_processor)
            .await;

        // spawn the MT sending task
        let all_users_mt_concurrency = 4;
        let join_handle = instance.consume_mt_stream(all_users_mt_concurrency, telegram_mt_stream);
        instance
            .join_handle
            .lock()
            .await
            .replace(join_handle);

        instance
    }

    pub async fn await_termination(self: &Arc<Self>) -> Result<()> {
        match self
            .join_handle
            .lock()
            .await
            .take()
        {
            Some(join_handle) => join_handle
                .await
                .map_err(|err| anyhow!("Telegram Gateway MT task ended in ERROR: {err}")),
            None => Err(anyhow!("Telegram Gateway MT task didn't start yet or has already finished")),
        }
    }

    pub fn bot(self: &Arc<Self>) -> &Bot {
        &self.bot
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
        self.teloxide_mo_producer
            .send(TelegramMo::Message(Box::new(msg)))
            .await
            .map_err(|_err| RequestError::RetryAfter(Seconds::from_seconds(15)))
    }

    async fn on_callback(self: &Arc<Self>, _bot: Bot, callback_query: CallbackQuery) -> ResponseResult<()> {
        self.teloxide_mo_producer
            .send(TelegramMo::CallbackQuery(Box::new(callback_query)))
            .await
            .map_err(|_err| RequestError::RetryAfter(Seconds::from_seconds(15)))
    }
}

struct TelegramHandleSupplier(Bot);
impl MessagingPlatformHandleSupplier<Bot> for TelegramHandleSupplier {
    async fn supply(&self) -> Bot {
        self.0
            .clone()
    }
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
                    let sender = Party::new(
                        from.id
                            .0,
                        from.clone(),
                    );
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
                            .id
                            .0,
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
    //use super::*;
}
