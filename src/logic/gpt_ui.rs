//! NOTE: this module is yet to undergo a refactoring.
//! Here we must keep only the telegram logic and not:
//!   - The driver for the program -- now called `run()`
//!   - The HTTPS server

use crate::models::config::BotConfig;
use log::{error, info};
use std::env;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};
use teloxide::{
    prelude::*,
    types::{InputFile, InputMedia, InputMediaPhoto},
    utils::command::BotCommands,
};
use tokio::time::{Duration, sleep};

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Available commands:")]
enum Cmd {
    /// Show this help
    Help,
    /// Demonstrate progress updates with text edits
    Run,
    /// Demonstrate replacing an image in place
    Render,
    /// Open the inline menu with buttons
    Menu,
}

pub async fn run(config: &BotConfig) -> anyhow::Result<()> {
    unsafe {
        std::env::set_var(
            "TELOXIDE_TOKEN",
            config.telegram_config.teloxide_token.clone(),
        );
    }
    let bot = Bot::from_env(); // expects TELOXIDE_TOKEN. How to not involve the environment to pass in this information?
    let mode = env::var("MODE").unwrap_or_else(|_| "polling".into());

    match mode.as_str() {
        "webhook" => run_webhook(bot).await?,
        _ => run_polling(bot).await?,
    }
    Ok(())
}

async fn run_polling(bot: Bot) -> anyhow::Result<()> {
    info!("Starting in LONG-POLLING mode");
    teloxide::repl(bot, handler).await;
    Ok(())
}

async fn run_webhook(bot: Bot) -> anyhow::Result<()> {
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
        .branch(dptree::endpoint(handler))
        .branch(Update::filter_callback_query().endpoint(on_callback));
    Dispatcher::builder(bot, handlers)
        .enable_ctrlc_handler()
        .build()
        .dispatch_with_listener(listener, LoggingErrorHandler::new())
        .await;
    Ok(())
}

fn main_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("🧪 Progress demo", "demo:progress"),
            InlineKeyboardButton::callback("🖼️ Swap media demo", "demo:swap"),
        ],
        vec![
            InlineKeyboardButton::callback("⚙️ Settings", "menu:settings"),
            InlineKeyboardButton::callback("❌ Close", "menu:close"),
        ],
    ])
}

async fn handler(bot: Bot, msg: Message) -> ResponseResult<()> {
    if let Some(text) = msg.text() {
        if let Ok(cmd) = Cmd::parse(text, "tg_demoscene_bot") {
            match cmd {
                Cmd::Help => {
                    bot.send_message(msg.chat.id, Cmd::descriptions().to_string())
                        .await?;
                }
                Cmd::Run => run_long_job(bot, msg).await?,
                Cmd::Render => render_swap(bot, msg).await?,
                Cmd::Menu => {
                    bot.send_message(msg.chat.id, "Main menu:")
                        .reply_markup(main_menu())
                        .await?;
                }
            }
        } else {
            bot.send_message(msg.chat.id, "Try /help").await?;
        }
    }
    Ok(())
}

async fn on_callback(bot: Bot, callback_query: CallbackQuery) -> ResponseResult<()> {
    eprintln!("CALLBACK CALLED");
    if let Some(callback_data) = callback_query.data.as_deref() {
        // Always answer callback so the client stops the “loading” spinner
        let _ = bot.answer_callback_query(callback_query.id).await;
        let chat_id = callback_query
            .message
            .as_ref()
            .map(|message| message.chat().id);
        match (callback_data, chat_id) {
            ("demo:progress", Some(chat)) => {
                bot.send_message(chat, "Starting progress demo…").await?;
                // Spawn to avoid blocking webhook/polling loop
                let bot2 = bot.clone();
                tokio::spawn(async move {
                    // fake Message wrapper: send placeholder within run_long_job itself
                    if let Err(e) = bot2.send_message(chat, "/run").await {
                        error!("Failed to enqueue /run: {e}");
                    }
                });
            }
            ("demo:swap", Some(chat)) => {
                bot.send_message(chat, "Starting media-swap demo…").await?;
                let bot2 = bot.clone();
                tokio::spawn(async move {
                    // enqueue the /render path for simplicity; you can call render_swap() directly if you refactor it to accept ChatId
                    if let Err(e) = bot2.send_message(chat, "/render").await {
                        error!("Failed to enqueue /render: {e}");
                    }
                });
            }
            ("menu:settings", Some(chat)) => {
                // Placeholder; we’ll hook redb-backed prefs here next.
                bot.edit_message_text(
                    chat,
                    callback_query.message.as_ref().unwrap().id(),
                    "Settings (TBD)…",
                )
                .reply_markup(main_menu())
                .await?;
            }
            ("menu:close", Some(chat)) => {
                let mid = callback_query.message.as_ref().unwrap().id();
                let _ = bot.edit_message_reply_markup(chat, mid).await; // remove buttons
            }
            _ => {
                eprintln!("BUG! menu command not recognized: {callback_data}");
            }
        }
    }
    Ok(())
}

async fn run_long_job(bot: Bot, msg: Message) -> ResponseResult<()> {
    let chat = msg.chat.id;
    let mut m = bot.send_message(chat, "Working… 0%").await?;

    for p in [5, 15, 35, 60, 85, 100] {
        sleep(Duration::from_millis(900)).await; // keep ≲1 edit/sec per chat
        m = bot
            .edit_message_text(chat, m.id, format!("Working… {p}%"))
            .await?;
    }

    // Final artifact
    bot.send_document(chat, InputFile::file("result.zip"))
        .caption("Here’s your result.")
        .await?;
    bot.edit_message_text(chat, m.id, "✅ Done. See the file above.")
        .await?;
    Ok(())
}

async fn render_swap(bot: Bot, msg: Message) -> ResponseResult<()> {
    let chat = msg.chat.id;
    let m = bot
        .send_photo(chat, InputFile::file("frame0.png"))
        .caption("Rendering 0%")
        .await?;

    let frames = [
        ("frame1.png", "25%"),
        ("frame2.png", "70%"),
        ("frame3.png", "99%"),
    ];
    for (f, cap) in frames {
        sleep(Duration::from_millis(900)).await;
        let media = InputMedia::Photo(
            InputMediaPhoto::new(InputFile::file(f)).caption(format!("Rendering {cap}")),
        );
        bot.edit_message_media(chat, m.id, media).await?;
    }
    bot.edit_message_caption(chat, m.id)
        .caption("✅ Render complete.")
        .await?;
    Ok(())
}
