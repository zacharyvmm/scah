use std::fs::File;
use std::io::BufReader;
use std::io::prelude::*;

use onego::{Query, Save, parse};

#[test]
fn test_all_anchor_tags_for_whatwg_html_spec() -> std::io::Result<()> {
    // 26th of december 2025 16:50
    let file = File::open("/home/zmm/Music/html.spec.whatwg.index.html")?;
    let mut buf_reader = BufReader::new(file);
    let mut contents = String::new();
    buf_reader.read_to_string(&mut contents)?;

    let queries = &[Query::all("a", Save::all()).build()];
    let map = parse(&contents, queries);

    //println!("{:#?}", map);
    Ok(())
}

#[test]
fn test_all_anchor_tags_for_albert_einstein_wikipedia<'q>() -> std::io::Result<()> {
    // 26th of december 2025 16:50
    let file = File::open("/home/zmm/Music/Albert_Einstein.html")?;
    let mut buf_reader = BufReader::new(file);
    let mut contents = String::new();
    buf_reader.read_to_string(&mut contents)?;

    let queries = &[Query::all("a", Save::all()).build()];
    let map = parse(&contents, queries);

    assert_eq!(map[0]["a"].iter().unwrap().count(), 3848);
    //println!("{:#?}", map);

    Ok(())
}
