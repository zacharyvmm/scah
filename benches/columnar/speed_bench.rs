//! `scah::parse` (arena-of-structs `Store`) against `scah::parse_columnar`
//! (`ColumnarStore`) on the same workloads.
//!
//! Per workload and store:
//! - `parse`: parse only;
//! - `parse_read`: parse, then read every field of every match;
//! - `read`: read every field of every match on an already parsed store;
//! - `traverse`: walk `get` and nested `get` lists on a parsed store.

mod workloads;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use scah::{parse, parse_columnar};
use std::hint::black_box;
use workloads::{read_columnar, read_store, traverse_columnar, traverse_store, workloads};

fn bench_columnar(c: &mut Criterion) {
    for workload in workloads() {
        let html = workload.html;
        let queries = workload.queries.as_slice();
        let mut group = c.benchmark_group(format!("columnar_store/{}", workload.name));
        group.throughput(Throughput::Bytes(html.len() as u64));
        if html.len() > 1_000_000 {
            group.sample_size(20);
        }

        let store = parse(html, queries).unwrap();
        let columnar = parse_columnar(html, queries).unwrap();
        let expected = read_store(&store, &workload);
        assert_eq!(expected, read_columnar(&columnar, &workload));

        group.bench_function(BenchmarkId::new("parse", "store"), |b| {
            b.iter(|| black_box(parse(black_box(html), black_box(queries)).unwrap()))
        });
        group.bench_function(BenchmarkId::new("parse", "columnar"), |b| {
            b.iter(|| black_box(parse_columnar(black_box(html), black_box(queries)).unwrap()))
        });

        group.bench_function(BenchmarkId::new("parse_read", "store"), |b| {
            b.iter(|| {
                let store = parse(black_box(html), black_box(queries)).unwrap();
                black_box(read_store(&store, &workload))
            })
        });
        group.bench_function(BenchmarkId::new("parse_read", "columnar"), |b| {
            b.iter(|| {
                let store = parse_columnar(black_box(html), black_box(queries)).unwrap();
                black_box(read_columnar(&store, &workload))
            })
        });

        group.bench_function(BenchmarkId::new("read", "store"), |b| {
            b.iter(|| black_box(read_store(black_box(&store), &workload)))
        });
        group.bench_function(BenchmarkId::new("read", "columnar"), |b| {
            b.iter(|| black_box(read_columnar(black_box(&columnar), &workload)))
        });

        group.bench_function(BenchmarkId::new("traverse", "store"), |b| {
            b.iter(|| black_box(traverse_store(black_box(&store), &workload)))
        });
        group.bench_function(BenchmarkId::new("traverse", "columnar"), |b| {
            b.iter(|| black_box(traverse_columnar(black_box(&columnar), &workload)))
        });
        group.finish();
    }
}

criterion_group!(benches, bench_columnar);
criterion_main!(benches);
