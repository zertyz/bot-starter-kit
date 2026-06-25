use crate::db::redb::AsyncReDb;
use crate::redb_mmap_value;
use anyhow::{Result, anyhow};
use futures::{Stream, StreamExt, stream};
use redb::TableDefinition;
use sqlx::FromRow;
use std::time::Instant;

#[repr(C)]
#[derive(Debug, Clone, Copy, FromRow, bytemuck::Pod, bytemuck::Zeroable)]
struct Event {
    seq: i64,
    score: i64,
    payload: [u8; 53],
    _pad: [u8; 3], // makes the above struct honor the 8-byte alignment requirement due to i64
                   // note: no id field here, as the ID will be in the database in the form of the "key", hence we've omitted it from this "value" struct
}
redb_mmap_value!(Event);

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
    report: impl AsyncFn(String) -> Result<(), teloxide::errors::RequestError> + Send + Sync + 'static,
) -> Result<()> {
    const EVENTS_TABLE: TableDefinition<u64, Event> = TableDefinition::new("redb_wrapper_data");

    let println = |msg: String| {
        println!("{msg}");
        report(msg)
    };

    let db_path = "/tmp/telegram_redb_benchmark/events.redb";
    let expected_records = 1024 * 1024;

    let redb = AsyncReDb::open(db_path).await?;
    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| anyhow!("system clock is before Unix epoch: {err}"))?
        .as_nanos();
    let key = |i| (run_id << 32) as u64 + i as u64;

    println("STARTING ReDb BENCHMARK".to_string()).await?;
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
            (key(i), event)
        });

    let inserted = redb.ingest_stream(EVENTS_TABLE, input_stream).await?;

    let elapsed = started.elapsed();
    let rows_per_sec = inserted as f64 / elapsed.as_secs_f64();
    println(format!("✅ Ingestion completed -- rows/sec: {rows_per_sec:.0}. Now querying rows where `score` >= 99999 as a Stream -- but only showing about 1 for every million records")).await?;

    let query_started = Instant::now();
    let mut matched_rows = 0usize;

    {
        let read_txn = redb
            .begin_read()
            .await
            .map_err(|err| anyhow!("Failed creating the read txt for the query: {err}"))?;

        let table = read_txn
            .inner()
            .open_table(EVENTS_TABLE)
            .map_err(|err| anyhow!("Could not open table for the query: {err}"))?;

        // by-value search -- uses the index effectively
        for i in (0..expected_records).step_by(100000) {
            let key = key(i);
            let value = table.get(key)
                .map_err(|err| anyhow!("Error retrieving record for key {key}, derived from i={i} and run_id={run_id}: {err}"))?
                .ok_or_else(|| anyhow!("Record for key {key}, derived from i={i} and run_id={run_id} was not present"))?;
            let event = value.value();
            matched_rows += 1;
            if matched_rows.is_multiple_of(10) {
                println(format!("✅ Ingestion completed -- rows/sec: {rows_per_sec:.0}. Now Querying: matched_rows={matched_rows}; payload={}, seq={}, score={:?}",
                                str::from_utf8(&event.payload).unwrap_or("<invalid-utf8>"),
                                event.seq,
                                event.score)).await?;
            }
        }

        // by-range search on field without an index -- not effective in this `redb` model, as we are not indexing by score
        /*let mut result_stream = AsyncReDb::redb_iter_to_stream(table.range::<u64>(..))
            .map_err(|err| anyhow!("Failed creating the Stream: {err}"))?
            .try_filter(|(key, value)| future::ready(value.value().score >= 99999));

        while let Some((_key, value)) = result_stream.try_next().await.map_err(|err| anyhow!("fetch query row: {err}"))? {
            let event = value.value();
            matched_rows += 1;
            if matched_rows % 10 == 0 {
                println(format!("result row {matched_rows}: id={}, seq={}, score={:?}, payload_len={}",
                                str::from_utf8(&event.id).unwrap_or("<invalid-utf8>"),
                                event.seq,
                                event.score,
                                event.payload.len(),
                )).await?;
            }
        }*/
    }
    let query_elapsed = query_started.elapsed();

    // disabled now for being very slow
    /*println("\nCompacting...".to_string()).await?;
    let start = Instant::now();
    redb.compact().await?;
    println(format!("Done -- took {:?}", start.elapsed())).await?;*/

    println(format!("✅ Ingestion completed -- rows/sec: {rows_per_sec:.0}. ✅ Querying completed in {query_elapsed:.3?}. Closing the Database...")).await?;
    let start = Instant::now();
    redb.close().await?;
    let close_elapsed = start.elapsed();
    println(format!("🏁 Ingestion completed -- rows/sec: {rows_per_sec:.0}. 🏁 Querying completed in {query_elapsed:.3?}. 🏁 Database Closed in {close_elapsed:?}...")).await?;

    Ok(())
}
