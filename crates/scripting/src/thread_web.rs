//! The console's runner on a browser page: Lua 5.4 (wasmoon) in a worker
//! of the page's own (web/lua-worker.js), so a long script never holds the
//! page.
//!
//! The holder's side is the desktop's: a job starts, every command the
//! script calls reaches the holder as an [`Event::Call`], and the job
//! finishes with what it printed and returned. The script waits for each
//! answer as the desktop's does; here it awaits a promise the page
//! settles when the holder replies. `app.commands` and `app.search` are
//! answered here, as the desktop's engine answers them. Stop ends the
//! worker and starts another: nothing a script does can hold it.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Duration;

use core_document::{CommandArgs, CommandResult, CommandSpec};
use serde::Deserialize;
use serde_json::{Value, json};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use crate::{Event, Job, RunOutput, STOPPED, list_commands, search_commands};

/// The worker's script, beside the page.
const WORKER_SCRIPT: &str = "lua-worker.js";
const PRELUDE: &str = include_str!("prelude.lua");
const WEB_PRELUDE: &str = include_str!("web_prelude.lua");

/// What the worker tells the page.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum News {
    Ready,
    Print {
        text: String,
    },
    Call {
        call: u64,
        id: String,
        args: String,
    },
    Done {
        printed: Vec<String>,
        value: Option<String>,
        returned: Option<String>,
        error: Option<String>,
    },
}

/// The worker and what it has said that the holder has not yet heard.
struct Worker {
    worker: web_sys::Worker,
    news: Rc<RefCell<VecDeque<News>>>,
    ready: Rc<RefCell<bool>>,
    _on_message: Closure<dyn FnMut(web_sys::MessageEvent)>,
}

impl Worker {
    fn start(wake: &Arc<dyn Fn() + Send + Sync>) -> Self {
        let worker =
            web_sys::Worker::new(WORKER_SCRIPT).expect("the page starts the console's worker");
        let news = Rc::new(RefCell::new(VecDeque::new()));
        let ready = Rc::new(RefCell::new(false));
        let on_message = {
            let (news, ready, wake) = (news.clone(), ready.clone(), wake.clone());
            Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
                let Some(text) = event.data().as_string() else {
                    return;
                };
                match serde_json::from_str::<News>(&text) {
                    Ok(News::Ready) => *ready.borrow_mut() = true,
                    Ok(other) => news.borrow_mut().push_back(other),
                    Err(err) => {
                        tracing::error!("the console's worker said what did not read: {err}")
                    }
                }
                wake();
            })
        };
        worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        let init = json!({"type": "init", "webPrelude": WEB_PRELUDE, "prelude": PRELUDE});
        let _ = worker.post_message(&init.to_string().into());
        Self {
            worker,
            news,
            ready,
            _on_message: on_message,
        }
    }

    fn post(&self, message: &Value) {
        if let Err(err) = self.worker.post_message(&message.to_string().into()) {
            tracing::error!("the console's worker took no message: {err:?}");
        }
    }
}

/// The job out: its label, the commands `help` reads, and the command a
/// single-command job waits on.
struct Running {
    label: String,
    commands: Vec<CommandSpec>,
    command: Option<(String, Receiver<CommandResult>)>,
}

/// The holder's end of the runner.
pub struct ScriptThread {
    worker: Worker,
    wake: Arc<dyn Fn() + Send + Sync>,
    jobs: VecDeque<(Job, Vec<CommandSpec>)>,
    events: VecDeque<Event>,
    running: Option<Running>,
    /// The script's calls handed to the holder, by the worker's call id.
    asked: HashMap<u64, (String, Receiver<CommandResult>)>,
    /// Jobs submitted and not yet finished.
    pending: usize,
}

impl ScriptThread {
    /// Start the runner. `wake` is called when the worker has news, for a
    /// holder that sleeps between events.
    pub fn spawn(wake: impl Fn() + Send + Sync + 'static) -> Self {
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(wake);
        Self {
            worker: Worker::start(&wake),
            wake,
            jobs: VecDeque::new(),
            events: VecDeque::new(),
            running: None,
            asked: HashMap::new(),
            pending: 0,
        }
    }

    /// Queue `job`, with the commands it may call (for `help`). Jobs run
    /// one after another.
    pub fn submit(&mut self, job: Job, commands: Vec<CommandSpec>) {
        self.jobs.push_back((job, commands));
        self.pending += 1;
    }

    /// Whether a job is running or waiting to.
    pub fn busy(&self) -> bool {
        self.pending > 0
    }

    /// Stop the running job now: its worker ends and another starts, the
    /// console's globals with it. Queued jobs still run.
    pub fn stop(&mut self) {
        let Some(running) = self.running.take() else {
            return;
        };
        self.worker.worker.terminate();
        self.worker = Worker::start(&self.wake);
        self.asked.clear();
        self.events.push_back(Event::Finished {
            label: running.label,
            output: RunOutput {
                error: Some(STOPPED.to_string()),
                ..RunOutput::default()
            },
        });
    }

    /// The next event. Never waits: the worker's news is here once the page
    /// has had it.
    pub fn next_event(&mut self, _wait: Duration) -> Option<Event> {
        if self.events.is_empty() {
            self.advance();
        }
        let event = self.events.pop_front();
        if matches!(event, Some(Event::Finished { .. })) {
            self.pending = self.pending.saturating_sub(1);
        }
        event
    }

    fn advance(&mut self) {
        self.reply_to_answered();
        self.finish_command();
        let news: Vec<News> = self.worker.news.borrow_mut().drain(..).collect();
        for news in news {
            self.hear(news);
        }
        if self.running.is_none() && *self.worker.ready.borrow() {
            self.start_next();
        }
    }

    /// Start the next job: a single command goes to the holder, a line or
    /// a script to the worker.
    fn start_next(&mut self) {
        let Some((job, commands)) = self.jobs.pop_front() else {
            return;
        };
        let label = job.label();
        self.events.push_back(Event::Started {
            label: label.clone(),
        });
        let command = match job {
            Job::Command { id, args } => {
                let (reply, answer) = channel();
                self.events.push_back(Event::Call {
                    id: id.clone(),
                    args,
                    reply,
                });
                Some((id, answer))
            }
            Job::Line(line) => {
                self.worker
                    .post(&json!({"type": "run", "source": line, "name": "console", "line": true}));
                None
            }
            Job::Script { source, name } => {
                self.worker
                    .post(&json!({"type": "run", "source": source, "name": name, "line": false}));
                None
            }
        };
        self.running = Some(Running {
            label,
            commands,
            command,
        });
    }

    /// A single-command job finishes with the holder's answer.
    fn finish_command(&mut self) {
        let Some(running) = &self.running else {
            return;
        };
        let Some((id, answer)) = &running.command else {
            return;
        };
        let answer = match answer.try_recv() {
            Ok(answer) => answer,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err(core_document::CommandError::failed(
                "the application is closing",
            )),
        };
        let output = match answer {
            Ok(answer) => RunOutput {
                value: Some(
                    serde_json::to_string_pretty(&answer).unwrap_or_else(|_| answer.to_string()),
                ),
                returned: Some(answer),
                ..RunOutput::default()
            },
            Err(err) => RunOutput {
                error: Some(format!("{id}: {err}")),
                ..RunOutput::default()
            },
        };
        let label = self.running.take().expect("a job runs").label;
        self.events.push_back(Event::Finished { label, output });
        (self.wake)();
    }

    /// Hand the worker the holder's answers that have come.
    fn reply_to_answered(&mut self) {
        let answered: Vec<u64> = self
            .asked
            .iter()
            .filter_map(|(call, (_, answer))| match answer.try_recv() {
                Err(TryRecvError::Empty) => None,
                found => {
                    let id = &self.asked[call].0;
                    let message = match found {
                        Ok(Ok(value)) => json!({"type": "reply", "call": call, "answer": value.to_string()}),
                        Ok(Err(err)) => json!({"type": "reply", "call": call, "error": format!("{id}: {err}")}),
                        Err(_) => json!({"type": "reply", "call": call, "error": "the application is closing"}),
                    };
                    self.worker.post(&message);
                    Some(*call)
                }
            })
            .collect();
        for call in answered {
            self.asked.remove(&call);
        }
    }

    fn hear(&mut self, news: News) {
        match news {
            News::Ready => {}
            News::Print { text } => self.events.push_back(Event::Printed(text)),
            News::Call { call, id, args } => {
                let args: CommandArgs = match serde_json::from_str::<Value>(&args) {
                    Ok(Value::Object(map)) => map,
                    _ => CommandArgs::new(),
                };
                // What the desktop's engine answers itself.
                let listed = self
                    .running
                    .as_ref()
                    .map(|r| r.commands.clone())
                    .unwrap_or_default();
                let own = match id.as_str() {
                    "app.commands" => Some(list_commands(listed, &args)),
                    "app.search" => Some(search_commands(listed, &args)),
                    _ => None,
                };
                if let Some(answer) = own {
                    self.worker.post(
                        &json!({"type": "reply", "call": call, "answer": answer.to_string()}),
                    );
                    return;
                }
                let (reply, answer) = channel();
                self.events.push_back(Event::Call {
                    id: id.clone(),
                    args,
                    reply,
                });
                self.asked.insert(call, (id, answer));
            }
            News::Done {
                printed,
                value,
                returned,
                error,
            } => {
                let Some(running) = self.running.take() else {
                    return;
                };
                let returned = returned.and_then(|text| serde_json::from_str(&text).ok());
                self.events.push_back(Event::Finished {
                    label: running.label,
                    output: RunOutput {
                        printed,
                        value,
                        returned,
                        error,
                    },
                });
            }
        }
    }
}
