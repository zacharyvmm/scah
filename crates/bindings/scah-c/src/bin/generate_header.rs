//! Regenerate `include/scah.h` from this crate's exported items.
//!
//! Run with `cargo run -p scah-c --features header-gen --bin generate_header`.

use std::path::Path;

fn main() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let config_path = crate_dir.join("cbindgen.toml");
    let header_path = crate_dir.join("include").join("scah.h");

    let config = cbindgen::Config::from_file(&config_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", config_path.display()));
    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()
        .expect("failed to generate the C header")
        .write_to_file(&header_path);

    println!("wrote {}", header_path.display());
}
