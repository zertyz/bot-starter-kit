/// Platforms that are source of user information
#[derive(Debug, PartialEq)]
pub enum UserRealm {
    Telegram,
    Whatsapp,
}

/// Represents a person/user or group we are interacting with
/// -- the id is provider by the external platform and is assumed
/// to be unique within that platform's realm
// annotations for mmap
#[repr(C)]
#[derive(Clone, Copy)]
// #[derive(bytemuck::Pod, bytemuck::Zeroable)]     omitted as this is not supported for enums
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
#[rkyv(compare(PartialEq), derive(Debug, PartialEq))]
#[derive(Debug, PartialEq)]
pub enum User {
    TelegramUserId(u64),
    WhatsappUserId(u64),
}

impl User {
    /// Extract the raw number representing the user id.
    /// Please note that this method discards the realm where the user id is valid.
    pub fn user_id(&self) -> u64 {
        match self {
            User::TelegramUserId(user_id) => *user_id,
            User::WhatsappUserId(user_id) => *user_id,
        }
    }
}
