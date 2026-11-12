//! Puts the built parts into the program: samples/out/samples.json and the files of web/dist,
//! either of which is empty when it was not built, and the SQL files of `migrations/`, which are
//! found by looking in the folder, so a new file is used without being listed anywhere.

use std::path::{Path, PathBuf};
use std::{env, fs};

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    samples(&root, &out);
    page(&root, &out);
    migrations(&out);
}

/// Writes `MIGRATIONS`, each SQL file of `migrations/` as its number, its name and its text. A
/// file is called `001_what_it_does.sql`, and the numbers go 1, 2, 3 with none missing, since the
/// database keeps the number of the last one it had.
fn migrations(out: &Path) {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    println!("cargo:rerun-if-changed={}", folder.display());
    let mut files: Vec<(u32, String, PathBuf)> = fs::read_dir(&folder)
        .expect("there is a migrations folder")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "sql"))
        .map(|path| {
            let stem = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            let (number, name) = stem
                .split_once('_')
                .unwrap_or_else(|| panic!("{stem}.sql should be called like 001_what_it_does.sql"));
            let number: u32 = number
                .parse()
                .unwrap_or_else(|_| panic!("{stem}.sql should start with its number, like 001_"));
            (number, name.to_string(), path)
        })
        .collect();
    files.sort();
    for (index, (number, name, _)) in files.iter().enumerate() {
        let wanted = u32::try_from(index + 1).expect("there are not that many migrations");
        assert_eq!(
            *number, wanted,
            "the migrations should be numbered 1, 2, 3 with none missing, but {number:03}_{name}.sql \
             comes where {wanted:03} should"
        );
    }
    let mut code = String::from("pub static MIGRATIONS: &[(u32, &str, &str)] = &[\n");
    for (number, name, path) in files {
        code.push_str(&format!(
            "    ({number}, {name:?}, include_str!({:?})),\n",
            path.display().to_string()
        ));
    }
    code.push_str("];\n");
    fs::write(out.join("migrations.rs"), code)
        .expect("the migrations can be written to the build folder");
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
