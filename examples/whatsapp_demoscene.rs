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
//!    the token used by this example to send messages, and this example derives
//!    it from `WHATSAPP_APP_ID` and `WHATSAPP_APP_SECRET` for webhook registration.
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
//! - `WHATSAPP_WEBHOOK_CERTIFICATE_FILE`
//! - `WHATSAPP_WEBHOOK_PRIVATE_KEY_FILE`
//!
//! Required environment for webhook registration modes:
//!
//! - `WHATSAPP_APP_ID`
//! - `WHATSAPP_APP_SECRET`
//! - `WHATSAPP_WEBHOOK_PUBLIC_URL`
//! - `WHATSAPP_WEBHOOK_VERIFY_TOKEN`
//! - `WHATSAPP_WEBHOOK_CERTIFICATE_FILE`
//! - `WHATSAPP_WEBHOOK_PRIVATE_KEY_FILE`
//!
//! HTTPS and local testing:
//!
//! Meta verifies and delivers webhooks through the public callback URL, and that
//! URL must be HTTPS. This example terminates HTTPS itself using the certificate
//! and private key files above. It binds `0.0.0.0` on the port from
//! `WHATSAPP_WEBHOOK_PUBLIC_URL`; if the URL has no explicit port, it binds
//! `0.0.0.0:443`. The example derives the local route from the public URL path,
//! so the operator only writes the webhook path once. Use a certificate chain
//! trusted by public clients for the Meta-facing URL. A self-signed certificate
//! is useful for local `curl` checks, not for Meta callback verification.
//!
//! Certificate files:
//!
//! For Meta webhook registration, use a public DNS name and a publicly trusted
//! certificate. Use the CachyOS' package `lego` to create one:
//!
//! Example:
//!
//! lego run --path /operations/your_bot/lego --server letsencrypt --email 'admin@your-domain.com' --domains 'bot.your-domain.com' --domains 'whatsapp.bot.your-domain.com' --domains 'telegram.bot.your-domain.com' --cert.name 'bot.your-domain.com' --accept-tos --http --http.address ':80' --key-type RSA2048 --deploy-hook your-script-to-restart-the-service
//! (the above script must be run periodically before the 90 days expiry).
//!
//! Then, you'll be able to configure:
//!
//! - `WHATSAPP_WEBHOOK_CERTIFICATE_FILE=/operations/your_bot/lego/certificates/XXXX.crt`
//! - `WHATSAPP_WEBHOOK_PRIVATE_KEY_FILE=/operations/your_bot/lego/certificates/XXXX.key`
//!
//! Run:
//!
//! - `cargo run --example whatsapp_demoscene -- send`
//! - `cargo run --example whatsapp_demoscene -- send-batch`
//! - `cargo run --example whatsapp_demoscene -- serve`
//! - `cargo run --example whatsapp_demoscene -- register-webhook`
//! - `cargo run --example whatsapp_demoscene -- serve-and-register`
//!
//! Meta verifies the callback URL while registration is running. Use
//! `serve-and-register` for the full bot loop. Use `register-webhook` only when
//! you want to configure Meta and exit; it starts a temporary HTTPS verification
//! endpoint, registers the callback URL, then stops. A standalone registration
//! request without a reachable webhook server fails with Meta error 2200.
//!
//! Runtime diagnostics:
//!
//! - `WEBHOOK HTTP <- ...` means a request reached this process. If no such
//!   line appears after sending a WhatsApp message to the sender number, Meta
//!   has not delivered a webhook to this server.
//! - `WEBHOOK HTTP -> ... status=401|400|500` means the request reached this
//!   process but was rejected before it could become a parsed incoming message.
//! - `MO ...` means the SDK parsed an incoming message and the example attempted
//!   to reply.
//!
//! SDK issues found while building this example:
//!
//! 1. `whatsapp-business-rs` keeps token-scope correctness as a runtime concern.
//!    The compiler cannot tell an app token, system-user token, or phone-number
//!    token apart.
//! 2. Webhook payload verification is opt-in in the SDK builders. Without it,
//!    webhook POSTs are easier to spoof in non-local environments. This example
//!    requires the app secret in webhook modes and always enables payload
//!    verification.
//! 3. `Handler` methods return `()`, so reply failures cannot be bubbled to the
//!    server loop. This example logs every failed handler-side API call.
//! 4. `ClientBuilder::api_version` requires `&'static str`, so runtime API
//!    version selection is awkward. This example uses the crate default.
//! 5. The README webhook snippet references fields that do not match the 0.5.0
//!    source shape; this example uses `IncomingMessage::message()`.

use anyhow::{Result, anyhow};
use axum::{
    Router,
    extract::{Query, Request},
    http::{StatusCode, header},
    routing::get,
};
use rustls::{
    ServerConfig,
    crypto::ring,
    pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
};
use serde::Deserialize;
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
    message::{Content, Draft, Media, Message, MessageCreate},
    server::{ErrorContext, EventContext, IncomingMessage, MessageUpdate, WabaEvent},
    webhook_service::WebhookService,
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
    access_token: Option<String>,
    phone_number_id: Option<String>,
    recipient_phone_number: Option<String>,
    app_id: Option<String>,
    app_secret: Option<String>,
    webhook_verify_token: Option<String>,
    webhook_public_url: Option<Url>,
    webhook_certificate_file: Option<String>,
    webhook_private_key_file: Option<String>,
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
            webhook_certificate_file: env_optional("WHATSAPP_WEBHOOK_CERTIFICATE_FILE"),
            webhook_private_key_file: env_optional("WHATSAPP_WEBHOOK_PRIVATE_KEY_FILE"),
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

    fn webhook_certificate_file(&self) -> Result<&str> {
        self.webhook_certificate_file
            .as_deref()
            .ok_or_else(|| anyhow!("WHATSAPP_WEBHOOK_CERTIFICATE_FILE is required for webhook modes"))
    }

    fn webhook_private_key_file(&self) -> Result<&str> {
        self.webhook_private_key_file
            .as_deref()
            .ok_or_else(|| anyhow!("WHATSAPP_WEBHOOK_PRIVATE_KEY_FILE is required for webhook modes"))
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
        let public_url = self.webhook_public_url()?;
        let port = public_url
            .port_or_known_default()
            .ok_or_else(|| anyhow!("WHATSAPP_WEBHOOK_PUBLIC_URL must include an HTTPS port"))?;
        Ok(([0, 0, 0, 0], port).into())
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

async fn register_webhook_with_temporary_server(config: &Config, app_client: Client) -> Result<()> {
    let verify_token = config
        .webhook_verify_token()?
        .to_owned();
    let public_url = config.webhook_public_url()?;
    let route = config.webhook_route()?;
    let bind_addr = config.webhook_bind_addr()?;
    let tls_config = load_tls_server_config(config.webhook_certificate_file()?, config.webhook_private_key_file()?)?;
    let tcp_listener = TcpListener::bind(bind_addr)
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: couldn't bind temporary HTTPS webhook verification listener to {bind_addr}: {err}"))?;
    let local_addr = tcp_listener
        .local_addr()
        .map_err(|err| anyhow!("WhatsApp demoscene: couldn't read temporary HTTPS webhook verification listener address: {err}"))?;
    let app = Router::new().route(
        &route,
        get({
            move |Query(query): Query<WebhookVerificationQuery>| {
                let verify_token = verify_token.clone();
                async move { verify_webhook_challenge(&verify_token, query) }
            }
        }),
    );

    println!("serving temporary WhatsApp webhook verifier on {local_addr}{route}");
    println!("asking Meta to verify public HTTPS URL {public_url}");

    let (shutdown_sender, shutdown_receiver) = oneshot::channel::<()>();
    let server_task = tokio::spawn(run_tls_webhook_server(tcp_listener, tls_config, app, async {
        _ = shutdown_receiver.await;
    }));
    wait_for_webhook_listener().await;

    let registration_result = register_webhook(config, &app_client).await;
    _ = shutdown_sender.send(());
    if let Err(shutdown_err) = server_task
        .await
        .map_err(|join_err| anyhow!("WhatsApp demoscene: temporary HTTPS verifier task failed to join: {join_err}"))
        .and_then(|server_result| server_result)
    {
        eprintln!("WhatsApp demoscene: temporary HTTPS verifier shutdown failed: {shutdown_err}");
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
    let bind_addr = config.webhook_bind_addr()?;
    let tls_config = load_tls_server_config(config.webhook_certificate_file()?, config.webhook_private_key_file()?)?;
    let tcp_listener = TcpListener::bind(bind_addr)
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: couldn't bind HTTPS webhook listener to {bind_addr}: {err}"))?;
    let local_addr = tcp_listener
        .local_addr()
        .map_err(|err| anyhow!("WhatsApp demoscene: couldn't read HTTPS webhook listener address: {err}"))?;
    let service = WebhookService::<WhatsAppDemosceneHandler>::builder()
        .verify_token(verify_token)
        .verify_payload(app_secret)
        .build(WhatsAppDemosceneHandler, client.clone());
    let app = Router::new()
        .route(
            &route,
            get({
                let service = service.clone();
                move |req: Request| {
                    let service = service.clone();
                    async move { handle_logged_webhook(service, req).await }
                }
            })
            .post({
                let service = service.clone();
                move |req: Request| {
                    let service = service.clone();
                    async move { handle_logged_webhook(service, req).await }
                }
            }),
        )
        .fallback(handle_unmatched_webhook_route);

    println!("serving WhatsApp HTTPS webhook on {local_addr}{route}");
    println!("expecting Meta to call public HTTPS URL {public_url}");

    let Some(app_client) = app_client else {
        return run_tls_webhook_server(tcp_listener, tls_config, app, future::pending()).await;
    };

    let (shutdown_sender, shutdown_receiver) = oneshot::channel::<()>();
    let server_task = tokio::spawn(run_tls_webhook_server(tcp_listener, tls_config, app, async {
        _ = shutdown_receiver.await;
    }));
    wait_for_webhook_listener().await;

    let registration = config.webhook_registration()?;
    let registration_result = app_client
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
        .map_err(|err| anyhow!("WhatsApp demoscene: registering webhook while serving failed: {err}"));

    if let Err(err) = registration_result {
        _ = shutdown_sender.send(());
        if let Err(shutdown_err) = server_task
            .await
            .map_err(|join_err| anyhow!("WhatsApp demoscene: HTTPS webhook task failed to join after registration failure: {join_err}"))
            .and_then(|server_result| server_result)
        {
            eprintln!("WhatsApp demoscene: HTTPS webhook shutdown after registration failure also failed: {shutdown_err}");
        }
        return Err(err);
    }

    println!("webhook registered for {public_url}");
    server_task
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: HTTPS webhook task failed to join: {err}"))?
}

async fn handle_logged_webhook(service: WebhookService<WhatsAppDemosceneHandler>, req: Request) -> axum::response::Response {
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
    let response = service
        .handle(req)
        .await;
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

            match self
                .acceptor
                .accept(stream)
                .await
            {
                Ok(stream) => return (stream, addr),
                Err(err) => eprintln!("WhatsApp demoscene: HTTPS webhook TLS handshake failed for {addr}: {err}"),
            }
        }
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.listener
            .local_addr()
    }
}

async fn run_tls_webhook_server<Shutdown>(tcp_listener: TcpListener, tls_config: ServerConfig, app: Router, shutdown: Shutdown) -> Result<()>
where
    Shutdown: Future<Output = ()> + Send + 'static,
{
    let listener = TlsListener { listener: tcp_listener, acceptor: TlsAcceptor::from(Arc::new(tls_config)) };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
        .map_err(|err| anyhow!("WhatsApp demoscene: HTTPS webhook server failed: {err}"))
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
