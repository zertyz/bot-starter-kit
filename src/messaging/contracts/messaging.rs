//! Resting place of [Messaging] & related models.

use futures::Stream;
use std::num::NonZeroU64;

/// Abstraction to send messages to every supported [super::messaging_platform::MessagingPlatform].
pub trait Messaging<PartyType, MoPayloadType, MtPayloadType> {
    /// Returns the `Stream` of "Mobile Originated" (MO) messages for the backend logic to use as "inputs".
    /// NOTE: This method can be called just once. It will return `None` in any subsequent calls.
    fn get_mo_stream(mo_rx: async_channel::Receiver<MoPayloadType>) -> impl Stream<Item = Mo<PartyType, MoPayloadType>>;

    /// Takes in a `Stream` of "Mobile Terminated" (MT) messages -- produced by the backend logic and targeted at users.
    /// This method should return immediately, returning a Join Handle to the spawned tokio task that will process the Stream to completion.
    fn consume_mt_stream(&self, concurrency: usize, all_users_mt_stream: impl Stream<Item = MtPayloadType> + Send + 'static) -> tokio::task::JoinHandle<()>;
}

/// Model for "Mobile Originated" messages for a given "account" within the message platform
#[derive(Debug)]
pub struct Mo<PartyType, PayloadType> {
    /// Any unique numeric id given to the message by the messaging platform
    id: Option<NonZeroU64>,
    /// The originator's identity
    sender: Party<PartyType>,
    /// Where this MO flowed through to get here.
    /// Also called: "chat"/"bot" (Telegram parlance), "short-code" (SMS parlance), "id" (Whatsapp), "Application " (Slack), ...
    dialog: Dialog,
    /// The message contents
    payload: PayloadType,
}
impl<PartyType, PayloadType> Mo<PartyType, PayloadType> {
    pub fn new(id: u64, sender: Party<PartyType>, dialog: Dialog, payload: PayloadType) -> Self {
        Mo { id: NonZeroU64::new(id), sender, dialog, payload }
    }

    pub fn id(&self) -> u64 {
        self.id
            .map(NonZeroU64::get)
            .unwrap_or_default()
    }
    pub fn sender(&self) -> &Party<PartyType> {
        &self.sender
    }

    pub fn dialog(&self) -> &Dialog {
        &self.dialog
    }

    pub fn payload(&self) -> &PayloadType {
        &self.payload
    }

    pub fn into_payload(self) -> PayloadType {
        self.payload
    }
}

/// Model for "Mobile Terminated" messages for a given "account" within the message platform
pub struct Mt<PartyType, PayloadType> {
    /// Any unique numeric id given to a previous message by the messaging platform.
    /// If present, means we are editing an already sent message instead of sending a new one
    pub edit_id: Option<NonZeroU64>,
    // The addressee's identity
    pub recipient: Party<PartyType>,
    /// Where this MO flowed through to get here.
    /// Also called: "chat"/"bot" (Telegram parlance), "short-code" (SMS parlance), "id" (Whatsapp), "Application " (Slack), ...
    pub dialog: Dialog,
    /// The message contents
    pub payload: PayloadType,
}

/// Model for a message party identification within a given messaging platform
/// OBS: telegram does provide additional info we can use when starting a dialog:
///      * is_bot, is_premium, language,
#[derive(Debug, Clone)]
pub struct Party<PartyType> {
    pub inner: PartyType,
    /// Any unique numeric id given to the user by the messaging platform
    id: Option<NonZeroU64>,
    /// How to address the sender in the platform -- a phone number, a nickname, ... ?
    pub address: Option<String>,
    /// The name of the sender
    pub name: Option<String>,
    // note: the fields bellow should be accessible through traits
    // e.g: impl MessagingPlatformParty<teloxide::Bot> for Party<teloxide::Bot> { ... } and so on
    /*
    real_mobile_phone_number: Option<NonZeroU64>,
    username: Option<String>,
    first_name: Option<String>,
    last_name: Option<String>,*/
}
impl<PartyType: Clone> Party<PartyType> {
    pub fn new(id: u64, inner: PartyType) -> Self {
        Self { inner, id: NonZeroU64::new(id), address: None, name: None }
    }

    pub fn with_address(mut self, address: String) -> Self {
        self.address
            .replace(address);
        self
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.name
            .replace(name);
        self
    }

    pub fn id(&self) -> u64 {
        self.id
            .map(NonZeroU64::get)
            .unwrap_or_default()
    }
}

/// not sure about this one... think about it.
#[derive(Debug)]
pub struct Dialog {
    /// Any unique numeric id given to the user by the messaging platform
    /// "chat_id" (Telegram", ...
    id: Option<NonZeroU64>,
    /// Is this dialog private? Or shared by a group?
    kind: DialogKind,
    /// The preferred language, as informed by the messaging platform
    language: Language,
}

impl Dialog {
    pub fn new(id: u64, kind: DialogKind, language: Language) -> Self {
        Self { id: NonZeroU64::new(id), kind, language }
    }

    pub fn id(&self) -> u64 {
        self.id
            .map(NonZeroU64::get)
            .unwrap_or_default()
    }

    pub fn kind(&self) -> &DialogKind {
        &self.kind
    }

    pub fn language(&self) -> &Language {
        &self.language
    }
}

#[derive(Debug)]
pub enum DialogKind {
    Private,
    Group,
    Unspecified,
}

#[derive(Debug)]
pub enum Language {
    English,
    Portuguese,
    Unknown,
    Unspecified,
}

pub enum Payload {
    /// Plain text, possibly with simple markups if the messaging platform supports it
    Text {
        text: String,
    },
    Reply {
        reference_text: String,
        new_text: String,
    },

    /// If supported by the messaging platform, notifies the user-side UI to set / change the "title" for the dialog
    Pin {
        text: String,
    },
    // /// When supported, present the user a visually appealing menu of options to pick.
    // /// A vector inside a vector is used to give some layout hints: the inner vector contains columns, wheras the
    // /// outter vector specifies lines.
    // /// IMPLEMENTATION NOTE: right now we are using the Telegram models; once we support more messaging platforms,
    // /// a generalized version may be introduced
    // Options { options: Vec<Vec<InlineKeyboardButton>>},
    /// Photo URL -- either remote or a local file. The messaging platform might show a thumbnail and might reduce the size & quality as well
    Photo {
        url: Url,
        caption: String,
    },
    /// Whatever file -- the messaging platform won't change the contents
    Document {
        url: Url,
        caption: String,
    },
}

pub type Url = String;
