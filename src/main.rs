use onego::{Query, Save, parse};
use std::env;
use std::fs;
use std::time::Instant;

// Just a simple program to check the performance of 1go
// cargo run --release -- /home/zmm/Music/html.spec.whatwg.index.html
fn main() {
    let args: Vec<String> = env::args().collect();
    let filename = args
        .get(1)
        .expect("Please provide a file name as an argument");

    let content = fs::read_to_string(filename).expect("Could not read file");

    let start = Instant::now();

    let queries = &[Query::first("a", Save::all()).build()];

    let map = parse(content.as_str(), queries);
    // assert_eq!(map["a"].len()?, 7);
    // println!("{:#?}", map);

    let duration = start.elapsed();
    println!(
        "Time elapsed: {:?} ({}s), Tags Found: {}",
        duration,
        duration.as_secs_f64(),
        map["a"].len().unwrap()
    );
}
