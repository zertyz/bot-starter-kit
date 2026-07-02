//! Telegram/`teloxide` setup & integration for message trafficking

use crate::messaging::contracts::messaging::{Dialog, DialogKind, Language, Messaging, Mo, Party};
use crate::messaging::contracts::messaging_platform::MessagingPlatform;
use crate::messaging::user_router::{MessagingPlatformHandleSupplier, UserMoProcessor, UserRouter};
use crate::models::config::{BotConfig, TelegramIntegrationMode};
use anyhow::{Result, anyhow};
use futures::{Stream, StreamExt};
use log::{debug, error, info};
use rustls::{
    ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
};
use std::fmt::{Debug, Display};
use std::future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use teloxide::dispatching::{Dispatcher, UpdateFilterExt};
use teloxide::error_handlers::LoggingErrorHandler;
use teloxide::prelude::{CallbackQuery, Message, Request, ResponseResult, Update};
use teloxide::requests::Payload;
use teloxide::types::{ChatKind, InputFile, Seconds, User};
use teloxide::update_listeners::UpdateListener;
use teloxide::{Bot, RequestError, dptree};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_rustls::{TlsAcceptor, server::TlsStream};
use url::Url;

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
                    TelegramIntegrationMode::WebHook { url, secret, certificate_file, private_key_file } => {
                        instance
                            .run_webhook(bot, url, secret, certificate_file, private_key_file)
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

    async fn run_webhook(self: &Arc<Self>, bot: Bot, webhook_url: &str, webhook_secret: &str, webhook_certificate_file: &str, webhook_private_key_file: &str) -> anyhow::Result<()> {
        info!("Telegram: Starting in WEBHOOK mode");
        // WEBHOOK_URL must be public HTTPS: e.g. https://bot.yourdomain.com/webhook/abc123
        let url = parse_webhook_url(webhook_url)?;
        let addr = webhook_bind_addr(&url)?; // local bind; reverse-proxy can front on :443

        if let Some((certificate_file, private_key_file)) = webhook_tls_files(webhook_certificate_file, webhook_private_key_file)? {
            let tls_config = load_tls_server_config(certificate_file, private_key_file)?;
            let tcp_listener = TcpListener::bind(addr)
                .await
                .map_err(|err| anyhow!("couldn't bind HTTPS webhook listener to {addr}: {err}"))?;
            let options = teloxide::update_listeners::webhooks::Options::new(addr, url)
                .secret_token(webhook_secret.to_string())
                .certificate(InputFile::file(certificate_file.to_string()));
            let (listener, stop_flag, app) = teloxide::update_listeners::webhooks::axum_to_router(bot.clone(), options)
                .await
                .map_err(|err| anyhow!("webhook setup failed: {err}"))?;
            tokio::spawn(async move {
                _ = run_tls_webhook_server(tcp_listener, tls_config, app, stop_flag)
                    .await
                    .inspect_err(|err| error!("HTTPS webhook server exited with error: {err}"));
            });

            info!("HTTPS webhook listening; press Ctrl+C to stop");
            self.dispatch_webhook_listener(bot, listener)
                .await
        } else {
            // teloxide spins up an Axum server & calls setWebhook for you:
            let listener = teloxide::update_listeners::webhooks::axum(bot.clone(), teloxide::update_listeners::webhooks::Options::new(addr, url).secret_token(webhook_secret.to_string()))
                .await
                .map_err(|err| anyhow!("webhook setup failed: {err}"))?;

            info!("Webhook listening; press Ctrl+C to stop");
            self.dispatch_webhook_listener(bot, listener)
                .await
        }
    }

    async fn dispatch_webhook_listener<UListener>(self: &Arc<Self>, bot: Bot, listener: UListener) -> anyhow::Result<()>
    where
        UListener: UpdateListener + Send,
        UListener::Err: Debug,
    {
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

        let handlers = dptree::entry()
            .branch(Update::filter_message().endpoint(message_handler))
            .branch(Update::filter_callback_query().endpoint(callback_handler));
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

        fn map_user(teloxide_user: &User) -> Party<User> {
            let username = teloxide_user
                .username
                .as_deref()
                .unwrap_or_default();
            let first_name = teloxide_user
                .first_name
                .as_str();
            let last_name = teloxide_user
                .last_name
                .as_deref()
                .unwrap_or_default();
            Party::new(
                teloxide_user
                    .id
                    .0,
                teloxide_user.clone(),
            )
            .with_address(username)
            .with_name(format!("{first_name} {last_name}").as_str())
        }

        fn map_mo(telegram_mo: TelegramMo) -> Option<Mo<User, TelegramMo>> {
            match &telegram_mo {
                TelegramMo::Message(message) => {
                    let id = message
                        .id
                        .0 as u64;
                    let from = message
                        .from
                        .as_ref()?;
                    let sender = map_user(from);

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
                    let sender = map_user(&callback_query.from);
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

struct TlsListener {
    listener: TcpListener,
    acceptor: TlsAcceptor,
}

impl axum::serve::Listener for TlsListener {
    type Io = TlsStream<TcpStream>;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            let (stream, addr) = match self
                .listener
                .accept()
                .await
            {
                Ok(accepted) => accepted,
                Err(err) => {
                    error!("HTTPS webhook TCP accept failed: {err}");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
            };

            match self
                .acceptor
                .accept(stream)
                .await
            {
                Ok(stream) => return (stream, addr),
                Err(err) => error!("HTTPS webhook TLS handshake failed for {addr}: {err}"),
            }
        }
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.listener
            .local_addr()
    }
}

fn parse_webhook_url(webhook_url: &str) -> Result<Url> {
    if webhook_url
        .trim()
        .is_empty()
    {
        let err = "not present in configuration";
        return Err(anyhow!("WEBHOOK_URL is required in webhook mode: {err}"));
    }
    let url = webhook_url
        .parse::<Url>()
        .map_err(|err| anyhow!("WEBHOOK_URL is not a valid URL: {err}"))?;
    if url.scheme() != "https" {
        return Err(anyhow!("WEBHOOK_URL must use HTTPS for Telegram webhook mode"));
    }
    let port = url
        .port_or_known_default()
        .ok_or_else(|| anyhow!("WEBHOOK_URL must include a supported Telegram webhook port"))?;
    if !matches!(port, 80 | 88 | 443 | 8443) {
        return Err(anyhow!("WEBHOOK_URL port {port} is not supported by Telegram webhooks; use 80, 88, 443, or 8443"));
    }
    Ok(url)
}

fn webhook_bind_addr(webhook_url: &Url) -> Result<SocketAddr> {
    let port = webhook_url
        .port_or_known_default()
        .ok_or_else(|| anyhow!("WEBHOOK_URL must include a supported Telegram webhook port"))?;
    Ok(([0, 0, 0, 0], port).into())
}

fn webhook_tls_files<'a>(certificate_file: &'a str, private_key_file: &'a str) -> Result<Option<(&'a str, &'a str)>> {
    let certificate_file = certificate_file.trim();
    let certificate_file = if certificate_file.is_empty() { None } else { Some(certificate_file) };
    let private_key_file = private_key_file.trim();
    let private_key_file = if private_key_file.is_empty() { None } else { Some(private_key_file) };

    match (certificate_file, private_key_file) {
        (Some(certificate_file), Some(private_key_file)) => Ok(Some((certificate_file, private_key_file))),
        (None, None) => Ok(None),
        (Some(_), None) => Err(anyhow!("Telegram Gateway: Configuration Error: TELOXIDE_WEBHOOK_PRIVATE_KEY_FILE is required when TELOXIDE_WEBHOOK_CERTIFICATE_FILE is specified")),
        (None, Some(_)) => Err(anyhow!("Telegram Gateway: Configuration Error: TELOXIDE_WEBHOOK_CERTIFICATE_FILE is required when TELOXIDE_WEBHOOK_PRIVATE_KEY_FILE is specified")),
    }
}

async fn run_tls_webhook_server<StopFlag>(tcp_listener: TcpListener, tls_config: ServerConfig, app: axum::Router, stop_flag: StopFlag) -> Result<()>
where
    StopFlag: Future<Output = ()> + Send + 'static,
{
    let listener = TlsListener { listener: tcp_listener, acceptor: TlsAcceptor::from(Arc::new(tls_config)) };

    axum::serve(listener, app)
        .with_graceful_shutdown(stop_flag)
        .await
        .map_err(|err| anyhow!("HTTPS webhook server failed: {err}"))
}

fn load_tls_server_config(certificate_file: &str, private_key_file: &str) -> Result<ServerConfig> {
    let certificate_chain = CertificateDer::pem_file_iter(certificate_file)
        .map_err(|err| anyhow!("couldn't open TLS certificate file '{certificate_file}': {err}"))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|err| anyhow!("couldn't parse TLS certificate file '{certificate_file}': {err}"))?;
    if certificate_chain.is_empty() {
        return Err(anyhow!("TLS certificate file '{certificate_file}' did not contain any certificates"));
    }

    let private_key = PrivateKeyDer::from_pem_file(private_key_file).map_err(|err| anyhow!("couldn't read TLS private key file '{private_key_file}': {err}"))?;

    ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certificate_chain, private_key)
        .map_err(|err| anyhow!("couldn't build TLS server configuration: {err}"))
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

    #[test]
    fn webhook_bind_addr_uses_explicit_url_port() {
        let url = parse_webhook_url("https://bot.example.com:8443/webhook").expect("URL should be accepted");
        assert_eq!(
            webhook_bind_addr(&url)
                .expect("bind address should be derived")
                .port(),
            8443,
            "Explicit webhook URL port should be used as the local bind port"
        );
    }

    #[test]
    fn webhook_bind_addr_uses_https_default_port() {
        let url = parse_webhook_url("https://bot.example.com/webhook").expect("URL should be accepted");
        assert_eq!(
            webhook_bind_addr(&url)
                .expect("bind address should be derived")
                .port(),
            443,
            "HTTPS webhook URLs without explicit ports should bind to 443"
        );
    }

    #[test]
    fn webhook_url_rejects_unsupported_port() {
        let err = parse_webhook_url("https://bot.example.com:3000/webhook").expect_err("Unsupported Telegram webhook port should be rejected");
        assert!(
            err.to_string()
                .contains("not supported by Telegram webhooks"),
            "Unexpected error: {err}"
        );
    }

    #[test]
    fn webhook_url_rejects_plain_http() {
        let err = parse_webhook_url("http://bot.example.com:8443/webhook").expect_err("Plain HTTP webhook URL should be rejected");
        assert!(
            err.to_string()
                .contains("must use HTTPS"),
            "Unexpected error: {err}"
        );
    }
}
