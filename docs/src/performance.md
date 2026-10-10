# Performance

Measure the path you are changing, using the same workload and environment before and after the change. Separate build time, backend execution, data preparation and desktop rendering. None of these measurements alone represents end-to-end application latency.

## Desktop rendering

Use an optimized build for frame-rate measurements:

```sh
cargo run --release
```

Keep the graph size, window dimensions, display scaling, refresh rate and interaction sequence fixed. Record whether the bottleneck occurs during panning, zooming, dragging, connecting nodes or displaying results. Honor reduced-motion settings when comparing animation behavior.

A static preview does not prove sustained frame rate or redraw correctness. Physical input, multiple windows and target-platform behavior still require their own acceptance. See the [desktop module](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-desktop-gpui/README.md) for the current scenarios.

## Dataset storage and queries

The `dataset_engine_bench` example measures synthetic dataset generation, reading, edits, compaction and type conversion. Run each full repetition in a new temporary folder because the measurement changes the dataset:

```sh
benchmark_dir="$(mktemp -d)"
cargo run -p yss-database-store --release --example dataset_engine_bench -- "$benchmark_dir" generate
cargo run -p yss-database-store --release --example dataset_engine_bench -- "$benchmark_dir" measure
```

The example emits per-scenario JSON. Keep the workload, result correctness checks, build profile and process boundary with any measurements you share. Do not label the first query as a cold-storage read unless you controlled the operating-system cache.

Source: [dataset_engine_bench.rs](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-database-store/examples/dataset_engine_bench.rs).

## Scientific computation

The standalone OLS example measures a synthetic 100,000-row computation:

```sh
cargo run -p yss-graph-execution --release --example ols_bench
```

Dataset preparation and numerical computation are separate measurements. Results collected from an older combined process do not describe this standalone command's memory or timing.

Source: [ols_bench.rs](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-graph-execution/examples/ols_bench.rs).

## Graph resolution

Run the ignored manual timing case explicitly:

```sh
cargo test -p yss-graph-runtime --release --lib benchmark_repeated_resolution_and_projection -- --ignored --nocapture
```

This measures repeated resolution and projection, not a complete Project commit, native input path or rendered frame. Use [Graph Runtime](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-graph-runtime/README.md) for its current contract.

## Assistant workflows

The [Harness measurement example](https://github.com/zhouyi207/YssBI/blob/main/crates/yss-application/examples/measure_harness/README.md) exercises the real Core, Rig, Application and SQLite path with configured models. Follow its configuration and project-isolation instructions before running it. Real provider calls can send data externally and incur charges.

Do not infer model quality, replay correctness or desktop acceptance from an isolated timing run.

## Record a useful comparison

Record the commit, operating system, CPU, memory, GPU when relevant, build profile, input size and exact command. Separate warm-cache runs from controlled cold-cache runs and report background load and measurement variation. Sampled memory peaks can miss short-lived allocations; a query memory budget is not a process-memory limit.

Keep measurements with the issue or change they support. The book documents how to measure; it does not keep historical output dumps or turn old local numbers into current guarantees.

## See also

- [Testing](./development/testing.md)
- [Architecture](./development/architecture.md)
- [Graphs & Results](./graphs-and-results.md)
