//! This is a Telegram-exclusive logic

use crate::db::{heed, redb, sqlite};
use crate::messaging::contracts::messaging::Messaging;
use crate::messaging::impls::telegram_gateway::{TelegramGateway, TelegramMo, mt, mts};
use crate::models::config::BotConfig;
use crate::plot;
use crate::resources::{FRAMES, RESULT};
use anyhow::{Result, anyhow};
use futures::StreamExt;
use std::sync::Arc;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};
use teloxide::{
    prelude::*,
    types::{InputFile, InputMedia, InputMediaPhoto},
    utils::command::BotCommands,
};
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

const MT_CONCURRENCY: usize = 4;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "Available commands:")]
enum Cmd {
    /// Show this help message
    Help,
    /// Also show this help message. Meant to be used when first interacting with this bot.
    Start,
    /// Show your ChatID
    ChatId,
    /// Reply to a message then type `/pin`: that message will be pinned to the Chat Window
    Pin,
    /// Go into graphics mode -- click-based navigation instead of typing
    Graphical,
    /// Same as `/graphical`: open the inline menu with buttons
    Menu,
    /// Demonstrate progress updates with text edits
    Run,
    /// Demonstrate replacing an image in place
    Render,
    /// Perform the SQLite benchmark
    SqliteBenchmark,
    /// Perform the ReDB benchmark
    RedbBenchmark,
    /// Perform the Heed benchmark
    HeedBenchmark,
    /// Perform a Live Trade Analysis simulation
    FollowAsset,
}

pub async fn run(config: BotConfig) -> Result<()> {
    let (telegram_gateway, mo_stream) = TelegramGateway::new(config);
    let bot = telegram_gateway
        .bot()
        .clone();
    #[cfg(debug_assertions)]
    let mo_stream = mo_stream.inspect(|mo| log::info!("MO: {mo:?}"));
    let mt_stream = mo_stream.map(move |telegram_mo| {
        let bot = bot.clone();
        let chat_id = ChatId(
            telegram_mo
                .dialog()
                .id() as i64,
        );
        let payload = telegram_mo.payload();
        match payload {
            TelegramMo::Message(msg) => {
                if let Some(text) = msg.text() {
                    if let Ok(cmd) = Cmd::parse(text, "tg_demoscene_bot") {
                        match cmd {
                            Cmd::Start => mt(bot.send_message(chat_id, format!("Welcome to the <b>OgreRobot's Telegram Demoscene</b>!\nPick an option:\n{}", Cmd::descriptions()))),
                            Cmd::Help => mt(bot.send_message(chat_id, Cmd::descriptions().to_string())),
                            Cmd::ChatId => {
                                mt(bot.send_message(chat_id, format!("Your <u>UserId</u>/<u>ChatId</u> is <b>{}</b>\nIt can be used to send you <i>daily messages</i>.\nShare wisely...", chat_id)))
                            }
                            Cmd::Run => mts(run_long_job(bot.clone(), chat_id)),
                            Cmd::Render => mts(render_swap(bot.clone(), chat_id)),
                            Cmd::Graphical | Cmd::Menu => mt(bot
                                .send_message(chat_id, "Choose:")
                                .reply_markup(main_menu())),
                            Cmd::Pin => {
                                if let Some(reply) = msg.reply_to_message() {
                                    mt(bot.pin_chat_message(chat_id, reply.id))
                                } else {
                                    mt(bot
                                        .send_message(chat_id, "Reply to a textual message, then type `/pin`")
                                        .reply_markup(main_menu()))
                                }
                            }
                            Cmd::SqliteBenchmark => mts(sqlite_benchmark(bot.clone(), chat_id)),
                            Cmd::RedbBenchmark => mts(redb_benchmark(bot.clone(), chat_id)),
                            Cmd::HeedBenchmark => mts(heed_benchmark(bot.clone(), chat_id)),
                            Cmd::FollowAsset => mts(follow_asset(bot.clone(), chat_id)),
                        }
                    } else {
                        mt(bot.send_message(chat_id, "Unknown command. Try /help"))
                    }
                } else {
                    mt(bot.send_message(chat_id, "Try sending the text /help"))
                }
            }
            TelegramMo::CallbackQuery(callback_query) => {
                let callback_data = callback_query
                    .data
                    .as_deref()
                    .unwrap_or_default()
                    .to_string();
                let msg_id = callback_query
                    .message
                    .as_ref()
                    .and_then(|message| message.regular_message())
                    .map(|a| a.id)
                    .unwrap_or_default();
                let callback_id = callback_query
                    .id
                    .clone();
                mts(async move {
                    match callback_data.as_str() {
                        "mm:run" => run_long_job(bot.clone(), chat_id).await?,
                        "mm:render" => render_swap(bot.clone(), chat_id).await?,
                        "mm:chatid" => bot
                            .send_message(chat_id, format!("Your <u>UserId</u>/<u>ChatId</u> is <b>{}</b>\nIt can be used to send you <i>daily messages</i>.\nShare wisely...", chat_id))
                            .await
                            .map(|_| ())
                            .map_err(|err| anyhow!("`chat_id` failed: {err}"))?,
                        "mm:followasset" => follow_asset(bot.clone(), chat_id).await?,
                        "mm:sqlitebenchmark" => sqlite_benchmark(bot.clone(), chat_id).await?,
                        "mm:redbbenchmark" => redb_benchmark(bot.clone(), chat_id).await?,
                        "mm:heedbenchmark" => heed_benchmark(bot.clone(), chat_id).await?,
                        "mm:close" => bot
                            .edit_message_reply_markup(chat_id, msg_id)
                            .await
                            .map(|_| ())?, // remove the "main menu" buttons
                        _ => bot
                            .send_message(chat_id, format!("BUG! Unknown callback query '{callback_data}'"))
                            .await
                            .map(|_| ())?,
                    };
                    bot.answer_callback_query(callback_id)
                        .await
                        .map_err(|err| anyhow!("Updating the callback's spinner failed: {err}")) // Answer the callback request so the client stops the “loading” spinner
                })
            }
        }
    });
    let join_handle = telegram_gateway.consume_mt_stream(MT_CONCURRENCY, mt_stream);
    join_handle
        .await
        .map_err(|err| anyhow!("MT task failed: {err}"))
}

fn main_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback("🧪 Progress demo", "mm:run"), InlineKeyboardButton::callback("🖼️ Swap media demo", "mm:render")],
        vec![InlineKeyboardButton::callback("💩 Show Your Chat ID", "mm:chatid"), InlineKeyboardButton::callback("📈 Live Trade Simulation", "mm:followasset")],
        vec![InlineKeyboardButton::callback("🪶 SQLite Benchmarks", "mm:sqlitebenchmark"), InlineKeyboardButton::callback("🦀 ReDB Benchmarks", "mm:redbbenchmark")],
        vec![InlineKeyboardButton::callback("🧠 Heed Benchmarks", "mm:heedbenchmark")],
        vec![InlineKeyboardButton::callback("⚙️ Settings", "mm:settings"), InlineKeyboardButton::callback("❌ Close", "mm:close")],
    ])
}

async fn run_long_job(bot: Bot, chat_id: ChatId) -> Result<()> {
    let mut m = bot
        .send_message(chat_id, "Working… 0%")
        .await?;

    for p in [5, 15, 35, 60, 85, 100] {
        sleep(Duration::from_millis(900)).await; // keep ≲1 edit/sec per chat
        m = bot
            .edit_message_text(chat_id, m.id, format!("Working… {p}%"))
            .await?;
    }

    bot.edit_message_text(chat_id, m.id, "✅ Done. See the file bellow.")
        .await?;
    // Final artifact
    bot.send_document(chat_id, InputFile::memory(RESULT).file_name("result.zip"))
        .caption("Here’s your result.")
        .await
        .map(|_| ())
        .map_err(|err| anyhow!("`run_long_job` failed: {err}"))
}

async fn render_swap(bot: Bot, chat_id: ChatId) -> Result<()> {
    let m = bot
        .send_photo(chat_id, InputFile::memory(FRAMES[0]).file_name("frame0.png"))
        .caption("Rendering 0%")
        .await?;

    let frames = [(FRAMES[1], "25%", "frame1.png"), (FRAMES[2], "70%", "frame2.png"), (FRAMES[3], "99%", "frame3.png")];
    for (f, cap, name) in frames {
        sleep(Duration::from_millis(900)).await;
        let media = InputMedia::Photo(InputMediaPhoto::new(InputFile::memory(f).file_name(name)).caption(format!("Rendering {cap}")));
        bot.edit_message_media(chat_id, m.id, media)
            .await?;
    }
    bot.edit_message_caption(chat_id, m.id)
        .caption("✅ Render complete.")
        .await
        .map(|_| ())
        .map_err(|err| anyhow!("`render_swap` failed: {err}"))
}

async fn sqlite_benchmark(bot: Bot, chat_id: ChatId) -> Result<()> {
    let m = Arc::new(Mutex::new(
        bot.send_message(chat_id, "Starting SQLite Benchmark...")
            .await?,
    ));
    sqlite::benchmark::ui_benchmark(async move |mt| {
        let mut m = m
            .lock()
            .await;
        *m = bot
            .edit_message_text(chat_id, m.id, mt)
            .await?;
        Ok::<(), teloxide::RequestError>(())
    })
    .await
    .map_err(|err| anyhow!("`sqlitebenchmark` failed: {err}"))
}

async fn redb_benchmark(bot: Bot, chat_id: ChatId) -> Result<()> {
    let m = Arc::new(Mutex::new(
        bot.send_message(chat_id, "Starting ReDB Benchmark...")
            .await?,
    ));
    redb::benchmark::ui_benchmark(async move |mt| {
        let mut m = m
            .lock()
            .await;
        *m = bot
            .edit_message_text(chat_id, m.id, mt)
            .await?;
        Ok::<(), teloxide::RequestError>(())
    })
    .await
    .map_err(|err| anyhow!("`redbbenchmark` failed: {err}"))
}

async fn heed_benchmark(bot: Bot, chat_id: ChatId) -> Result<()> {
    let m = Arc::new(Mutex::new(
        bot.send_message(chat_id, "Starting Heed Benchmark...")
            .await?,
    ));
    heed::benchmark::benchmark(async move |mt| {
        let mut m = m
            .lock()
            .await;
        *m = bot
            .edit_message_text(chat_id, m.id, mt)
            .await?;
        Ok::<(), teloxide::RequestError>(())
    })
    .await
    .map_err(|err| anyhow!("`heedbenchmark` failed: {err}"))
}

async fn follow_asset(bot: Bot, chat_id: ChatId) -> Result<()> {
    plot::demo::demo(&bot, chat_id)
        .await
        .map_err(|err| anyhow!("`follow-asset` demo failed: {err}"))
}
