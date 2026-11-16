//! Two commands over the WIT world in `../wit/plugin.wit`:
//! `count-words` has a check callback (a Markdown note is active) and reads the
//! note and posts a notice; `append-line` has none, and reads the note, appends
//! a line through `vault.modify`, and posts a notice.

wit_bindgen::generate!({
    world: "plugin",
    path: "../wit",
});

use scoria::plugin::{vault, workspace};

struct Spike;

impl Guest for Spike {
    fn commands() -> Vec<Command> {
        vec![
            Command {
                id: "count-words".into(),
                name: "Count words".into(),
                has_check: true,
            },
            Command {
                id: "append-line".into(),
                name: "Append a line".into(),
                has_check: false,
            },
        ]
    }

    fn check(id: String, active_path: Option<String>) -> bool {
        match id.as_str() {
            "count-words" => active_path.is_some_and(|p| p.ends_with(".md")),
            "append-line" => true,
            _ => false,
        }
    }

    fn run(id: String, active_path: Option<String>) -> Result<(), String> {
        let path = active_path.ok_or("no active note")?;
        let text = vault::read(&path)?;
        match id.as_str() {
            "count-words" => {
                let words = text.split_whitespace().count();
                workspace::notice(&format!("{path}: {words} words"));
                Ok(())
            }
            "append-line" => {
                let mut next = text;
                if !next.ends_with('\n') {
                    next.push('\n');
                }
                next.push_str("Appended by the spike plugin.\n");
                vault::modify(&path, &next)?;
                workspace::notice(&format!("Appended a line to {path}"));
                Ok(())
            }
            other => Err(format!("unknown command {other}")),
        }
    }
}

export!(Spike);
