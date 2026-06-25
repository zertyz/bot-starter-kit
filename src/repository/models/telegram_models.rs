use std::borrow::Cow;

/// Represents a person/user or group we are interacting with
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(compare(PartialEq))]
#[derive(Debug, PartialEq)]
pub struct TelegramUser<'a> {
    pub user_id: u64,
    pub user_name: Cow<'a, str>,
    pub first_name: Cow<'a, str>,
    pub last_name: Cow<'a, str>,
    pub language_code: Cow<'a, str>,
    pub is_bot: bool,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug, PartialEq))]
#[derive(Debug, PartialEq)]
pub enum TelegramUserChat {
    PrivateChat { chat_id: u64, user_id: u64 },
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(compare(PartialEq))]
#[derive(Debug, PartialEq)]
pub enum TelegramMessage<'a> {
    MO { chat_id: u64, message: TelegramContent<'a> },
    MT { chat_id: u64, message: TelegramContent<'a> },
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(compare(PartialEq))]
#[derive(Debug, PartialEq)]
pub enum TelegramContent<'a> {
    PlainText(Cow<'a, str>),
    RichText(Cow<'a, str>),
    InlinedImage(TelegramFile<'a>),
    InlinedVideo(TelegramFile<'a>),
    InlinedAudio(TelegramFile<'a>),
    Sticker(TelegramFile<'a>),
    Document(TelegramFile<'a>),
    Location(Cow<'a, str>),
    Contact(Cow<'a, str>),
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(compare(PartialEq))]
#[derive(Debug, PartialEq)]
pub struct TelegramFile<'a> {
    pub content_type: Cow<'a, str>,
    pub file_name: Cow<'a, str>,
    pub contents: Cow<'a, [u8]>,
    pub caption: Cow<'a, str>,
}

impl TelegramUserChat {
    pub fn user_id(&self) -> u64 {
        match self {
            TelegramUserChat::PrivateChat { user_id, .. } => *user_id,
        }
    }
    pub fn chat_id(&self) -> u64 {
        match self {
            TelegramUserChat::PrivateChat { chat_id, .. } => *chat_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::models::common_models::User;
    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::Ordering::Relaxed;

    #[test]
    fn hello_world() {
        let (user, _telegram_user) = new_telegram_user("test_user", "John", "Tester", "en");
        let user_chat = new_private_chat(&user);
        let _user_message = new_mo_message(&user_chat, "Hi, there! I've sent this message from my mobile!");
        let _server_message = new_mo_message(&user_chat, "Hi, folk! I am the server and I've received your message");
    }

    // helper functions
    ////////////////////
    // these are the precursor of the repository operations

    fn new_telegram_user<'a>(user_name: &'a str, first_name: &'a str, last_name: &'a str, language_code: &'a str) -> (User, TelegramUser<'a>) {
        static USER_ID_COUNTER: AtomicU64 = AtomicU64::new(0);
        let user_id = USER_ID_COUNTER.fetch_add(1, Relaxed);
        let user = User::TelegramUserId(user_id);
        let telegram_user = TelegramUser {
            user_id,
            user_name: Cow::Borrowed(user_name),
            first_name: Cow::Borrowed(first_name),
            last_name: Cow::Borrowed(last_name),
            language_code: Cow::Borrowed(language_code),
            is_bot: false,
        };
        (user, telegram_user)
    }

    /// Prepares for a new chat between this bot and the given user
    fn new_private_chat(user: &User) -> TelegramUserChat {
        static CHAT_ID_COUNTER: AtomicU64 = AtomicU64::new(0);
        let chat_id = CHAT_ID_COUNTER.fetch_add(1, Relaxed);
        TelegramUserChat::PrivateChat { user_id: user.user_id(), chat_id }
    }

    fn new_mo_message<'a>(user_chat: &TelegramUserChat, plain_text: &'a str) -> TelegramMessage<'a> {
        TelegramMessage::MO {
            chat_id: user_chat.chat_id(),
            message: TelegramContent::PlainText(Cow::Borrowed(plain_text)),
        }
    }

    fn _new_mt_message<'a>(user_chat: &TelegramUserChat, plain_text: &'a str) -> TelegramMessage<'a> {
        TelegramMessage::MT {
            chat_id: user_chat.chat_id(),
            message: TelegramContent::PlainText(Cow::Borrowed(plain_text)),
        }
    }
}
