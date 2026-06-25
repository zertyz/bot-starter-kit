use crate::db::redb::AsyncReDb;
use crate::redb_mmap_value;
use anyhow::{Result, anyhow};
use futures::{Stream, StreamExt, stream};
use redb::TableDefinition;
use std::{
    fmt::Display,
    path::PathBuf,
    time::{Duration, Instant},
};

const EVENTS_TABLE: TableDefinition<u64, Event> = TableDefinition::new("redb_wrapper_data");

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Event {
    seq: i64,
    score: i64,
    payload: [u8; 53],
    _pad: [u8; 3], // makes the above struct honor the 8-byte alignment requirement due to i64
                   // note: no id field here, as the ID will be in the database in the form of the "key", hence we've omitted it from this "value" struct
}
redb_mmap_value!(Event);

#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub db_path: PathBuf,
    pub expected_records: usize,
    pub point_query_step: usize,
    pub progress_every: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub inserted: usize,
    pub ingest_elapsed: Duration,
    pub point_query: QueryResult,
    pub close_elapsed: Duration,
}

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub matched_rows: usize,
    pub elapsed: Duration,
}

fn to_byte_array<const N: usize>(text: String) -> [u8; N] {
    let mut byte_array = [0u8; N];
    byte_array.copy_from_slice(text.as_bytes());
    byte_array
}

fn make_event_stream(run_id: u128, records_per_task: usize) -> impl Stream<Item = Event> {
    stream::unfold(0usize, move |seq| async move {
        if seq >= records_per_task {
            return None;
        }

        let score = (seq % 100000) as i64;
        let event = Event {
            seq: seq as i64,
            score,
            payload: to_byte_array::<53>(format!("run-{run_id:032x}-seq-{seq:012}")),
            _pad: Default::default(),
        };

        Some((event, seq + 1))
    })
}

fn record_key(run_id: u128, seq: usize) -> u64 {
    ((run_id << 32) as u64).wrapping_add(seq as u64)
}

async fn report_progress<Report, ReportError>(report: &Report, msg: String) -> Result<()>
where
    Report: AsyncFn(String) -> std::result::Result<(), ReportError> + ?Sized,
    ReportError: Display + Send + Sync + 'static,
{
    report(msg).await.map_err(|err| anyhow!("{err}"))
}

pub async fn ui_benchmark(
    report: impl AsyncFn(String) -> Result<(), teloxide::errors::RequestError> + Send + Sync + 'static,
) -> Result<()> {
    run_benchmark(
        BenchmarkConfig {
            db_path: PathBuf::from("/tmp/telegram_redb_benchmark/events.redb"),
            expected_records: 1024 * 1024,
            point_query_step: 100_000,
            progress_every: Some(100_000),
        },
        report,
    )
    .await
    .map(|_result| ())
}

pub async fn run_benchmark<Report, ReportError>(
    config: BenchmarkConfig,
    report: Report,
) -> Result<BenchmarkResult>
where
    Report: AsyncFn(String) -> std::result::Result<(), ReportError> + Send + Sync + 'static,
    ReportError: Display + Send + Sync + 'static,
{
    let redb = AsyncReDb::open(&config.db_path).await?;
    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| anyhow!("system clock is before Unix epoch: {err}"))?
        .as_nanos();

    report_progress(&report, "STARTING ReDb BENCHMARK".to_string()).await?;
    report_progress(
        &report,
        format!(
            "starting ingestion: {} total records",
            config.expected_records
        ),
    )
    .await?;

    let started = Instant::now();
    let expected_records = config.expected_records;
    let progress_every = config.progress_every;

    // ReDB has one writer. Merge producer streams into one transaction pipeline so memory stays
    // bounded and the database does not thrash on writer contention.
    let input_stream = make_event_stream(run_id, expected_records)
        .enumerate()
        .then(|(i, event)| {
            let report = &report;
            async move {
                if progress_every.is_some_and(|progress_every| {
                    progress_every > 0 && i.is_multiple_of(progress_every)
                }) {
                    _ = report_progress(
                        report,
                        format!("Inserted records: {i} / {expected_records}..."),
                    )
                    .await;
                }
                (record_key(run_id, i), event)
            }
        });

    let inserted = redb.ingest_stream(EVENTS_TABLE, input_stream).await?;

    let ingest_elapsed = started.elapsed();
    let inserted_rows_per_sec = inserted as f64 / ingest_elapsed.as_secs_f64();
    report_progress(
        &report,
        format!(
            "✅ Ingestion completed -- rows/sec: {inserted_rows_per_sec:.0}. Starting sampled point lookups."
        ),
    )
    .await?;

    let point_query = benchmark_point_query(
        &redb,
        run_id,
        config.expected_records,
        config.point_query_step,
        &report,
        inserted_rows_per_sec,
    )
    .await?;

    report_progress(
        &report,
        format!("✅ Ingestion completed -- rows/sec: {inserted_rows_per_sec:.0}; ✅ Querying matched {} rows in {:?}. Closing the Database...", point_query.matched_rows, point_query.elapsed),
    )
    .await?;

    let start = Instant::now();
    redb.close().await?;
    let close_elapsed = start.elapsed();
    report_progress(
        &report,
        format!("🏁 Ingestion completed -- rows/sec: {inserted_rows_per_sec:.0}; 🏁 Querying matched {} rows in {:?}; Database closed in {close_elapsed:?}.", point_query.matched_rows, point_query.elapsed),
    )
    .await?;

    Ok(BenchmarkResult {
        inserted,
        ingest_elapsed,
        point_query,
        close_elapsed,
    })
}

pub async fn benchmark_point_query<Report, ReportError>(
    redb: &AsyncReDb,
    run_id: u128,
    expected_records: usize,
    point_query_step: usize,
    report: &Report,
    inserted_rows_per_sec: f64,
) -> Result<QueryResult>
where
    Report: AsyncFn(String) -> std::result::Result<(), ReportError> + ?Sized,
    ReportError: Display + Send + Sync + 'static,
{
    let query_started = Instant::now();
    let mut matched_rows = 0usize;
    let point_query_step = point_query_step.max(1);

    {
        let read_txn = redb
            .begin_read()
            .await
            .map_err(|err| anyhow!("Failed creating the read txt for the query: {err}"))?;

        let table = read_txn
            .inner()
            .open_table(EVENTS_TABLE)
            .map_err(|err| anyhow!("Could not open table for the query: {err}"))?;

        for i in (0..expected_records).step_by(point_query_step) {
            let key = record_key(run_id, i);
            let value = table.get(key)
                .map_err(|err| anyhow!("Error retrieving record for key {key}, derived from i={i} and run_id={run_id}: {err}"))?
                .ok_or_else(|| anyhow!("Record for key {key}, derived from i={i} and run_id={run_id} was not present"))?;
            let event = value.value();
            matched_rows += 1;
            if matched_rows.is_multiple_of(10) {
                report_progress(
                    report,
                    format!(
                        "✅ Ingestion completed -- rows/sec: {inserted_rows_per_sec:.0}; ReDB point querying: matched_rows={matched_rows}; payload={}, seq={}, score={:?}",
                        str::from_utf8(&event.payload).unwrap_or("<invalid-utf8>"),
                        event.seq,
                        event.score
                    ),
                )
                .await?;
            }
        }
    }
    let elapsed = query_started.elapsed();
    report_progress(
        report,
        format!("✅ Ingestion completed -- rows/sec: {inserted_rows_per_sec:.0}; ✅ ReDB point querying completed -- matched_rows={matched_rows}, elapsed: {elapsed:.3?}."),
    )
    .await?;

    Ok(QueryResult {
        matched_rows,
        elapsed,
    })
}
