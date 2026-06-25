//! `redb`-specific helpers & wrappers for async & stream operation using the zero-copy mmap
//! and read zero-copy rkyv serializers from the [super::serde] submodule.
//!
//! NOTE: As of May, 2026, `redb` falsely claims "zero-copy". It is not mmap based nor uses async IO.
//!       It requires loading data into its own buffers, hence:
//!       1) it is not zero-copy;
//!       2) it requires more RAM than a real zero-copy solution.
//!       Look at [crate::db::heed] for superior performance.

use anyhow::{Result, anyhow};
use futures::{Stream, StreamExt, stream};
use redb::{ReadTransaction, ReadableDatabase, TableDefinition, WriteTransaction};
use std::{path::Path, pin::pin};
use tokio::sync::{Mutex, MutexGuard, Semaphore, SemaphorePermit};

/// By using `redb` through the following API, and only through the following API, we make it
/// more async-friendly than using `redb` directly from arbitrary Tokio tasks.
///
/// Features:
/// 1. Only one write transaction can be issued at a time, enforced by an async writer mutex.
///    Without this gate, `redb` may block the current Tokio worker while waiting for its own
///    internal single-writer lock.
/// 2. Read transactions are admitted through an async semaphore. This limits the maximum number
///    of simultaneously active readers, avoiding unbounded read fan-out from many Tokio tasks.
/// 3. Reads are not guarded by the writer mutex. They rely on `redb`'s MVCC model and may run
///    concurrently with a writer.
/// 4. Compaction obtains both:
///    - the writer mutex, so no wrapper-managed writer can be active;
///    - all reader permits, so no wrapper-managed reader can be active.
/// 5. Bounded synchronous ReDB iterators can be converted to streams.
///
/// Important note:
/// `redb_iter_to_stream()` is still backed by synchronous `redb` iteration. Polling the returned
/// stream may briefly block the Tokio task while `redb` performs mmap/page-cache/filesystem work.
/// This is acceptable for small/bounded reads. Larger scans should later be routed through a
/// channel/spawn-blocking based API.
pub struct AsyncReDb {
    redb: redb::Database,
    w_lock: Mutex<()>,
    r_lock: Semaphore,
    max_readers: u32,
}

impl AsyncReDb {
    pub async fn open(db_path: impl AsRef<Path>) -> Result<Self> {
        pub const DEFAULT_MAX_READERS: u32 = 16;
        Self::open_with_max_readers(db_path, DEFAULT_MAX_READERS).await
    }

    pub async fn open_with_max_readers(
        db_path: impl AsRef<Path>,
        max_readers: u32,
    ) -> Result<Self> {
        let db_path = db_path.as_ref();

        let redb = redb::Database::create(db_path).map_err(|err| {
            anyhow!(
                "ReDB: Failed to open or create `database {:?}: {err}",
                db_path
            )
        })?;

        Ok(Self {
            redb,
            w_lock: Mutex::new(()),
            r_lock: Semaphore::new(max_readers as usize),
            max_readers,
        })
    }

    /// Close the database wrapper.
    ///
    /// This does not compact. Dropping `redb::Database` performs the normal close behavior.
    /// Use `compact()` explicitly when compaction is desired.
    pub async fn close(self) -> Result<()> {
        Ok(())
    }

    /// Compact the database.
    ///
    /// This waits for:
    /// 1. the writer mutex, so no wrapper-managed write transaction can be active;
    /// 2. all reader semaphore permits, so no wrapper-managed read transaction can be active.
    ///
    /// Once the request for all reader permits is queued, Tokio's semaphore fairness prevents
    /// later reader requests from jumping ahead of compaction.
    ///
    /// The actual `redb` compaction is synchronous and may block the Tokio task while filesystem
    /// work is performed.
    pub async fn compact(&mut self) -> Result<()> {
        let _write_permit = self.w_lock.lock().await;

        let _read_permits = self
            .r_lock
            .acquire_many(self.max_readers)
            .await
            .map_err(|err| {
                anyhow!("Failed to acquire all `redb` reader permits for compaction: {err}")
            })?;

        self.redb
            .compact()
            .map_err(|err| anyhow!("Failed to compact `redb` database: {err}"))
            .map(|_success| ())
    }

    pub async fn begin_read(&self) -> Result<ReDbReadTransaction<'_>> {
        let read_permit = self
            .r_lock
            .acquire()
            .await
            .map_err(|err| anyhow!("Failed to acquire `redb` reader permit: {err}"))?;

        let read_txn = self.redb.begin_read()?;

        Ok(ReDbReadTransaction::new(read_txn, read_permit))
    }

    pub fn redb_iter_to_stream<KeyType, ValueType>(
        redb_iter_result: std::result::Result<
            redb::Range<'_, KeyType, ValueType>,
            redb::StorageError,
        >,
    ) -> Result<
        impl Stream<
            Item = Result<(
                redb::AccessGuard<'_, KeyType>,
                redb::AccessGuard<'_, ValueType>,
            )>,
        >,
    >
    where
        KeyType: redb::Key,
        ValueType: redb::Value,
    {
        match redb_iter_result {
            Ok(redb_iter) => {
                let stream = stream::iter(redb_iter)
                    .map(|std_result| std_result.map_err(anyhow::Error::new));

                Ok(stream)
            }
            Err(std_err) => Err(anyhow::Error::new(std_err)),
        }
    }

    pub async fn begin_write(&self) -> Result<ReDbWriteTransaction<'_>> {
        let write_guard = self.w_lock.lock().await;
        let write_txn = self.redb.begin_write()?;

        Ok(ReDbWriteTransaction::new(write_txn, write_guard))
    }

    pub async fn ingest_stream<KeyType: redb::Key + 'static, ValueType: redb::Value + 'static>(
        &self,
        table_definition: TableDefinition<'_, KeyType, ValueType>,
        input_stream: impl Stream<Item = (KeyType, ValueType)>,
    ) -> Result<usize>
    where
        for<'r> &'r KeyType: std::borrow::Borrow<<KeyType as redb::Value>::SelfType<'r>>,
        for<'r> <ValueType as redb::Value>::SelfType<'r>: From<&'r ValueType>,
    {
        let write_txn = self.begin_write().await
            .map_err(|err| anyhow!("ReDb: beginning Stream ingestion into table '{table_definition}' -- couldn't create the write transaction: {err}"))?;

        let mut table = write_txn
            .inner()
            .open_table(table_definition)
            .map_err(|err| {
                anyhow!(
                    "ReDb: failed to open table '{table_definition}' for Stream ingestion: {err}"
                )
            })?;

        let mut count = 0;
        let mut input_stream = pin!(input_stream);
        while let Some((key, value)) = input_stream.next().await {
            let redb_value: <ValueType as redb::Value>::SelfType<'_> = (&value).into();
            table
                    .insert(&key, redb_value)
                    .map_err(|err| anyhow!("ReDb: Couldn't insert/replace item #{count} during Stream ingestion into table '{table_definition}': {err}"))?;
            count += 1;
        }

        drop(table);
        write_txn
            .commit()
            .await
            .map_err(|err| anyhow!("ReDb: Could not commit transaction after Stream ingestion into table '{table_definition}': {err}"))?;

        Ok(count)
    }
}

/// A read transaction guarded by one reader semaphore permit.
///
/// Field order is intentional:
/// 1. `inner` is dropped first.
/// 2. `_read_permit` is dropped second.
///
/// Therefore, if the read transaction is dropped without explicit `close()`, the underlying
/// `redb::ReadTransaction` is dropped before the reader permit is released.
pub struct ReDbReadTransaction<'db> {
    inner: ReadTransaction,
    _read_permit: SemaphorePermit<'db>,
}

impl<'db> ReDbReadTransaction<'db> {
    fn new(read_txn: ReadTransaction, read_permit: SemaphorePermit<'db>) -> Self {
        Self {
            inner: read_txn,
            _read_permit: read_permit,
        }
    }

    /// Access the underlying `redb::ReadTransaction`.
    ///
    /// This avoids re-declaring the full `redb::ReadTransaction` API on the wrapper.
    pub fn inner(&self) -> &ReadTransaction {
        &self.inner
    }

    /// Close the read transaction while still holding the reader permit.
    ///
    /// This method is async only to preserve the wrapper's async API shape.
    pub async fn close(self) -> Result<()> {
        self.inner
            .close()
            .map_err(|err| anyhow!("Failed to close `redb` reader: {err}"))
    }
}

/// A write transaction guarded by the wrapper's async writer mutex.
///
/// This type intentionally keeps the mutex guard and the `redb::WriteTransaction` together.
/// That prevents callers from accidentally dropping the guard while the write transaction is
/// still alive.
///
/// Field order is intentional:
/// 1. `inner` is dropped first.
/// 2. `_write_guard` is dropped second.
///
/// Therefore, if the transaction is dropped without `commit()`, `redb` gets to abort/drop the
/// write transaction before the async writer slot is released.
pub struct ReDbWriteTransaction<'db> {
    inner: WriteTransaction,
    _write_guard: MutexGuard<'db, ()>,
}

impl<'db> ReDbWriteTransaction<'db> {
    fn new(write_txn: WriteTransaction, write_guard: MutexGuard<'db, ()>) -> Self {
        Self {
            inner: write_txn,
            _write_guard: write_guard,
        }
    }

    /// Access the underlying `redb::WriteTransaction`.
    ///
    /// This avoids re-declaring the full `redb::WriteTransaction` API on the wrapper.
    pub fn inner(&self) -> &WriteTransaction {
        &self.inner
    }

    /// Mutably access the underlying `redb::WriteTransaction`.
    ///
    /// This is provided for API completeness, even though most `redb::WriteTransaction` methods
    /// currently use shared references.
    pub fn inner_mut(&mut self) -> &mut WriteTransaction {
        &mut self.inner
    }

    /// Commit the write transaction while still holding the async writer mutex.
    ///
    /// The actual `redb` commit is synchronous and may block the Tokio task while filesystem work
    /// is performed.
    pub async fn commit(self) -> Result<()> {
        self.inner
            .commit()
            .map_err(|err| anyhow!("Failed to commit `redb` transaction: {err}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commons::test_commons::redb_test_commons::TestModel;
    use redb::TableDefinition;

    #[tokio::test]
    async fn hello_api() {
        const TABLE: TableDefinition<&str, TestModel> = TableDefinition::new("redb_wrapper_data");

        let expected_data_iter = || {
            (0..128).map(|i| TestModel {
                count: 127 - i,
                whatever: i,
            })
        };

        let key = |i| format!("my_key{i:05}");

        let file_path = "/tmp/test_redb_wrapper.redb";

        let mut db = AsyncReDb::open_with_max_readers(file_path, 16)
            .await
            .expect("Could not create database");

        let write_txn = db
            .begin_write()
            .await
            .expect("Could not begin write transaction");

        let mut table = write_txn
            .inner()
            .open_table(TABLE)
            .expect("Could not open/create table");

        for (i, value) in expected_data_iter().enumerate() {
            table
                .insert(key(i).as_str(), &value)
                .expect("Could not insert/replace #{i}");
        }

        drop(table);
        write_txn
            .commit()
            .await
            .expect("Could not commit transaction");

        let read_txn = db
            .begin_read()
            .await
            .expect("Could not begin read transaction");

        let table = read_txn
            .inner()
            .open_table(TABLE)
            .expect("Could not open table");

        let mut observed_stream = AsyncReDb::redb_iter_to_stream(
            table.range::<&str>(key(0).as_str()..key(expected_data_iter().count()).as_str()),
        )
        .expect("Could not get the Stream");

        let mut expected_stream = stream::iter(expected_data_iter())
            .enumerate()
            .map(|(k, v)| anyhow::Result::<(String, TestModel), anyhow::Error>::Ok((key(k), v)));

        while let Some(Ok((expected_k, expected_v))) = expected_stream.next().await {
            if let Some(Ok((observed_k, observed_v))) = observed_stream.next().await {
                assert_eq!(
                    observed_k.value(),
                    expected_k,
                    "Stream query failed: row #{expected_k} yielded the value of row#{}",
                    observed_k.value()
                );

                assert_eq!(
                    observed_v.value(),
                    &expected_v,
                    "Stream query failed @ row #{expected_k}"
                );
            } else {
                panic!("Stream query failed: row #{expected_k} is not present in the Stream");
            }
        }

        assert!(
            observed_stream.next().await.is_none(),
            "Stream query failed: more elements than expected were produced"
        );

        drop(observed_stream);
        drop(table);

        read_txn
            .close()
            .await
            .expect("Could not close read transaction");

        db.compact().await.expect("Could not compact database");

        db.close().await.expect("Could not close database");
    }
}
