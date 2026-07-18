//! WhatsApp Business Cloud API demoscene using `whatsapp-business-rs`.
//!
//! This is an integration example, not a production bot. It exercises text and
//! formatting, media, reply/list/URL buttons, locations, reactions, quoted
//! replies, read/typing indicators, batching, webhook registration, and signed
//! webhook handling against a real Meta test number.
//!
//! # First setup
//!
//! 1. At <https://developers.facebook.com/apps/>, create an app, add the
//!    **WhatsApp** product, and finish **WhatsApp > API setup**. Meta creates or
//!    selects a business portfolio, test WhatsApp Business Account (WABA), and
//!    test sender number during this flow.
//! 2. In **API setup**, add and verify your mobile number in the **To** field.
//!    Copy the temporary access token and sender **Phone number ID**. The latter
//!    is an opaque ID, not the visible phone number.
//! 3. In **App settings > Basic**, copy the **App ID** and reveal the
//!    **App secret**. Treat both the access token and app secret as secrets.
//! 4. Give the example a public HTTPS URL whose path is dedicated to this
//!    webhook, for example `https://bot.example.com/whatsapp`. Route that URL
//!    through an HTTPS reverse proxy or tunnel to `http://127.0.0.1:8080`.
//! 5. Run `serve-and-register`. It registers the callback and the `messages`
//!    webhook field. In **WhatsApp > Configuration**, confirm that `messages` is
//!    subscribed.
//! 6. In **Business Manager > Settings > WhatsApp Business Account**, copy the
//!    WABA ID. Use Meta's [**Subscribe to your WABA**][subscribe-waba] request
//!    once so that events for its phone numbers reach this webhook (the WABA ID
//!    is not a program env var):
//!
//! ```text
//! curl --request POST \
//!   'https://graph.facebook.com/<CURRENT-GRAPH-API-VERSION>/<WABA-ID>/subscribed_apps' \
//!   --header "Authorization: Bearer ${WHATSAPP_ACCESS_TOKEN}"
//! ```
//!
//! A successful response is `{"success": true}`. Meta's test setup may already
//! have made this subscription; repeating the request is safe.
//!
//! [subscribe-waba]: https://www.postman.com/meta/whatsapp-business-platform/request/c1ai24q/subscribe-to-your-waba
//!
//! Meta limits development-mode sends to configured test recipients. Temporary
//! tokens expire; use a suitably permissioned system-user token for longer-lived
//! testing. Business-initiated messages outside the customer-service window need
//! an approved template; this crate version cannot build those template payloads.
//!
//! # Environment
//!
//! | Variable | Value and where it comes from | Used by |
//! | --- | --- | --- |
//! | `WHATSAPP_ACCESS_TOKEN` | **API setup > Temporary access token**, or a system-user token | send, serve |
//! | `WHATSAPP_PHONE_NUMBER_ID` | **API setup > Phone number ID**; not the visible number | send |
//! | `WHATSAPP_RECIPIENT_PHONE_NUMBER` | Your verified **To** number in E.164 form, such as `+15551234567` | send |
//! | `WHATSAPP_APP_ID` | **App settings > Basic > App ID** | registration |
//! | `WHATSAPP_APP_SECRET` | **App settings > Basic > App secret**; also verifies webhook signatures | serve, registration |
//! | `WHATSAPP_WEBHOOK_VERIFY_TOKEN` | A random value you create, for example with `openssl rand -hex 32` | serve, registration |
//! | `WHATSAPP_WEBHOOK_PUBLIC_URL` | Your public HTTPS callback URL, including its path | serve, registration |
//! | `WHATSAPP_WEBHOOK_BIND_ADDR` | Local listener; defaults to `127.0.0.1:8080` without direct TLS | webhook modes |
//! | `WHATSAPP_WEBHOOK_CERTIFICATE_FILE` | Optional direct-TLS full certificate chain from your ACME client | webhook modes |
//! | `WHATSAPP_WEBHOOK_PRIVATE_KEY_FILE` | Optional matching PEM private key; set both TLS file variables or neither | webhook modes |
//!
//! Example shell setup (replace every placeholder):
//!
//! ```text
//! export WHATSAPP_ACCESS_TOKEN='<API setup temporary token>'
//! export WHATSAPP_PHONE_NUMBER_ID='<API setup phone number ID>'
//! export WHATSAPP_RECIPIENT_PHONE_NUMBER='+15551234567'
//! export WHATSAPP_APP_ID='<App settings Basic app ID>'
//! export WHATSAPP_APP_SECRET='<App settings Basic app secret>'
//! export WHATSAPP_WEBHOOK_VERIFY_TOKEN='<output of openssl rand -hex 32>'
//! export WHATSAPP_WEBHOOK_PUBLIC_URL='https://bot.example.com/whatsapp'
//! export WHATSAPP_WEBHOOK_BIND_ADDR='127.0.0.1:8080'
//! cargo run --example whatsapp_demoscene -- serve-and-register
//! ```
//!
//! The normal topology terminates public TLS at a proxy/tunnel and needs no TLS
//! file variables. For direct TLS, bind an externally reachable address (usually
//! `0.0.0.0:443`) and point both file variables at the full-chain `.crt` and
//! private `.key` produced by an ACME client such as `lego`. Self-signed
//! certificates work for local `curl` checks, not for Meta's callback.
//! Media samples are compiled in through `src/resources.rs`; regenerate them with
//! `scripts/generate_demo_media --force`. No media-path environment variable is
//! required.
//!
//! Other modes are `send`, `send-batch`, `serve`, and `register-webhook`.
//! `register-webhook` serves the verification challenge only while Meta registers
//! the already-routable public URL; use `serve-and-register` for the bot loop.
//!
//! # Diagnostics
//!
//! - No `WEBHOOK HTTP <-`: Meta did not reach this process; check URL routing,
//!   the `messages` field, and the app-to-WABA subscription.
//! - HTTP `401`: the app secret/signature is wrong. HTTP `400`/`500`: the body
//!   reached the parser but was invalid or unsupported.
//! - `MO`: a message was parsed. `MT status`: a delivery status was parsed.
//!
//! Logs contain message IDs and account/user identifiers, but not message bodies.
//! They are still sensitive operational data and need production-grade retention
//! and access controls outside this example.
//!
//! # Feature and SDK assessment (2026-07-16)
//!
//! WhatsApp also offers approved templates, contacts, catalogs/products, Flows,
//! and media carousels. Version 0.5.0 has partial catalog/product support but no
//! complete outgoing templates, contacts, Flows, or carousel model. WhatsApp has
//! no documented Cloud API operation for editing an already delivered message or
//! media. Interactive rows/buttons have no arbitrary icon field; use emoji,
//! media headers, or product imagery where the message type permits it.
//! The official API also distinguishes voice audio with `voice: true`, but this
//! crate cannot express that field. ZIP is not one of Meta's supported document
//! MIME types, so the existing ZIP resource remains Telegram-only.
//! Meta does not publish an official Rust SDK. No reviewed community crate is a
//! clear production replacement: `wacloudapi` 0.1.0 exposes a non-cryptographic
//! placeholder as signature verification, `whatsapp_handler` 0.2.0 unwraps
//! untrusted webhook JSON, and `whatsapp-cloud-api` 0.5.4 is much narrower. Keep
//! this mandated crate for the demo; for production, own a small Graph API
//! adapter or a reviewed fork behind a project interface.
//!
//! Known crate risks found by this work item:
//!
//! 1. `WebhookService`'s built-in verification paths use unsound `unsafe`
//!    `Arc` transmutes between `Option<T>` and `T`. This example never invokes
//!    those paths: it verifies GET tokens and POST HMAC-SHA256 signatures first,
//!    then gives only verified message payloads to the unconfigured service.
//! 2. Real status callbacks can omit fields required by the crate's shared
//!    message/status context model, producing HTTP 500 and Meta retries. This
//!    example parses and acknowledges signed status-only callbacks separately.
//! 3. Draft builders do not validate Meta field limits. This example validates
//!    the 60-character interactive-footer limit before sending.
//! 4. The crate detaches one Tokio task per event with no bound, ordering,
//!    backpressure, or shutdown drain. That is acceptable only for this demo.
//! 5. Token roles are plain strings, handler failures cannot propagate to the
//!    webhook response, API-version selection requires `&'static str`, and the
//!    README webhook example does not match the 0.5.0 source API.
//! 6. The optional direct-TLS listener performs one bounded handshake at a time.
//!    Keep the documented reverse-proxy topology for concurrent public traffic.

use anyhow::{Result, anyhow};
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{Query, Request},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use bot_starter_kit::resources::{DEMO_AUDIO, DEMO_IMAGE, DEMO_STICKER, DEMO_VIDEO, DEMO_VOICE, EmbeddedMedia};
use hmac::{Hmac, KeyInit, Mac};
use rustls::{
    ServerConfig,
    crypto::ring,
    pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
};
use serde::Deserialize;
use sha2::Sha256;
use std::{
    env,
    future::{self, Future},
    io,
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::oneshot,
};
use tokio_rustls::{TlsAcceptor, server::TlsStream};
use url::Url;
use whatsapp_business_rs::{
    Client, Fields, WebhookHandler,
    app::SubscriptionField,
    message::{Content, Draft, InteractiveAction, InteractiveContent, InteractiveMessage, Location, Media, Message, MessageCreate},
    server::{ErrorContext, EventContext, IncomingMessage, MessageUpdate, WabaEvent},
    webhook_service::WebhookService,
};

const DEFAULT_WEBHOOK_BIND_ADDR: &str = "127.0.0.1:8080";
const MAX_WEBHOOK_BODY_BYTES: usize = 1024 * 1024;
const META_INTERACTIVE_FOOTER_MAX_CHARS: usize = 60;
type HmacSha256 = Hmac<Sha256>;

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
    access_token: Option<String>,
    phone_number_id: Option<String>,
    recipient_phone_number: Option<String>,
    app_id: Option<String>,
    app_secret: Option<String>,
    webhook_verify_token: Option<String>,
    webhook_public_url: Option<Url>,
    webhook_bind_addr: Option<SocketAddr>,
    webhook_certificate_file: Option<String>,
    webhook_private_key_file: Option<String>,
}

#[derive(Clone, Debug)]
struct WhatsAppDemosceneHandler;

impl WebhookHandler for WhatsAppDemosceneHandler {
    async fn handle_message(&self, _ctx: EventContext, incoming: IncomingMessage) {
        let message = incoming.message();
        println!(
            "MO id={} from={} to={} type={}",
            message.id,
            message
                .sender
                .phone_id,
            message
                .recipient
                .phone_id,
            message_content_kind(message)
        );

        if let Err(err) = incoming
            .set_read()
            .await
        {
            eprintln!("WhatsApp demoscene: failed to mark inbound message {} as read: {err}", message.id);
        }
        if let Err(err) = incoming
            .set_replying()
            .await
        {
            eprintln!("WhatsApp demoscene: failed to set the replying indicator for {}: {err}", message.id);
        }

        let action = response_for(message);
        if let Err(err) = execute_demo_action(&incoming, action).await {
            eprintln!("WhatsApp demoscene: failed to handle inbound message {}: {err}", message.id);
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

    async fn handle_error(&self, _ctx: ErrorContext, error: Box<dyn std::error::Error + Send>) {
        eprintln!("WhatsApp demoscene: webhook processing error: {error}");
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
    match mode {
        Mode::Send => {
            let client = whatsapp_client(&config).await?;
            send_demos(&config, &client).await
        }
        Mode::SendBatch => {
            let client = whatsapp_client(&config).await?;
            send_batch_demo(&config, &client).await
        }
        Mode::Serve => {
            let client = whatsapp_client(&config).await?;
            serve_webhook(&config, client, None).await
        }
        Mode::RegisterWebhook => {
            let app_client = app_client(&config).await?;
            register_webhook_with_temporary_server(&config, app_client).await
        }
        Mode::ServeAndRegister => {
            let client = whatsapp_client(&config).await?;
            let app_client = app_client(&config).await?;
            serve_webhook(&config, client, Some(app_client)).await
        }
        Mode::Help => Ok(()),
    }
}

impl Config {
    fn from_env() -> Result<Self> {
        Ok(Self {
            access_token: env_optional("WHATSAPP_ACCESS_TOKEN"),
            phone_number_id: env_optional("WHATSAPP_PHONE_NUMBER_ID"),
            recipient_phone_number: env_optional("WHATSAPP_RECIPIENT_PHONE_NUMBER"),
            app_id: env_optional("WHATSAPP_APP_ID"),
            app_secret: env_optional("WHATSAPP_APP_SECRET"),
            webhook_verify_token: env_optional("WHATSAPP_WEBHOOK_VERIFY_TOKEN"),
            webhook_public_url: parse_public_webhook_url("WHATSAPP_WEBHOOK_PUBLIC_URL")?,
            webhook_bind_addr: parse_socket_addr_env("WHATSAPP_WEBHOOK_BIND_ADDR")?,
            webhook_certificate_file: env_optional("WHATSAPP_WEBHOOK_CERTIFICATE_FILE"),
            webhook_private_key_file: env_optional("WHATSAPP_WEBHOOK_PRIVATE_KEY_FILE"),
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

    fn access_token(&self) -> Result<&str> {
        self.access_token
            .as_deref()
            .ok_or_else(|| anyhow!("WHATSAPP_ACCESS_TOKEN is required for send, send-batch, serve, and serve-and-register modes"))
    }

    fn app_secret(&self) -> Result<&str> {
        self.app_secret
            .as_deref()
            .ok_or_else(|| anyhow!("WHATSAPP_APP_SECRET is required for webhook modes and app-level webhook registration"))
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

    fn webhook_bind_addr(&self) -> Result<SocketAddr> {
        if let Some(bind_addr) = self.webhook_bind_addr {
            return Ok(bind_addr);
        }

        if self
            .webhook_certificate_file
            .is_none()
            && self
                .webhook_private_key_file
                .is_none()
        {
            return DEFAULT_WEBHOOK_BIND_ADDR
                .parse()
                .map_err(|err| anyhow!("invalid built-in webhook bind address {DEFAULT_WEBHOOK_BIND_ADDR:?}: {err}"));
        }

        let public_url = self.webhook_public_url()?;
        let port = public_url
            .port_or_known_default()
            .ok_or_else(|| anyhow!("WHATSAPP_WEBHOOK_PUBLIC_URL must include an HTTPS port"))?;
        Ok(([0, 0, 0, 0], port).into())
    }

    fn webhook_tls_config(&self) -> Result<Option<ServerConfig>> {
        match (
            self.webhook_certificate_file
                .as_deref(),
            self.webhook_private_key_file
                .as_deref(),
        ) {
            (None, None) => Ok(None),
            (Some(certificate_file), Some(private_key_file)) => load_tls_server_config(certificate_file, private_key_file).map(Some),
            _ => Err(anyhow!("WHATSAPP_WEBHOOK_CERTIFICATE_FILE and WHATSAPP_WEBHOOK_PRIVATE_KEY_FILE must be set together")),
        }
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

#[derive(Debug, Deserialize)]
struct WebhookVerificationQuery {
    #[serde(rename = "hub.mode")]
    mode: Option<String>,
    #[serde(rename = "hub.verify_token")]
    verify_token: Option<String>,
    #[serde(rename = "hub.challenge")]
    challenge: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StatusWebhookEnvelope {
    object: String,
    #[serde(default)]
    entry: Vec<StatusWebhookEntry>,
}

#[derive(Debug, Deserialize)]
struct StatusWebhookEntry {
    #[serde(default)]
    changes: Vec<StatusWebhookChange>,
}

#[derive(Debug, Deserialize)]
struct StatusWebhookChange {
    value: StatusWebhookValue,
}

#[derive(Debug, Deserialize)]
struct StatusWebhookValue {
    statuses: Option<Vec<StatusWebhookUpdate>>,
    messages: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct StatusWebhookUpdate {
    id: String,
    status: String,
    #[serde(default)]
    biz_opaque_callback_data: Option<String>,
    #[serde(default)]
    errors: Vec<StatusWebhookError>,
}

#[derive(Debug, Deserialize)]
struct StatusWebhookError {
    #[serde(default)]
    code: Option<u64>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

async fn whatsapp_client(config: &Config) -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(20))
        .connect(
            config
                .access_token()?
                .to_owned(),
        )
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: building WhatsApp send/reply client failed: {err}"))
}

async fn app_client(config: &Config) -> Result<Client> {
    let registration = config.webhook_registration()?;
    Client::builder()
        .timeout(Duration::from_secs(20))
        .connect((
            registration
                .app_id
                .to_owned(),
            config
                .app_secret()?
                .to_owned(),
        ))
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: building Meta app client failed: {err}"))
}

async fn send_demos(config: &Config, client: &Client) -> Result<()> {
    let outbound = config.outbound()?;
    let sender = client.message(outbound.phone_number_id);

    let text = sender
        .send(
            outbound.recipient_phone_number,
            Draft::text("OgreRobot Demoscene: text message with no link preview")
                .preview_url(false)
                .with_biz_opaque_callback_data("whatsapp-demoscene:text"),
        )
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: sending text demo failed: {err}"))?;
    print_sent("text", &text);

    let formatted = sender
        .send(outbound.recipient_phone_number, formatted_text_draft())
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: sending formatted-text demo failed: {err}"))?;
    print_sent("formatted text", &formatted);

    let menu_draft = menu_draft();
    validate_demo_draft(&menu_draft)?;
    let menu = sender
        .send(outbound.recipient_phone_number, menu_draft)
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: sending quick-reply menu failed: {err}"))?;
    print_sent("quick replies", &menu);

    let list_draft = list_draft();
    validate_demo_draft(&list_draft)?;
    let list = sender
        .send(outbound.recipient_phone_number, list_draft)
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: sending list demo failed: {err}"))?;
    print_sent("list", &list);

    let cta_draft = cta_draft();
    validate_demo_draft(&cta_draft)?;
    let cta = sender
        .send(outbound.recipient_phone_number, cta_draft)
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: sending CTA demo failed: {err}"))?;
    print_sent("cta", &cta);

    let location = sender
        .send(outbound.recipient_phone_number, location_draft())
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: sending location demo failed: {err}"))?;
    print_sent("location", &location);

    let location_request_draft = location_request_draft();
    validate_demo_draft(&location_request_draft)?;
    let location_request = sender
        .send(outbound.recipient_phone_number, location_request_draft)
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: sending location-request demo failed: {err}"))?;
    print_sent("location request", &location_request);

    for sample in embedded_media_drafts()? {
        let metadata = sender
            .send(outbound.recipient_phone_number, sample.draft)
            .await
            .map_err(|err| anyhow!("WhatsApp demoscene: sending {} demo failed: {err}", sample.label))?;
        print_sent(sample.label, &metadata);
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
        .events(Fields::new().with(SubscriptionField::Messages))
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: registering webhook failed: {err}"))?;
    println!("webhook registered for {}", registration.public_url);
    Ok(())
}

async fn register_webhook_with_temporary_server(config: &Config, app_client: Client) -> Result<()> {
    let verify_token = config
        .webhook_verify_token()?
        .to_owned();
    let public_url = config.webhook_public_url()?;
    let route = config.webhook_route()?;
    let tls_config = config.webhook_tls_config()?;
    let bind_addr = config.webhook_bind_addr()?;
    let tcp_listener = TcpListener::bind(bind_addr)
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: couldn't bind temporary webhook verification listener to {bind_addr}: {err}"))?;
    let local_addr = tcp_listener
        .local_addr()
        .map_err(|err| anyhow!("WhatsApp demoscene: couldn't read temporary webhook verification listener address: {err}"))?;
    let app = Router::new().route(
        &route,
        get({
            move |Query(query): Query<WebhookVerificationQuery>| {
                let verify_token = verify_token.clone();
                async move { verify_webhook_challenge(&verify_token, query) }
            }
        }),
    );

    println!("serving temporary WhatsApp {} webhook verifier on {local_addr}{route}", transport_name(&tls_config));
    println!("asking Meta to verify public HTTPS URL {public_url}");

    let (shutdown_sender, shutdown_receiver) = oneshot::channel::<()>();
    let server_task = tokio::spawn(run_webhook_server(tcp_listener, tls_config, app, async {
        _ = shutdown_receiver.await;
    }));
    wait_for_webhook_listener().await;

    let registration_result = register_webhook(config, &app_client).await;
    _ = shutdown_sender.send(());
    if let Err(shutdown_err) = server_task
        .await
        .map_err(|join_err| anyhow!("WhatsApp demoscene: temporary verifier task failed to join: {join_err}"))
        .and_then(|server_result| server_result)
    {
        eprintln!("WhatsApp demoscene: temporary verifier shutdown failed: {shutdown_err}");
    }
    registration_result
}

async fn serve_webhook(config: &Config, client: Client, app_client: Option<Client>) -> Result<()> {
    let verify_token = config
        .webhook_verify_token()?
        .to_owned();
    let app_secret = config
        .app_secret()?
        .to_owned();
    let public_url = config.webhook_public_url()?;
    let route = config.webhook_route()?;
    let tls_config = config.webhook_tls_config()?;
    let bind_addr = config.webhook_bind_addr()?;
    let tcp_listener = TcpListener::bind(bind_addr)
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: couldn't bind webhook listener to {bind_addr}: {err}"))?;
    let local_addr = tcp_listener
        .local_addr()
        .map_err(|err| anyhow!("WhatsApp demoscene: couldn't read webhook listener address: {err}"))?;

    // Do not configure WebhookService's verify-token or verify-payload builders:
    // version 0.5.0 reaches those paths through unsound Arc transmutes. The
    // routes below perform both checks before forwarding message payloads.
    let service = WebhookService::<WhatsAppDemosceneHandler>::builder().build(WhatsAppDemosceneHandler, client);
    let app_secret: Arc<str> = Arc::from(app_secret);
    let app = Router::new()
        .route(
            &route,
            get({
                let verify_token = verify_token.clone();
                move |Query(query): Query<WebhookVerificationQuery>| {
                    let verify_token = verify_token.clone();
                    async move { verify_webhook_challenge(&verify_token, query) }
                }
            })
            .post({
                let service = service.clone();
                let app_secret = app_secret.clone();
                move |req: Request| {
                    let service = service.clone();
                    let app_secret = app_secret.clone();
                    async move { handle_signed_webhook(service, app_secret, req).await }
                }
            }),
        )
        .fallback(handle_unmatched_webhook_route);

    println!("serving WhatsApp {} webhook on {local_addr}{route}", transport_name(&tls_config));
    println!("expecting Meta to call public HTTPS URL {public_url}");

    let Some(app_client) = app_client else {
        return run_webhook_server(tcp_listener, tls_config, app, future::pending()).await;
    };

    let (shutdown_sender, shutdown_receiver) = oneshot::channel::<()>();
    let server_task = tokio::spawn(run_webhook_server(tcp_listener, tls_config, app, async {
        _ = shutdown_receiver.await;
    }));
    wait_for_webhook_listener().await;

    let registration_result = register_webhook(config, &app_client).await;

    if let Err(err) = registration_result {
        _ = shutdown_sender.send(());
        if let Err(shutdown_err) = server_task
            .await
            .map_err(|join_err| anyhow!("WhatsApp demoscene: webhook task failed to join after registration failure: {join_err}"))
            .and_then(|server_result| server_result)
        {
            eprintln!("WhatsApp demoscene: webhook shutdown after registration failure also failed: {shutdown_err}");
        }
        return Err(err);
    }
    server_task
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: webhook task failed to join: {err}"))?
}

async fn handle_signed_webhook(service: WebhookService<WhatsAppDemosceneHandler>, app_secret: Arc<str>, req: Request) -> Response {
    let method = req
        .method()
        .clone();
    let path = req
        .uri()
        .path()
        .to_owned();
    let content_length = req
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| {
            value
                .to_str()
                .ok()
        })
        .unwrap_or("unknown")
        .to_owned();
    let signature_present = req
        .headers()
        .contains_key("x-hub-signature-256");

    println!("WEBHOOK HTTP <- method={method} path={path} content_length={content_length} x_hub_signature_256_present={signature_present}");

    let (parts, body) = req.into_parts();
    let body = match to_bytes(body, MAX_WEBHOOK_BODY_BYTES).await {
        Ok(body) => body,
        Err(err) => {
            eprintln!("WhatsApp demoscene: webhook body rejected: {err}");
            let response = (StatusCode::PAYLOAD_TOO_LARGE, "Webhook body is unreadable or exceeds 1 MiB").into_response();
            println!("WEBHOOK HTTP -> method={method} path={path} status={}", response.status());
            return response;
        }
    };

    if let Err(reason) = verify_meta_signature(&app_secret, &parts.headers, &body) {
        eprintln!("WhatsApp demoscene: webhook signature verification failed: {reason}");
        let response = (StatusCode::UNAUTHORIZED, "Webhook signature verification failed").into_response();
        println!("WEBHOOK HTTP -> method={method} path={path} status={}", response.status());
        return response;
    }

    let response = if let Some(status_updates) = status_updates_if_only(&body) {
        log_status_updates(&status_updates);
        StatusCode::OK.into_response()
    } else {
        service
            .handle(Request::from_parts(parts, Body::from(body)))
            .await
    };
    println!("WEBHOOK HTTP -> method={method} path={path} status={}", response.status());
    response
}

async fn handle_unmatched_webhook_route(req: Request) -> (StatusCode, &'static str) {
    let method = req
        .method()
        .clone();
    let path = req
        .uri()
        .path()
        .to_owned();

    println!("WEBHOOK HTTP <- method={method} path={path} route=unmatched");
    (StatusCode::NOT_FOUND, "No WhatsApp webhook route matches this path")
}

async fn wait_for_webhook_listener() {
    tokio::time::sleep(Duration::from_millis(400)).await;
}

fn verify_webhook_challenge(expected_verify_token: &str, query: WebhookVerificationQuery) -> (StatusCode, String) {
    if query
        .mode
        .as_deref()
        != Some("subscribe")
    {
        return (StatusCode::BAD_REQUEST, "unsupported webhook verification mode".to_owned());
    }
    if query
        .verify_token
        .as_deref()
        != Some(expected_verify_token)
    {
        return (StatusCode::FORBIDDEN, "webhook verify token mismatch".to_owned());
    }

    match query.challenge {
        Some(challenge) => (StatusCode::OK, challenge),
        None => (StatusCode::BAD_REQUEST, "missing webhook challenge".to_owned()),
    }
}

fn verify_meta_signature(app_secret: &str, headers: &HeaderMap, body: &[u8]) -> std::result::Result<(), &'static str> {
    let signature = headers
        .get("x-hub-signature-256")
        .ok_or("missing X-Hub-Signature-256 header")?
        .to_str()
        .map_err(|_| "X-Hub-Signature-256 is not valid ASCII")?
        .strip_prefix("sha256=")
        .ok_or("X-Hub-Signature-256 must start with sha256=")?;
    let signature = hex::decode(signature).map_err(|_| "X-Hub-Signature-256 is not valid hexadecimal")?;
    let mut mac = HmacSha256::new_from_slice(app_secret.as_bytes()).map_err(|_| "app secret cannot initialize HMAC-SHA256")?;
    mac.update(body);
    mac.verify_slice(&signature)
        .map_err(|_| "signature does not match the request body")
}

fn status_updates_if_only(body: &[u8]) -> Option<Vec<StatusWebhookUpdate>> {
    let envelope: StatusWebhookEnvelope = serde_json::from_slice(body).ok()?;
    if envelope.object != "whatsapp_business_account" {
        return None;
    }

    let mut updates = Vec::new();
    for entry in envelope.entry {
        for change in entry.changes {
            if change
                .value
                .messages
                .is_some()
            {
                return None;
            }
            let statuses = change
                .value
                .statuses?;
            updates.extend(statuses);
        }
    }

    (!updates.is_empty()).then_some(updates)
}

fn log_status_updates(updates: &[StatusWebhookUpdate]) {
    for update in updates {
        println!("MT status id={} status={} callback={:?}", update.id, update.status, update.biz_opaque_callback_data);
        for error in &update.errors {
            let detail = error
                .title
                .as_deref()
                .or(error
                    .message
                    .as_deref())
                .unwrap_or("<no error description>");
            eprintln!(
                "WhatsApp demoscene: Meta status error for {}: code={} {detail}",
                update.id,
                error
                    .code
                    .map_or_else(|| "unknown".to_owned(), |code| code.to_string())
            );
        }
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
                    eprintln!("WhatsApp demoscene: HTTPS webhook TCP accept failed: {err}");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
            };

            match tokio::time::timeout(
                Duration::from_secs(10),
                self.acceptor
                    .accept(stream),
            )
            .await
            {
                Ok(Ok(stream)) => return (stream, addr),
                Ok(Err(err)) => eprintln!("WhatsApp demoscene: HTTPS webhook TLS handshake failed for {addr}: {err}"),
                Err(_) => eprintln!("WhatsApp demoscene: HTTPS webhook TLS handshake timed out for {addr}"),
            }
        }
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.listener
            .local_addr()
    }
}

fn transport_name(tls_config: &Option<ServerConfig>) -> &'static str {
    if tls_config.is_some() { "HTTPS" } else { "HTTP" }
}

async fn run_webhook_server<Shutdown>(tcp_listener: TcpListener, tls_config: Option<ServerConfig>, app: Router, shutdown: Shutdown) -> Result<()>
where
    Shutdown: Future<Output = ()> + Send + 'static,
{
    match tls_config {
        Some(tls_config) => {
            let listener = TlsListener { listener: tcp_listener, acceptor: TlsAcceptor::from(Arc::new(tls_config)) };
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown)
                .await
                .map_err(|err| anyhow!("WhatsApp demoscene: HTTPS webhook server failed: {err}"))
        }
        None => axum::serve(tcp_listener, app)
            .with_graceful_shutdown(shutdown)
            .await
            .map_err(|err| anyhow!("WhatsApp demoscene: HTTP webhook server failed: {err}")),
    }
}

fn load_tls_server_config(certificate_file: &str, private_key_file: &str) -> Result<ServerConfig> {
    let certificate_chain = CertificateDer::pem_file_iter(certificate_file)
        .map_err(|err| anyhow!("WhatsApp demoscene: couldn't open TLS certificate file '{certificate_file}': {err}"))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|err| anyhow!("WhatsApp demoscene: couldn't parse TLS certificate file '{certificate_file}': {err}"))?;
    if certificate_chain.is_empty() {
        return Err(anyhow!("WhatsApp demoscene: TLS certificate file '{certificate_file}' did not contain any certificates"));
    }

    let private_key = PrivateKeyDer::from_pem_file(private_key_file).map_err(|err| anyhow!("WhatsApp demoscene: couldn't read TLS private key file '{private_key_file}': {err}"))?;

    ServerConfig::builder_with_provider(ring::default_provider().into())
        .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
        .map_err(|err| anyhow!("WhatsApp demoscene: couldn't build TLS protocol configuration: {err}"))?
        .with_no_client_auth()
        .with_single_cert(certificate_chain, private_key)
        .map_err(|err| anyhow!("WhatsApp demoscene: couldn't build TLS server configuration: {err}"))
}

enum DemoAction {
    Reply(Draft),
    Replies(Vec<Draft>),
    SwipeReply(Draft),
    React(char),
}

async fn execute_demo_action(incoming: &IncomingMessage, action: Result<DemoAction>) -> Result<()> {
    match action? {
        DemoAction::Reply(draft) => {
            validate_demo_draft(&draft)?;
            incoming
                .reply(draft)
                .await
                .map(|_| ())
                .map_err(|err| anyhow!("sending reply failed: {err}"))
        }
        DemoAction::Replies(drafts) => {
            for (index, draft) in drafts
                .into_iter()
                .enumerate()
            {
                validate_demo_draft(&draft)?;
                incoming
                    .reply(draft)
                    .await
                    .map_err(|err| anyhow!("sending embedded media reply {} failed: {err}", index + 1))?;
            }
            Ok(())
        }
        DemoAction::SwipeReply(draft) => {
            validate_demo_draft(&draft)?;
            incoming
                .swipe_reply(draft)
                .await
                .map(|_| ())
                .map_err(|err| anyhow!("sending quoted reply failed: {err}"))
        }
        DemoAction::React(emoji) => incoming
            .react(emoji)
            .await
            .map(|_| ())
            .map_err(|err| anyhow!("sending reaction failed: {err}")),
    }
}

fn response_for(message: &Message) -> Result<DemoAction> {
    if let Content::Location(location) = &message.content {
        return Ok(DemoAction::Reply(shifted_location_draft(location)?));
    }

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

    let action = match command.as_str() {
        "/start" | "start" | "help" | "menu" | "demo:menu" => DemoAction::Reply(menu_draft()),
        "demo:list" | "list" => DemoAction::Reply(list_draft()),
        "demo:formatting" | "formatting" => DemoAction::Reply(formatted_text_draft()),
        "demo:media" | "media" => DemoAction::Replies(
            embedded_media_drafts()?
                .into_iter()
                .map(|sample| sample.draft)
                .collect(),
        ),
        "demo:reaction" | "reaction" => DemoAction::React('👍'),
        "demo:quote" | "quote" => DemoAction::SwipeReply(Draft::text("This is a quoted (swipe-style) reply.")),
        "demo:location" | "location" => DemoAction::Reply(location_draft()),
        "demo:location-request" | "location request" => DemoAction::Reply(location_request_draft()),
        "demo:cta" | "cta" => DemoAction::Reply(cta_draft()),
        "demo:inspect" | "inspect" => DemoAction::Reply(inspect_message_draft(message)),
        "demo:echo" | "echo" => DemoAction::Reply(Draft::text("Send any text and this demoscene will echo the primary text field.")),
        _ => DemoAction::Reply(echo_or_menu_draft(message)),
    };
    Ok(action)
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
        .header("Feature menu")
        .footer("Selections return as signed interactive webhook events.")
        .list("Features")
        .add_list_section("Messages")
        .add_list_option("demo:menu", "Quick replies", "Buttons using Draft::add_reply_button")
        .add_list_option("demo:formatting", "Formatting", "Bold, italic, strike, and monospace text")
        .add_list_option("demo:media", "Media", "Embedded PNG, MP3, OGG/Opus, MP4, and WebP sticker")
        .add_list_option("demo:reaction", "Reaction", "React to the selected message with an emoji")
        .add_list_option("demo:quote", "Quoted reply", "Reply with the selected message as context")
        .add_list_section("Interactive")
        .add_list_option("demo:location", "Location", "Latitude/longitude with a name and address")
        .add_list_option("demo:location-request", "Request location", "Share a location; receive a point about 100 m south")
        .add_list_option("demo:cta", "CTA URL", "A native call-to-action link button")
        .add_list_option("demo:inspect", "Inspect", "Show safe metadata for the selected message")
        .with_biz_opaque_callback_data("whatsapp-demoscene:list")
}

fn cta_draft() -> Draft {
    const BOT_STARTER_KIT_REPOSITORY_URL: &str = "https://github.com/zertyz/bot-starter-kit";

    Draft::new()
        .body("Open the OgreRobot bot-starter-kit repository.")
        .footer("CTA buttons open a URL and do not send a callback.")
        .with_cta_url(BOT_STARTER_KIT_REPOSITORY_URL, "Open repository")
        .with_biz_opaque_callback_data("whatsapp-demoscene:cta")
}

fn formatted_text_draft() -> Draft {
    Draft::text("*Bold*\n_Italic_\n~Strikethrough~\n```Monospace```").with_biz_opaque_callback_data("whatsapp-demoscene:formatting")
}

struct DemoMediaDraft {
    label: &'static str,
    draft: Draft,
}

fn embedded_media_drafts() -> Result<Vec<DemoMediaDraft>> {
    Ok(vec![
        embedded_media_draft(DEMO_IMAGE, "image", Some("Embedded PNG image."), "image")?,
        embedded_media_draft(DEMO_AUDIO, "MP3 audio", None, "audio-mp3")?,
        embedded_media_draft(DEMO_VOICE, "OGG/Opus audio", None, "audio-opus")?,
        embedded_media_draft(DEMO_VIDEO, "MP4 video", Some("Embedded H.264 MP4 video."), "video-mp4")?,
        embedded_media_draft(DEMO_STICKER, "WebP sticker", None, "sticker-webp")?,
    ])
}

fn embedded_media_draft(resource: EmbeddedMedia, label: &'static str, caption: Option<&str>, callback_suffix: &str) -> Result<DemoMediaDraft> {
    let media_type = resource
        .mime_type
        .parse()
        .map_err(|err: String| anyhow!("embedded resource {} has unsupported MIME type {}: {err}", resource.file_name, resource.mime_type))?;
    let mut media = Media::new(
        resource
            .bytes
            .to_vec(),
        media_type,
    );
    if let Some(caption) = caption {
        media = media.caption(caption);
    }
    Ok(DemoMediaDraft {
        label,
        draft: Draft::media(media).with_biz_opaque_callback_data(format!("whatsapp-demoscene:media:{callback_suffix}")),
    })
}

fn location_draft() -> Draft {
    Draft::location(-23.55052, -46.633308)
        .location_name("Sao Paulo")
        .location_address("Sao Paulo, SP, Brazil")
        .with_biz_opaque_callback_data("whatsapp-demoscene:location")
}

fn shifted_location_draft(location: &Location) -> Result<Draft> {
    const EARTH_MEAN_RADIUS_METERS: f64 = 6_371_008.8;
    const LOCATION_REPLY_SHIFT_METERS: f64 = 100.0;

    if !location
        .latitude
        .is_finite()
        || !location
            .longitude
            .is_finite()
        || !(-90.0..=90.0).contains(&location.latitude)
        || !(-180.0..=180.0).contains(&location.longitude)
    {
        return Err(anyhow!("received location coordinates are outside the valid latitude/longitude ranges"));
    }

    let latitude_shift = (LOCATION_REPLY_SHIFT_METERS / EARTH_MEAN_RADIUS_METERS).to_degrees();
    let shifted_latitude = (location.latitude - latitude_shift).max(-90.0);
    Ok(Draft::location(shifted_latitude, location.longitude)
        .location_name("About 100 m south of your location")
        .location_address(format!("Original: {:.6}, {:.6}", location.latitude, location.longitude))
        .with_biz_opaque_callback_data("whatsapp-demoscene:location-shifted-south"))
}

fn location_request_draft() -> Draft {
    Draft::interactive(InteractiveMessage::new(InteractiveAction::LocationRequest, "Share your location. The demo replies with a point about 100 m south."))
        .with_biz_opaque_callback_data("whatsapp-demoscene:location-request")
}

fn inspect_message_draft(message: &Message) -> Draft {
    let primary_text = message
        .content
        .text()
        .unwrap_or("<no primary text>");
    let content_kind = message_content_kind(message);

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

fn message_content_kind(message: &Message) -> &'static str {
    match &message.content {
        Content::Text(_) => "text",
        Content::Media(_) => "media",
        Content::Reaction(_) => "reaction",
        Content::Location(_) => "location",
        Content::Interactive(_) => "interactive",
        Content::Order(_) => "order",
        Content::Error(_) => "error",
    }
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

fn validate_demo_draft(draft: &Draft) -> Result<()> {
    let Content::Interactive(InteractiveContent::Message(interactive)) = &draft.content else {
        return Ok(());
    };
    let Some(footer) = &interactive.footer else {
        return Ok(());
    };
    let footer_chars = footer
        .body
        .chars()
        .count();
    if footer_chars > META_INTERACTIVE_FOOTER_MAX_CHARS {
        return Err(anyhow!("interactive footer is {footer_chars} characters; Meta allows at most {META_INTERACTIVE_FOOTER_MAX_CHARS}"));
    }
    Ok(())
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

fn parse_socket_addr_env(name: &str) -> Result<Option<SocketAddr>> {
    env_optional(name)
        .map(|value| {
            value
                .parse()
                .map_err(|err| anyhow!("{name} must be an IP socket address such as {DEFAULT_WEBHOOK_BIND_ADDR}: {err}"))
        })
        .transpose()
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn all_demo_interactive_footers_fit_meta_limit() {
        for draft in [menu_draft(), list_draft(), cta_draft(), location_request_draft()] {
            validate_demo_draft(&draft).unwrap();
        }
    }

    #[test]
    fn cta_opens_the_bot_starter_kit_repository() {
        let Content::Interactive(InteractiveContent::Message(interactive)) = cta_draft().content else {
            panic!("CTA draft must be an outgoing interactive message");
        };
        let InteractiveAction::Cta(button) = interactive.action else {
            panic!("CTA draft must use a URL action");
        };

        assert!(
            button
                .url
                .contains("bot-starter-kit")
        );
    }

    #[test]
    fn inbound_location_is_returned_about_one_hundred_meters_south() {
        let inbound = Location::new(-23.55052, -46.633308);
        let draft = shifted_location_draft(&inbound).unwrap();
        let Content::Location(outbound) = draft.content else {
            panic!("shifted location reply must contain location content");
        };

        let north_south_distance = (inbound.latitude - outbound.latitude).to_radians() * 1.0;
        assert!((north_south_distance).abs() < 0.001, "North-South distance failed with {north_south_distance}");
        assert_eq!(outbound.longitude, inbound.longitude);
        assert_eq!(
            outbound
                .name
                .as_deref(),
            Some("About 100 m south of your location")
        );
        assert_eq!(
            outbound
                .address
                .as_deref(),
            Some("Original: -23.550520, -46.633308")
        );
        assert!(shifted_location_draft(&Location::new(91.0, 0.0)).is_err());
    }

    #[test]
    fn overlong_interactive_footer_is_rejected_before_meta() {
        let draft = Draft::new()
            .body("Body")
            .footer("x".repeat(META_INTERACTIVE_FOOTER_MAX_CHARS + 1))
            .add_reply_button("ok", "OK");

        let err = validate_demo_draft(&draft)
            .unwrap_err()
            .to_string();
        assert!(err.contains("61 characters"));
        assert!(err.contains("at most 60"));
    }

    #[test]
    fn feature_list_has_nine_rows_in_two_sections() {
        let draft = list_draft();
        let Content::Interactive(InteractiveContent::Message(interactive)) = draft.content else {
            panic!("feature list must be an outgoing interactive message");
        };
        let InteractiveAction::OptionList(list) = interactive.action else {
            panic!("feature list must use the list action");
        };

        assert_eq!(
            list.sections
                .len(),
            2
        );
        assert_eq!(
            list.sections
                .iter()
                .map(|section| section
                    .items
                    .len())
                .sum::<usize>(),
            9
        );
    }

    #[test]
    fn embedded_media_builds_five_outbound_drafts() {
        let samples = embedded_media_drafts().unwrap();

        assert_eq!(
            samples
                .iter()
                .map(|sample| sample.label)
                .collect::<Vec<_>>(),
            ["image", "MP3 audio", "OGG/Opus audio", "MP4 video", "WebP sticker"]
        );
        for sample in samples {
            let Content::Media(media) = sample
                .draft
                .content
            else {
                panic!("{} did not build a media draft", sample.label);
            };
            match sample.label {
                "image" => assert!(media.is_image()),
                "MP3 audio" | "OGG/Opus audio" => assert!(media.is_audio()),
                "MP4 video" => assert!(media.is_video()),
                "WebP sticker" => assert!(media.is_sticker()),
                label => panic!("unexpected media sample: {label}"),
            }
        }
    }

    #[test]
    fn webhook_challenge_accepts_only_the_configured_token() {
        let accepted = verify_webhook_challenge(
            "expected",
            WebhookVerificationQuery {
                mode: Some("subscribe".to_owned()),
                verify_token: Some("expected".to_owned()),
                challenge: Some("challenge".to_owned()),
            },
        );
        assert_eq!(accepted, (StatusCode::OK, "challenge".to_owned()));

        let rejected = verify_webhook_challenge(
            "expected",
            WebhookVerificationQuery {
                mode: Some("subscribe".to_owned()),
                verify_token: Some("wrong".to_owned()),
                challenge: Some("challenge".to_owned()),
            },
        );
        assert_eq!(rejected.0, StatusCode::FORBIDDEN);
        assert!(
            !rejected
                .1
                .contains("expected")
        );
    }

    #[test]
    fn meta_signature_verification_accepts_matching_body_only() {
        let secret = "app-secret";
        let body = br#"{"object":"whatsapp_business_account"}"#;
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let signature = format!(
            "sha256={}",
            hex::encode(
                mac.finalize()
                    .into_bytes()
            )
        );
        let mut headers = HeaderMap::new();
        headers.insert("x-hub-signature-256", HeaderValue::from_str(&signature).unwrap());

        verify_meta_signature(secret, &headers, body).unwrap();
        assert!(verify_meta_signature(secret, &headers, b"different body").is_err());
    }

    #[test]
    fn status_only_payload_does_not_require_message_context_fields() {
        let body = br#"
        {
          "object": "whatsapp_business_account",
          "entry": [{
            "changes": [{
              "value": {
                "messaging_product": "whatsapp",
                "metadata": {"phone_number_id": "sender-id"},
                "statuses": [{
                  "id": "wamid.status",
                  "status": "failed",
                  "errors": [{"code": 131009, "title": "Parameter value is not valid"}]
                }]
              }
            }]
          }]
        }
        "#;

        let updates = status_updates_if_only(body).unwrap();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].id, "wamid.status");
        assert_eq!(updates[0].status, "failed");
        assert_eq!(updates[0].errors[0].code, Some(131009));
    }

    #[tokio::test]
    async fn signed_status_callback_returns_ok_without_sdk_status_parser() {
        let secret = "app-secret";
        let body = br#"
        {
          "object": "whatsapp_business_account",
          "entry": [{
            "changes": [{
              "value": {
                "statuses": [{"id": "wamid.status", "status": "delivered"}]
              }
            }]
          }]
        }
        "#;
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let signature = format!(
            "sha256={}",
            hex::encode(
                mac.finalize()
                    .into_bytes()
            )
        );
        let request = Request::post("https://example.test/whatsapp")
            .header("x-hub-signature-256", signature)
            .body(Body::from(body.as_slice()))
            .unwrap();
        let client = Client::builder()
            .connect("unused-test-token".to_owned())
            .await
            .unwrap();
        let service = WebhookService::<WhatsAppDemosceneHandler>::builder().build(WhatsAppDemosceneHandler, client);

        let response = handle_signed_webhook(service, Arc::from(secret), request).await;

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn message_payload_is_left_for_the_sdk_parser() {
        let body = br#"
        {
          "object": "whatsapp_business_account",
          "entry": [{
            "changes": [{
              "value": {
                "messages": [{"id": "wamid.message"}]
              }
            }]
          }]
        }
        "#;

        assert!(status_updates_if_only(body).is_none());
    }
}
