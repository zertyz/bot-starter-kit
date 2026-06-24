use redb::{Database, ReadTransaction, WriteTransaction};
use tokio::sync::{Mutex, MutexGuard};
use crate::repository::error::Error;

pub struct AsyncRedb {
    db: Database,
    write_transaction: Mutex<Option<WriteTransaction>>
}

impl AsyncRedb {

    /// Open's a table for read-only operations with data in the state of
    /// the previous commited transaction.
    pub async fn ro_table(&self, ) {}
    // pub async fn rw_table(&self, ) -> LeakedTransactionManager {}

    /// Locks the mutex, gives you the full transaction object
    pub async fn leak_transaction(&self) -> Option<WriteTransaction> {
        None
    }

    /// Puts the transaction back in and unlocks the Mutex
    pub async fn unleak_transaction(&self, transaction: WriteTransaction) {

    }

    TODO continue from here:
    1) This AsyncRedb: should control transactions that are created and are in transit: only 1 can be created; async block if a new request arives before the last transaction is returned
    2) A new TransactionManager trait is to be created: also get write transactions and return them, just as above. Read transactions are free. Uses #1 -- leak transaction seems to still make sense.
    3) A new type SharedTransaction implements the transaction manager above
    4) Repositories ingest a TransactionManager -- `from_transaction_manager()`
    5) If the above doesn't wrap up together, a new `DisposableTransaction` -- so repositories may commit on every change or commit only at the end
    6) Both should set 'auto-commit' and return the transaction when dropped

    pub async fn abort(&self) -> Result<(), Error> {
        let mut locked_transaction = self.write_transaction.lock().await;
        let Some(write_transaction) = locked_transaction.take() else {
            return Ok(())
        };
        write_transaction.abort()
            .map_err(|redb_err| Error::ReDbStorage { message: String::from("Error aborting `redb` transaction"), cause: redb_err } )
    }

    pub async fn commit(&self) -> Result<(), Error> {
        let mut locked_transaction = self.write_transaction.lock().await;
        let Some(write_transaction) = locked_transaction.take() else {
            return Ok(())
        };
        write_transaction.commit()
            .map_err(|redb_err| Error::ReDbCommit { message: String::from("Error commiting `redb` transaction"), cause: redb_err } )
    }

}


pub trait RedbRepository<'r, ReadTxnProviderFut: Future<Output=ReadTransaction>,
                             ReadTxnProviderFn: Fn() -> ReadTxnProviderFut,
                             WriteTxnProviderFut: Future<Output=MutexGuard<'r, Option<WriteTransaction>>>,
                             WriteTxnProviderFn: Fn() -> WriteTxnProviderFut> {

    async fn from_read_write_transaction_providers(read_transaction_provider:  ReadTxnProviderFn,
                                                   write_transaction_provider: WriteTxnProviderFn) -> Self;

}

