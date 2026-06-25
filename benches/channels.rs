//! Channel microbenchmarks for the project message-passing boundary.
//!
//! The Messaging gateway currently publishes incoming messages into an
//! `async_channel::Receiver`, and the messaging contract exposes that receiver
//! as a `futures::Stream`. These benchmarks start one level below the full
//! `Stream` pipeline: they isolate channel send/receive handoff cost using the
//! non-blocking APIs available across the channel implementations already in
//! this dependency graph.
//!
//! What this measures:
//! - same-thread latency: one send followed by one receive on the same thread;
//! - same-thread throughput: fill the bounded channel, then drain it;
//! - inter-thread latency: a producer thread is signaled to send one message,
//!   while the benchmark thread receives that message;
//! - inter-thread throughput: a producer thread keeps the bounded channel hot
//!   while the benchmark thread drains fixed-size receive batches.
//!
//! What this intentionally does not measure yet:
//! - `StreamExt::next()` polling overhead;
//! - project-specific mapping such as `TelegramGateway::get_mo_stream()`;
//! - downstream MT stream consumption with `for_each_concurrent`.
//!
//! Those should be added as separate end-to-end Stream benchmarks when we want
//! to compare candidate channel crates against the actual MO/MT pipeline.
//! `Std sync_channel (baseline)` is a familiar standard-library comparison
//! point, not a theoretical lower bound; faster third-party channels are
//! entirely possible.
//! Crates worth trying next are `kanal` for sync/async channels, `flume` for an
//! MPMC async-capable channel, `thingbuf` for bounded MPSC channels with fewer
//! steady-state allocations, and `crossbeam-channel` as a strong synchronous
//! channel baseline.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering::Relaxed},
    },
    thread,
};

use criterion::{
    BenchmarkGroup, Criterion, Throughput, criterion_group, criterion_main, measurement::WallTime,
};

#[derive(Debug)]
struct Message {
    _data: [u8; 2048],
}

impl Default for Message {
    fn default() -> Self {
        Self { _data: [0; 2048] }
    }
}

const BUFFER_SIZE: usize = 1 << 10;
const THROUGHPUT_RECEIVE_BATCH: usize = BUFFER_SIZE >> 5;
const ASYNC_CHANNEL_BOUNDED: &str = "async-channel bounded";
const TOKIO_MPSC: &str = "Tokio MPSC channel";
const FUTURES_MPSC: &str = "Futures MPSC channel";
const STD_SYNC_CHANNEL_BASELINE: &str = "Std sync_channel (baseline)";
const THREAD_SIGNAL_BASELINE: &str = "Thread signal only (baseline)";

/// Repeatedly tries a non-blocking send or receive until it succeeds.
///
/// The busy loop keeps these benchmarks focused on raw channel availability and
/// avoids measuring async runtime scheduling. That makes the numbers useful for
/// relative channel comparisons, but not for estimating idle application CPU.
fn spin_until_success(mut operation: impl FnMut() -> bool) {
    while !operation() {
        std::hint::spin_loop();
    }
}

/// Briefly yields to the CPU when a saturated producer hits a full channel.
///
/// The throughput benchmarks keep the sender hot, so an occasional full channel
/// is expected. A tiny pause avoids turning that path into a pure contention
/// loop while still preserving pressure on the receiver side.
fn pause_after_full_channel() {
    std::hint::spin_loop();
    std::hint::spin_loop();
    std::hint::spin_loop();
}

/// Measures the minimum cost of a channel handoff with no cross-thread wakeup.
///
/// Each iteration sends one message and immediately receives it on the same
/// thread. This is closest to the raw cost below the project's MO `Stream`
/// wrapper: it does not include `Stream` polling or message mapping.
fn bench_same_thread_latency(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("Same-thread LATENCY");

    let (async_sender, async_receiver) = async_channel::bounded::<Message>(BUFFER_SIZE);
    let (tokio_sender, mut tokio_receiver) = tokio::sync::mpsc::channel::<Message>(BUFFER_SIZE);
    let (mut futures_sender, mut futures_receiver) =
        futures::channel::mpsc::channel::<Message>(BUFFER_SIZE);
    let (std_sender, std_receiver) = std::sync::mpsc::sync_channel::<Message>(BUFFER_SIZE);

    group.bench_function(ASYNC_CHANNEL_BOUNDED, |bencher| {
        bencher.iter(|| {
            spin_until_success(|| async_sender.try_send(Message::default()).is_ok());
            spin_until_success(|| async_receiver.try_recv().is_ok());
        })
    });

    group.bench_function(TOKIO_MPSC, |bencher| {
        bencher.iter(|| {
            spin_until_success(|| tokio_sender.try_send(Message::default()).is_ok());
            spin_until_success(|| tokio_receiver.try_recv().is_ok());
        })
    });

    group.bench_function(FUTURES_MPSC, |bencher| {
        bencher.iter(|| {
            spin_until_success(|| futures_sender.try_send(Message::default()).is_ok());
            spin_until_success(|| futures_receiver.try_recv().is_ok());
        })
    });

    group.bench_function(STD_SYNC_CHANNEL_BASELINE, |bencher| {
        bencher.iter(|| {
            spin_until_success(|| std_sender.try_send(Message::default()).is_ok());
            spin_until_success(|| std_receiver.try_recv().is_ok());
        })
    });

    group.finish();
}

/// Measures bulk channel throughput without cross-thread synchronization.
///
/// Each iteration fills the bounded buffer and then drains it. The reported
/// throughput is the number of messages drained per iteration. This favors
/// queue operations and buffer management rather than producer/consumer wakeup
/// behavior.
fn bench_same_thread_throughput(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("Same-thread THROUGHPUT");
    group.throughput(Throughput::Elements(BUFFER_SIZE as u64));

    let (async_sender, async_receiver) = async_channel::bounded::<Message>(BUFFER_SIZE);
    let (tokio_sender, mut tokio_receiver) = tokio::sync::mpsc::channel::<Message>(BUFFER_SIZE);
    let (mut futures_sender, mut futures_receiver) =
        futures::channel::mpsc::channel::<Message>(BUFFER_SIZE);
    let (std_sender, std_receiver) = std::sync::mpsc::sync_channel::<Message>(BUFFER_SIZE);

    group.bench_function(ASYNC_CHANNEL_BOUNDED, |bencher| {
        bencher.iter(|| {
            for _ in 0..BUFFER_SIZE {
                spin_until_success(|| async_sender.try_send(Message::default()).is_ok());
            }
            for _ in 0..BUFFER_SIZE {
                spin_until_success(|| async_receiver.try_recv().is_ok());
            }
        })
    });

    group.bench_function(TOKIO_MPSC, |bencher| {
        bencher.iter(|| {
            for _ in 0..BUFFER_SIZE {
                spin_until_success(|| tokio_sender.try_send(Message::default()).is_ok());
            }
            for _ in 0..BUFFER_SIZE {
                spin_until_success(|| tokio_receiver.try_recv().is_ok());
            }
        })
    });

    group.bench_function(FUTURES_MPSC, |bencher| {
        bencher.iter(|| {
            for _ in 0..BUFFER_SIZE {
                spin_until_success(|| futures_sender.try_send(Message::default()).is_ok());
            }
            for _ in 0..BUFFER_SIZE {
                spin_until_success(|| futures_receiver.try_recv().is_ok());
            }
        })
    });

    group.bench_function(STD_SYNC_CHANNEL_BASELINE, |bencher| {
        bencher.iter(|| {
            for _ in 0..BUFFER_SIZE {
                spin_until_success(|| std_sender.try_send(Message::default()).is_ok());
            }
            for _ in 0..BUFFER_SIZE {
                spin_until_success(|| std_receiver.try_recv().is_ok());
            }
        })
    });

    group.finish();
}

/// Measures the atomic signaling overhead used by inter-thread latency cases.
///
/// This baseline is not expected to be faster than every channel. It is a
/// measurement control: inter-thread channel latency includes both this signal
/// cost and the channel handoff cost.
fn bench_inter_thread_baseline(group: &mut BenchmarkGroup<WallTime>) {
    thread::scope(|scope| {
        let keep_running = Arc::new(AtomicBool::new(true));
        let keep_running_ref = Arc::clone(&keep_running);
        let counter = Arc::new(AtomicU64::new(0));
        let counter_ref = Arc::clone(&counter);

        scope.spawn(move || {
            while keep_running.load(Relaxed) {
                counter.fetch_add(1, Relaxed);
            }
        });

        group.bench_function(THREAD_SIGNAL_BASELINE, |bencher| {
            bencher.iter(|| {
                let last_count = counter_ref.load(Relaxed);
                loop {
                    let current_count = counter_ref.load(Relaxed);
                    if current_count != last_count {
                        break;
                    }
                    std::hint::spin_loop();
                }
            })
        });

        keep_running_ref.store(false, Relaxed);
    });
}

/// Runs one inter-thread latency case.
///
/// The benchmark thread flips an atomic flag, the producer thread notices that
/// flag and sends exactly one message, and the benchmark thread receives it.
/// The result includes cross-thread signaling overhead; compare each channel to
/// `THREAD_SIGNAL_BASELINE` to estimate the extra cost of the channel itself.
fn bench_inter_thread_latency_case(
    group: &mut BenchmarkGroup<WallTime>,
    bench_id: &'static str,
    mut send_fn: impl FnMut() + Send,
    mut receive_fn: impl FnMut(),
) {
    thread::scope(|scope| {
        let keep_running = Arc::new(AtomicBool::new(true));
        let keep_running_ref = Arc::clone(&keep_running);
        let send = Arc::new(AtomicBool::new(false));
        let send_ref = Arc::clone(&send);

        scope.spawn(move || {
            while keep_running.load(Relaxed) {
                while !send.swap(false, Relaxed) {
                    if !keep_running.load(Relaxed) {
                        return;
                    }
                    std::hint::spin_loop();
                }
                send_fn();
            }
        });

        group.bench_function(bench_id, |bencher| {
            bencher.iter(|| {
                send_ref.store(true, Relaxed);
                receive_fn();
            })
        });

        keep_running_ref.store(false, Relaxed);
        send_ref.store(true, Relaxed);
    });
}

/// Measures one-message handoff latency between two OS threads.
///
/// This is closer than the same-thread group to the real gateway shape, where
/// a producer task publishes messages and downstream logic consumes them later.
/// It still measures the raw channel boundary, not the full `Stream` pipeline.
fn bench_inter_thread_latency(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("Inter-thread LATENCY");

    let (async_sender, async_receiver) = async_channel::bounded::<Message>(BUFFER_SIZE);
    let (tokio_sender, mut tokio_receiver) = tokio::sync::mpsc::channel::<Message>(BUFFER_SIZE);
    let (mut futures_sender, mut futures_receiver) =
        futures::channel::mpsc::channel::<Message>(BUFFER_SIZE);
    let (std_sender, std_receiver) = std::sync::mpsc::sync_channel::<Message>(BUFFER_SIZE);

    bench_inter_thread_baseline(&mut group);
    bench_inter_thread_latency_case(
        &mut group,
        ASYNC_CHANNEL_BOUNDED,
        || spin_until_success(|| async_sender.try_send(Message::default()).is_ok()),
        || spin_until_success(|| async_receiver.try_recv().is_ok()),
    );
    bench_inter_thread_latency_case(
        &mut group,
        TOKIO_MPSC,
        || spin_until_success(|| tokio_sender.try_send(Message::default()).is_ok()),
        || spin_until_success(|| tokio_receiver.try_recv().is_ok()),
    );
    bench_inter_thread_latency_case(
        &mut group,
        FUTURES_MPSC,
        || spin_until_success(|| futures_sender.try_send(Message::default()).is_ok()),
        || spin_until_success(|| futures_receiver.try_recv().is_ok()),
    );
    bench_inter_thread_latency_case(
        &mut group,
        STD_SYNC_CHANNEL_BASELINE,
        || spin_until_success(|| std_sender.try_send(Message::default()).is_ok()),
        || spin_until_success(|| std_receiver.try_recv().is_ok()),
    );

    group.finish();
}

/// Runs one saturated producer/consumer throughput case.
///
/// The producer thread repeatedly attempts to fill the bounded buffer while the
/// benchmark thread drains a fixed receive batch. This approximates sustained
/// publishing into a channel-backed stream under load, without including the
/// cost of `StreamExt::next()` or project message mapping.
fn bench_inter_thread_throughput_case(
    group: &mut BenchmarkGroup<WallTime>,
    bench_id: &'static str,
    mut send_batch_fn: impl FnMut() + Send,
    mut receive_batch_fn: impl FnMut(),
) {
    thread::scope(|scope| {
        let keep_running = Arc::new(AtomicBool::new(true));
        let keep_running_ref = Arc::clone(&keep_running);

        scope.spawn(move || {
            while keep_running.load(Relaxed) {
                send_batch_fn();
            }
        });

        group.bench_function(bench_id, |bencher| {
            bencher.iter(|| {
                receive_batch_fn();
            })
        });

        keep_running_ref.store(false, Relaxed);
    });
}

/// Measures sustained cross-thread channel throughput.
///
/// This is the most relevant raw benchmark for the project's "publish into a
/// channel, consume from a Stream" concern, because it keeps producer pressure
/// active while the consumer drains messages. Add a separate Stream benchmark
/// before choosing a replacement channel solely from these numbers.
fn bench_inter_thread_throughput(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("Inter-thread THROUGHPUT");
    group.throughput(Throughput::Elements(THROUGHPUT_RECEIVE_BATCH as u64));

    let (async_sender, async_receiver) = async_channel::bounded::<Message>(BUFFER_SIZE);
    let (tokio_sender, mut tokio_receiver) = tokio::sync::mpsc::channel::<Message>(BUFFER_SIZE);
    let (mut futures_sender, mut futures_receiver) =
        futures::channel::mpsc::channel::<Message>(BUFFER_SIZE);
    let (std_sender, std_receiver) = std::sync::mpsc::sync_channel::<Message>(BUFFER_SIZE);

    bench_inter_thread_throughput_case(
        &mut group,
        ASYNC_CHANNEL_BOUNDED,
        || {
            for _ in 0..BUFFER_SIZE {
                if async_sender.try_send(Message::default()).is_err() {
                    pause_after_full_channel();
                }
            }
        },
        || {
            for _ in 0..THROUGHPUT_RECEIVE_BATCH {
                spin_until_success(|| async_receiver.try_recv().is_ok());
            }
        },
    );
    bench_inter_thread_throughput_case(
        &mut group,
        TOKIO_MPSC,
        || {
            for _ in 0..BUFFER_SIZE {
                if tokio_sender.try_send(Message::default()).is_err() {
                    pause_after_full_channel();
                }
            }
        },
        || {
            for _ in 0..THROUGHPUT_RECEIVE_BATCH {
                spin_until_success(|| tokio_receiver.try_recv().is_ok());
            }
        },
    );
    bench_inter_thread_throughput_case(
        &mut group,
        FUTURES_MPSC,
        || {
            for _ in 0..BUFFER_SIZE {
                if futures_sender.try_send(Message::default()).is_err() {
                    pause_after_full_channel();
                }
            }
        },
        || {
            for _ in 0..THROUGHPUT_RECEIVE_BATCH {
                spin_until_success(|| futures_receiver.try_recv().is_ok());
            }
        },
    );
    bench_inter_thread_throughput_case(
        &mut group,
        STD_SYNC_CHANNEL_BASELINE,
        || {
            for _ in 0..BUFFER_SIZE {
                if std_sender.try_send(Message::default()).is_err() {
                    pause_after_full_channel();
                }
            }
        },
        || {
            for _ in 0..THROUGHPUT_RECEIVE_BATCH {
                spin_until_success(|| std_receiver.try_recv().is_ok());
            }
        },
    );

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(20);
    targets =
        bench_same_thread_latency,
        bench_same_thread_throughput,
        bench_inter_thread_latency,
        bench_inter_thread_throughput
}
criterion_main!(benches);
