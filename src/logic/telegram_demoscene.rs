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
    /// Simulates you received a Broadcast message for the starting day
    Broadcast,
}

struct ProcessUserMo;

/// Texts to be used both in the InlineKeyboardMarkup Menu (MT leg) and the parsing (MO leg)
mod callbacks {
    pub const RUN: &str = "mm:run";
    pub const RENDER: &str = "mm:render";
    pub const ADDITIONAL_MEDIA: &str = "mm:additionalmedia";
    pub const CHAT_ID: &str = "mm:chatid";
    pub const FOLLOW_ASSET: &str = "mm:followasset";
    pub const SQLITE_BENCHMARK: &str = "mm:sqlitebenchmark";
    pub const REDB_BENCHMARK: &str = "mm:redbbenchmark";
    pub const HEED_BENCHMARK: &str = "mm:heedbenchmark";
    pub const BROADCAST: &str = "mm:broadcast";
    pub const SETTINGS: &str = "mm:settings";
    pub const CLOSE: &str = "mm:close";
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
                                Cmd::Broadcast => mts(broadcast(bot.clone(), chat_id)),
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
                            callbacks::BROADCAST => broadcast(bot.clone(), chat_id).await?,
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
        vec![InlineKeyboardButton::callback("💩 Show Your Chat ID", callbacks::CHAT_ID)],
        vec![InlineKeyboardButton::callback("📈 Live Trade Simulation", callbacks::FOLLOW_ASSET), InlineKeyboardButton::callback("💩 Broadcast demo", callbacks::BROADCAST)],
        vec![InlineKeyboardButton::callback("🧪 Progress demo", callbacks::RUN), InlineKeyboardButton::callback("🖼️ Swap media demo", callbacks::RENDER)],
        vec![
            InlineKeyboardButton::callback("🎞️ Additional Media", callbacks::ADDITIONAL_MEDIA),
            InlineKeyboardButton::callback("🧠 Heed Benchmarks", callbacks::HEED_BENCHMARK),
        ],
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
    // bot.send_video_note(chat_id, InputFile::memory(DEMO_VIDEO.bytes).file_name("demo_video_note.mp4"))
    //     .duration(3)
    //     .length(320)
    //     .await?;
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

async fn broadcast(bot: Bot, chat_id: ChatId) -> Result<()> {
    let ai_analysis = r#"<b>Boletim Diário do Mercado - Análise para Day-Trade (Referente a 15/07/2026)</b><br><br>

<i>Bom dia, trader! Para planejar sua estratégia de hoje, analisamos o comportamento do mercado no pregão de ontem (quarta-feira, 15 de julho de 2026). O cenário apresentou oportunidades claras tanto na ponta compradora quanto na vendedora, com forte concentração de liquidez nos ativos tradicionais e movimentos extremos fora do índice.</i><br><br>

<b>1. Panorama do IBOVESPA (Altas e Baixas)</b><br>
O índice mostrou comportamento misto, excelente para estratégias de momentum e reversão:<br>
• <b>Maiores Altas:</b> Lideradas por <b>TOTS3 (+4,17%)</b> e <b>GGBR4 (+3,77%)</b>. Estes ativos demonstraram forte pressão compradora e devem ser monitorados na abertura de hoje para possíveis operações de continuidade de tendência (<i>Trend Following</i>).<br>
• <b>Maiores Baixas:</b> A ponta vendedora foi puxada por <b>BRKM5 (-6,14%)</b> e <b>EGIE3 (-5,11%)</b>. Fique atento a esses papéis na perda das mínimas de ontem para trades de venda descoberta (<i>Shorting</i>).<br><br>

<b>2. Liquidez e Volume (Onde o dinheiro está posicionado)</b><br>
Como day-traders, precisamos de liquidez para entrar e sair rapidamente das posições. Ontem, o volume se concentrou fortemente em:<br>
• <b>IBOV11:</b> Dominou com 31,48% de participação.<br>
• <b>VALE3 (3,93% part.)</b> e <b>PETR4 (3,54% part.):</b> Mantêm-se como as melhores opções para trades rápidos com spreads curtos.<br>
• <b>AXIA3:</b> Chamou atenção com 3,31% de participação no mercado à vista, fechando em queda de <b>-4,19%</b>. O alto volume acompanhado de desvalorização sugere forte presença institucional na venda. Pode abrir espaço para forte volatilidade hoje.<br><br>

<b>3. Alerta de Volatilidade Extrema (Fora do Índice)</b><br>
Para quem busca trades de risco agressivo em papéis menores do mercado à vista:<br>
• <b>RDLI11</b> disparou extraordinários <b>+58,33%</b>.<br>
• Na contramão, <b>ONCO11</b> despencou impressionantes <b>-66,66%</b>, seguida por <b>CARE11 (-35,93%)</b>. <i>Atenção:</i> Opere esses ativos com mão muito reduzida devido ao risco de gap e falta de contraparte instantânea.<br><br>

<b>4. Insights de Estratégia para Hoje:</b><br>
<pre>
+------------------+-----------------------+------------------------------------------+
| Ativo            | Direção Estimada      | Gatilho / Justificativa                  |
+------------------+-----------------------+------------------------------------------+
| TOTS3 / GGBR4    | Compra (Continuidade) | Rompimento da máxima de ontem.           |
| BRKM5            | Venda (Continuidade)  | Perda da mínima de ontem (forte momentum)|
| AXIA3            | Compra/Venda (Volume) | Monitorar abertura; alto volume ontem.   |
| VALE3 / PETR4    | Scalping              | Operações rápidas aproveitando o book.   |
+------------------+-----------------------+------------------------------------------+
</pre>

<i>Lembre-se sempre de gerenciar seu risco, definir seus stops antes de entrar na operação e acompanhar a abertura dos mercados internacionais para alinhar a tendência macro. Bons trades!</i>"#;
    bot.send_message(chat_id, ai_analysis)
        .parse_mode(ParseMode::Html)
        .await
        .map(|_message| ())
        .map_err(|err| anyhow!("`broadcast` demo failed: {err}"))
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
