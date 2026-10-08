//! Copies samples/out/samples.json into the build, or an empty list when it was not built.

use std::path::{Path, PathBuf};
use std::{env, fs};

fn main() {
    let made = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/out/samples.json");
    println!("cargo:rerun-if-changed={}", made.display());
    let data = fs::read_to_string(&made).unwrap_or_else(|_| "[]".to_string());
    let target =
        PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("samples.json");
    fs::write(target, data).expect("the samples can be written to the build folder");
}
