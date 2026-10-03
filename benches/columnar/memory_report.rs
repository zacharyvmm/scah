//! Heap bytes per matched element for `Store` and `ColumnarStore`.
//!
//! Run with `cargo bench -p scah-benches --bench columnar_memory_report`.
//! Figures are allocated capacity (`capacity * size_of`), as reported by
//! `heap_usage`, divided by the number of matched elements. Both stores
//! reserve the same number of element slots from the HTML length, so the
//! "per slot" column isolates the row layout from that reservation policy.
//! The columnar store swaps its reserved link column for an exactly sized
//! result-list id array when parsing ends, so its "per slot" figure counts
//! 4 bytes per match rather than per slot for that column.

#[allow(dead_code)]
mod workloads;

use scah::{HeapUsage, parse, parse_columnar};

fn per(bytes: usize, count: usize) -> f64 {
    bytes as f64 / count.max(1) as f64
}

fn row(store: &str, usage: HeapUsage, matches: usize, slots: usize) {
    println!(
        "| {store:<13} | {:>10.1} | {:>9.1} | {:>8.1} | {:>8.1} | {:>8.1} | {:>9.1} |",
        per(usage.elements, slots),
        per(usage.elements, matches),
        per(usage.query_nodes, matches),
        per(usage.attributes, matches),
        per(usage.text_tapes, matches),
        per(usage.total(), matches),
    );
}

fn main() {
    // Ignore criterion's CLI arguments (`--bench`).
    for workload in workloads::workloads() {
        let queries = workload.queries.as_slice();
        let store = parse(workload.html, queries).unwrap();
        let columnar = parse_columnar(workload.html, queries).unwrap();
        let matches = store.elements.len();
        assert_eq!(matches, columnar.len());

        println!(
            "\n{} ({} matches, {} element slots)",
            workload.name,
            matches,
            store.elements.capacity()
        );
        println!(
            "| store         | elem/slot  | elem/match | nodes/m  | attrs/m  | text/m   | total/m   |"
        );
        println!(
            "|---------------|-----------:|----------:|---------:|---------:|---------:|----------:|"
        );
        row(
            "Store",
            store.heap_usage(),
            matches,
            store.elements.capacity(),
        );
        row(
            "Columnar",
            columnar.heap_usage(),
            matches,
            store.elements.capacity(),
        );
    }
}
