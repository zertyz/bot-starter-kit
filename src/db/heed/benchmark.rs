use std::time::Instant;

use ::heed::Database;
use ::heed::byteorder::BigEndian;
use ::heed::types::U64;
use anyhow::{Context, Result, anyhow};
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

pub async fn benchmark(
    report: impl AsyncFn(String) -> Result<(), teloxide::errors::RequestError> + Send + 'static,
) -> Result<()> {
    let println = |msg: String| {
        println!("{msg}");
        report(msg)
    };

    let db_path = "/tmp/telegram_heed_benchmark";
    let expected_records = 1024 * 1024;

    let heed = AsyncHeed::open(db_path).await?;
    let events: Database<EventKey, EventValue> = heed.create_database(Some("events")).await?;

    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("system clock is before Unix epoch")?
        .as_nanos();
    let key = |i| (run_id << 32) as u64 + i as u64;

    println("STARTING Heed BENCHMARK".to_string()).await?;
    println(format!(
        "starting ingestion: {expected_records} total records"
    ))
    .await?;

    let started = Instant::now();

    let mut input_stream = Box::pin(
        make_event_stream(run_id, expected_records)
            .enumerate()
            .then(|(i, event)| async move {
                if i % 100000 == 0 {
                    _ = println(format!("Inserted records: {i} / {expected_records}...")).await;
                }
                (key(i), event)
            }),
    );

    let mut write_txn = heed.begin_write().await?;
    let mut inserted = 0usize;
    while let Some((event_key, event)) = input_stream.next().await {
        events
            .put(write_txn.inner_mut(), &event_key, &event)
            .with_context(|| {
                format!("Heed: could not insert/replace item #{inserted} in benchmark")
            })?;
        inserted += 1;
    }
    write_txn.commit().await?;

    let elapsed = started.elapsed();
    let rows_per_sec = inserted as f64 / elapsed.as_secs_f64();
    println(format!("✅ Ingestion completed -- rows/sec: {rows_per_sec:.0}. Now querying borrowed mmap bytes; field display uses one explicit unaligned value copy -- showing about 1 for every million records")).await?;

    let query_started = Instant::now();
    let mut matched_rows = 0usize;

    {
        let read_txn = heed
            .begin_read()
            .await
            .context("Failed creating the read txn for the Heed query")?;

        for i in (0..expected_records).step_by(100000) {
            let record_key = key(i);
            let value = events
                .get(read_txn.inner(), &record_key)
                .with_context(|| {
                    format!(
                        "Error retrieving record for key {record_key}, derived from i={i} and run_id={run_id}"
                    )
                })?
                .ok_or_else(|| {
                    anyhow!(
                        "Record for key {record_key}, derived from i={i} and run_id={run_id} was not present"
                    )
                })?;

            let event = value.read_unaligned();
            matched_rows += 1;
            if matched_rows.is_multiple_of(10) {
                println(format!("✅ Ingestion completed -- rows/sec: {rows_per_sec:.0}. Now Querying: matched_rows={matched_rows}; payload={}, seq={}, score={:?}, bytes={}",
                                str::from_utf8(&event.payload).unwrap_or("<invalid-utf8>"),
                                event.seq,
                                event.score,
                                value.as_bytes().len())).await?;
            }
        }
    }
    let query_elapsed = query_started.elapsed();

    println(format!("✅ Ingestion completed -- rows/sec: {rows_per_sec:.0}. ✅ Querying completed in {query_elapsed:.3?}. Force-syncing the mmap...")).await?;
    let start = Instant::now();
    heed.force_sync().await?;
    let sync_elapsed = start.elapsed();
    println(format!("✅ Ingestion completed -- rows/sec: {rows_per_sec:.0}. ✅ Querying completed in {query_elapsed:.3?}. ✅ Force-sync completed in {sync_elapsed:?}. Closing the Database...")).await?;

    let start = Instant::now();
    heed.close().await?;
    let close_elapsed = start.elapsed();
    println(format!("🏁 Ingestion completed -- rows/sec: {rows_per_sec:.0}. 🏁 Querying completed in {query_elapsed:.3?}. 🏁 Force-sync completed in {sync_elapsed:?}. 🏁 Database Closed in {close_elapsed:?}...")).await?;

    Ok(())
}
