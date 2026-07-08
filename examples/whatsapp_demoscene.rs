//! WhatsApp Business Cloud API demoscene.
//! +5521991234899
//! 1617456909739506
//! a3309c8e1b1dc2e48b3a88288d9c3435
//!
//! token_de_app: 1617456909739506|hAMXJcW5KY5eBwxOsYITZxoCass
//! token_de_usuário: EAAWZCEYiLZAfIBRZBZAJrLbVPe6BUixxZAgrvbpB69hlNY7Holk9yFzQHh4im8bqn3f5K1dfmZB64nV76jnEFeqR2g8TwH55i7JL4HUhplZCztUl80Uzfd8csvXEOdCmL2YXvaUVpDfVjQMZBL7Qm3Y9ds4czmTRjN244E0c9ZBeIDPuObwM7yGg2s0Yf01xNmgx3spjo37fjoMfZCVZA46EJ3gx56ZCOp2kQI0pt7OOM5NGl7esIsNj7k1d2sdMG1G01snfqrZBASFjHWP1fUiJKmWhn9QZDZD
//!
//! From Use Cases, 1: experiment
//! Test phone number: +1 (555) 620-2558
//! Phone Number ID: 1190399224161359
//! Whatsapp Business Account ID: 2399533690534984
//! Access Token: EAAWZCEYiLZAfIBR1ytc4vutU3FTPHDkTkkWyiiaUPicXkcVVZBPUOPX83hlemzSC0RSha3mDxKHoCc4URJO4Xruc2hsQs0wtGUMutRNNNZBRR1tnupwGtf80QmHUUAk0i3GfgVDBADvpMfTb5nFLZCx3O48e0lk15Sz2SHrvfuM65v921J7qICirtju5cpSc54AQWXVgg5QhHFcbQ2eZAiFB5Cgp1UjXFktdd14RgqQaLMy23td3aoIBNoCTEmiRoQaixjDGNB69Nc4F8lAxD7ChUZD
//!
//! Meta setup:
//!
//! 1. Create or open a Meta app with the WhatsApp product:
//!    https://developers.facebook.com/docs/whatsapp/cloud-api/get-started
//! 2. In the WhatsApp product, collect the temporary or system-user access token,
//!    the Phone Number ID, and the App ID/App Secret.
//! 3. Add your test recipient while using the Meta test number, or use a real
//!    approved business number when you leave development mode.
//! 4. For webhook modes, expose this example through a public HTTPS URL and use
//!    that exact URL in `WHATSAPP_WEBHOOK_PUBLIC_URL`.
//!
//! Required environment for outbound modes:
//!
//! - `WHATSAPP_ACCESS_TOKEN`
//! - `WHATSAPP_BUSINESS_PHONE_NUMBER_ID`
//! - `WHATSAPP_TO`
//!
//! Required environment for webhook modes:
//!
//! - `WHATSAPP_ACCESS_TOKEN`
//! - `WHATSAPP_WEBHOOK_VERIFY_TOKEN`
//! - `WHATSAPP_APP_SECRET`, optional for local trials but required to validate
//!   Meta signatures.
//! - `WHATSAPP_WEBHOOK_LISTEN_ADDR`, optional. Defaults to `127.0.0.1:8080`.
//! - `WHATSAPP_WEBHOOK_ROUTE`, optional. Defaults to `/whatsapp`.
//!
//! Required environment for webhook registration modes:
//!
//! - `WHATSAPP_APP_ID`
//! - `WHATSAPP_WEBHOOK_PUBLIC_URL`
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
//!    easier to spoof in non-local environments.
//! 3. `Handler` methods return `()`, so reply failures cannot be bubbled to the
//!    server loop. This example logs every failed handler-side API call.
//! 4. `ClientBuilder::api_version` requires `&'static str`, so runtime API
//!    version selection is awkward. This example uses the crate default.
//! 5. The README webhook snippet references fields that do not match the 0.5.0
//!    source shape; this example uses `IncomingMessage::message()`.

use anyhow::{Result, anyhow};
use std::{env, net::SocketAddr, time::Duration};
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
    business_phone_number_id: Option<String>,
    recipient_phone_number: Option<String>,
    app_id: Option<String>,
    app_secret: Option<String>,
    webhook_verify_token: Option<String>,
    webhook_public_url: Option<String>,
    webhook_listen_addr: SocketAddr,
    webhook_route: String,
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
            business_phone_number_id: env_optional("WHATSAPP_BUSINESS_PHONE_NUMBER_ID"),
            recipient_phone_number: env_optional("WHATSAPP_TO"),
            app_id: env_optional("WHATSAPP_APP_ID"),
            app_secret: env_optional("WHATSAPP_APP_SECRET"),
            webhook_verify_token: env_optional("WHATSAPP_WEBHOOK_VERIFY_TOKEN"),
            webhook_public_url: env_optional("WHATSAPP_WEBHOOK_PUBLIC_URL"),
            webhook_listen_addr: listen_addr,
            webhook_route: env_optional("WHATSAPP_WEBHOOK_ROUTE").unwrap_or_else(|| "/whatsapp".to_owned()),
            media_path: env_optional("WHATSAPP_DEMO_MEDIA_PATH"),
        })
    }

    fn outbound(&self) -> Result<OutboundConfig<'_>> {
        Ok(OutboundConfig {
            business_phone_number_id: self
                .business_phone_number_id
                .as_deref()
                .ok_or_else(|| anyhow!("WHATSAPP_BUSINESS_PHONE_NUMBER_ID is required for outbound demo modes"))?,
            recipient_phone_number: self
                .recipient_phone_number
                .as_deref()
                .ok_or_else(|| anyhow!("WHATSAPP_TO is required for outbound demo modes"))?,
        })
    }

    fn webhook_verify_token(&self) -> Result<&str> {
        self.webhook_verify_token
            .as_deref()
            .ok_or_else(|| anyhow!("WHATSAPP_WEBHOOK_VERIFY_TOKEN is required for webhook modes"))
    }

    fn webhook_registration(&self) -> Result<WebhookRegistrationConfig<'_>> {
        Ok(WebhookRegistrationConfig {
            app_id: self
                .app_id
                .as_deref()
                .ok_or_else(|| anyhow!("WHATSAPP_APP_ID is required for webhook registration"))?,
            verify_token: self.webhook_verify_token()?,
            public_url: self
                .webhook_public_url
                .as_deref()
                .ok_or_else(|| anyhow!("WHATSAPP_WEBHOOK_PUBLIC_URL is required for webhook registration"))?,
        })
    }
}

struct OutboundConfig<'a> {
    business_phone_number_id: &'a str,
    recipient_phone_number: &'a str,
}

struct WebhookRegistrationConfig<'a> {
    app_id: &'a str,
    verify_token: &'a str,
    public_url: &'a str,
}

async fn send_demos(config: &Config, client: &Client) -> Result<()> {
    let outbound = config.outbound()?;
    let sender = client.message(outbound.business_phone_number_id);

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
                .message(outbound.business_phone_number_id)
                .send(outbound.recipient_phone_number, Draft::text("OgreRobot WhatsApp Demoscene batch: first message.")),
        )
        .include(
            client
                .message(outbound.business_phone_number_id)
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
    let mut builder = Server::builder()
        .endpoint(config.webhook_listen_addr)
        .route(
            config
                .webhook_route
                .clone(),
        )
        .verify_token(verify_token);

    if let Some(app_secret) = &config.app_secret {
        builder = builder.verify_payload(app_secret);
    } else {
        eprintln!("WHATSAPP_APP_SECRET is not set; inbound webhook payload signatures will not be validated.");
    }

    println!("serving WhatsApp webhook on http://{}{}", config.webhook_listen_addr, config.webhook_route);

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
