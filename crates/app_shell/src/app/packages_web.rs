//! Workbench packages on a browser page. A page has no folders: each
//! installed package is its archive, kept in the page's database
//! (IndexedDB) with where it came from, and held in memory once read.
//! Archives come from a picked file or a workbench store's copy, the
//! network through the page's own requests; what they find comes back as
//! `PackageNews`, as a desktop's package threads do.

use std::cell::RefCell;
use std::collections::HashMap;

use js_sys::{Array, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use workbenches::{Package, PackageState, PackageStatus};

use super::packages::{PackageNews, Ready, capabilities};
use crate::PrintCadApp;
use crate::log_panel as app_log;

/// Where a package's source is held beside its files.
const SOURCE: &str = "source.json";

#[wasm_bindgen(inline_js = r#"
const DB = "printcad-packages";
const STORE = "packages";

function open() {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB, 1);
    request.onupgradeneeded = () => request.result.createObjectStore(STORE, { keyPath: "id" });
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

async function within(mode, work) {
  const db = await open();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE, mode);
    const request = work(tx.objectStore(STORE));
    tx.oncomplete = () => resolve(request.result);
    tx.onerror = () => reject(tx.error);
  });
}

export function kept_packages() {
  return within("readonly", (store) => store.getAll());
}

export function keep_package(id, archive, source) {
  return within("readwrite", (store) => store.put({ id, archive, source }));
}

export function forget_package(id) {
  return within("readwrite", (store) => store.delete(id));
}

export async function fetch_bytes(url) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url} answered ${response.status} ${response.statusText}`);
  return new Uint8Array(await response.arrayBuffer());
}

export async function fetch_text(url) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url} answered ${response.status} ${response.statusText}`);
  return response.text();
}
"#)]
extern "C" {
    fn kept_packages() -> js_sys::Promise;
    fn keep_package(id: &str, archive: &Uint8Array, source: Option<String>) -> js_sys::Promise;
    fn forget_package(id: &str) -> js_sys::Promise;
    fn fetch_bytes(url: &str) -> js_sys::Promise;
    fn fetch_text(url: &str) -> js_sys::Promise;
}

thread_local! {
    /// The packages read this session, by id: what turning one on again
    /// starts from.
    static HELD: RefCell<HashMap<String, Package>> = RefCell::default();
}

fn js_error(error: JsValue) -> String {
    error
        .dyn_ref::<js_sys::Error>()
        .map(|e| String::from(e.message()))
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"))
}

/// A kept archive and its source read back into a package.
fn package_of(archive: &[u8], source: Option<String>) -> Result<Package, String> {
    let package = Package::from_archive(archive)?;
    Ok(match source {
        Some(source) => package.holding(SOURCE, source.into_bytes()),
        None => package,
    })
}

/// The text of `url`, through the page's own request.
pub(crate) async fn text_at(url: &str) -> Result<String, String> {
    let text = JsFuture::from(fetch_text(url)).await.map_err(js_error)?;
    text.as_string()
        .ok_or_else(|| format!("{url} answered no text"))
}

impl PrintCadApp {
    /// Read every package the page keeps and start those turned on.
    pub(crate) fn start_page_packages(&mut self) {
        self.package_news(async {
            let kept = JsFuture::from(kept_packages()).await.map_err(js_error)?;
            let mut found = Vec::new();
            for record in Array::from(&kept).iter() {
                let field = |key: &str| Reflect::get(&record, &key.into()).unwrap_or_default();
                let id = field("id").as_string().unwrap_or_default();
                let archive = Uint8Array::new(&field("archive")).to_vec();
                found.push(package_of(&archive, field("source").as_string()).map_err(|e| (id, e)));
            }
            Ok(PackageNews::Kept(found))
        });
    }

    /// Run `work` on the page, its news read by `drain_package_news`; a
    /// failure is news too.
    fn package_news(
        &mut self,
        work: impl std::future::Future<Output = Result<PackageNews, String>> + 'static,
    ) {
        let tx = self.package_work.tx.clone();
        self.package_work.pending += 1;
        wasm_bindgen_futures::spawn_local(async move {
            let news = work.await.unwrap_or_else(PackageNews::Failed);
            let _ = tx.send(news);
        });
    }

    /// The packages the page kept: each turned on starts, the others are
    /// listed as off.
    pub(crate) fn take_kept(&mut self, found: Vec<Result<Package, (String, String)>>) {
        for package in found {
            match package {
                Ok(package) => {
                    let id = package.manifest.id.clone();
                    HELD.with(|held| held.borrow_mut().insert(id.clone(), package.clone()));
                    if self.user_settings.packages.enabled(&id) {
                        self.start_page_package(package, "Loaded");
                    } else {
                        self.packages
                            .push(PackageStatus::of(&package, PackageState::Disabled));
                    }
                }
                Err((id, reason)) => {
                    app_log::warn(format!("The workbench package {id} did not load: {reason}"));
                    self.packages.push(PackageStatus {
                        name: id.clone(),
                        id,
                        version: String::new(),
                        description: String::new(),
                        dir: Default::default(),
                        requested: Default::default(),
                        state: PackageState::Failed(reason),
                        source: None,
                        update: None,
                    });
                }
            }
        }
    }

    /// Start `package` on the page as the settings allow; `done` names it
    /// for the notice.
    fn start_page_package(&mut self, package: Package, done: &'static str) {
        let id = package.manifest.id.clone();
        if !self.user_settings.packages.enabled(&id) {
            let tx = self.package_work.tx.clone();
            self.package_work.pending += 1;
            let _ = tx.send(PackageNews::Ready(Box::new(Ready {
                done,
                package,
                bench: None,
            })));
            return;
        }
        let tx = self.package_work.tx.clone();
        self.package_work.pending += 1;
        let granted = capabilities(self.user_settings.packages.grant(&id));
        workbenches::prepare_package_later(package.clone(), granted, move |bench| {
            let _ = tx.send(PackageNews::Ready(Box::new(Ready {
                done,
                package,
                bench: Some(bench),
            })));
        });
    }

    /// Keep `package`, read from `archive`, and start it.
    pub(crate) fn take_held(&mut self, done: &'static str, package: Package, archive: Vec<u8>) {
        let id = package.manifest.id.clone();
        let source = package
            .held
            .as_ref()
            .and_then(|held| held.file(SOURCE))
            .map(|bytes| String::from_utf8_lossy(bytes).into_owned());
        let kept = keep_package(&id, &Uint8Array::from(&archive[..]), source);
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(e) = JsFuture::from(kept).await {
                app_log::warn(format!(
                    "The page could not keep the package for next time: {}",
                    js_error(e)
                ));
            }
        });
        HELD.with(|held| held.borrow_mut().insert(id, package.clone()));
        self.start_page_package(package, done);
    }

    /// Install the archive the page holds as `path` (a picked file).
    pub(crate) fn install_page_archive(&mut self, path: &std::path::Path) {
        let read = crate::platform::read(path).map_err(|e| e.to_string());
        match read.and_then(|bytes| Package::from_archive(&bytes).map(|p| (p, bytes))) {
            Ok((package, archive)) => self.take_held("Installed", package, archive),
            Err(e) => app_log::error(format!("Could not install {}: {e}", path.display())),
        }
    }

    /// Install what store `listing` lists, fetched from the registry's
    /// copy.
    pub(crate) fn install_page_listed(&mut self, listing: workbenches::Listing) {
        let url = match listing.installable() {
            Ok(listed) => listed.page_url().to_string(),
            Err(e) => {
                app_log::error(format!("Could not install {}: {e}", listing.name));
                return;
            }
        };
        self.package_news(async move {
            let fail = |e: String| format!("Could not install {}: {e}", listing.name);
            let bytes = JsFuture::from(fetch_bytes(&url))
                .await
                .map_err(|e| fail(js_error(e)))?;
            let archive = Uint8Array::new(&bytes).to_vec();
            let package = workbenches::listed_package(&listing, &archive).map_err(fail)?;
            Ok(PackageNews::Held {
                done: "Installed",
                package,
                archive,
            })
        });
    }

    /// Start again the package `id` the page holds (turned on, or given
    /// other grants).
    pub(crate) fn restart_page_package(&mut self, id: &str) {
        match HELD.with(|held| held.borrow().get(id).cloned()) {
            Some(package) => self.start_page_package(package, "Loaded"),
            None => app_log::error(format!("The page holds no package {id}")),
        }
    }

    /// Forget package `id`: the page keeps it no more.
    pub(crate) fn forget_page_package(&mut self, id: &str) {
        HELD.with(|held| held.borrow_mut().remove(id));
        let forgotten = forget_package(id);
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(e) = JsFuture::from(forgotten).await {
                app_log::warn(format!(
                    "The page could not forget the package: {}",
                    js_error(e)
                ));
            }
        });
    }

    /// Fetch store `url`'s index through the page's request.
    pub(crate) fn look_at_page_store(&mut self, url: String, quiet: bool) {
        self.package_news(async move {
            let found = match text_at(&url).await {
                Ok(text) => workbenches::read_store(&text),
                Err(e) => Err(e),
            };
            Ok(PackageNews::Store { url, found, quiet })
        });
    }
}
