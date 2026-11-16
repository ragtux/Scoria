//! The baseline: the same two commands as the guests, written into the host,
//! over the same tempdir vault and the same timing code. Its stripped release
//! binary is what the runtime hosts are measured against.

#[path = "../../common/spike.rs"]
mod spike;

use std::hint::black_box;

struct Command {
    id: &'static str,
    name: &'static str,
    has_check: bool,
}

const COMMANDS: &[Command] = &[
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
];

fn check(id: &str, active_path: Option<&str>) -> bool {
    match id {
        "count-words" => active_path.is_some_and(|p| p.ends_with(".md")),
        "append-line" => true,
        _ => false,
    }
}

fn run(
    vault: &spike::Vault,
    notices: &mut Vec<String>,
    id: &str,
    path: &str,
) -> Result<(), String> {
    let text = vault.read(path)?;
    match id {
        "count-words" => {
            notices.push(format!("{path}: {} words", text.split_whitespace().count()));
            Ok(())
        }
        "append-line" => {
            let mut next = text;
            if !next.ends_with('\n') {
                next.push('\n');
            }
            next.push_str("Appended by the spike plugin.\n");
            vault.modify(path, &next)?;
            notices.push(format!("Appended a line to {path}"));
            Ok(())
        }
        other => Err(format!("unknown command {other}")),
    }
}

fn main() {
    println!("tier=native");
    let vault = spike::Vault::new("baseline");
    let mut notices = Vec::new();
    for c in COMMANDS {
        println!(
            "command=spike:{} name={:?} check={}",
            c.id, c.name, c.has_check
        );
    }
    spike::sample("check", 100_000, || {
        black_box(check(
            black_box("count-words"),
            black_box(Some(spike::NOTE)),
        ));
    });
    for c in COMMANDS {
        run(&vault, &mut notices, c.id, spike::NOTE).unwrap();
    }
    for n in &notices {
        println!("notice={n:?}");
    }
    vault.check_appended();
    println!("ok");
}
