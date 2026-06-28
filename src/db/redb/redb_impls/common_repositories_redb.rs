use std::cmp::Ordering;
use futures::{stream, Stream, StreamExt};
use redb::{ReadTransaction, ReadableTable, TableDefinition, Value, WriteTransaction};
use tokio::sync::MutexGuard;
use crate::mmap_value;
use crate::repository::error::Error;
use crate::repository::models::common_models::{User, UserRealm};
use crate::repository::traits::common_repositories::UsersRepository;
use crate::db::redb::redb_wrapper::redb_repository::*;

const COMMON_USERS_TABLE_NAME: &str = "users";

pub struct UsersRepositoryRedb<'r, ReadTxnProviderFut: Future<Output=ReadTransaction>,
                                   ReadTxnProviderFn: Fn() -> ReadTxnProviderFut,
                                   WriteTxnProviderFut: Future<Output=MutexGuard<'r, Option<WriteTransaction>>>,
                                   WriteTxnProviderFn: Fn() -> WriteTxnProviderFut> {
    read_transaction_provider:  ReadTxnProviderFn,
    write_transaction_provider: WriteTxnProviderFn,
}

impl<'r, ReadTxnProviderFut: Future<Output=ReadTransaction>,
         ReadTxnProviderFn: Fn() -> ReadTxnProviderFut,
         WriteTxnProviderFut: Future<Output=MutexGuard<'r, Option<WriteTransaction>>>,
         WriteTxnProviderFn: Fn() -> WriteTxnProviderFut>
RedbRepository<'r, ReadTxnProviderFut, ReadTxnProviderFn, WriteTxnProviderFut, WriteTxnProviderFn> for
UsersRepositoryRedb<'r, ReadTxnProviderFut, ReadTxnProviderFn, WriteTxnProviderFut, WriteTxnProviderFn> {

    async fn from_read_write_transaction_providers(read_transaction_provider:  ReadTxnProviderFn,
                                                   write_transaction_provider: WriteTxnProviderFn) -> Self {
        Self {
            read_transaction_provider,
            write_transaction_provider,
        }
    }

}

impl<'r, ReadTxnProviderFut: Future<Output=ReadTransaction>,
         ReadTxnProviderFn: Fn() -> ReadTxnProviderFut,
         WriteTxnProviderFut: Future<Output=MutexGuard<'r, Option<WriteTransaction>>>,
         WriteTxnProviderFn: Fn() -> WriteTxnProviderFut>
UsersRepository for
UsersRepositoryRedb<'r, ReadTxnProviderFut, ReadTxnProviderFn, WriteTxnProviderFut, WriteTxnProviderFn> {

    type User = User;

    async fn ensure_user(&self, user: &User) -> Result<(), Error> {
        let transaction = (self.write_transaction_provider)().await;
        let mut users_table = Self::open_users_rw_table(&transaction).await?;
        let exists = users_table.get(&user)
            .map_err(|redb_err| Error::ReDbStorage { message: String::from("Couldn't fetch a common users table record"), cause: redb_err })?
            .is_some();
        if !exists {
            users_table.insert(&user, ())
                .map_err(|redb_err| Error::ReDbStorage { message: String::from("Couldn't insert a common users table record"), cause: redb_err })?;
        }
        Ok(())
    }

    async fn enumerate_users_by_realm(&self, realm: UserRealm) -> impl Stream<Item=Result<Self::User, Error>> {
        let users_table = match self.open_users_ro_table().await {
            Ok(users_table) => users_table,
            Err(err) => return stream::iter(vec![Err(err)]).left_stream(),
        };
        let range = match realm {
            UserRealm::Telegram => &User::TelegramUserId(u64::MIN)..=&User::TelegramUserId(u64::MAX),
            UserRealm::Whatsapp => &User::WhatsappUserId(u64::MIN)..=&User::WhatsappUserId(u64::MAX),
        };
        let iter = match users_table.range::<&User>(range) {
            Ok(iter) => iter,
            Err(redb_err) => return stream::iter(vec![Err(Error::ReDbStorage { message: String::from("user range failed"), cause: redb_err })]).left_stream(),
        }
            .map(|r| r
                .map(|(key, _value)| key.value().clone())
                .map_err(|redb_err| Error::ReDbStorage { message: String::from("Error enumerating users from the common users table"), cause: redb_err }));
        stream::iter(iter).right_stream()
    }

    async fn enumerate_all_users(&self) -> impl Stream<Item=Result<Self::User, Error>> {
        let users_table = match self.open_users_ro_table().await {
            Ok(users_table) => users_table,
            Err(err) => return stream::iter(vec![Err(err)]).left_stream(),
        };
        let range = &User::TelegramUserId(u64::MIN)..=&User::WhatsappUserId(u64::MAX);
        let iter = match users_table.range::<&User>(range) {
            Ok(iter) => iter,
            Err(redb_err) => return stream::iter(vec![Err(Error::ReDbStorage { message: String::from("user range failed"), cause: redb_err })]).left_stream(),
        }
            .map(|r| r
                .map(|(key, _value)| key.value().clone())
                .map_err(|redb_err| Error::ReDbStorage { message: String::from("Error enumerating users from the common users table"), cause: redb_err }));
        stream::iter(iter).right_stream()
    }
}

impl<'r, ReadTxnProviderFut: Future<Output=ReadTransaction>,
         ReadTxnProviderFn: Fn() -> ReadTxnProviderFut,
         WriteTxnProviderFut: Future<Output=MutexGuard<'r, Option<WriteTransaction>>>,
         WriteTxnProviderFn: Fn() -> WriteTxnProviderFut>
UsersRepositoryRedb<'r, ReadTxnProviderFut, ReadTxnProviderFn, WriteTxnProviderFut, WriteTxnProviderFn> {

    async fn open_users_ro_table(&self) -> Result<redb::ReadOnlyTable<User, ()>, Error> {
        (self.read_transaction_provider)().await
            .open_table(TableDefinition::<User, ()>::new(COMMON_USERS_TABLE_NAME))
            .map_err(|redb_err| Error::ReDbTable {
                message: format!("Couldn't open common users table {COMMON_USERS_TABLE_NAME} in read-only mode"),
                cause: redb_err
            })
    }

    async fn open_users_rw_table<'a>(transaction: &'a MutexGuard<'a, Option<WriteTransaction>>) -> Result<redb::Table<'a, User, ()>, Error> {
        transaction
            .as_ref()
            .ok_or_else(|| Error::MissingWriteTransaction { message: String::from("Write transaction is absent! :(") })?
            .open_table(TableDefinition::<User, ()>::new(COMMON_USERS_TABLE_NAME))
            .map_err(|redb_err| Error::ReDbTable {
                message: format!("Couldn't open common users table {COMMON_USERS_TABLE_NAME} in read/write mode"),
                cause: redb_err
            })
    }

}

mmap_value!(User);

impl redb::Key for User {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        Ord::cmp(data1, data2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use once_cell::sync::Lazy;
    use redb::{Database, ReadableDatabase};
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn users_basic_usage() {
        let db = db_singleton().await;
        let write_transaction = Mutex::new(Some(write_transaction().await));
        let read_transactions_provider = || async { db.begin_read().expect("Couldn't start a read transaction") };
        let write_transactions_provider = || async { write_transaction.lock().await };
        let users_repository = UsersRepositoryRedb::from_read_write_transaction_providers(read_transactions_provider, write_transactions_provider).await;


        let expected_user = User::TelegramUserId(1001);
        users_repository.ensure_user(&expected_user).await
            .expect("Couldn't ensure user exists");

        {
            let mut locked = write_transaction.lock().await;
            let old_transaction = locked.take().expect("write transaction is empty");
            old_transaction.commit()
                .expect("Couldn't commit first write transaction");
            let new_transaction = db.begin_write().expect("Couldn't start a new write transaction");
            locked.replace(new_transaction);
        }

        let observed_user = users_repository.enumerate_all_users().await
            .next().await
            .expect("Query returned zero elements")
            .expect("First element is Err");

        assert_eq!(observed_user, expected_user, "Users mismatch");
    }

    async fn db_singleton() -> &'static Database {
        static DATABASE: Lazy<Database> = Lazy::new(|| {
            Database::create("/tmp/users_basic_usage.redb")
                .expect("Could not open (or create) database")
        });
        &DATABASE
    }

    async fn write_transaction() -> WriteTransaction {
        db_singleton().await.begin_write()
            .expect("Couldn't start a write transaction")
    }
}
