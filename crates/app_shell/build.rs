//! On Windows the program carries its icon, which Explorer, the taskbar and
//! shortcuts show. Every build carries the recipes under `docs/recipes/`,
//! which agents find through `search` and `describe`.

fn main() {
    println!("cargo::rerun-if-changed=assets/icon/printcad.ico");
    recipes();
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icon/printcad.ico");
        if let Err(err) = resource.compile() {
            println!("cargo::warning=the program goes without its icon: {err}");
        }
    }
}

/// `$OUT_DIR/recipes.rs`: every `docs/recipes/*.md` as `(stem, text)`,
/// by name.
fn recipes() {
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let docs = manifest.join("../../docs");
    let dir = docs.join("recipes");
    // Watched: the recipes folder, or while there is none the docs folder
    // it would appear in (a path that does not exist reruns every build).
    let watched = if dir.is_dir() { &dir } else { &docs };
    println!("cargo::rerun-if-changed={}", watched.display());
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    files.sort();
    let mut out = String::from("&[\n");
    for path in files {
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let path = path.canonicalize().unwrap_or(path);
        out.push_str(&format!(
            "    ({stem:?}, include_str!({:?})),\n",
            path.display().to_string()
        ));
    }
    out.push(']');
    let target = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("recipes.rs");
    std::fs::write(target, out).expect("the recipes list is written");
}
