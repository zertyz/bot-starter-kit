//! Central point for instantiating sqlx & sqlite connections

use anyhow::{Result, anyhow};
use futures::{Stream, StreamExt};
use sqlx::{
    SqlitePool,
    query::Query,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};
use std::pin::pin;
use std::{path::Path, str::FromStr, time::Duration};

pub struct Sqlite {
    pool: SqlitePool,
}

impl Sqlite {
    pub async fn open(db_path: impl AsRef<Path>, model_setup_sqls: &[&str]) -> Result<Self> {
        let options = SqliteConnectOptions::from_str(&format!(
            "sqlite://{}",
            db_path
                .as_ref()
                .display()
        ))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Off) // data is still handled to OS -- unsafe if OS or VM may crash. Use "Normal" for tighter constraints
        .busy_timeout(Duration::from_secs(120))
        // Negative cache_size is KiB. Keep this tiny for constrained environments.
        .pragma("cache_size", "-65536")
        // Keep temp b-trees/files on disk instead of reserving extra memory.
        .pragma("temp_store", "FILE")
        // the bellow line is optimized for BTRFS with a compressed DB file but uncompressed WAL files:
        // data from inserts and updates are put into the uncompressed WAL files -- hence requiring no additional CPU
        // and WAL checkpointing is done at the end of the day, moving data from WAL to the compressed DB file -- taking as many resources as needed.
        // NOTE: reads of older data will come from the compressed DB file, but still will require minor CPU as decompressing zstd is cheap.
        // The end of the day routine checkpoints WAL with:
        //   sqlx::query("PRAGMA wal_checkpoint(PASSIVE)").execute(&pool).await?;   // execute 3 times in a row, as passive doesn't block but may leave some records unprocessed
        //   sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)").execute(&pool).await?;  // this one will lock readers and writers, but will reclaim WAL files space
        //   sqlx::query("PRAGMA optimize").execute(&pool).await?;                  // update planner stats. cheap-ish. may be moved to a bigger weekend maintenance window.
        // hint: passive may be executed on "soft" maintenance windows before being executed again just before TRUNCATE.
        //       this way, BTRFS will be busy compressing things, but the system won't be locked for reads nor for writes in the "soft" period -- and the "hard" period will be shorter.
        // BTRFS compression hint:
        // 1) Choose the database file to be inside DB_DIR, where DB_DIR is meant to only contain sqlite files.
        // 2) Do `mkdir -p DB_DIR; touch DB_DIR/DB_FILE; chattr +C DB_DIR` -- no CoW. This option will be used for WAL files (which are temporary).
        // 3) Do `lsattr -la DB_DIR`. You want to see No CoW for the directory and don't want to see that for the DB file.
        // 4) Run the program for the first time and `lsattr -la DB_DIR` again. If the DB_FILE shows "do not compress", quit the application and compress it:
        //    chattr -m DB_DIR/DB_FILE  # re-enables compression
        //    btrfs filesystem defragment -czstd DB_DIR/DB_FILE
        // Additional Hints:
        // a) During the "hard" maintenance window, make the program say this to the user and bail out immediately:
        //    "We are on daily data maintenance window. It started X minutes ago and is expected to last for the next Y minutes. Plase try again later."
        // b) There may be an even "harder" maintenance window to shrink the DB file and optimize everything. The sequence:
        //    PRAGMA wal_checkpoint(TRUNCATE); PRAGMA optimize; VACUUM; PRAGMA wal_checkpoint(TRUNCATE);
        //    the last VACUUM and TRUNCATE commands are optional: only when the DB needs to be optimized and shrunk. Check with:
        //    SELECT name, sum(pgsize) AS total_bytes, sum(unused) AS unused_bytes, 100.0 * sum(unused) / sum(pgsize) AS unused_pct FROM dbstat GROUP BY name ORDER BY unused_bytes DESC;
        //    if unused_pct is higher than 5% for the big tables and we are on a weekend maintenance window, then VACUUM may be applied. It is also a good idea to update the maintenance message to users for the extended time off.
        //    PS: the above query seems to be dependent on the DB file size and might take a considerable time to run >100s on a micro VM with a compressed 10G database.
        //    PS 2: PRAGMA optimize is not as cheap as mentioned... costs about the same as the above query. So it might be applied only on weekend windows as well.
        .pragma("wal_autocheckpoint", "32768") // accept WAL files up to 128M (that number of 4kb pages)
        .pragma("mmap_size", "17179869184") // mmaps up to that portion of the DB file to help with zero-copy reads
        .pragma("locking_mode", "EXCLUSIVE");

        let pool = SqlitePoolOptions::new()
            // EXCLUSIVE locking above requires a single connection -- or else you'll see 'pool timed out while waiting for an open connection' errors.
            // on real code, make the bellow 16 and the locking_mode to NORMAL
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(|err| anyhow!("opening SQLite database: {err}"))?;

        for (i, model_setup_sql) in model_setup_sqls
            .iter()
            .enumerate()
        {
            sqlx::query(model_setup_sql)
                .execute(&pool)
                .await
                .map_err(|err| anyhow!("SQLite: applying the model setup SQL #{i}: {model_setup_sql:?}: {err}"))?;
        }

        Ok(Self { pool })
    }

    pub async fn close_db(&self) -> Result<()> {
        self.pool
            .close()
            .await;
        Ok(())
    }

    /// Can be used on low usage periods
    /// -- will cause most of WAL content to be incorporated into the database, excluding those that would require a database lock.
    pub async fn passive_synchronize_wal(&self) -> Result<()> {
        sqlx::query("PRAGMA wal_checkpoint(PASSIVE)")
            .execute(&self.pool)
            .await
            .map_err(|err| anyhow!("WAL checkpoint PASSIVE: {err}"))?;
        Ok(())
    }

    /// Should only be used when the system won't be used -- the database will be locked while it runs.
    /// -- will cause all of the WAL content to be incorporated into the database, and later dropping the WAL files.
    pub async fn perform_daily_data_maintenance(&self) -> Result<()> {
        // no db lock, but will still consume resources
        // -- the TRUNCATE checkpoint will have less work to do
        self.passive_synchronize_wal()
            .await?;

        // full db lock while this runs
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&self.pool)
            .await
            .map_err(|err| anyhow!("WAL checkpoint TRUNCATE: {err}"))?;

        Ok(())
    }

    /// Run this instead of `perform_daily_data_maintenance()` on Sunday early mornings
    pub async fn perform_weekly_data_maintenance(&self) -> Result<()> {
        self.perform_daily_data_maintenance()
            .await?;

        // will lock and take, potentially, a very long time
        // hint: if you set "PRAGMA page-size n" before the VACUUM command, the db's page size will, effectively, change
        //       -- where n is a power of 2, max 64k, and defaults to 4k
        sqlx::query("VACUUM")
            .execute(&self.pool)
            .await
            .map_err(|err| anyhow!("Error during VACUUM: {err}"))?;

        // this one might take some time, but doesn't lock
        sqlx::query("PRAGMA optimize")
            .execute(&self.pool)
            .await
            .map_err(|err| anyhow!("Error during optimize: {err}"))?;

        // might not lock for too much time after all of the above
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&self.pool)
            .await
            .map_err(|err| anyhow!("Final WAL checkpoint TRUNCATE: {err}"))?;

        // after this, the following can be run to re-compress the database without locking it
        // chattr -m sqlite.db; btrfs -v filesystem defragment -r -czstd sqlite.db
        // or -- to investigate: this might need only to be ran once after every restart
        // (sqlite seems to reset the -m "do not compress" file attribute on each open operation)

        Ok(())
    }

    pub async fn ingest_stream<ItemType>(
        &self,
        insert_sql: &str,
        input_stream: impl Stream<Item = ItemType>,
        bind_fn: impl for<'r> Fn(Query<'r, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'r>>, ItemType) -> Query<'r, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'r>>,
    ) -> Result<u64> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| anyhow!("SQLite: beginning Stream ingestion with sql '{insert_sql}' -- couldn't create the transaction: {err}"))?;

        let mut count = 0;
        let mut input_stream = pin!(input_stream);
        while let Some(item) = input_stream
            .next()
            .await
        {
            bind_fn(sqlx::query(insert_sql), item)
                .execute(&mut *tx)
                .await
                .map(|_sqlite_query_result| ())
                .map_err(|err| anyhow!("SQLite: ingesting stream with sql '{insert_sql}' failed at element #{count}: {err}"))?;
            count += 1;
        }

        tx.commit()
            .await
            .map_err(|err| anyhow!("SQLite: ingesting stream with sql '{insert_sql}' failed @ the final commit: {err}"))?;

        Ok(count)
    }

    pub fn inner(&self) -> &SqlitePool {
        &self.pool
    }
}
