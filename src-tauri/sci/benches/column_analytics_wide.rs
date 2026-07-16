use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use polars::prelude::{Column, DataFrame, NamedFrom, Series};
use yss_sci::database::{compute_all_column_distributions, compute_all_column_stats};

fn mixed_dataframe(rows: usize, columns: usize) -> DataFrame {
    let mut data = Vec::with_capacity(columns);

    for column in 0..columns {
        let name = format!("column_{column}");
        if column % 2 == 0 {
            let values: Vec<f64> = (0..rows)
                .map(|row| ((row * (column + 1)) % 10_000) as f64 / 10.0)
                .collect();
            data.push(Column::from(Series::new(name.into(), values)));
        } else {
            let values: Vec<String> = (0..rows)
                .map(|row| format!("category_{:02}", (row + column) % 20))
                .collect();
            data.push(Column::from(Series::new(name.into(), values)));
        }
    }

    DataFrame::new(rows, data).expect("benchmark dataframe must be valid")
}

fn high_cardinality_string_dataframe(rows: usize, columns: usize) -> DataFrame {
    let data = (0..columns)
        .map(|column| {
            let name = format!("string_{column}");
            let values: Vec<String> = (0..rows)
                .map(|row| format!("value_{column}_{row}"))
                .collect();
            Column::from(Series::new(name.into(), values))
        })
        .collect();

    DataFrame::new(rows, data).expect("benchmark dataframe must be valid")
}

fn benchmark_wide_column_analytics(c: &mut Criterion) {
    let mixed_shapes = [(10_000, 32), (10_000, 128), (100_000, 32)];
    let mut stats = c.benchmark_group("all_column_stats_wide_mixed");
    stats.sample_size(10);
    for (rows, columns) in mixed_shapes {
        let dataframe = mixed_dataframe(rows, columns);
        stats.throughput(Throughput::Elements((rows * columns) as u64));
        stats.bench_with_input(
            BenchmarkId::from_parameter(format!("{rows}x{columns}")),
            &dataframe,
            |bench, dataframe| {
                bench.iter(|| black_box(compute_all_column_stats(black_box(dataframe))))
            },
        );
    }
    stats.finish();

    let mixed_shapes = [(10_000, 32), (10_000, 128), (100_000, 32)];
    let mut distributions = c.benchmark_group("all_column_distributions_wide_mixed");
    distributions.sample_size(10);
    for (rows, columns) in mixed_shapes {
        let dataframe = mixed_dataframe(rows, columns);
        distributions.throughput(Throughput::Elements((rows * columns) as u64));
        distributions.bench_with_input(
            BenchmarkId::from_parameter(format!("{rows}x{columns}")),
            &dataframe,
            |bench, dataframe| {
                bench.iter(|| black_box(compute_all_column_distributions(black_box(dataframe))))
            },
        );
    }
    distributions.finish();

    let high_cardinality_shapes = [(10_000, 32), (10_000, 128)];
    let mut distributions = c.benchmark_group("all_column_distributions_high_cardinality");
    distributions.sample_size(10);
    for (rows, columns) in high_cardinality_shapes {
        let dataframe = high_cardinality_string_dataframe(rows, columns);
        distributions.throughput(Throughput::Elements((rows * columns) as u64));
        distributions.bench_with_input(
            BenchmarkId::from_parameter(format!("{rows}x{columns}")),
            &dataframe,
            |bench, dataframe| {
                bench.iter(|| black_box(compute_all_column_distributions(black_box(dataframe))))
            },
        );
    }
    distributions.finish();
}

criterion_group!(benches, benchmark_wide_column_analytics);
criterion_main!(benches);
