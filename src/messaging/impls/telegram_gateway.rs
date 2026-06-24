//! Setup for telegram

use crate::messaging::contracts::messaging::{Dialog, DialogKind, Language, Messaging, Mo, Party};
use crate::models::config::BotConfig;
use anyhow::{anyhow, Result};
use futures::{Stream, StreamExt};
use log::{error, info};
use std::fmt::{Debug, Display};
use std::pin::Pin;
use std::sync::Arc;
use std::future;
use std::env;
use teloxide::dispatching::{Dispatcher, UpdateFilterExt};
use teloxide::error_handlers::LoggingErrorHandler;
use teloxide::prelude::{CallbackQuery, Message, Request, ResponseResult, Update};
use teloxide::requests::Payload;
use teloxide::types::{ChatKind, Seconds, User};
use teloxide::{Bot, RequestError, dptree};

#[derive(Debug)]
pub enum TelegramMo {
    /// Usual text messages sent by the user
    Message(Message),
    /// Clicks on [teloxide::types::InlineKeyboardMarkup] buttons
    CallbackQuery(CallbackQuery),
}

pub struct TelegramGateway {
    mo_tx: async_channel::Sender<TelegramMo>,
    bot: Bot,
}

impl TelegramGateway {
    pub fn new(config: BotConfig) -> (Arc<Self>, impl Stream<Item = Mo<User, TelegramMo>>) {
        unsafe {
            std::env::set_var(
                "TELOXIDE_TOKEN",
                config.telegram_config.teloxide_token.clone(),
            );
        }
        let bot = Bot::from_env(); // expects TELOXIDE_TOKEN. How to not involve the environment to pass in this information?
        let mode = env::var("MODE").unwrap_or_else(|_| "polling".into());

        let (mo_tx, mo_rx) = async_channel::bounded(64);
        let instance = Arc::new(Self {
            mo_tx,
            bot: bot.clone(),
        });

        // spawn the Teloxide gateway
        tokio::spawn({
            let instance_clone = instance.clone();
            async move {
                _ = match mode.as_str() {
                    "webhook" => instance_clone.run_webhook(bot).await,
                    _ => instance_clone.run_polling(bot).await,
                }
                .inspect_err(|err| eprintln!("Telegram loop exited with error: {}", err));
            }
        });

        let mo_stream = Self::get_mo_stream(mo_rx);
        (instance, mo_stream)
    }

    async fn run_polling(self: &Arc<Self>, bot: Bot) -> anyhow::Result<()> {
        let message_handler = {
            let self_clone = self.clone();
            move |bot: Bot, msg: Message| {
                let self_clone = self_clone.clone();
                async move { self_clone.handler(bot, msg).await }
            }
        };

        let callback_handler = {
            let self_clone = self.clone();
            move |bot: Bot, q: CallbackQuery| {
                let self_clone = self_clone.clone();
                async move { self_clone.on_callback(bot, q).await }
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
    async fn run_webhook(self: &Arc<Self>, bot: Bot) -> anyhow::Result<()> {
        info!("Telegram: Starting in WEBHOOK mode");
        // WEBHOOK_URL must be public HTTPS: e.g. https://bot.yourdomain.com/webhook/abc123
        let url = env::var("WEBHOOK_URL").expect("WEBHOOK_URL is required in webhook mode");
        let addr = ([0, 0, 0, 0], 8443).into(); // local bind; reverse-proxy can front on :443

        // Optional extra security: a secret header on all webhook calls (setWebhook secret_token)
        let secret = env::var("WEBHOOK_SECRET").unwrap_or_else(|_| "changeme-42".into());

        // teloxide spins up an Axum server & calls setWebhook for you:
        let listener = teloxide::update_listeners::webhooks::axum(
            bot.clone(),
            teloxide::update_listeners::webhooks::Options::new(addr, url.parse()?)
                .secret_token(secret.clone()),
        )
        .await
        .expect("webhook setup failed");

        info!("Webhook listening; press Ctrl+C to stop");

        let handlers = Update::filter_message()
            .branch(dptree::endpoint({
                let self_clone = self.clone();
                move |bot: Bot, msg: Message| {
                    let self_clone = self_clone.clone();
                    async move { self_clone.handler(bot, msg).await }
                }
            }))
            .branch(Update::filter_callback_query().endpoint({
                let self_clone = self.clone();
                move |bot: Bot, callback_query: CallbackQuery| {
                    let self_clone = self_clone.clone();
                    async move { self_clone.on_callback(bot, callback_query).await }
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
        self.mo_tx
            .send(TelegramMo::Message(msg))
            .await
            .map_err(|_err| RequestError::RetryAfter(Seconds::from_seconds(15)))
    }

    async fn on_callback(
        self: &Arc<Self>,
        _bot: Bot,
        callback_query: CallbackQuery,
    ) -> ResponseResult<()> {
        self.mo_tx
            .send(TelegramMo::CallbackQuery(callback_query))
            .await
            .map_err(|_err| RequestError::RetryAfter(Seconds::from_seconds(15)))
    }

    pub fn bot(self: &Arc<Self>) -> &Bot {
        &self.bot
    }
}

impl Messaging<User, TelegramMo, TelegramBoxSendFuture> for TelegramGateway {
    fn get_mo_stream(
        mo_rx: async_channel::Receiver<TelegramMo>,
    ) -> impl Stream<Item = Mo<User, TelegramMo>> {
        fn kind_mapper(teloxide_kind: &ChatKind) -> DialogKind {
            match teloxide_kind {
                ChatKind::Public(_) => DialogKind::Group,
                ChatKind::Private(_) => DialogKind::Private,
            }
        }

        fn language_mapper(teloxide_language: Option<&String>) -> Language {
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
                    let from = message.from.as_ref()?;
                    let id = message.id.0 as u64;
                    let sender = Party::new(from.clone());
                    let dialog = Dialog::new(
                        message.chat.id.0 as u64,
                        kind_mapper(&message.chat.kind),
                        language_mapper(
                            message
                                .from
                                .as_ref()
                                .and_then(|from| from.language_code.as_ref()),
                        ),
                    );
                    Some(Mo::new(id, sender, dialog, telegram_mo))
                }
                TelegramMo::CallbackQuery(callback_query) => {
                    let message = callback_query
                        .message
                        .as_ref()
                        .and_then(|message| message.regular_message())?;
                    let id = message.id.0 as u64;
                    let sender = Party::new(callback_query.from.clone());
                    let dialog = Dialog::new(
                        message.chat.id.0 as u64,
                        kind_mapper(&message.chat.kind),
                        language_mapper(
                            message
                                .from
                                .as_ref()
                                .and_then(|from| from.language_code.as_ref()),
                        ),
                    );
                    Some(Mo::new(id, sender, dialog, telegram_mo))
                }
            }
        }

        mo_rx.filter_map(|telegram_mo| future::ready(map_mo(telegram_mo)))
    }

    fn consume_mt_stream(
        &self,
        concurrency: usize,
        stream: impl Stream<Item = TelegramBoxSendFuture> + Send + 'static,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(
            stream.for_each_concurrent(concurrency, |mt_future_result| async {
                _ = mt_future_result
                    .await
                    .inspect_err(|err| {
                        eprintln!("!!!GOT YOU!!!");
                        eprintln!(
                            "TELEGRAM: error processing or sending message #{{mt.id()}}: {err}"
                        );
                        error!("TELEGRAM: error processing or sending message #{{mt.id()}}: {err}")
                    })
                    .inspect(|response| println!("WE HAVE A RESPONSE! {response}"));
            }),
        )
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
            .inspect_err(|err| eprintln!("### ERROR: {err}"))
            .inspect_err(|err| log::error!("{err}"))
            .map(|output| format!("{output:?}"))
    })
}

/// Similar to [mt()], but meant to enqueue convoluted async processes or the sending of multiple messages
pub fn mts<OkType: Debug, ErrorType: Into<anyhow::Error> + Display> (
    process: impl Future<Output = Result<OkType, ErrorType>> + Send + 'static,
) -> TelegramBoxSendFuture {
    Box::pin(async move {
        process
            .await
            .map_err(|err| anyhow!("Telegram Gateway: error sending MTs: {err}"))
            .inspect_err(|err| eprintln!("### ERROR: {err}"))
            .inspect_err(|err| log::error!("{err}"))
            .map(|answer| format!("{answer:?}"))
    })
}
