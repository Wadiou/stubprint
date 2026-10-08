//! Takes the samples that samples/python/build.py wrote, if it has been run, to be embedded in the
//! program. Without them the program has no samples, and everything else works.

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
