//! On Windows the program carries its icon, which Explorer, the taskbar and
//! shortcuts show.

fn main() {
    println!("cargo::rerun-if-changed=assets/icon/printcad.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icon/printcad.ico");
        if let Err(err) = resource.compile() {
            println!("cargo::warning=the program goes without its icon: {err}");
        }
    }
}
