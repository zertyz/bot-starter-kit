use crate::db::{redb, sqlite};
use crate::models::config::BotConfig;
use crate::plot;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use teloxide::prelude::*;
use teloxide::types::{
    ChatId as TgChatId, ChatJoinRequest, InputFile, Me, Message, MessageId as TgMessageId,
    ParseMode, UserId as TgUserId,
};
use teloxide::{dptree, utils::command::BotCommands};

/// ---------- Platform-agnostic model ----------
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MediaKind {
    Photo,
    Video,
    Audio,
    Document,
    Animation,
    Voice,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ThrottlingHint {
    /// per-user messages per second (if known)
    pub per_user_mps: Option<f32>,
    /// per-chat messages per second (if known)
    pub per_chat_mps: Option<f32>,
}

#[derive(Debug, Clone /*Serialize, Deserialize*/)]
pub struct Capabilities {
    pub can_edit_messages: bool,
    pub rich_text_modes: Vec<&'static str>,
    pub media_supported: Vec<MediaKind>,
    pub supports_reactions: bool, // WhatsApp Cloud API: no edit, reactions differ; Telegram: yes
    pub supports_group_admin: bool, // ban/restrict/pin/invite/etc.
    pub throttling: Option<ThrottlingHint>,
}

#[derive(Debug, Clone)]
pub struct FormattedText {
    /// Use platform-supported formatting (Telegram: HTML/MarkdownV2)
    pub html: String,
}

#[derive(Debug, Clone)]
pub enum UnifiedMedia {
    Url {
        kind: MediaKind,
        url: String,
        caption_html: Option<String>,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct ChatId(i64);
#[derive(Debug, Clone, Copy)]
pub struct MessageId(i32);
#[derive(Debug, Clone, Copy)]
pub struct UserId(u64);

/// Channel surface (transport + meta).
pub trait Channel {
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> &Capabilities;
}

/// Send/edit + media; group admin in a separate trait so backends that
/// don’t support it can omit the impl cleanly.

pub trait Sender {
    async fn send_text(&self, chat: ChatId, text: &FormattedText) -> Result<MessageId>;
    async fn edit_text(&self, chat: ChatId, message: MessageId, text: &FormattedText)
    -> Result<()>;
    async fn send_media(&self, chat: ChatId, media: &UnifiedMedia) -> Result<MessageId>;
}


pub trait AdminOps {
    async fn pin_message(&self, chat: ChatId, message: MessageId) -> Result<()>;
    async fn create_invite_link(
        &self,
        chat: ChatId,
        name: &str,
        join_request: bool,
    ) -> Result<String>;
    async fn approve_join_request(&self, chat: ChatId, user: UserId) -> Result<()>;
    async fn ban_user(&self, chat: ChatId, user: UserId) -> Result<()>;
    async fn unban_user(&self, chat: ChatId, user: UserId) -> Result<()>;
}

/// ---------- Telegram adapter ----------
#[derive(Clone)]
pub struct TelegramAdapter {
    bot: Bot,
    caps: Capabilities,
}

impl TelegramAdapter {
    pub fn new(bot: Bot) -> Self {
        let caps = Capabilities {
            can_edit_messages: true,
            rich_text_modes: vec!["HTML", "MarkdownV2"],
            media_supported: vec![
                MediaKind::Photo,
                MediaKind::Video,
                MediaKind::Audio,
                MediaKind::Document,
                MediaKind::Animation,
                MediaKind::Voice,
            ],
            // Telegram supports reactions; `teloxide` mapping exists in current API range.
            supports_reactions: true,
            supports_group_admin: true,
            // WhatsApp has explicit throttles; Telegram also rate-limits (429).
            throttling: Some(ThrottlingHint {
                per_user_mps: None,
                per_chat_mps: None,
            }),
        };
        Self { bot, caps }
    }

    fn tchat(chat: ChatId) -> TgChatId {
        TgChatId(chat.0)
    }
    fn tmsg(msg: MessageId) -> TgMessageId {
        TgMessageId(msg.0)
    }
    fn tuser(u: UserId) -> TgUserId {
        TgUserId(u.0)
    }
}

impl Channel for TelegramAdapter {
    fn name(&self) -> &'static str {
        "telegram"
    }
    fn capabilities(&self) -> &Capabilities {
        &self.caps
    }
}

impl Sender for TelegramAdapter {
    async fn send_text(&self, chat: ChatId, text: &FormattedText) -> Result<MessageId> {
        let sent = self
            .bot
            .send_message(Self::tchat(chat), text.html.clone())
            .parse_mode(ParseMode::Html)
            .await?;
        Ok(MessageId(sent.id.0))
    }

    async fn edit_text(
        &self,
        chat: ChatId,
        message: MessageId,
        text: &FormattedText,
    ) -> Result<()> {
        self.bot
            .edit_message_text(Self::tchat(chat), Self::tmsg(message), text.html.clone())
            .parse_mode(ParseMode::Html)
            .await?;
        Ok(())
    }

    async fn send_media(&self, chat: ChatId, media: &UnifiedMedia) -> Result<MessageId> {
        match media {
            UnifiedMedia::Url {
                kind,
                url,
                caption_html,
            } => {
                let caption = caption_html.clone().unwrap_or_default();
                let mid = match kind {
                    MediaKind::Photo | MediaKind::Animation => {
                        let sent = self
                            .bot
                            .send_photo(Self::tchat(chat), InputFile::url(url.parse()?))
                            .caption(caption)
                            .parse_mode(ParseMode::Html)
                            .await?;
                        sent.id
                    }
                    MediaKind::Video => {
                        let sent = self
                            .bot
                            .send_video(Self::tchat(chat), InputFile::url(url.parse()?))
                            .caption(caption)
                            .parse_mode(ParseMode::Html)
                            .await?;
                        sent.id
                    }
                    MediaKind::Audio | MediaKind::Voice => {
                        let sent = self
                            .bot
                            .send_audio(Self::tchat(chat), InputFile::url(url.parse()?))
                            .caption(caption)
                            .parse_mode(ParseMode::Html)
                            .await?;
                        sent.id
                    }
                    MediaKind::Document => {
                        let sent = self
                            .bot
                            .send_document(Self::tchat(chat), InputFile::url(url.parse()?))
                            .caption(caption)
                            .parse_mode(ParseMode::Html)
                            .await?;
                        sent.id
                    }
                };
                Ok(MessageId(mid.0))
            }
        }
    }
}

impl AdminOps for TelegramAdapter {
    async fn pin_message(&self, chat: ChatId, message: MessageId) -> Result<()> {
        self.bot
            .pin_chat_message(Self::tchat(chat), Self::tmsg(message))
            .await?;
        Ok(())
    }

    async fn create_invite_link(
        &self,
        chat: ChatId,
        name: &str,
        join_request: bool,
    ) -> Result<String> {
        let link = self
            .bot
            .create_chat_invite_link(Self::tchat(chat))
            .name(name.to_string())
            .creates_join_request(join_request)
            .await?;
        Ok(link.invite_link)
    }

    async fn approve_join_request(&self, chat: ChatId, user: UserId) -> Result<()> {
        self.bot
            .approve_chat_join_request(Self::tchat(chat), Self::tuser(user))
            .await?;
        Ok(())
    }

    async fn ban_user(&self, chat: ChatId, user: UserId) -> Result<()> {
        self.bot
            .ban_chat_member(Self::tchat(chat), Self::tuser(user))
            .await?;
        Ok(())
    }

    async fn unban_user(&self, chat: ChatId, user: UserId) -> Result<()> {
        self.bot
            .unban_chat_member(Self::tchat(chat), Self::tuser(user))
            .await?;
        Ok(())
    }
}

/// ---------- Demo bot ----------
#[derive(Debug, BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
#[command(description = "Commands")]
enum Command {
    #[command(description = "Show capabilities")]
    Caps,
    #[command(description = "Show your ChatID")]
    ChatId,
    #[command(description = "Send and edit a message")]
    DemoEdit,
    #[command(description = "Send a photo. Usage: /photo <url?>")]
    Photo(String),
    #[command(description = "Create an invite link (join request). Usage: /invite <name>")]
    Invite(String),
    #[command(description = "Pin the message you replied to")]
    Pin,
    #[command(description = "Ban a user id (admin only). Usage: /ban <user_id>")]
    Ban(u64),
    #[command(description = "Perform the SQLite benchmark")]
    SqliteBenchmark,
    #[command(description = "Perform the ReDB benchmark")]
    RedbBenchmark,
    #[command(description = "Perform a Live Trade Analysis simulation")]
    FollowAsset,
    #[command(description = "Help")]
    Help,
    #[command(description = "Used when first interacting with this bot")]
    Start,
}

type HErr = anyhow::Error;

pub async fn run(config: &BotConfig) -> anyhow::Result<()> {
    unsafe {
        std::env::set_var(
            "TELOXIDE_TOKEN",
            config.telegram_config.teloxide_token.clone(),
        );
    }
    let bot = Bot::from_env();
    let me: Me = bot.get_me().await?;
    eprintln!("Running as @{}", me.username());

    let adapter = Arc::new(TelegramAdapter::new(bot.clone()));

    // Messages/commands
    let cmd_handler = Update::filter_message()
        .filter_command::<Command>()
//         .endpoint({
// eprintln!("### HANDLER WAS CALLED");
//             let adapter = adapter.clone(); // move into closure
//             move |bot: Bot, msg: Message, cmd: Command| {
// eprintln!("### PROCESSOR WAS CALLED: bot: {:?}, msg: {:?}. cmd: {:?}", bot, msg, cmd);
//                 let adapter = adapter.clone();
//                 async move {
//                     handle_command(&adapter, &bot, msg, cmd).await?;
//                     Ok::<(), HErr>(())
//                 }
//             }
//         });
        .endpoint({
            eprintln!("### HANDLER IS BEING REGISTERED");
            let adapter = adapter.clone();
            move |bot: Bot, mo_msg: Message, me: Me| {
eprintln!("### WEIRDO!!");
                let adapter = adapter.clone();
                async move {
                    if let Some(text) = mo_msg.text() {
                        if let Ok(cmd) = BotCommands::parse(text, me.username()) {
                            eprintln!("### PROCESSOR WAS CALLED: bot: {bot:?}, mo_msg: {mo_msg:?}, me: {me:?} cmd: {cmd:?}");
                            handle_command(&adapter, &bot, &mo_msg, cmd).await
                                .inspect_err(|err| {
                                    eprintln!("### ERROR PROCESSING LAST COMMAND: {err:?})");
                                    let err_msg = format!("{err:?}");
                                    let chat_id = ChatId(mo_msg.chat.id.0);
                                    tokio::task::spawn(async move {
                                        let html = format!("<b>Error</b> processing your request :( -- {err_msg}");
                                        let result = adapter.send_text(chat_id, &FormattedText { html }).await;
                                        if let Err(err) = result {
                                            eprintln!("    ### additionally, there was yet another error when informing the user of the previous error through a formatted text message: {err:?})");
                                        }
                                    });
                                })
                        } else {
                            eprintln!("### WTF 11!!!");
                            Ok(())
                        }
                    } else {
                        eprintln!("### WTF 22!!!");
                        Ok(())
                    }
                }
            }
        });

    // Auto-approve join requests demo (if bot has can_invite_users)
    let join_handler = Update::filter_chat_join_request().endpoint({
        let adapter = adapter.clone();
        move |req: ChatJoinRequest| {
            let adapter = adapter.clone();
            async move {
                let chat = ChatId(req.chat.id.0);
                let user = UserId(req.from.id.0);
                // Ignore errors (permission issues) silently for demo
                let _ = adapter.approve_join_request(chat, user).await;
                Ok::<(), HErr>(())
            }
        }
    });

    Dispatcher::builder(
        bot,
        dptree::entry()
            .branch(dptree::filter(|bot: Bot| {
                println!("RECEIVED bot: {bot:?}");
                false
            }))
            .branch(dptree::filter(|me: Me| {
                println!("RECEIVED me: {me:?}");
                false
            }))
            .branch(dptree::filter(|update: Update| {
                println!("RECEIVED update: {update:?}");
                false
            }))
            .branch(cmd_handler)
            .branch(join_handler),
    )
    .enable_ctrlc_handler()
    .build()
    .dispatch()
    .await;

    Ok(())
}

async fn handle_command(
    adapter: &TelegramAdapter,
    bot: &Bot,
    msg: &Message,
    cmd: Command,
) -> Result<()> {
    match cmd {
        Command::Caps => {
            let caps = adapter.capabilities();
            let media: Vec<String> = caps
                .media_supported
                .iter()
                .map(|m| format!("{m:?}"))
                .collect();
            let report = format!(
                "<b>Channel:</b> {}\n<b>Can edit:</b> {}\n<b>Rich text:</b> {:?}\n<b>Media:</b> {}\n<b>Reactions:</b> {}\n<b>Group admin:</b> {}\n<b>Throttling:</b> {:?}",
                adapter.name(),
                caps.can_edit_messages,
                caps.rich_text_modes,
                media.join(", "),
                caps.supports_reactions,
                caps.supports_group_admin,
                caps.throttling
                    .as_ref()
                    .map(|t| (t.per_user_mps, t.per_chat_mps)),
            );
            adapter
                .send_text(ChatId(msg.chat.id.0), &FormattedText { html: report })
                .await?;
        }
        Command::ChatId => {
            let html = format!(
                "Your <u>UserId</u>/<u>ChatId</u> is <b>{}</b>\nIt can be used to send you <i>daily messages</i>...",
                msg.chat.id.0
            );
            adapter
                .send_text(ChatId(msg.chat.id.0), &FormattedText { html })
                .await?;
        }
        Command::DemoEdit => {
            let text = FormattedText {
                html: "Sending… <i>(will edit in 2s)</i>".into(),
            };
            let chat = ChatId(msg.chat.id.0);
            let adapter = adapter.clone();
            let send_and_edit = || async move {
                let fallible_fut = async {
                    let mid = adapter.send_text(chat, &text).await?;
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    let edited = FormattedText {
                        html: "<b>Edited!</b> ✅".into(),
                    };
                    adapter.edit_text(chat, mid, &edited).await
                };
                if let Err(err) = fallible_fut.await {
                    let html =
                        format!("<b>Error</b> processing the <i>/demoedit</i> request :( -- {err}");
                    eprintln!("### ERROR: {html}");
                    let result = adapter.send_text(chat, &FormattedText { html }).await;
                    if let Err(err) = result {
                        eprintln!(
                            "    ### additionally, there was yet another error when informing the user of the previous error through a formatted text message: {err:?})"
                        );
                    };
                }
            };
            // TODO: any errors bellow will be hidden
            tokio::task::spawn(send_and_edit());
        }
        Command::Photo(url_opt) => {
            let url = if url_opt.trim().is_empty() {
                // Known working sample image (Telegram downloads by URL)
                "https://assets.science.nasa.gov/dynamicimage/assets/science/esd/eo/images/imagerecords/84000/84214/bluemarble_2014089.jpg"
                    .to_string()
            } else {
                url_opt
            };
            let media = UnifiedMedia::Url {
                kind: MediaKind::Photo,
                url,
                caption_html: Some("<b>Sample photo</b> with <i>HTML</i> caption".into()),
            };
            adapter.send_media(ChatId(msg.chat.id.0), &media).await?;
        }
        Command::Invite(name) => {
            let link = adapter
                .create_invite_link(ChatId(msg.chat.id.0), &name, /*join_request*/ true)
                .await?;
            let body = format!("<b>Invite link:</b>\n<code>{}</code>", link);
            adapter
                .send_text(ChatId(msg.chat.id.0), &FormattedText { html: body })
                .await?;
        }
        Command::Pin => {
            if let Some(reply) = msg.reply_to_message() {
                adapter
                    .pin_message(ChatId(msg.chat.id.0), MessageId(reply.id.0))
                    .await?;
            } else {
                bot.send_message(msg.chat.id, "Reply to a message and then run /pin")
                    .await?;
            }
        }
        Command::Ban(uid) => {
            adapter.ban_user(ChatId(msg.chat.id.0), UserId(uid)).await?;
            bot.send_message(msg.chat.id, format!("Banned user id {}", uid))
                .await?;
        }
        Command::SqliteBenchmark => {
            let bot = bot.clone();
            let chat_id = msg.chat.id;
            sqlite::benchmark::benchmark(async move |mt| {
                bot.send_message(chat_id, mt).await.map(|_| ())
            })
            .await?;
        }
        Command::RedbBenchmark => {
            let bot = bot.clone();
            let chat_id = msg.chat.id;
            redb::benchmark::benchmark(async move |mt| {
                bot.send_message(chat_id, mt).await.map(|_| ())
            })
            .await?;
        }
        Command::FollowAsset => {
            plot::demo::demo(bot, msg.chat.id).await?;
        }
        Command::Help => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?;
        }
        Command::Start => {
            let mt = format!(
                "Welcome to OgreRobot.\nPlease pick your option:\n{}",
                Command::descriptions()
            );
            bot.send_message(msg.chat.id, mt).await?;
        }
    }
    Ok(())
}
