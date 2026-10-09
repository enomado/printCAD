//! A package's jobs on a page: each in a worker of its own running the
//! package's transpiled module (`web/package-worker.js`), its progress
//! reported as it goes; stopping one ends its worker.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use js_sys::{Object, Reflect};
use wasm_bindgen::prelude::*;

use crate::host::PackageInfo;

struct Running {
    worker: web_sys::Worker,
    done: u64,
    total: u64,
    _on_message: Closure<dyn FnMut(web_sys::MessageEvent)>,
}

#[derive(Default)]
struct Board {
    running: HashMap<u64, Running>,
    finished: Vec<(u64, Result<String, String>)>,
}

/// A package's jobs: those running and those finished but not yet told to
/// the bench.
pub(crate) struct JobBoard {
    /// The transpiled package (`files`, `name`), handed to each job.
    prepared: JsValue,
    package: Arc<PackageInfo>,
    next: RefCell<u64>,
    board: Rc<RefCell<Board>>,
}

// SAFETY: as the page's `Guest`: this build runs on one thread.
unsafe impl Send for JobBoard {}
unsafe impl Sync for JobBoard {}

impl JobBoard {
    pub(crate) fn new(prepared: JsValue, package: Arc<PackageInfo>) -> Arc<Self> {
        Arc::new(Self {
            prepared,
            package,
            next: RefCell::new(1),
            board: Rc::default(),
        })
    }

    /// Start `entry` with `input`; its number, the result to follow.
    pub(crate) fn start(self: &Arc<Self>, entry: String, input: String) -> Result<u64, String> {
        let job = {
            let mut next = self.next.borrow_mut();
            *next += 1;
            *next - 1
        };
        let options = web_sys::WorkerOptions::new();
        options.set_type(web_sys::WorkerType::Module);
        let worker = web_sys::Worker::new_with_options("package-worker.js", &options)
            .map_err(|e| format!("cannot start a job: {e:?}"))?;
        let board = self.board.clone();
        let id = self.package.id.clone();
        let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(
            move |event: web_sys::MessageEvent| {
                let data = event.data();
                let field = |key: &str| Reflect::get(&data, &key.into()).unwrap_or_default();
                let mut board = board.borrow_mut();
                match field("type").as_string().as_deref() {
                    Some("progress") => {
                        if let Some(running) = board.running.get_mut(&job) {
                            running.done = field("done").as_f64().unwrap_or(0.0) as u64;
                            running.total = field("total").as_f64().unwrap_or(0.0) as u64;
                        }
                    }
                    Some("log") => {
                        let message = field("message").as_string().unwrap_or_default();
                        tracing::info!(target: "printcad.bench", package = %id, "{message}");
                    }
                    Some("done") => {
                        let result = match field("ok").as_string() {
                            Some(ok) => Ok(ok),
                            None => Err(field("err").as_string().unwrap_or_default()),
                        };
                        if let Some(running) = board.running.remove(&job) {
                            running.worker.terminate();
                        }
                        board.finished.push((job, result));
                    }
                    _ => {}
                }
            },
        );
        worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        let message = Object::new();
        let put = |key: &str, value: &JsValue| {
            let _ = Reflect::set(&message, &key.into(), value);
        };
        put("type", &"job".into());
        put(
            "files",
            &Reflect::get(&self.prepared, &"files".into()).unwrap_or_default(),
        );
        put(
            "name",
            &Reflect::get(&self.prepared, &"name".into()).unwrap_or_default(),
        );
        put("entry", &entry.into());
        put("input", &input.into());
        worker
            .post_message(&message)
            .map_err(|e| format!("cannot start a job: {e:?}"))?;
        self.board.borrow_mut().running.insert(
            job,
            Running {
                worker,
                done: 0,
                total: 0,
                _on_message: on_message,
            },
        );
        Ok(job)
    }

    /// Stop job `job`: its worker ends, and it finishes as stopped.
    pub(crate) fn cancel(&self, job: u64) {
        let mut board = self.board.borrow_mut();
        if let Some(running) = board.running.remove(&job) {
            running.worker.terminate();
            board.finished.push((job, Err("stopped".into())));
        }
    }

    pub(crate) fn cancel_all(&self) {
        let jobs: Vec<u64> = self.board.borrow().running.keys().copied().collect();
        for job in jobs {
            self.cancel(job);
        }
    }

    /// How far job `job` is, `(done, total)`, while it runs.
    pub(crate) fn progress(&self, job: u64) -> Option<(u64, u64)> {
        let board = self.board.borrow();
        let running = board.running.get(&job)?;
        Some((running.done, running.total))
    }

    /// The jobs finished since this was last asked.
    pub(crate) fn take_finished(&self) -> Vec<(u64, Result<String, String>)> {
        std::mem::take(&mut self.board.borrow_mut().finished)
    }

    pub(crate) fn running(&self) -> bool {
        !self.board.borrow().running.is_empty()
    }

    /// A job finished that the bench has not been told of.
    pub(crate) fn has_finished(&self) -> bool {
        !self.board.borrow().finished.is_empty()
    }
}

/// A page runs no native helpers.
pub(crate) fn run_helper(
    _dir: &Path,
    _name: &str,
    _input: &[u8],
    _cancelled: &AtomicBool,
) -> Result<Vec<u8>, String> {
    Err("helpers run in the desktop application".into())
}
