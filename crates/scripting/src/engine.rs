//! The Lua engine: the `pc` namespace over a [`Host`]'s commands.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use core_document::{CommandArgs, CommandSpec};
use mlua::serde::SerializeOptions;
use mlua::{HookTriggers, Lua, LuaSerdeExt, MultiValue, VmState};

use crate::{Host, RunOutput, STOPPED, command_entry};

/// How long a run may take before it is stopped, unless the caller sets
/// another limit.
const DEFAULT_TIME_LIMIT: Duration = Duration::from_secs(10);

/// Lua code every run starts with: the `pc` namespace, `print` into the
/// run's output, `show` and `help`.
const PRELUDE: &str = include_str!("prelude.lua");

/// Where printed lines go as they are printed, besides the run's output.
type PrintSink = Rc<RefCell<Option<Box<dyn Fn(&str)>>>>;

pub struct ScriptEngine {
    lua: Lua,
    printed: Rc<RefCell<Vec<String>>>,
    on_print: PrintSink,
    started: Rc<Cell<Instant>>,
    time_limit: Rc<Cell<Duration>>,
    stop: Rc<RefCell<Option<Arc<AtomicBool>>>>,
}

impl Default for ScriptEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptEngine {
    pub fn new() -> Self {
        let lua = Lua::new();
        let printed = Rc::new(RefCell::new(Vec::new()));
        let started = Rc::new(Cell::new(Instant::now()));
        let time_limit = Rc::new(Cell::new(DEFAULT_TIME_LIMIT));

        let sink = printed.clone();
        let on_print: PrintSink = Rc::new(RefCell::new(None));
        let stream = on_print.clone();
        let print = lua
            .create_function(move |_, line: String| {
                if let Some(f) = stream.borrow().as_ref() {
                    f(&line);
                }
                sink.borrow_mut().push(line);
                Ok(())
            })
            .expect("print function");
        lua.globals()
            .set("__pc_print", print)
            .expect("print global");
        // `array(...)`: a table marked as a list, so it converts to a list
        // even when it is empty.
        let array = lua
            .create_function(|lua, values: mlua::Variadic<mlua::Value>| {
                let list = lua.create_sequence_from(values)?;
                list.set_metatable(Some(lua.array_metatable()))?;
                Ok(list)
            })
            .expect("array function");
        lua.globals().set("array", array).expect("array global");

        let (since, limit) = (started.clone(), time_limit.clone());
        let stop: Rc<RefCell<Option<Arc<AtomicBool>>>> = Rc::new(RefCell::new(None));
        let stopped = stop.clone();
        lua.set_hook(
            HookTriggers::new().every_nth_instruction(10_000),
            move |_, _| {
                if stopped
                    .borrow()
                    .as_ref()
                    .is_some_and(|flag| flag.load(Ordering::Relaxed))
                {
                    Err(mlua::Error::runtime(STOPPED))
                } else if since.get().elapsed() > limit.get() {
                    Err(mlua::Error::runtime(format!(
                        "stopped after {} s",
                        limit.get().as_secs_f32()
                    )))
                } else {
                    Ok(VmState::Continue)
                }
            },
        )
        .expect("time limit hook");

        lua.load(PRELUDE)
            .set_name("prelude")
            .exec()
            .expect("the prelude runs");
        Self {
            lua,
            printed,
            on_print,
            started,
            time_limit,
            stop,
        }
    }

    /// Hand every printed line to `f` as it is printed, as well as keeping
    /// it for the run's output.
    pub fn on_print(&mut self, f: impl Fn(&str) + 'static) {
        *self.on_print.borrow_mut() = Some(Box::new(f));
    }

    /// Stop the running script once `flag` is set, at its next instruction
    /// check.
    pub fn stop_on(&mut self, flag: Arc<AtomicBool>) {
        *self.stop.borrow_mut() = Some(flag);
    }

    /// Give scripts `arg`, the list of words they were run with, as Lua's
    /// own interpreter does.
    pub fn set_args(&mut self, args: &[String]) -> Result<(), String> {
        let table = self
            .lua
            .create_sequence_from(args.iter().cloned())
            .map_err(|e| e.to_string())?;
        self.lua
            .globals()
            .set("arg", table)
            .map_err(|e| e.to_string())
    }

    /// Stop any run that takes longer than `limit`.
    pub fn set_time_limit(&mut self, limit: Duration) {
        self.time_limit.set(limit);
    }

    /// Run one console line. An expression answers its value; anything
    /// else runs as a statement, answering what it `return`s. Globals stay
    /// set for the next line.
    pub fn eval_line(&mut self, line: &str, host: &mut dyn Host) -> RunOutput {
        let expression = format!("return {line}");
        if self.lua.load(&expression).into_function().is_ok() {
            self.run(&expression, "console", host, true)
        } else {
            self.run(line, "console", host, true)
        }
    }

    /// Run a whole script; `name` names it in error messages.
    pub fn run_script(&mut self, source: &str, name: &str, host: &mut dyn Host) -> RunOutput {
        self.run(source, name, host, false)
    }

    /// Run a whole script, what it returns answered as `value` too, shown
    /// as text the way a console line's value is.
    pub fn run_script_shown(&mut self, source: &str, name: &str, host: &mut dyn Host) -> RunOutput {
        self.run(source, name, host, true)
    }

    fn run(&mut self, source: &str, name: &str, host: &mut dyn Host, show: bool) -> RunOutput {
        self.printed.borrow_mut().clear();
        self.started.set(Instant::now());
        let lua = &self.lua;
        let result = lua.scope(|scope| {
            let call =
                scope.create_function_mut(|lua, (id, args): (String, Option<mlua::Value>)| {
                    let args = to_args(lua, args)?;
                    let answer = if id == "app.commands" {
                        Ok(list_commands(host.commands(), &args))
                    } else if id == "app.search" {
                        Ok(search_commands(host.commands(), &args))
                    } else {
                        host.call(&id, args)
                    };
                    match answer {
                        Ok(value) => lua.to_value_with(
                            &value,
                            SerializeOptions::new()
                                .serialize_none_to_null(false)
                                .serialize_unit_to_null(false),
                        ),
                        Err(error) => Err(mlua::Error::external(ScriptCommandError(format!(
                            "{id}: {error}"
                        )))),
                    }
                })?;
            lua.globals().set("__pc_call", call)?;
            let values: MultiValue = lua.load(source).set_name(name).eval()?;
            if values.iter().all(|v| v.is_nil()) {
                return Ok((None, None));
            }
            let returned = Some(returned_json(lua, &values));
            if !show {
                return Ok((None, returned));
            }
            let show: mlua::Function = lua.globals().get("show")?;
            let mut parts = Vec::new();
            for value in values {
                parts.push(show.call::<String>(value)?);
            }
            Ok((Some(parts.join("\t")), returned))
        });
        let _ = self.lua.globals().set("__pc_call", mlua::Nil);
        let printed = std::mem::take(&mut *self.printed.borrow_mut());
        match result {
            Ok((value, returned)) => RunOutput {
                printed,
                value,
                returned,
                error: None,
            },
            Err(error) => RunOutput {
                printed,
                error: Some(error_text(&error)),
                ..RunOutput::default()
            },
        }
    }
}

/// The values a chunk returned as JSON: one value as itself, several as a
/// list. Ids are strings in Lua and stay strings.
fn returned_json(lua: &Lua, values: &MultiValue) -> serde_json::Value {
    match values.len() {
        1 => lua_json(lua, &values[0], 0),
        _ => serde_json::Value::Array(values.iter().map(|v| lua_json(lua, v, 0)).collect()),
    }
}

/// How deep a returned table may nest before the rest is written as text.
const RETURN_DEPTH: usize = 32;

/// A Lua value as JSON. A table whose keys are 1 to n (or one made with
/// `array`) is a list, any other an object, its keys written as text.
/// What JSON cannot hold (a function, a number that is not finite, a table
/// nested past [`RETURN_DEPTH`], as one that holds itself is) is written
/// as Lua's `tostring` of it, or null for a number.
fn lua_json(lua: &Lua, value: &mlua::Value, depth: usize) -> serde_json::Value {
    use serde_json::Value as J;
    match value {
        mlua::Value::Nil => J::Null,
        mlua::Value::LightUserData(u) if u.0.is_null() => J::Null,
        mlua::Value::Boolean(b) => J::Bool(*b),
        mlua::Value::Integer(i) => J::from(*i),
        mlua::Value::Number(n) => serde_json::Number::from_f64(*n).map_or(J::Null, J::Number),
        mlua::Value::String(s) => J::String(s.to_string_lossy()),
        mlua::Value::Table(table) if depth < RETURN_DEPTH => {
            let len = table.raw_len();
            let pairs: Vec<(mlua::Value, mlua::Value)> =
                table.pairs().filter_map(Result::ok).collect();
            let listed = table
                .metatable()
                .is_some_and(|m| m == lua.array_metatable());
            if listed || (len > 0 && pairs.len() == len) {
                J::Array(
                    (1..=len)
                        .map(|i| {
                            lua_json(
                                lua,
                                &table.raw_get(i).unwrap_or(mlua::Value::Nil),
                                depth + 1,
                            )
                        })
                        .collect(),
                )
            } else if pairs.is_empty() {
                J::Array(Vec::new())
            } else {
                J::Object(
                    pairs
                        .iter()
                        .map(|(k, v)| {
                            let key = match k {
                                mlua::Value::String(s) => s.to_string_lossy(),
                                other => other.to_string().unwrap_or_default(),
                            };
                            (key, lua_json(lua, v, depth + 1))
                        })
                        .collect(),
                )
            }
        }
        other => J::String(other.to_string().unwrap_or_default()),
    }
}

/// A command's refusal, carried through Lua's error unchanged.
#[derive(Debug)]
struct ScriptCommandError(String);

impl std::fmt::Display for ScriptCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ScriptCommandError {}

/// The message of an error, without the traceback Lua appends.
fn error_text(error: &mlua::Error) -> String {
    let text = match error {
        mlua::Error::CallbackError { cause, .. } => return error_text(cause),
        mlua::Error::ExternalError(inner) => inner.to_string(),
        other => other.to_string(),
    };
    text.split("\nstack traceback:")
        .next()
        .unwrap_or("")
        .to_string()
}

/// A Lua argument table as named arguments: nothing, or a table of names.
fn to_args(lua: &Lua, args: Option<mlua::Value>) -> mlua::Result<CommandArgs> {
    let Some(args) = args else {
        return Ok(CommandArgs::new());
    };
    if args.is_nil() {
        return Ok(CommandArgs::new());
    }
    match lua.from_value::<serde_json::Value>(args)? {
        serde_json::Value::Object(mut map) => {
            map.values_mut().for_each(empty_tables_as_lists);
            Ok(map)
        }
        serde_json::Value::Array(list) if list.is_empty() => Ok(CommandArgs::new()),
        _ => Err(mlua::Error::runtime(
            "a command takes a table of named arguments, like {length = 10}",
        )),
    }
}

/// An empty Lua table cannot say whether it is a list or a table of
/// names; inside a command's arguments it is taken as the empty list, the
/// one a script means when it writes `{}` for a list of items. A command
/// reading a table of names takes an empty list as none given.
fn empty_tables_as_lists(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) if map.is_empty() => {
            *value = serde_json::Value::Array(Vec::new());
        }
        serde_json::Value::Object(map) => map.values_mut().for_each(empty_tables_as_lists),
        serde_json::Value::Array(items) => items.iter_mut().for_each(empty_tables_as_lists),
        _ => {}
    }
}

/// `app.commands`: every command whose id starts with `prefix`.
fn list_commands(commands: Vec<CommandSpec>, args: &CommandArgs) -> serde_json::Value {
    let prefix = args.get("prefix").and_then(|v| v.as_str()).unwrap_or("");
    serde_json::Value::Array(
        commands
            .iter()
            .filter(|c| c.id.starts_with(prefix))
            .map(CommandSpec::to_json)
            .collect(),
    )
}

/// How many commands `help` lists for a word.
const HELP_HITS: usize = 10;

/// The commands that match `args.query` best, best first, as `help`
/// lists them.
fn search_commands(commands: Vec<CommandSpec>, args: &CommandArgs) -> serde_json::Value {
    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
    let catalog = agents::discovery::Catalog::new(commands.iter().map(command_entry).collect());
    let found = catalog.search(&[query], HELP_HITS);
    serde_json::Value::Array(
        found[0]
            .hits
            .iter()
            .filter_map(|h| commands.iter().find(|c| c.id == h.entry.id))
            .map(CommandSpec::to_json)
            .collect(),
    )
}
