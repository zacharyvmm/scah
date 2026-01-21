mod swar;
pub use swar::parse;

#[cfg(target_arch = "x86_64")]
mod x86_64;
