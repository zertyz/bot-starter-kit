//! Criterion database benchmarks.
//!
//! These share code with the UI-triggered benchmark demos.
//! Here, each iteration creates a fresh database under the OS temporary directory,
//! inserts a fixed number of records, commits the write transaction through the
//! same stream ingestion API used by the UI benchmarks, and performs
//! sampled point lookups over that same record count.
//!
//! Since the UI benchmarks are intended to provide the user elapsed times
//! for a specific database, there we may exercise a different set of features,
//! while here we are leveling all databases to the same exact operations
//! -- for instance: range queries are performed only in SQLite's UI benchmarks;
//! here we implement "point queries" / "single record fetch queries" for everyone.

use std::time::Duration;

use bot_starter_kit::db::{heed, redb, sqlite};
use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};

const RECORDS_PER_ITERATION: u64 = 16 * 1024;

async fn silent_report(_msg: String) -> anyhow::Result<()> {
    Ok(())
}

fn expected_sample_count(records: u64, step: u64) -> u64 {
    if records == 0 { 0 } else { ((records - 1) / step.max(1)) + 1 }
}

fn bench_database_ingest_and_point_query(criterion: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("create Tokio runtime for database benchmarks");

    let mut group = criterion.benchmark_group("Database ingest + sampled point query");
    group.throughput(Throughput::Elements(RECORDS_PER_ITERATION));

    group.bench_function("SQLite fresh DB", |bencher| {
        bencher.iter(|| {
            runtime.block_on(async {
                let temp_dir = tempfile::tempdir().expect("create SQLite benchmark temp dir");
                let config = sqlite::benchmark::BenchmarkConfig {
                    db_path: temp_dir
                        .path()
                        .join("events.db"),
                    expected_records: RECORDS_PER_ITERATION,
                    progress_every: None,
                    benchmark_point_query: true,
                    benchmark_range_query: false,
                    run_wal_maintenance: false,
                };
                let result = sqlite::benchmark::run_benchmark(config, silent_report)
                    .await
                    .expect("run SQLite benchmark");

                assert_eq!(result.inserted, RECORDS_PER_ITERATION);
                assert!(
                    result
                        .point_query
                        .is_some(),
                    "SQLite point query is enabled for Criterion"
                );
                assert!(
                    result
                        .range_query
                        .is_none(),
                    "SQLite ranged score query should stay disabled for Criterion"
                );
                black_box(result);
            })
        })
    });

    group.bench_function("ReDB fresh DB", |bencher| {
        bencher.iter(|| {
            runtime.block_on(async {
                let temp_dir = tempfile::tempdir().expect("create ReDB benchmark temp dir");
                let config = redb::benchmark::BenchmarkConfig {
                    db_path: temp_dir
                        .path()
                        .join("events.redb"),
                    expected_records: RECORDS_PER_ITERATION,
                    point_query_step: 1024,
                    progress_every: None,
                };
                let expected_queries = expected_sample_count(config.expected_records, config.point_query_step);

                let result = redb::benchmark::run_benchmark(config, silent_report)
                    .await
                    .expect("run ReDB benchmark");

                assert_eq!(result.inserted, RECORDS_PER_ITERATION);
                assert_eq!(
                    result
                        .point_query
                        .matched_rows,
                    expected_queries
                );
                black_box(result);
            })
        })
    });

    group.bench_function("Heed fresh DB", |bencher| {
        bencher.iter(|| {
            runtime.block_on(async {
                let temp_dir = tempfile::tempdir().expect("create Heed benchmark temp dir");
                let config = heed::benchmark::BenchmarkConfig {
                    db_path: temp_dir
                        .path()
                        .join("heed-env"),
                    expected_records: RECORDS_PER_ITERATION,
                    point_query_step: 1024,
                    progress_every: None,
                    force_sync_after_queries: false,
                };
                let expected_queries = expected_sample_count(config.expected_records, config.point_query_step);

                let result = heed::benchmark::run_benchmark(config, silent_report)
                    .await
                    .expect("run Heed benchmark");

                assert_eq!(result.inserted, RECORDS_PER_ITERATION);
                assert_eq!(
                    result
                        .point_query
                        .matched_rows,
                    expected_queries
                );
                black_box(result);
            })
        })
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(6));
    targets = bench_database_ingest_and_point_query
}
criterion_main!(benches);
