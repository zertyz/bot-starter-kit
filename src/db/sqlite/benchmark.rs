use crate::db::sqlite::sqlite_wrapper::Sqlite;
use anyhow::{Context, Result};
use futures::{Stream, StreamExt, TryStreamExt, stream};
use sqlx::FromRow;
use std::time::Instant;

#[derive(Debug, Clone, FromRow)]
struct Event {
    id: i64,
    seq: i64,
    score: i64,
    payload: String,
}

impl Event {
    const CREATE_TABLE_SQL: &'static str = r#"
        CREATE TABLE IF NOT EXISTS events (
            id      INTEGER NOT NULL, --  PRIMARY KEY not needed for this benchmark
            seq     INTEGER NOT NULL,
            score   INTEGER NOT NULL,
            payload TEXT NOT NULL
        )
        "#;

    // could also use the slower 'INSERT OR REPLACE INTO'
    const INSERT_SQL: &'static str = r#"
            INSERT INTO events (id, seq, score, payload)
            VALUES (?, ?, ?, ?)
            "#;

    /* not used, as this is an unoptimized index
       the optimized index version is defined bellow.
    const CREATE_SCORE_INDEX_SQL: &'static str =
        "CREATE INDEX IF NOT EXISTS idx_events_score ON events(score)";
     */

    /// Threshold used on the `SELECT_HIGH_SCORE_SQL` query and on the optimized partial index `INDEX_HIGH_SCORE_SQL`
    /// PRODUCTION WARNING: changing this value requires database maintenance -- the index have to be recalculated.
    const HIGH_SCORE_THRESHOLD: u64 = 99999;

    /// Optimized index for the query `SELECT_HIGH_SCORE_SQL`
    const INDEX_HIGH_SCORE_SQL: &'static str = const_format::concatcp!(
        "CREATE INDEX IF NOT EXISTS idx_events_score_high ON events(score) where score >= ",
        Event::HIGH_SCORE_THRESHOLD
    );

    /// Selects "high score" records as defined by `HIGH_SCORE_THRESHOLD`, forcibly using the optimized index `INDEX_HIGH_SCORE_SQL`
    const SELECT_HIGH_SCORE_SQL: &'static str = const_format::concatcp!(
        r#"
        SELECT id, seq, score, payload
        FROM events INDEXED BY idx_events_score_high
        WHERE score >= "#,
        Event::HIGH_SCORE_THRESHOLD
    );
}

fn make_event_stream(run_id: u128, items_count: usize) -> impl Stream<Item = Event> {
    stream::unfold(0usize, move |seq| async move {
        if seq >= items_count {
            return None;
        }

        let id = |i| (run_id << 32) as usize + i;

        let score = (seq % 100000) as i64;
        let event = Event {
            id: id(seq) as i64,
            seq: seq as i64,
            score,
            payload: format!("run-{run_id:032x}-seq-{seq:012}"),
        };

        Some((event, seq + 1))
    })
}

pub async fn benchmark(
    report: impl AsyncFn(String) -> Result<(), teloxide::errors::RequestError> + Send + 'static,
) -> Result<()> {
    let println = |msg: String| {
        println!("{msg}");
        report(msg)
    };

    let db_path = "/tmp/telegram_sqlite_benchmark/events.db";
    let expected_records = 1024 * 1024;

    let pool = Sqlite::open(
        db_path,
        &[Event::CREATE_TABLE_SQL, Event::INDEX_HIGH_SCORE_SQL],
    )
    .await?;
    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("system clock is before Unix epoch")?
        .as_nanos();

    println("STARTING SQLITE BENCHMARK".to_string()).await?;
    println(format!(
        "starting ingestion: {expected_records} total records"
    ))
    .await?;

    let started = Instant::now();

    // SQLite has one writer. Merge producer streams into one transaction pipeline so memory stays
    // bounded and the database does not thrash on writer contention.
    let input_stream = make_event_stream(run_id, expected_records)
        .enumerate()
        .then(|(i, event)| async move {
            if i % 100000 == 0 {
                _ = println(format!("Inserted records: {i} / {expected_records}...")).await;
            }
            event
        });
    let inserted = pool
        .ingest_stream(Event::INSERT_SQL, input_stream, |q, event| {
            q.bind(event.id)
                .bind(event.seq)
                .bind(event.score)
                .bind(event.payload)
        })
        .await?;

    let elapsed = started.elapsed();
    let rows_per_sec = inserted as f64 / elapsed.as_secs_f64();
    println(format!("✅ Ingestion completed -- rows/sec: {rows_per_sec:.0}. Now querying rows where `score` >= 99999 as a Stream -- but only showing about 1 for every 10 million records")).await?;

    let query_started = Instant::now();

    let mut result_stream = sqlx::query_as::<_, Event>(Event::SELECT_HIGH_SCORE_SQL)
        //.bind(99999_i64)  // no binding is happening now, since we moved to the optimized index -- the query now
        // has a hard coded value in it so the prepared statement is optimized
        .fetch(pool.inner());

    let mut matched_rows = 0usize;

    while let Some(row) = result_stream.try_next().await.context("fetch query row")? {
        matched_rows += 1;

        if matched_rows.is_multiple_of(100) {
            println(format!("✅ Ingestion completed -- rows/sec: {rows_per_sec:.0}. Now Querying: matched_rows={matched_rows}; id={}, seq={}, score={:?}, payload_len={}",
                            row.id,
                            row.seq,
                            row.score,
                            row.payload.len())).await?;
        }
    }
    let query_elapsed = query_started.elapsed();

    println(format!("✅ Ingestion completed -- rows/sec: {rows_per_sec:.0}. ✅ Querying completed in {query_elapsed:.3?}. Passively Synchronizing WAL...")).await?;
    let start = Instant::now();
    pool.passive_synchronize_wal().await?;
    let passive_elapsed = start.elapsed();
    println(format!("✅ Ingestion completed -- rows/sec: {rows_per_sec:.0}. ✅ Querying completed -- elapsed: {query_elapsed:.3?}. ✅ Passive WAL Sync Completed in {passive_elapsed:?}. Hard syncing WAL underway...")).await?;
    let start = Instant::now();
    pool.perform_daily_data_maintenance().await?;
    let hard_elapsed = start.elapsed();
    println(format!("✅ Ingestion completed -- rows/sec: {rows_per_sec:.0}. ✅ Querying completed -- elapsed: {query_elapsed:.3?}. ✅ Passive WAL Sync Completed in {passive_elapsed:?}. ✅ Hard Sync Completed in {hard_elapsed:?}. Closing the Database...")).await?;
    let start = Instant::now();
    pool.close_db().await?;
    let close_elapsed = start.elapsed();
    println(format!("🏁 Ingestion completed -- rows/sec: {rows_per_sec:.0}. 🏁 Querying completed -- elapsed: {query_elapsed:.3?}. 🏁 Passive WAL Sync Completed in {passive_elapsed:?}. 🏁 Hard Sync Completed in {hard_elapsed:?}. 🏁 Database Closed in {close_elapsed:?}...")).await?;

    Ok(())
}
