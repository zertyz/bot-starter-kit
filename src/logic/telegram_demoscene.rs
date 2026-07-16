//! This is a Telegram-exclusive logic

use crate::db::{heed, redb, sqlite};
use crate::messaging::contracts::messaging::Mo;
use crate::messaging::gateways::telegram_gateway::{TelegramBoxSendFuture, TelegramGateway, TelegramMo, mt, mts};
use crate::messaging::user_router::UserMoProcessor;
use crate::models::config::BotConfig;
use crate::plot;
use crate::resources::{DEMO_AUDIO, DEMO_STICKER, DEMO_VIDEO, DEMO_VOICE, FRAMES, RESULT};
use anyhow::{Result, anyhow};
use futures::{Stream, StreamExt};
use std::sync::Arc;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode, User};
use teloxide::{
    prelude::*,
    types::{InputFile, InputMedia, InputMediaPhoto},
    utils::command::BotCommands,
};
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};

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
    /// Send embedded audio, voice, video, video-note, and sticker samples
    AdditionalMedia,
    /// Perform the SQLite benchmark
    SqliteBenchmark,
    /// Perform the ReDB benchmark
    RedbBenchmark,
    /// Perform the Heed benchmark
    HeedBenchmark,
    /// Perform a Live Trade Analysis simulation
    FollowAsset,
}

struct ProcessUserMo;

/// Texts to be used both in the InlineKeyboardMarkup Menu (MT leg) and the parsing (MO leg)
mod callbacks {
    pub(super) const RUN: &str = "mm:run";
    pub(super) const RENDER: &str = "mm:render";
    pub(super) const ADDITIONAL_MEDIA: &str = "mm:additionalmedia";
    pub(super) const CHAT_ID: &str = "mm:chatid";
    pub(super) const FOLLOW_ASSET: &str = "mm:followasset";
    pub(super) const SQLITE_BENCHMARK: &str = "mm:sqlitebenchmark";
    pub(super) const REDB_BENCHMARK: &str = "mm:redbbenchmark";
    pub(super) const HEED_BENCHMARK: &str = "mm:heedbenchmark";
    pub(super) const SETTINGS: &str = "mm:settings";
    pub(super) const CLOSE: &str = "mm:close";
}

impl UserMoProcessor<User, Bot, TelegramMo, TelegramBoxSendFuture> for ProcessUserMo {
    async fn process<MoStream: Stream<Item = Mo<User, TelegramMo>> + Send>(&self, bot: Bot, user_mo_stream: MoStream) -> impl Stream<Item = TelegramBoxSendFuture> + Send {
        let user_mo_stream = user_mo_stream.inspect(move |mo| {
            log::debug!(
                "User '{}' MO: {mo:?}",
                mo.sender()
                    .inner
                    .first_name
            )
        });
        user_mo_stream.map(move |mo| {
            let chat_id = ChatId(
                mo.sender()
                    .inner
                    .id
                    .0 as i64,
            );
            let payload = mo.payload();
            match payload {
                TelegramMo::Message(msg) => {
                    if let Some(text) = msg.text() {
                        if let Ok(cmd) = Cmd::parse(text, "telegram_demoscene") {
                            match cmd {
                                Cmd::Start => mt(bot
                                    .send_message(chat_id, format!("Welcome to the <b>OgreRobot's Telegram Demoscene</b>!\nPick an option:\n{}", Cmd::descriptions()))
                                    .parse_mode(ParseMode::Html)),
                                Cmd::Help => mt(bot.send_message(chat_id, Cmd::descriptions().to_string())),
                                Cmd::ChatId => mt(bot
                                    .send_message(chat_id, format!("Your <u>UserId</u>/<u>ChatId</u> is <b>{}</b>\nIt can be used to send you <i>daily messages</i>.\nShare wisely...", chat_id))
                                    .parse_mode(ParseMode::Html)),
                                Cmd::Run => mts(run_long_job(bot.clone(), chat_id)),
                                Cmd::Render => mts(render_swap(bot.clone(), chat_id)),
                                Cmd::AdditionalMedia => mts(additional_media(bot.clone(), chat_id)),
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
                    let bot = bot.clone();
                    mts(async move {
                        match callback_data.as_str() {
                            callbacks::RUN => run_long_job(bot.clone(), chat_id).await?,
                            callbacks::RENDER => render_swap(bot.clone(), chat_id).await?,
                            callbacks::ADDITIONAL_MEDIA => additional_media(bot.clone(), chat_id).await?,
                            callbacks::CHAT_ID => bot
                                .send_message(chat_id, format!("Your <u>UserId</u>/<u>ChatId</u> is <b>{}</b>\nIt can be used to send you <i>daily messages</i>.\nShare wisely...", chat_id))
                                .parse_mode(ParseMode::Html)
                                .await
                                .map(|_| ())
                                .map_err(|err| anyhow!("`chat_id` failed: {err}"))?,
                            callbacks::FOLLOW_ASSET => follow_asset(bot.clone(), chat_id).await?,
                            callbacks::SQLITE_BENCHMARK => sqlite_benchmark(bot.clone(), chat_id).await?,
                            callbacks::REDB_BENCHMARK => redb_benchmark(bot.clone(), chat_id).await?,
                            callbacks::HEED_BENCHMARK => heed_benchmark(bot.clone(), chat_id).await?,
                            callbacks::CLOSE => bot
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
        })
    }
}

pub async fn run(config: BotConfig) -> Result<()> {
    let telegram_gateway = TelegramGateway::new(config, ProcessUserMo).await;
    telegram_gateway
        .await_termination()
        .await
}

fn main_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback("💩 Show Your Chat ID", callbacks::CHAT_ID), InlineKeyboardButton::callback("📈 Live Trade Simulation", callbacks::FOLLOW_ASSET)],
        vec![
            InlineKeyboardButton::callback("🎞️ Additional Media", callbacks::ADDITIONAL_MEDIA),
            InlineKeyboardButton::callback("🧠 Heed Benchmarks", callbacks::HEED_BENCHMARK),
        ],
        vec![InlineKeyboardButton::callback("🧪 Progress demo", callbacks::RUN), InlineKeyboardButton::callback("🖼️ Swap media demo", callbacks::RENDER)],
        vec![
            InlineKeyboardButton::callback("🪶 SQLite Benchmarks", callbacks::SQLITE_BENCHMARK),
            InlineKeyboardButton::callback("🦀 ReDB Benchmarks", callbacks::REDB_BENCHMARK),
        ],
        vec![InlineKeyboardButton::callback("⚙️ Settings", callbacks::SETTINGS), InlineKeyboardButton::callback("❌ Close", callbacks::CLOSE)],
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

async fn additional_media(bot: Bot, chat_id: ChatId) -> Result<()> {
    bot.send_audio(chat_id, InputFile::memory(DEMO_AUDIO.bytes).file_name(DEMO_AUDIO.file_name))
        .title("OgreRobot Demo Chime")
        .performer("OgreRobot")
        .caption("Embedded MP3 audio")
        .await?;
    bot.send_voice(chat_id, InputFile::memory(DEMO_VOICE.bytes).file_name(DEMO_VOICE.file_name))
        .caption("Embedded OGG/Opus voice message")
        .await?;
    bot.send_video(chat_id, InputFile::memory(DEMO_VIDEO.bytes).file_name(DEMO_VIDEO.file_name))
        .caption("Embedded H.264 MP4 video")
        .supports_streaming(true)
        .await?;
    bot.send_video_note(chat_id, InputFile::memory(DEMO_VIDEO.bytes).file_name("demo_video_note.mp4"))
        .duration(3)
        .length(320)
        .await?;
    bot.send_sticker(chat_id, InputFile::memory(DEMO_STICKER.bytes).file_name(DEMO_STICKER.file_name))
        .await
        .map(|_| ())
        .map_err(|err| anyhow!("`additional_media` failed: {err}"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use teloxide::types::InlineKeyboardButtonKind;

    #[test]
    fn additional_media_has_text_and_menu_commands() {
        assert!(matches!(Cmd::parse("/additionalmedia", "telegram_demoscene"), Ok(Cmd::AdditionalMedia)));

        let has_menu_button = main_menu()
            .inline_keyboard
            .into_iter()
            .flatten()
            .any(|button| {
                button
                    .text
                    .contains("Additional Media")
                    && matches!(button.kind, InlineKeyboardButtonKind::CallbackData(data) if data == callbacks::ADDITIONAL_MEDIA)
            });
        assert!(has_menu_button);
    }
}
