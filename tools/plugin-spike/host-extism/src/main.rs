//! The extism route: `spike-host-extism <guest.wasm>`.
//!
//! The same steps as `host-wit`: compile, instantiate, register the commands,
//! time the check callback, run both commands over a tempdir vault. Every
//! call crosses as JSON, serialised and parsed inside the timed loop because
//! the palette would do that per keystroke.

#[path = "../../common/spike.rs"]
mod spike;

use extism::convert::Json;
use extism::{host_fn, CompiledPlugin, Manifest, Plugin, PluginBuilder, UserData, Wasm, PTR};
use serde::{Deserialize, Serialize};
use std::time::Instant;

struct HostState {
    vault: spike::Vault,
    notices: Vec<String>,
}

host_fn!(vault_read(user_data: HostState; path: String) -> String {
    let state = user_data.get()?;
    let state = state.lock().unwrap();
    state.vault.read(&path).map_err(extism::Error::msg)
});

host_fn!(vault_modify(user_data: HostState; path: String, text: String) {
    let state = user_data.get()?;
    let state = state.lock().unwrap();
    state.vault.modify(&path, &text).map_err(extism::Error::msg)
});

host_fn!(notice(user_data: HostState; message: String) {
    let state = user_data.get()?;
    state.lock().unwrap().notices.push(message);
    Ok(())
});

#[derive(Deserialize)]
struct Command {
    id: String,
    name: String,
    has_check: bool,
}

#[derive(Serialize)]
struct Call<'a> {
    id: &'a str,
    active_path: Option<&'a str>,
}

fn main() -> Result<(), extism::Error> {
    let guest = std::env::args()
        .nth(1)
        .expect("usage: spike-host-extism <guest.wasm>");
    println!(
        "tier={}",
        if cfg!(feature = "default") {
            "extism-default"
        } else {
            "extism-minimal"
        }
    );
    let bytes = std::fs::read(&guest)?;

    let state = UserData::new(HostState {
        vault: spike::Vault::new("extism"),
        notices: Vec::new(),
    });
    // Extism turns on wasmtime's disk cache by default (in ~/.cache/wasmtime);
    // the spike times a cold compile unless SPIKE_EXTISM_CACHE=1 keeps it.
    let cache = std::env::var_os("SPIKE_EXTISM_CACHE").is_some();
    println!("cache={}", if cache { "system" } else { "off" });
    let builder = || {
        let b = PluginBuilder::new(Manifest::new([Wasm::data(bytes.clone())])).with_wasi(false);
        let b = if cache { b } else { b.with_cache_disabled() };
        b.with_function("vault_read", [PTR], [PTR], state.clone(), vault_read)
            .with_function("vault_modify", [PTR, PTR], [], state.clone(), vault_modify)
            .with_function("notice", [PTR], [], state.clone(), notice)
    };

    let t = Instant::now();
    let compiled = CompiledPlugin::new(builder())?;
    spike::report("compile", t.elapsed());

    let t = Instant::now();
    let mut plugin = Plugin::new_from_compiled(&compiled)?;
    spike::report("instantiate_first", t.elapsed());

    {
        let mut plugins = Vec::new();
        spike::sample("instantiate", 200, || {
            plugins.push(Plugin::new_from_compiled(&compiled).unwrap())
        });
    }

    let t = Instant::now();
    let Json(commands) = plugin.call::<&str, Json<Vec<Command>>>("commands", "")?;
    spike::report("commands", t.elapsed());
    for c in &commands {
        println!(
            "command=spike:{} name={:?} check={}",
            c.id, c.name, c.has_check
        );
    }

    let active = Some(spike::NOTE);
    let checked = commands
        .iter()
        .find(|c| c.has_check)
        .expect("a command with a check");
    let check = |plugin: &mut Plugin, path: Option<&str>| -> bool {
        let Json(ok) = plugin
            .call::<Json<Call>, Json<bool>>(
                "check",
                Json(Call {
                    id: &checked.id,
                    active_path: path,
                }),
            )
            .unwrap();
        ok
    };
    assert!(check(&mut plugin, active));
    assert!(!check(&mut plugin, Some("x.canvas")));
    spike::sample("check", 100_000, || {
        check(&mut plugin, active);
    });

    for c in &commands {
        let t = Instant::now();
        plugin.call::<Json<Call>, ()>(
            "run",
            Json(Call {
                id: &c.id,
                active_path: active,
            }),
        )?;
        spike::report(&format!("run_{}", c.id), t.elapsed());
    }
    let state = state.get()?;
    let state = state.lock().unwrap();
    for n in &state.notices {
        println!("notice={n:?}");
    }
    state.vault.check_appended();
    println!("ok");
    Ok(())
}
