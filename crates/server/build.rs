//! Puts the built parts into the program: samples/out/samples.json and the files of web/dist.
//! Either is empty when it was not built.

use std::path::{Path, PathBuf};
use std::{env, fs};

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    samples(&root, &out);
    page(&root, &out);
}

fn samples(root: &Path, out: &Path) {
    let made = root.join("samples/out/samples.json");
    println!("cargo:rerun-if-changed={}", made.display());
    let data = fs::read_to_string(&made).unwrap_or_else(|_| "[]".to_string());
    fs::write(out.join("samples.json"), data)
        .expect("the samples can be written to the build folder");
}

/// Writes `FILES`, each file of the built page as its route and its bytes.
fn page(root: &Path, out: &Path) {
    let dist = root.join("web/dist");
    println!("cargo:rerun-if-changed={}", dist.display());
    let mut files = Vec::new();
    if let Ok(dist) = dist.canonicalize() {
        collect(&dist, &dist, &mut files);
    }
    files.sort();
    let mut code = String::from("pub static FILES: &[(&str, &[u8])] = &[\n");
    for (route, path) in files {
        code.push_str(&format!(
            "    ({route:?}, include_bytes!({:?})),\n",
            path.display().to_string()
        ));
    }
    code.push_str("];\n");
    fs::write(out.join("web.rs"), code).expect("the page can be written to the build folder");
}

fn collect(base: &Path, folder: &Path, files: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(base, &path, files);
        } else if let Ok(relative) = path.strip_prefix(base) {
            let route = relative
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            files.push((route, path));
        }
    }
}
