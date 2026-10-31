use pyo3_stub_gen::Result;
use std::fs;
use std::path::Path;

fn main() -> Result<()> {
    scah::stub_info()?.generate()?;
    // pyo3-stub-gen writes the native module's stubs as a package,
    // `_scah/__init__.pyi`; keep them next to the extension module instead.
    let package = Path::new(env!("CARGO_MANIFEST_DIR")).join("python/scah");
    fs::rename(
        package.join("_scah/__init__.pyi"),
        package.join("_scah.pyi"),
    )?;
    fs::remove_dir(package.join("_scah"))?;
    Ok(())
}
