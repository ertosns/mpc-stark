//! A paired down version of the `gate_throughput` benchmarks that allows for tracing without
//! the overhead of criterion polluting stack samples

use clap::Parser;
use cpuprofiler::PROFILER;
use gperftools::HEAP_PROFILER;
use mpc_stark::{
    algebra::scalar::Scalar, beaver::DummySharedScalarSource, network::NoRecvNetwork, MpcFabric,
};
use rand::thread_rng;

// -----------
// | Helpers |
// -----------

/// The number of gates to use in the benchmark
const NUM_GATES: usize = 10_000_000;

/// Create a mock fabric for testing
pub fn mock_fabric(size_hint: usize) -> MpcFabric {
    let network = NoRecvNetwork::default();
    let beaver_source = DummySharedScalarSource::new();
    MpcFabric::new_with_size_hint(size_hint, network, beaver_source)
}

pub fn start_cpu_profiler(profiled: bool) {
    if profiled {
        PROFILER.lock().unwrap().start("./cpu.profile").unwrap();
    }
}

pub fn stop_cpu_profiler(profiled: bool) {
    if profiled {
        PROFILER.lock().unwrap().stop().unwrap();
    }
}

pub fn start_heap_profiler(profiled: bool) {
    if profiled {
        HEAP_PROFILER
            .lock()
            .unwrap()
            .start("./heap.profile")
            .unwrap();
    }
}

pub fn stop_heap_profiler(profiled: bool) {
    if profiled {
        HEAP_PROFILER.lock().unwrap().stop().unwrap();
    }
}

// --------------------
// | CLI + Benchmarks |
// --------------------

/// The command line interface for the test harness
#[derive(Clone, Parser, Debug)]
struct Args {
    /// Whether to enable on-cpu stack sampled profiling
    #[clap(long, takes_value = false, value_parser)]
    cpu_profiled: bool,
    /// Whether to enable heap profiling
    #[clap(long, takes_value = false, value_parser)]
    heap_profiled: bool,
    /// The bench argument, needed for all benchmarks
    #[clap(long, takes_value = true, value_parser)]
    bench: bool,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 3)]
async fn main() {
    // Parse args
    let args = Args::parse();
    start_cpu_profiler(args.cpu_profiled);
    start_heap_profiler(args.heap_profiled);

    // Setup benchmark
    let fabric = mock_fabric(NUM_GATES * 2);
    let mut rng = thread_rng();
    let base = Scalar::random(&mut rng);
    let base_res = fabric.allocate_scalar(base);

    let mut res = base_res;
    for _ in 0..NUM_GATES {
        res = &res + &res;
    }

    println!("finished constructing circuit");
    let _res = res.await;
    println!("finished awaiting result");

    fabric.shutdown();
    stop_cpu_profiler(args.cpu_profiled);
    stop_heap_profiler(args.heap_profiled);
}
