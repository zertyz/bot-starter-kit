use crate::db::sqlite::sqlite_wrapper::Sqlite;
use anyhow::{Result, anyhow};
use futures::{Stream, StreamExt, TryStreamExt, stream};
use sqlx::FromRow;
use std::{
    fmt::Display,
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, FromRow)]
pub struct Event {
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
    pub const HIGH_SCORE_THRESHOLD: u64 = 99999;

    /// Optimized index for the query `SELECT_HIGH_SCORE_SQL`
    const INDEX_HIGH_SCORE_SQL: &'static str = const_format::concatcp!("CREATE INDEX IF NOT EXISTS idx_events_score_high ON events(score) where score >= ", Event::HIGH_SCORE_THRESHOLD);

    /// Selects "high score" records as defined by `HIGH_SCORE_THRESHOLD`, forcibly using the optimized index `INDEX_HIGH_SCORE_SQL`
    const SELECT_HIGH_SCORE_SQL: &'static str = const_format::concatcp!(
        r#"
        SELECT id, seq, score, payload
        FROM events INDEXED BY idx_events_score_high
        WHERE score >= "#,
        Event::HIGH_SCORE_THRESHOLD
    );
}

#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub db_path: PathBuf,
    pub expected_records: u64,
    pub progress_every: Option<u64>,
    pub benchmark_point_query: bool,
    pub benchmark_range_query: bool,
    pub run_wal_maintenance: bool,
}

impl BenchmarkConfig {
    fn setup_sqls(&self) -> Vec<&'static str> {
        let mut setup_sqls = vec![Event::CREATE_TABLE_SQL];

        setup_sqls.push(Event::INDEX_HIGH_SCORE_SQL);

        setup_sqls
    }
}

#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub inserted: u64,
    pub ingest_elapsed: Duration,
    pub point_query: Option<QueryResult>,
    pub range_query: Option<QueryResult>,
    pub passive_sync_elapsed: Option<Duration>,
    pub hard_sync_elapsed: Option<Duration>,
    pub close_elapsed: Duration,
}

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub matched_rows: u64,
    pub elapsed: Duration,
}

fn record_id(run_id: u128, seq: u64) -> i64 {
    ((run_id << 32) as u64).wrapping_add(seq) as i64
}

fn make_event_stream(run_id: u128, items_count: u64) -> impl Stream<Item = Event> {
    stream::unfold(0, move |seq| async move {
        if seq >= items_count {
            return None;
        }

        let score = (seq % 100000) as i64;
        let event = Event {
            id: record_id(run_id, seq),
            seq: seq as i64,
            score,
            payload: format!("run-{run_id:032x}-seq-{seq:012}"),
        };

        Some((event, seq + 1))
    })
}

async fn report_progress<Report, ReportError>(report: &Report, msg: String) -> Result<()>
where
    Report: AsyncFn(String) -> std::result::Result<(), ReportError> + ?Sized,
    ReportError: Display + Send + Sync + 'static,
{
    report(msg)
        .await
        .map_err(|err| anyhow!("{err}"))
}

pub async fn ui_benchmark(report: impl AsyncFn(String) -> Result<(), teloxide::errors::RequestError> + Send + Sync + 'static) -> Result<()> {
    run_benchmark(
        BenchmarkConfig {
            db_path: PathBuf::from("/tmp/telegram_sqlite_benchmark/events.db"),
            expected_records: 1024 * 1024,
            progress_every: Some(100_000),
            benchmark_point_query: false,
            benchmark_range_query: true,
            run_wal_maintenance: true,
        },
        report,
    )
    .await
    .map(|_result| ())
}

pub async fn run_benchmark<Report, ReportError>(config: BenchmarkConfig, report: Report) -> Result<BenchmarkResult>
where
    Report: AsyncFn(String) -> std::result::Result<(), ReportError> + Send + Sync + 'static,
    ReportError: Display + Send + Sync + 'static,
{
    let setup_sqls = config.setup_sqls();
    let pool = Sqlite::open(&config.db_path, &setup_sqls).await?;
    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| anyhow!("system clock is before Unix epoch: {err}"))?
        .as_nanos();

    report_progress(&report, "STARTING SQLITE BENCHMARK".to_string()).await?;
    report_progress(&report, format!("starting ingestion: {} total records", config.expected_records)).await?;

    let started = Instant::now();
    let expected_records = config.expected_records;
    let progress_every = config.progress_every;

    // SQLite has one writer. Merge producer streams into one transaction pipeline so memory stays
    // bounded and the database does not thrash on writer contention.
    let input_stream = make_event_stream(run_id, expected_records)
        .enumerate()
        .then(|(i, event)| {
            let report = &report;
            async move {
                if progress_every.is_some_and(|progress_every| progress_every > 0 && i.is_multiple_of(progress_every as usize)) {
                    _ = report_progress(report, format!("Inserted records: {i} / {expected_records}...")).await;
                }
                event
            }
        });
    let inserted = pool
        .ingest_stream(Event::INSERT_SQL, input_stream, |q, event| {
            q.bind(event.id)
                .bind(event.seq)
                .bind(event.score)
                .bind(event.payload)
        })
        .await?;

    let ingest_elapsed = started.elapsed();
    let inserted_rows_per_sec = inserted as f64 / ingest_elapsed.as_secs_f64();
    report_progress(&report, format!("✅ Ingestion completed -- rows/sec: {inserted_rows_per_sec:.0}.")).await?;

    let point_query = if config.benchmark_point_query { Some(benchmark_point_query(&pool, &report, inserted_rows_per_sec).await?) } else { None };
    let range_query = if config.benchmark_range_query { Some(benchmark_range_query(&pool, &report, inserted_rows_per_sec).await?) } else { None };

    let query = range_query
        .as_ref()
        .or(point_query.as_ref())
        .expect("No Query was selected!");
    report_progress(
        &report,
        format!(
            "✅ Ingestion completed -- rows/sec: {inserted_rows_per_sec:.0}; ✅ Querying matched {} rows in {:?}. Closing the Database...",
            query.matched_rows, query.elapsed
        ),
    )
    .await?;

    let (passive_sync_elapsed, hard_sync_elapsed) = if config.run_wal_maintenance {
        report_progress(&report, "SQLite queries completed. Passively synchronizing WAL...".to_string()).await?;
        let start = Instant::now();
        pool.passive_synchronize_wal()
            .await?;
        let passive_elapsed = start.elapsed();
        report_progress(&report, format!("✅ Passive WAL Sync Completed in {passive_elapsed:?}. Hard syncing WAL underway...")).await?;
        let start = Instant::now();
        pool.perform_daily_data_maintenance()
            .await?;
        let hard_elapsed = start.elapsed();
        report_progress(&report, format!("✅ Hard Sync Completed in {hard_elapsed:?}. Closing the Database...")).await?;
        (Some(passive_elapsed), Some(hard_elapsed))
    } else {
        (None, None)
    };

    let start = Instant::now();
    pool.close_db()
        .await?;
    let close_elapsed = start.elapsed();
    report_progress(
        &report,
        format!(
            "🏁 Ingestion completed -- rows/sec: {inserted_rows_per_sec:.0}; 🏁 Querying matched {} rows in {:?}; Database closed in {close_elapsed:?}.",
            query.matched_rows, query.elapsed
        ),
    )
    .await?;

    Ok(BenchmarkResult {
        inserted,
        ingest_elapsed,
        point_query,
        range_query,
        passive_sync_elapsed,
        hard_sync_elapsed,
        close_elapsed,
    })
}

pub async fn benchmark_point_query<Report, ReportError>(pool: &Sqlite, report: &Report, inserted_rows_per_sec: f64) -> Result<QueryResult>
where
    Report: AsyncFn(String) -> std::result::Result<(), ReportError> + ?Sized,
    ReportError: Display + Send + Sync + 'static,
{
    // here we take advantage of SQLite's range queries: since we only want 1 every Event::HIGH_SCORE_THRESHOLD records
    benchmark_range_query(pool, report, inserted_rows_per_sec).await
}

pub async fn benchmark_range_query<Report, ReportError>(pool: &Sqlite, report: &Report, inserted_rows_per_sec: f64) -> Result<QueryResult>
where
    Report: AsyncFn(String) -> std::result::Result<(), ReportError> + ?Sized,
    ReportError: Display + Send + Sync + 'static,
{
    let query_started = Instant::now();
    let mut result_stream = sqlx::query_as::<_, Event>(Event::SELECT_HIGH_SCORE_SQL)
        //.bind(99999_i64)  // no binding is happening now, since we moved to the optimized index -- the query now
        // has a hard coded value in it so the prepared statement is optimized
        .fetch(pool.inner());

    let mut matched_rows = 0u64;

    while let Some(row) = result_stream
        .try_next()
        .await
        .map_err(|err| anyhow!("fetch indexed query row: {err}"))?
    {
        matched_rows += 1;

        if matched_rows.is_multiple_of(100) {
            report_progress(
                report,
                format!(
                    "✅ Ingestion completed -- rows/sec: {inserted_rows_per_sec:.0}. SQLite indexed querying: matched_rows={matched_rows}; id={}, seq={}, score={:?}, payload_len={}",
                    row.id,
                    row.seq,
                    row.score,
                    row.payload
                        .len()
                ),
            )
            .await?;
        }
    }
    let elapsed = query_started.elapsed();
    report_progress(report, format!("✅ SQLite indexed querying completed -- matched_rows={matched_rows}, elapsed: {elapsed:.3?}.")).await?;

    Ok(QueryResult { matched_rows, elapsed })
}
