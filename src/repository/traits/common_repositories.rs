use crate::repository::error::Error;
use crate::repository::models::common_models::{User, UserRealm};
use futures::Stream;

#[allow(async_fn_in_trait)]
pub trait UsersRepository {
    /// Type for the user, as stored in the database
    /// -- representing the same information as [User]
    type User;

    /// Ensures the given user exists in the database, creating it if necessary
    async fn ensure_user(&self, user: &User) -> Result<(), Error>;

    async fn enumerate_users_by_realm(&self, realm: UserRealm) -> impl Stream<Item = Result<Self::User, Error>>;

    /// Usually you'd never enumerate all users, as this will return
    /// old, inactive, ... all users.
    ///
    /// Other repositories, such as:
    ///  - [super::telegram_repositories::TelegramUsersRepository]
    ///  - [UserStatesRepository]
    ///  - [UserConfigsRepository],
    ///
    /// may provide finer-grained queries over the users.
    async fn enumerate_all_users(&self) -> impl Stream<Item = Result<Self::User, Error>>;
}
