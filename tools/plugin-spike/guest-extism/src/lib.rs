//! The same two commands as `guest-wit`, on extism-pdk: every export takes and
//! returns bytes (JSON here), and the host functions live in extism's
//! `extism:host/user` namespace.

use extism_pdk::*;
use serde::{Deserialize, Serialize};

#[host_fn]
extern "ExtismHost" {
    fn vault_read(path: String) -> String;
    fn vault_modify(path: String, text: String);
    fn notice(message: String);
}

#[derive(Serialize)]
struct Command {
    id: &'static str,
    name: &'static str,
    has_check: bool,
}

#[derive(Deserialize)]
struct Call {
    id: String,
    active_path: Option<String>,
}

#[plugin_fn]
pub fn commands() -> FnResult<Json<Vec<Command>>> {
    Ok(Json(vec![
        Command {
            id: "count-words",
            name: "Count words",
            has_check: true,
        },
        Command {
            id: "append-line",
            name: "Append a line",
            has_check: false,
        },
    ]))
}

#[plugin_fn]
pub fn check(Json(call): Json<Call>) -> FnResult<Json<bool>> {
    let ok = match call.id.as_str() {
        "count-words" => call.active_path.is_some_and(|p| p.ends_with(".md")),
        "append-line" => true,
        _ => false,
    };
    Ok(Json(ok))
}

#[plugin_fn]
pub fn run(Json(call): Json<Call>) -> FnResult<()> {
    let path = call
        .active_path
        .ok_or_else(|| Error::msg("no active note"))?;
    let text = unsafe { vault_read(path.clone())? };
    match call.id.as_str() {
        "count-words" => {
            let words = text.split_whitespace().count();
            unsafe { notice(format!("{path}: {words} words"))? };
            Ok(())
        }
        "append-line" => {
            let mut next = text;
            if !next.ends_with('\n') {
                next.push('\n');
            }
            next.push_str("Appended by the spike plugin.\n");
            unsafe { vault_modify(path.clone(), next)? };
            unsafe { notice(format!("Appended a line to {path}"))? };
            Ok(())
        }
        other => Err(WithReturnCode::new(
            Error::msg(format!("unknown command {other}")),
            1,
        )),
    }
}
