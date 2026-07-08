//! WhatsApp Business Cloud API demoscene.
//!
//! This example is meant for the WhatsApp Cloud API test phase: create a Meta
//! app, use Meta's test phone number or your onboarded sender number, add a test
//! recipient, and exchange real messages. It is not a production bot template.
//!
//! Do not paste tokens, app secrets, phone numbers, or real account IDs in this
//! source file. Put test values in your shell environment and rotate them if
//! they were ever committed, shared, or pasted into source.
//!
//! Meta setup map:
//!
//! 1. Go to Meta for Developers -> My Apps -> your app -> WhatsApp -> API setup.
//! 2. Copy "Temporary access token" into `WHATSAPP_ACCESS_TOKEN`. A system-user
//!    token with WhatsApp permissions can replace it later. The app token is not
//!    the token used by this example to send messages.
//! 3. Copy the sender's "Phone number ID" into `WHATSAPP_PHONE_NUMBER_ID`.
//!    This is not the display phone number. "WhatsApp Business Account ID" is
//!    shown in the same area but is not needed by this example.
//! 4. Add and verify your own phone number in the recipient/"To" test area, then
//!    put it in `WHATSAPP_RECIPIENT_PHONE_NUMBER` using E.164 format such as
//!    `+15551234567`.
//! 5. Go to App settings -> Basic. Copy "App ID" into `WHATSAPP_APP_ID` and
//!    "App secret" into `WHATSAPP_APP_SECRET`.
//! 6. Go to WhatsApp -> Configuration. The "Callback URL" is the same value as
//!    `WHATSAPP_WEBHOOK_PUBLIC_URL`. The "Verify token" is a random secret you
//!    choose yourself; use the same value in Meta and in
//!    `WHATSAPP_WEBHOOK_VERIFY_TOKEN`.
//!
//! Required environment for outbound modes:
//!
//! - `WHATSAPP_ACCESS_TOKEN`
//! - `WHATSAPP_PHONE_NUMBER_ID`
//! - `WHATSAPP_RECIPIENT_PHONE_NUMBER`
//!
//! Required environment for webhook modes that receive messages from Meta:
//!
//! - `WHATSAPP_ACCESS_TOKEN`
//! - `WHATSAPP_APP_SECRET`
//! - `WHATSAPP_WEBHOOK_PUBLIC_URL`
//! - `WHATSAPP_WEBHOOK_VERIFY_TOKEN`
//! - `WHATSAPP_WEBHOOK_LISTEN_ADDR`, optional. Defaults to `127.0.0.1:8080`.
//!
//! Required environment for webhook registration modes:
//!
//! - `WHATSAPP_APP_ID`
//!
//! HTTPS and local testing:
//!
//! Meta verifies and delivers webhooks through the public callback URL, and that
//! URL must be HTTPS. The `whatsapp-business-rs` managed server used here binds a
//! local plain HTTP listener. During test development, put a public HTTPS tunnel
//! or reverse proxy in front of it and set `WHATSAPP_WEBHOOK_PUBLIC_URL` to that
//! public URL. The example derives the local route from the public URL path, so
//! the operator only writes the webhook path once. Do not use a self-signed
//! certificate for the Meta-facing URL; use a tunnel/proxy or certificate trusted
//! by public clients.
//!
//! Run:
//!
//! - `cargo run --example whatsapp_demoscene -- send`
//! - `cargo run --example whatsapp_demoscene -- send-batch`
//! - `cargo run --example whatsapp_demoscene -- serve`
//! - `cargo run --example whatsapp_demoscene -- register-webhook`
//! - `cargo run --example whatsapp_demoscene -- serve-and-register`
//!
//! SDK issues found while building this example:
//!
//! 1. `whatsapp-business-rs` keeps token-scope correctness as a runtime concern.
//!    The compiler cannot tell an app token, system-user token, or phone-number
//!    token apart.
//! 2. `ServerBuilder::verify_payload` is opt-in. Without it, webhook POSTs are
//!    easier to spoof in non-local environments. This example requires the app
//!    secret in webhook modes and always enables payload verification.
//! 3. `Handler` methods return `()`, so reply failures cannot be bubbled to the
//!    server loop. This example logs every failed handler-side API call.
//! 4. `ClientBuilder::api_version` requires `&'static str`, so runtime API
//!    version selection is awkward. This example uses the crate default.
//! 5. The README webhook snippet references fields that do not match the 0.5.0
//!    source shape; this example uses `IncomingMessage::message()`.

use anyhow::{Result, anyhow};
use std::{env, net::SocketAddr, time::Duration};
use url::Url;
use whatsapp_business_rs::{
    Client, Fields, Server, WebhookHandler,
    app::SubscriptionField,
    message::{Content, Draft, Media, Message, MessageCreate},
    server::{EventContext, IncomingMessage, MessageUpdate, WabaEvent},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Send,
    SendBatch,
    Serve,
    RegisterWebhook,
    ServeAndRegister,
    Help,
}

struct Config {
    access_token: String,
    phone_number_id: Option<String>,
    recipient_phone_number: Option<String>,
    app_id: Option<String>,
    app_secret: Option<String>,
    webhook_verify_token: Option<String>,
    webhook_public_url: Option<Url>,
    webhook_listen_addr: SocketAddr,
    media_path: Option<String>,
}

#[derive(Clone, Debug)]
struct WhatsAppDemosceneHandler;

impl WebhookHandler for WhatsAppDemosceneHandler {
    async fn handle_message(&self, _ctx: EventContext, incoming: IncomingMessage) {
        let message = incoming.message();
        println!(
            "MO id={} from={} to={} content={:?}",
            message.id,
            message
                .sender
                .phone_id,
            message
                .recipient
                .phone_id,
            message.content
        );

        let response = response_for(message);
        if let Err(err) = incoming
            .reply(response)
            .await
        {
            eprintln!("WhatsApp demoscene: failed to reply to inbound message {}: {err}", message.id);
        }
    }

    async fn handle_message_update(&self, _ctx: EventContext, update: MessageUpdate) {
        println!(
            "MT status id={} status={:?} callback={:?}",
            update
                .message
                .message_id(),
            update.status,
            update.callback()
        );

        for platform_error in &update
            .context
            .errors
        {
            eprintln!(
                "WhatsApp demoscene: Meta status error for {}: {platform_error}",
                update
                    .message
                    .message_id()
            );
        }
    }

    async fn handle_waba_event(&self, _ctx: EventContext, waba_event: WabaEvent) {
        println!("WABA event: {waba_event:?}");
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mode = parse_mode()?;
    if mode == Mode::Help {
        print_usage();
        return Ok(());
    }

    let config = Config::from_env()?;
    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .connect(
            config
                .access_token
                .clone(),
        )
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: building WhatsApp client failed: {err}"))?;

    match mode {
        Mode::Send => send_demos(&config, &client).await,
        Mode::SendBatch => send_batch_demo(&config, &client).await,
        Mode::Serve => serve_webhook(&config, client, false).await,
        Mode::RegisterWebhook => register_webhook(&config, &client).await,
        Mode::ServeAndRegister => serve_webhook(&config, client, true).await,
        Mode::Help => Ok(()),
    }
}

impl Config {
    fn from_env() -> Result<Self> {
        let listen_addr = env_optional("WHATSAPP_WEBHOOK_LISTEN_ADDR")
            .unwrap_or_else(|| "127.0.0.1:8080".to_owned())
            .parse::<SocketAddr>()
            .map_err(|err| anyhow!("WHATSAPP_WEBHOOK_LISTEN_ADDR must be a socket address such as 127.0.0.1:8080: {err}"))?;

        Ok(Self {
            access_token: env_required("WHATSAPP_ACCESS_TOKEN")?,
            phone_number_id: env_optional("WHATSAPP_PHONE_NUMBER_ID"),
            recipient_phone_number: env_optional("WHATSAPP_RECIPIENT_PHONE_NUMBER"),
            app_id: env_optional("WHATSAPP_APP_ID"),
            app_secret: env_optional("WHATSAPP_APP_SECRET"),
            webhook_verify_token: env_optional("WHATSAPP_WEBHOOK_VERIFY_TOKEN"),
            webhook_public_url: parse_public_webhook_url("WHATSAPP_WEBHOOK_PUBLIC_URL")?,
            webhook_listen_addr: listen_addr,
            media_path: env_optional("WHATSAPP_DEMO_MEDIA_PATH"),
        })
    }

    fn outbound(&self) -> Result<OutboundConfig<'_>> {
        Ok(OutboundConfig {
            phone_number_id: self
                .phone_number_id
                .as_deref()
                .ok_or_else(|| anyhow!("WHATSAPP_PHONE_NUMBER_ID is required for outbound demo modes"))?,
            recipient_phone_number: self
                .recipient_phone_number
                .as_deref()
                .ok_or_else(|| anyhow!("WHATSAPP_RECIPIENT_PHONE_NUMBER is required for outbound demo modes"))?,
        })
    }

    fn app_secret(&self) -> Result<&str> {
        self.app_secret
            .as_deref()
            .ok_or_else(|| anyhow!("WHATSAPP_APP_SECRET is required for webhook modes so payload signatures can be verified"))
    }

    fn webhook_verify_token(&self) -> Result<&str> {
        self.webhook_verify_token
            .as_deref()
            .ok_or_else(|| anyhow!("WHATSAPP_WEBHOOK_VERIFY_TOKEN is required for webhook modes"))
    }

    fn webhook_public_url(&self) -> Result<&Url> {
        self.webhook_public_url
            .as_ref()
            .ok_or_else(|| anyhow!("WHATSAPP_WEBHOOK_PUBLIC_URL is required for webhook modes"))
    }

    fn webhook_registration(&self) -> Result<WebhookRegistrationConfig<'_>> {
        Ok(WebhookRegistrationConfig {
            app_id: self
                .app_id
                .as_deref()
                .ok_or_else(|| anyhow!("WHATSAPP_APP_ID is required for webhook registration"))?,
            verify_token: self.webhook_verify_token()?,
            public_url: self.webhook_public_url()?,
        })
    }

    fn webhook_route(&self) -> Result<String> {
        Ok(self
            .webhook_public_url()?
            .path()
            .to_owned())
    }
}

struct OutboundConfig<'a> {
    phone_number_id: &'a str,
    recipient_phone_number: &'a str,
}

struct WebhookRegistrationConfig<'a> {
    app_id: &'a str,
    verify_token: &'a str,
    public_url: &'a Url,
}

async fn send_demos(config: &Config, client: &Client) -> Result<()> {
    let outbound = config.outbound()?;
    let sender = client.message(outbound.phone_number_id);

    let text = sender
        .send(
            outbound.recipient_phone_number,
            Draft::text("OgreRobot WhatsApp Demoscene: text message with link preview disabled.")
                .preview_url(false)
                .with_biz_opaque_callback_data("whatsapp-demoscene:text"),
        )
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: sending text demo failed: {err}"))?;
    print_sent("text", &text);

    let menu = sender
        .send(outbound.recipient_phone_number, menu_draft())
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: sending quick-reply menu failed: {err}"))?;
    print_sent("quick replies", &menu);

    let list = sender
        .send(outbound.recipient_phone_number, list_draft())
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: sending list demo failed: {err}"))?;
    print_sent("list", &list);

    let cta = sender
        .send(outbound.recipient_phone_number, cta_draft())
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: sending CTA demo failed: {err}"))?;
    print_sent("cta", &cta);

    let location = sender
        .send(outbound.recipient_phone_number, location_draft())
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: sending location demo failed: {err}"))?;
    print_sent("location", &location);

    if let Some(media_path) = &config.media_path {
        let media = Media::from_path(media_path)
            .await
            .map_err(|err| anyhow!("WhatsApp demoscene: loading media from {media_path:?} failed: {err}"))?
            .caption("OgreRobot WhatsApp Demoscene: media loaded from WHATSAPP_DEMO_MEDIA_PATH.");
        let media = sender
            .send(outbound.recipient_phone_number, Draft::media(media).with_biz_opaque_callback_data("whatsapp-demoscene:media"))
            .await
            .map_err(|err| anyhow!("WhatsApp demoscene: sending media demo failed: {err}"))?;
        print_sent("media", &media);
    } else {
        println!("media: skipped because WHATSAPP_DEMO_MEDIA_PATH is not set");
    }

    Ok(())
}

async fn send_batch_demo(config: &Config, client: &Client) -> Result<()> {
    let outbound = config.outbound()?;
    let output = client
        .batch()
        .include(
            client
                .message(outbound.phone_number_id)
                .send(outbound.recipient_phone_number, Draft::text("OgreRobot WhatsApp Demoscene batch: first message.")),
        )
        .include(
            client
                .message(outbound.phone_number_id)
                .send(outbound.recipient_phone_number, Draft::text("OgreRobot WhatsApp Demoscene batch: second message.")),
        )
        .execute()
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: batch request failed: {err}"))?;

    let (first, second) = output
        .flatten()
        .map_err(|err| anyhow!("WhatsApp demoscene: parsing batch response failed: {err}"))?;
    print_batch_result("batch first", first)?;
    print_batch_result("batch second", second)?;
    Ok(())
}

async fn register_webhook(config: &Config, client: &Client) -> Result<()> {
    let registration = config.webhook_registration()?;
    client
        .app(registration.app_id)
        .configure_webhook((
            registration
                .verify_token
                .to_owned(),
            registration
                .public_url
                .as_str()
                .to_owned(),
        ))
        .events(
            Fields::new()
                .with(SubscriptionField::Messages)
                .with(SubscriptionField::MessageTemplateStatusUpdate)
                .with(SubscriptionField::AccountUpdate),
        )
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: registering webhook failed: {err}"))?;
    println!("webhook registered for {}", registration.public_url);
    Ok(())
}

async fn serve_webhook(config: &Config, client: Client, register: bool) -> Result<()> {
    let verify_token = config
        .webhook_verify_token()?
        .to_owned();
    let app_secret = config
        .app_secret()?
        .to_owned();
    let public_url = config.webhook_public_url()?;
    let route = config.webhook_route()?;
    let builder = Server::builder()
        .endpoint(config.webhook_listen_addr)
        .route(route.clone())
        .verify_token(verify_token)
        .verify_payload(app_secret);

    println!("serving local WhatsApp webhook on http://{}{}", config.webhook_listen_addr, route);
    println!("expecting Meta to call public HTTPS URL {public_url}");

    let serve = builder
        .build()
        .serve(WhatsAppDemosceneHandler, client.clone());

    if register {
        let registration = config.webhook_registration()?;
        serve
            .register_webhook(
                client
                    .app(registration.app_id)
                    .configure_webhook((
                        registration
                            .verify_token
                            .to_owned(),
                        registration
                            .public_url
                            .as_str()
                            .to_owned(),
                    ))
                    .events(Fields::new().with(SubscriptionField::Messages)),
            )
            .await
            .map_err(|err| anyhow!("WhatsApp demoscene: serving and registering webhook failed: {err}"))
    } else {
        serve
            .await
            .map_err(|err| anyhow!("WhatsApp demoscene: webhook server failed: {err}"))
    }
}

fn response_for(message: &Message) -> Draft {
    let command = message
        .content
        .button_click()
        .and_then(|button| button.callback_id())
        .or_else(|| {
            message
                .content
                .text()
        })
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();

    match command.as_str() {
        "/start" | "start" | "help" | "menu" | "demo:menu" => menu_draft(),
        "demo:list" | "list" => list_draft(),
        "demo:location" | "location" => location_draft(),
        "demo:cta" | "cta" => cta_draft(),
        "demo:inspect" | "inspect" => inspect_message_draft(message),
        "demo:echo" | "echo" => Draft::text("Send any text and this demoscene will echo the primary text field."),
        _ => echo_or_menu_draft(message),
    }
}

fn menu_draft() -> Draft {
    Draft::new()
        .body("OgreRobot WhatsApp Demoscene")
        .footer("Pick a WhatsApp feature.")
        .add_reply_button("demo:list", "List")
        .add_reply_button("demo:location", "Location")
        .add_reply_button("demo:inspect", "Inspect")
        .with_biz_opaque_callback_data("whatsapp-demoscene:menu")
}

fn list_draft() -> Draft {
    Draft::new()
        .body("Choose a WhatsApp Business feature to inspect.")
        .footer("The selected row is delivered back as an interactive webhook payload.")
        .list("Features")
        .add_list_section("Demos")
        .add_list_option("demo:menu", "Quick replies", "Buttons using Draft::add_reply_button")
        .add_list_option("demo:location", "Location", "Latitude/longitude with a name and address")
        .add_list_option("demo:cta", "CTA URL", "A native call-to-action link button")
        .with_biz_opaque_callback_data("whatsapp-demoscene:list")
}

fn cta_draft() -> Draft {
    Draft::new()
        .body("Open the SDK repository used by this demoscene.")
        .footer("CTA buttons do not post a callback; the client opens the URL.")
        .with_cta_url("https://github.com/veecore/whatsapp-business-rs", "Open SDK")
        .with_biz_opaque_callback_data("whatsapp-demoscene:cta")
}

fn location_draft() -> Draft {
    Draft::location(-23.55052, -46.633308)
        .location_name("Sao Paulo")
        .location_address("Sao Paulo, SP, Brazil")
        .with_biz_opaque_callback_data("whatsapp-demoscene:location")
}

fn inspect_message_draft(message: &Message) -> Draft {
    let primary_text = message
        .content
        .text()
        .unwrap_or("<no primary text>");
    let content_kind = match &message.content {
        Content::Text(_) => "text",
        Content::Media(_) => "media",
        Content::Reaction(_) => "reaction",
        Content::Location(_) => "location",
        Content::Interactive(_) => "interactive",
        Content::Order(_) => "order",
        Content::Error(_) => "error",
    };

    Draft::text(format!(
        "Message id: {}\nFrom: {}\nType: {}\nPrimary text: {}",
        message.id,
        message
            .sender
            .phone_id,
        content_kind,
        primary_text
    ))
}

fn echo_or_menu_draft(message: &Message) -> Draft {
    if let Some(text) = message
        .content
        .text()
    {
        Draft::text(format!("Echo: {text}\n\nSend `menu` for feature buttons.")).preview_url(false)
    } else {
        Draft::text("Received a non-text message. Send `menu` for feature buttons.")
    }
}

fn print_sent(label: &str, metadata: &MessageCreate) {
    println!("{label}: sent id={} status={:?}", metadata.message_id(), metadata.message_status);
}

fn print_batch_result(label: &str, result: Result<MessageCreate, whatsapp_business_rs::Error>) -> Result<()> {
    let metadata = result.map_err(|err| anyhow!("WhatsApp demoscene: {label} failed inside batch: {err}"))?;
    print_sent(label, &metadata);
    Ok(())
}

fn parse_mode() -> Result<Mode> {
    let mode = match env::args()
        .nth(1)
        .as_deref()
    {
        Some("send") => Mode::Send,
        Some("send-batch") => Mode::SendBatch,
        Some("serve") => Mode::Serve,
        Some("register-webhook") => Mode::RegisterWebhook,
        Some("serve-and-register") => Mode::ServeAndRegister,
        Some("help") | Some("--help") | Some("-h") | None => Mode::Help,
        Some(other) => return Err(anyhow!("unknown mode {other:?}; run with `help` for usage")),
    };
    Ok(mode)
}

fn print_usage() {
    println!("Usage: cargo run --example whatsapp_demoscene -- <send|send-batch|serve|register-webhook|serve-and-register>");
}

fn env_required(name: &str) -> Result<String> {
    env::var(name).map_err(|err| anyhow!("{name} is required: {err}"))
}

fn env_optional(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| {
            value
                .trim()
                .to_owned()
        })
        .filter(|value| !value.is_empty())
}

fn parse_public_webhook_url(name: &str) -> Result<Option<Url>> {
    let Some(raw_url) = env_optional(name) else {
        return Ok(None);
    };
    let url = Url::parse(&raw_url).map_err(|err| anyhow!("{name} must be a full HTTPS URL such as https://example.ngrok-free.app/whatsapp: {err}"))?;

    if url.scheme() != "https" {
        return Err(anyhow!("{name} must use HTTPS because Meta verifies and delivers webhooks through HTTPS callback URLs"));
    }
    if url
        .host_str()
        .is_none()
    {
        return Err(anyhow!("{name} must include a public host name"));
    }
    if url
        .query()
        .is_some()
        || url
            .fragment()
            .is_some()
    {
        return Err(anyhow!("{name} must not include query parameters or a fragment; use only the path as the webhook route"));
    }

    Ok(Some(url))
}
