use ::heed::{Database, Env, EnvOpenOptions, RoTxn, RwTxn, WithoutTls};
use anyhow::{Result, anyhow, ensure};
use futures::{Stream, StreamExt};
use std::path::Path;
use std::pin::pin;
use tokio::sync::{Mutex, MutexGuard, Semaphore, SemaphorePermit};

/// Maximum map size for the mmapped database.
/// DB operations will fail after this limit is reached.
pub const MAX_MAP_SIZE_BYTES: usize = 30 * 1024 * 1024 * 1024;
pub const DEFAULT_MAX_READERS: u32 = 16;
pub const DEFAULT_MAX_DBS: u32 = 16;

/// Async-friendly wrapper around `heed::Env`.
///
/// LMDB is already thread-safe, but it has a single writer. If a second writer calls
/// `heed::Env::write_txn()` directly, that Tokio worker can block on LMDB's internal mutex.
/// This wrapper puts an async mutex in front of write transaction creation, so contending bot
/// tasks await cooperatively before entering LMDB.
///
/// Read transactions use `read_txn_without_tls()`, which makes them `Send` and suitable for
/// Tokio's work-stealing runtime. They are still gated by a semaphore because long-lived LMDB
/// readers keep old pages alive and can grow the database.
pub struct AsyncHeed {
    env: Env<WithoutTls>,
    w_lock: Mutex<()>,
    r_lock: Semaphore,
    max_readers: u32,
}

impl AsyncHeed {
    pub async fn open(env_path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_options(
            env_path,
            MAX_MAP_SIZE_BYTES,
            DEFAULT_MAX_READERS,
            DEFAULT_MAX_DBS,
        )
        .await
    }

    pub async fn open_with_options(
        env_path: impl AsRef<Path>,
        max_map_size_bytes: usize,
        max_readers: u32,
        max_dbs: u32,
    ) -> Result<Self> {
        ensure!(
            max_readers > 0,
            "`heed` max_readers must be greater than zero"
        );
        ensure!(max_dbs > 0, "`heed` max_dbs must be greater than zero");

        let env_path = env_path.as_ref();
        std::fs::create_dir_all(env_path)
            .map_err(|err| anyhow!("Heed: failed to create env directory {env_path:?}: {err}"))?;

        let mut options = EnvOpenOptions::new().read_txn_without_tls();
        options
            .map_size(max_map_size_bytes)
            .max_readers(max_readers)
            .max_dbs(max_dbs);

        // SAFETY:
        // - The wrapper does not set NO_LOCK.
        // - The environment directory is created and then managed by LMDB.
        // - Concurrent access goes through LMDB's locks plus the async writer/read gates here.
        // Callers must still avoid external mutation of the LMDB files and remote filesystems.
        let env = unsafe { options.open(env_path) }
            .map_err(|err| anyhow!("Heed: failed to open env at {env_path:?}: {err}"))?;

        env.clear_stale_readers()
            .map_err(|err| anyhow!("Heed: failed to clear stale reader slots: {err}"))?;

        Ok(Self {
            env,
            w_lock: Mutex::new(()),
            r_lock: Semaphore::new(max_readers as usize),
            max_readers,
        })
    }

    pub fn env(&self) -> &Env<WithoutTls> {
        &self.env
    }

    pub async fn begin_read(&self) -> Result<HeedReadTransaction<'_>> {
        let read_permit = self
            .r_lock
            .acquire()
            .await
            .map_err(|err| anyhow!("Failed to acquire `heed` reader permit: {err}"))?;

        let read_txn = self
            .env
            .read_txn()
            .map_err(|err| anyhow!("Failed to begin `heed` read transaction: {err}"))?;

        Ok(HeedReadTransaction::new(read_txn, read_permit))
    }

    pub async fn begin_write(&self) -> Result<HeedWriteTransaction<'_>> {
        let write_guard = self.w_lock.lock().await;
        let write_txn = self
            .env
            .write_txn()
            .map_err(|err| anyhow!("Failed to begin `heed` write transaction: {err}"))?;

        Ok(HeedWriteTransaction::new(write_txn, write_guard))
    }

    pub async fn create_database<KC, DC>(&self, name: Option<&str>) -> Result<Database<KC, DC>>
    where
        KC: 'static,
        DC: 'static,
    {
        let mut write_txn = self.begin_write().await?;
        let db = self
            .env
            .create_database(write_txn.inner_mut(), name)
            .map_err(|err| anyhow!("Heed: failed to create/open database {name:?}: {err}"))?;
        write_txn.commit().await?;
        Ok(db)
    }

    pub async fn open_database<KC, DC>(
        &self,
        name: Option<&str>,
    ) -> Result<Option<Database<KC, DC>>>
    where
        KC: 'static,
        DC: 'static,
    {
        let read_txn = self.begin_read().await?;
        let db = self
            .env
            .open_database(read_txn.inner(), name)
            .map_err(|err| anyhow!("Heed: failed to open database {name:?}: {err}"))?;
        read_txn.close().await?;
        Ok(db)
    }

    pub async fn ingest_stream<KC, DC, KeyType, ValueType>(
        &self,
        database: &Database<KC, DC>,
        input_stream: impl Stream<Item = (KeyType, ValueType)>,
    ) -> Result<usize>
    where
        for<'item> KC: heed::BytesEncode<'item, EItem = KeyType>,
        for<'item> DC: heed::BytesEncode<'item, EItem = ValueType>,
    {
        let mut write_txn = self.begin_write().await.map_err(|err| {
            anyhow!(
                "Heed: beginning Stream ingestion -- couldn't create the write transaction: {err}"
            )
        })?;

        let mut count = 0;
        let mut input_stream = pin!(input_stream);
        while let Some((key, value)) = input_stream.next().await {
            database
                .put(write_txn.inner_mut(), &key, &value)
                .map_err(|err| {
                    anyhow!(
                        "Heed: Couldn't insert/replace item #{count} during Stream ingestion: {err}"
                    )
                })?;
            count += 1;
        }

        write_txn.commit().await.map_err(|err| {
            anyhow!("Heed: Could not commit transaction after Stream ingestion: {err}")
        })?;

        Ok(count)
    }

    /// Wait for all wrapper-managed readers, then resize the LMDB map.
    ///
    /// LMDB requires no active transaction while resizing. The async gates make that true for
    /// transactions created through this wrapper.
    pub async fn resize(&self, new_map_size_bytes: usize) -> Result<()> {
        let _write_guard = self.w_lock.lock().await;
        let _read_permits = self
            .r_lock
            .acquire_many(self.max_readers)
            .await
            .map_err(|err| {
                anyhow!("Failed to acquire all `heed` reader permits for resize: {err}")
            })?;

        // SAFETY: all wrapper-created read and write transactions are excluded above.
        unsafe { self.env.resize(new_map_size_bytes) }
            .map_err(|err| anyhow!("Failed to resize `heed` map: {err}"))
    }

    /// Force dirty mmap pages to disk.
    ///
    /// This is synchronous at the LMDB layer and may block the current Tokio task briefly.
    pub async fn force_sync(&self) -> Result<()> {
        self.env
            .force_sync()
            .map_err(|err| anyhow!("Failed to force-sync `heed` env: {err}"))
    }

    pub async fn close(self) -> Result<()> {
        self.env.prepare_for_closing().wait();
        Ok(())
    }
}

/// Read transaction guarded by one reader semaphore permit.
///
/// Field order is intentional: `inner` drops before `_read_permit`, so the LMDB read transaction
/// closes before another wrapper-managed reader can take this slot.
pub struct HeedReadTransaction<'env> {
    inner: RoTxn<'env, WithoutTls>,
    _read_permit: SemaphorePermit<'env>,
}

impl<'env> HeedReadTransaction<'env> {
    fn new(read_txn: RoTxn<'env, WithoutTls>, read_permit: SemaphorePermit<'env>) -> Self {
        Self {
            inner: read_txn,
            _read_permit: read_permit,
        }
    }

    pub fn inner(&self) -> &RoTxn<'env, WithoutTls> {
        &self.inner
    }

    pub async fn close(self) -> Result<()> {
        self.inner
            .commit()
            .map_err(|err| anyhow!("Failed to close `heed` reader: {err}"))
    }
}

/// Write transaction guarded by the wrapper's async writer mutex.
///
/// Field order is intentional: `inner` drops before `_write_guard`, so LMDB aborts an uncommitted
/// transaction before another wrapper-managed writer can be admitted.
pub struct HeedWriteTransaction<'env> {
    inner: RwTxn<'env>,
    _write_guard: MutexGuard<'env, ()>,
}

impl<'env> HeedWriteTransaction<'env> {
    fn new(write_txn: RwTxn<'env>, write_guard: MutexGuard<'env, ()>) -> Self {
        Self {
            inner: write_txn,
            _write_guard: write_guard,
        }
    }

    pub fn inner(&self) -> &RwTxn<'env> {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut RwTxn<'env> {
        &mut self.inner
    }

    pub async fn commit(self) -> Result<()> {
        self.inner
            .commit()
            .map_err(|err| anyhow!("Failed to commit `heed` transaction: {err}"))
    }

    pub async fn abort(self) {
        self.inner.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::heed::HeedPod;
    use ::heed::byteorder::BigEndian;
    use ::heed::types::U64;
    use futures::stream;
    use std::path::PathBuf;

    type TestKey = U64<BigEndian>;
    type TestValue = HeedPod<TestModel>;

    #[repr(C)]
    #[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
    struct TestModel {
        count: u64,
        whatever: u64,
    }

    fn expected_data_iter() -> impl Iterator<Item = TestModel> {
        (0..128).map(|i| TestModel {
            count: 127 - i,
            whatever: i,
        })
    }

    fn key(i: usize) -> u64 {
        i as u64
    }

    fn unique_env_path() -> PathBuf {
        let run_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock is before Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "test_heed_wrapper_{}_{}",
            std::process::id(),
            run_id
        ))
    }

    #[tokio::test]
    async fn hello_api() -> Result<()> {
        let env_path = unique_env_path();
        let _ = std::fs::remove_dir_all(&env_path);

        let heed = AsyncHeed::open_with_options(&env_path, 16 * 1024 * 1024, 16, 4)
            .await
            .expect("Could not create database");

        let database: Database<TestKey, TestValue> = heed
            .create_database(Some("heed_wrapper_data"))
            .await
            .expect("Could not open/create database");

        let expected_count = expected_data_iter().count();
        let input_stream = stream::iter(
            expected_data_iter()
                .enumerate()
                .map(|(i, value)| (key(i), value)),
        );

        let inserted = heed
            .ingest_stream(&database, input_stream)
            .await
            .expect("Could not ingest Stream");
        assert_eq!(inserted, expected_count);

        let reopened_database = heed
            .open_database::<TestKey, TestValue>(Some("heed_wrapper_data"))
            .await
            .expect("Could not open existing database")
            .expect("Database should exist");

        let read_txn = heed
            .begin_read()
            .await
            .expect("Could not begin read transaction");

        for (i, expected_value) in expected_data_iter().enumerate() {
            let observed_value = reopened_database
                .get(read_txn.inner(), &key(i))
                .expect("Could not read test record")
                .unwrap_or_else(|| panic!("Test record #{i} is not present"));

            assert_eq!(
                observed_value.read_unaligned(),
                expected_value,
                "Point query failed @ row #{i}"
            );
        }

        let mut observed_iter = reopened_database
            .iter(read_txn.inner())
            .expect("Could not iterate database");

        for (i, expected_value) in expected_data_iter().enumerate() {
            let (observed_key, observed_value) = observed_iter
                .next()
                .transpose()
                .expect("Could not read next iterator item")
                .unwrap_or_else(|| panic!("Stream query failed: row #{i} is not present"));

            assert_eq!(
                observed_key,
                key(i),
                "Stream query failed: row #{i} yielded key {observed_key}"
            );
            assert_eq!(
                observed_value.read_unaligned(),
                expected_value,
                "Stream query failed @ row #{i}"
            );
        }

        assert!(
            observed_iter
                .next()
                .transpose()
                .expect("Could not check end of iterator")
                .is_none(),
            "Stream query failed: more elements than expected were produced"
        );

        drop(observed_iter);
        read_txn
            .close()
            .await
            .expect("Could not close read transaction");

        heed.force_sync()
            .await
            .expect("Could not force-sync database");
        heed.close().await.expect("Could not close database");
        let _ = std::fs::remove_dir_all(&env_path);

        Ok(())
    }
}
