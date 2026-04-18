use pyo3_stub_gen::Result;

fn main() -> Result<()> {
    scah::stub_info()?.generate()?;
    Ok(())
}
