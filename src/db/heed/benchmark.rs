use std::{
    fmt::Display,
    path::PathBuf,
    time::{Duration, Instant},
};

use ::heed::Database;
use ::heed::byteorder::BigEndian;
use ::heed::types::U64;
use anyhow::{Result, anyhow};
use futures::{Stream, StreamExt, stream};

use crate::db::heed::{AsyncHeed, HeedPod};

type EventKey = U64<BigEndian>;
type EventValue = HeedPod<Event>;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Event {
    seq: i64,
    score: i64,
    payload: [u8; 53],
    _pad: [u8; 3],
}

#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub db_path: PathBuf,
    pub expected_records: u64,
    pub point_query_step: u64,
    pub progress_every: Option<u64>,
    pub force_sync_after_queries: bool,
}

#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub inserted: u64,
    pub ingest_elapsed: Duration,
    pub point_query: QueryResult,
    pub sync_elapsed: Option<Duration>,
    pub close_elapsed: Duration,
}

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub matched_rows: u64,
    pub elapsed: Duration,
}

fn to_byte_array<const N: usize>(text: String) -> [u8; N] {
    let mut byte_array = [0u8; N];
    byte_array.copy_from_slice(text.as_bytes());
    byte_array
}

fn make_event_stream(run_id: u128, records_per_task: u64) -> impl Stream<Item = Event> {
    stream::unfold(0, move |seq| async move {
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

fn record_key(run_id: u128, seq: u64) -> u64 {
    ((run_id << 32) as u64).wrapping_add(seq)
}

async fn report_progress<Report, ReportError>(report: &Report, msg: String) -> Result<()>
where
    Report: AsyncFn(String) -> std::result::Result<(), ReportError> + ?Sized,
    ReportError: Display + Send + Sync + 'static,
{
    log::info!("{msg}");
    report(msg)
        .await
        .map_err(|err| anyhow!("{err}"))
}

pub async fn benchmark(report: impl AsyncFn(String) -> Result<(), teloxide::errors::RequestError> + Send + Sync + 'static) -> Result<()> {
    run_benchmark(
        BenchmarkConfig {
            db_path: PathBuf::from("/tmp/telegram_heed_benchmark"),
            expected_records: 64 * 1024 * 1024,
            point_query_step: 100_000,
            progress_every: Some(100_000),
            force_sync_after_queries: true,
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
    let heed = AsyncHeed::open(&config.db_path).await?;
    let events: Database<EventKey, EventValue> = heed
        .create_database(Some("events"))
        .await?;

    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| anyhow!("system clock is before Unix epoch: {err}"))?
        .as_nanos();

    report_progress(&report, "STARTING Heed BENCHMARK".to_string()).await?;
    report_progress(&report, format!("starting ingestion: {} total records", config.expected_records)).await?;

    let started = Instant::now();
    let expected_records = config.expected_records;
    let progress_every = config.progress_every;

    let input_stream = make_event_stream(run_id, expected_records)
        .enumerate()
        .then(|(i, event)| {
            let report = &report;
            async move {
                if progress_every.is_some_and(|progress_every| progress_every > 0 && i.is_multiple_of(progress_every as usize)) {
                    _ = report_progress(report, format!("Inserted records: {i} / {expected_records}...")).await;
                }
                (record_key(run_id, i as u64), event)
            }
        });
    let inserted = heed
        .ingest_stream(&events, input_stream)
        .await?;

    let ingest_elapsed = started.elapsed();
    let inserted_rows_per_sec = inserted as f64 / ingest_elapsed.as_secs_f64();
    report_progress(&report, format!("✅ Ingestion completed -- rows/sec: {inserted_rows_per_sec:.0}. Starting sampled borrowed mmap point lookups.")).await?;

    let point_query = benchmark_point_query(&heed, &events, run_id, config.expected_records, config.point_query_step, &report, inserted_rows_per_sec).await?;

    report_progress(
        &report,
        format!(
            "✅ Ingestion completed -- rows/sec: {inserted_rows_per_sec:.0}; ✅ Querying matched {} rows in {:?}. Closing the Database...",
            point_query.matched_rows, point_query.elapsed
        ),
    )
    .await?;

    let sync_elapsed = if config.force_sync_after_queries {
        report_progress(&report, "Force-syncing the mmap...".to_string()).await?;
        let start = Instant::now();
        heed.force_sync()
            .await?;
        let sync_elapsed = start.elapsed();
        report_progress(
            &report,
            format!(
                "✅ Ingestion completed -- rows/sec: {inserted_rows_per_sec:.0}; ✅ Querying matched {} rows in {:?}. ✅ Force-sync completed in {sync_elapsed:?}. Closing the Database...",
                point_query.matched_rows, point_query.elapsed
            ),
        )
        .await?;
        Some(sync_elapsed)
    } else {
        None
    };

    let start = Instant::now();
    heed.close()
        .await?;
    let close_elapsed = start.elapsed();
    report_progress(
        &report,
        format!(
            "🏁 Ingestion completed -- rows/sec: {inserted_rows_per_sec:.0}; 🏁 Querying matched {} rows in {:?}; Database closed in {close_elapsed:?}.",
            point_query.matched_rows, point_query.elapsed
        ),
    )
    .await?;

    Ok(BenchmarkResult { inserted, ingest_elapsed, point_query, sync_elapsed, close_elapsed })
}

async fn benchmark_point_query<Report, ReportError>(
    heed: &AsyncHeed,
    events: &Database<EventKey, EventValue>,
    run_id: u128,
    expected_records: u64,
    point_query_step: u64,
    report: &Report,
    inserted_rows_per_sec: f64,
) -> Result<QueryResult>
where
    Report: AsyncFn(String) -> std::result::Result<(), ReportError> + ?Sized,
    ReportError: Display + Send + Sync + 'static,
{
    let query_started = Instant::now();
    let mut matched_rows: u64 = 0;
    let point_query_step = point_query_step.max(1);

    {
        let read_txn = heed
            .begin_read()
            .await
            .map_err(|err| anyhow!("Failed creating the read txn for the Heed query: {err}"))?;

        for i in (0..expected_records).step_by(point_query_step as usize) {
            let record_key = record_key(run_id, i);
            let value = events
                .get(read_txn.inner(), &record_key)
                .map_err(|err| anyhow!("Error retrieving record for key {record_key}, derived from i={i} and run_id={run_id}: {err}"))?
                .ok_or_else(|| anyhow!("Record for key {record_key}, derived from i={i} and run_id={run_id} was not present"))?;

            let event = unsafe { value.as_aligned_unchecked() };
            matched_rows += 1;
            if matched_rows.is_multiple_of(1000) {
                report_progress(
                    report,
                    format!(
                        "✅ Ingestion completed -- rows/sec: {inserted_rows_per_sec:.0}. Heed point querying: matched_rows={matched_rows}; payload={}, seq={}, score={:?}, bytes={}",
                        str::from_utf8(&event.payload).unwrap_or("<invalid-utf8>"),
                        event.seq,
                        event.score,
                        value
                            .as_bytes()
                            .len()
                    ),
                )
                .await?;
            }
        }
    }
    let elapsed = query_started.elapsed();
    report_progress(report, format!("✅ Heed point querying completed -- matched_rows={matched_rows}, elapsed: {elapsed:.3?}.")).await?;

    Ok(QueryResult { matched_rows, elapsed })
}
