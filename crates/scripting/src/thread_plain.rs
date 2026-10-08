//! The script thread's interface without Lua, for a build that has no
//! threads and cannot build the engine (a browser page).
//!
//! It keeps the holder's side unchanged: a job starts, a single command
//! travels to the holder as an [`Event::Call`] and the job finishes with its
//! answer once the holder replies. A console line or a script finishes at
//! once, saying that Lua runs in the desktop application. Nothing waits:
//! the holder drives everything through [`ScriptThread::next_event`].

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::Duration;

use core_document::{CommandArgs, CommandResult, CommandSpec};

use crate::RunOutput;

/// What to run.
#[derive(Debug, Clone)]
pub enum Job {
    /// A console line: an expression answers its value.
    Line(String),
    /// A whole script; `name` names it in messages.
    Script { source: String, name: String },
    /// One command: the output's value is its answer as JSON.
    Command {
        id: String,
        args: core_document::CommandArgs,
    },
}

impl Job {
    /// How a run of this job is named: the line itself, the script's name,
    /// or the command's id.
    pub fn label(&self) -> String {
        match self {
            Job::Line(line) => line.clone(),
            Job::Script { name, .. } => name.clone(),
            Job::Command { id, .. } => id.clone(),
        }
    }
}

/// What the runner tells its holder.
#[derive(Debug)]
pub enum Event {
    /// A job began.
    Started { label: String },
    /// A line the script printed.
    Printed(String),
    /// Run a command and send its answer back on `reply`.
    Call {
        id: String,
        args: CommandArgs,
        reply: Sender<CommandResult>,
    },
    /// A job ended; `output.printed` repeats what came as `Printed`.
    Finished { label: String, output: RunOutput },
}

/// What a line or a script answers without the engine.
const NO_LUA: &str = "Lua scripts run in the desktop application; this build runs single \
                      commands only";

/// A command handed to the holder, waiting for its answer.
struct Asked {
    label: String,
    id: String,
    answer: Receiver<CommandResult>,
}

/// The holder's end of the runner.
pub struct ScriptThread {
    jobs: VecDeque<Job>,
    events: VecDeque<Event>,
    asked: Option<Asked>,
    /// Jobs submitted and not yet finished.
    pending: usize,
}

impl ScriptThread {
    /// Start the runner. Every event is ready when the holder next asks, so
    /// `wake` is not needed.
    pub fn spawn(_wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            jobs: VecDeque::new(),
            events: VecDeque::new(),
            asked: None,
            pending: 0,
        }
    }

    /// Queue `job`. Jobs run one after another.
    pub fn submit(&mut self, job: Job, _commands: Vec<CommandSpec>) {
        self.jobs.push_back(job);
        self.pending += 1;
    }

    /// Whether a job is running or waiting to.
    pub fn busy(&self) -> bool {
        self.pending > 0
    }

    /// Nothing runs long enough to stop: a command is the holder's own.
    pub fn stop(&self) {}

    /// The next event. Never waits: a command's answer is there once the
    /// holder has replied to its call.
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

    /// Finish the command waiting for its answer, or start the next job.
    fn advance(&mut self) {
        if let Some(asked) = &self.asked {
            let answer = match asked.answer.try_recv() {
                Ok(answer) => answer,
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => Err(core_document::CommandError::failed(
                    "the application is closing",
                )),
            };
            let Asked { label, id, .. } = self.asked.take().expect("a command was asked");
            let output = match answer {
                Ok(answer) => RunOutput {
                    value: Some(
                        serde_json::to_string_pretty(&answer)
                            .unwrap_or_else(|_| answer.to_string()),
                    ),
                    returned: Some(answer),
                    ..RunOutput::default()
                },
                Err(err) => RunOutput {
                    error: Some(format!("{id}: {err}")),
                    ..RunOutput::default()
                },
            };
            self.events.push_back(Event::Finished { label, output });
            return;
        }
        let Some(job) = self.jobs.pop_front() else {
            return;
        };
        let label = job.label();
        self.events.push_back(Event::Started {
            label: label.clone(),
        });
        match job {
            Job::Command { id, args } => {
                let (reply, answer) = channel();
                self.events.push_back(Event::Call {
                    id: id.clone(),
                    args,
                    reply,
                });
                self.asked = Some(Asked { label, id, answer });
            }
            Job::Line(_) | Job::Script { .. } => self.events.push_back(Event::Finished {
                label,
                output: RunOutput {
                    error: Some(NO_LUA.to_string()),
                    ..RunOutput::default()
                },
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_command_finishes_with_the_holders_answer() {
        let mut runner = ScriptThread::spawn(|| {});
        runner.submit(
            Job::Command {
                id: "doc.bodies".into(),
                args: CommandArgs::default(),
            },
            Vec::new(),
        );
        assert!(matches!(
            runner.next_event(Duration::ZERO),
            Some(Event::Started { .. })
        ));
        let Some(Event::Call { id, reply, .. }) = runner.next_event(Duration::ZERO) else {
            panic!("the command is asked of the holder");
        };
        assert_eq!(id, "doc.bodies");
        // Unanswered, the job is still running.
        assert!(runner.next_event(Duration::ZERO).is_none());
        assert!(runner.busy());
        reply.send(Ok(json!([1, 2]))).unwrap();
        let Some(Event::Finished { output, .. }) = runner.next_event(Duration::ZERO) else {
            panic!("the job finishes");
        };
        assert_eq!(output.returned, Some(json!([1, 2])));
        assert!(!runner.busy());
    }

    #[test]
    fn a_line_says_lua_needs_the_desktop_app() {
        let mut runner = ScriptThread::spawn(|| {});
        runner.submit(Job::Line("1 + 1".into()), Vec::new());
        runner.next_event(Duration::ZERO);
        let Some(Event::Finished { output, .. }) = runner.next_event(Duration::ZERO) else {
            panic!("the line finishes at once");
        };
        assert!(output.error.unwrap().contains("desktop application"));
        assert!(!runner.busy());
    }
}
