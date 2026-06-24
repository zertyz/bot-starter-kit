use crate::repository::error::Error;
use crate::repository::models::telegram_models::TelegramUser;
use futures::Stream;

#[allow(async_fn_in_trait)]
pub trait TelegramUsersRepository {
    /// Type for the telegram user, as stored in the database
    /// -- representing the same information as [TelegramUser]
    type TelegramUser;

    async fn ensure_user(&self, user: TelegramUser) -> Result<&Self::TelegramUser, Error>;

    async fn get_user_by_id(&self, user_id: u64) -> Result<&Self::TelegramUser, Error>;

    /// Usually you'd never enumerate all users, as this will return
    /// old, inactive, ... all users.
    ///
    /// Other repositories, such as:
    ///  - [UserStatesRepository]
    ///  - [TelegramUsersRepository]
    ///  - [UserConfigsRepository],
    /// 
    ///  may provide finer-grained queries over the users.
    async fn enumerate_all_users(&self) -> impl Stream<Item = Result<Self::TelegramUser, Error>>;
}
