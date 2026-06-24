use std::path::Path;

use ::heed::{Database, Env, EnvOpenOptions, RoTxn, RwTxn, WithoutTls};
use anyhow::{Context, Result, ensure};
use tokio::sync::{Mutex, MutexGuard, Semaphore, SemaphorePermit};

/// Default map size for the starter-kit demos.
///
/// LMDB reserves virtual address space for this value; it does not allocate this much RAM up
/// front. The 2 GiB default should give enough room for several operations before maintenance.
pub const DEFAULT_MAP_SIZE_BYTES: usize = 2 * 1024 * 1024 * 1024;
pub const DEFAULT_MAX_READERS: u32 = 64;
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
            DEFAULT_MAP_SIZE_BYTES,
            DEFAULT_MAX_READERS,
            DEFAULT_MAX_DBS,
        )
        .await
    }

    pub async fn open_with_options(
        env_path: impl AsRef<Path>,
        map_size_bytes: usize,
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
            .with_context(|| format!("Heed: failed to create env directory {env_path:?}"))?;

        let mut options = EnvOpenOptions::new().read_txn_without_tls();
        options
            .map_size(map_size_bytes)
            .max_readers(max_readers)
            .max_dbs(max_dbs);

        // SAFETY:
        // - The wrapper does not set NO_LOCK.
        // - The environment directory is created and then managed by LMDB.
        // - Concurrent access goes through LMDB's locks plus the async writer/read gates here.
        // Callers must still avoid external mutation of the LMDB files and remote filesystems.
        let env = unsafe { options.open(env_path) }
            .with_context(|| format!("Heed: failed to open env at {env_path:?}"))?;

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
            .context("Failed to acquire `heed` reader permit")?;

        let read_txn = self
            .env
            .read_txn()
            .context("Failed to begin `heed` read transaction")?;

        Ok(HeedReadTransaction::new(read_txn, read_permit))
    }

    pub async fn begin_write(&self) -> Result<HeedWriteTransaction<'_>> {
        let write_guard = self.w_lock.lock().await;
        let write_txn = self
            .env
            .write_txn()
            .context("Failed to begin `heed` write transaction")?;

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
            .with_context(|| format!("Heed: failed to create/open database {name:?}"))?;
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
            .with_context(|| format!("Heed: failed to open database {name:?}"))?;
        read_txn.close().await?;
        Ok(db)
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
            .context("Failed to acquire all `heed` reader permits for resize")?;

        // SAFETY: all wrapper-created read and write transactions are excluded above.
        unsafe { self.env.resize(new_map_size_bytes) }.context("Failed to resize `heed` map")
    }

    /// Force dirty mmap pages to disk.
    ///
    /// This is synchronous at the LMDB layer and may block the current Tokio task briefly.
    pub async fn force_sync(&self) -> Result<()> {
        self.env
            .force_sync()
            .context("Failed to force-sync `heed` env")
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
        self.inner.commit().context("Failed to close `heed` reader")
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
            .context("Failed to commit `heed` transaction")
    }

    pub async fn abort(self) {
        self.inner.abort();
    }
}
