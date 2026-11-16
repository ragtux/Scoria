//! The WIT route: `spike-host-wit <guest.component.wasm | guest.cwasm> [--precompile <out.cwasm>]`.
//!
//! Loads the component, registers its commands, times a check callback the
//! way the palette would call it, and runs both commands over a tempdir vault.
//! The compiler tier is the cargo feature it was built with (see Cargo.toml).

#[path = "../../common/spike.rs"]
mod spike;

use std::rc::Rc;
use std::time::Instant;
use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Config, Engine, Store};

wasmtime::component::bindgen!({ world: "plugin", path: "../wit" });

struct Host {
    vault: Rc<spike::Vault>,
    notices: Vec<String>,
}

impl scoria::plugin::vault::Host for Host {
    fn read(&mut self, path: String) -> Result<String, String> {
        self.vault.read(&path)
    }

    fn modify(&mut self, path: String, text: String) -> Result<(), String> {
        self.vault.modify(&path, &text)
    }
}

impl scoria::plugin::workspace::Host for Host {
    fn notice(&mut self, message: String) {
        self.notices.push(message);
    }
}

impl scoria::plugin::types::Host for Host {}

const TIER: &str = if cfg!(feature = "wasmtime-default") {
    "wasmtime-default"
} else if cfg!(feature = "winch") {
    "winch"
} else if cfg!(feature = "pulley") {
    "pulley"
} else if cfg!(feature = "runtime-pulley") {
    "runtime-pulley"
} else if cfg!(feature = "cranelift-parallel") {
    "cranelift-parallel"
} else if cfg!(feature = "cranelift") {
    "cranelift"
} else {
    "runtime"
};

/// `SPIKE_BUDGET=epoch` or `fuel` compiles the guest with that interruption
/// check, and the check loop re-arms the budget before every call, as a host
/// that bounds each callback would.
fn budget() -> String {
    std::env::var("SPIKE_BUDGET").unwrap_or_else(|_| "none".into())
}

fn arm(store: &mut Store<Host>) {
    match budget().as_str() {
        "epoch" => store.set_epoch_deadline(1),
        "fuel" => store.set_fuel(10_000_000).unwrap(),
        _ => {}
    }
}

fn config() -> wasmtime::Result<Config> {
    let mut config = Config::new();
    match budget().as_str() {
        "epoch" => {
            config.epoch_interruption(true);
        }
        "fuel" => {
            config.consume_fuel(true);
        }
        _ => {}
    }
    #[cfg(feature = "winch")]
    config.strategy(wasmtime::Strategy::Winch);
    if cfg!(any(feature = "pulley", feature = "runtime-pulley")) {
        config.target("pulley64")?;
    }
    Ok(config)
}

fn main() -> wasmtime::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let guest = args
        .get(1)
        .expect("usage: spike-host-wit <guest> [--precompile <out>]");
    println!("tier={TIER} budget={}", budget());

    let t = Instant::now();
    let engine = Engine::new(&config()?)?;
    spike::report("engine", t.elapsed());

    let t = Instant::now();
    let component = if guest.ends_with(".cwasm") {
        // SAFETY: the spike's own precompiled artifact, written by --precompile.
        unsafe { Component::deserialize_file(&engine, guest)? }
    } else {
        compile(&engine, guest)?
    };
    spike::report(
        if guest.ends_with(".cwasm") {
            "deserialize"
        } else {
            "compile"
        },
        t.elapsed(),
    );

    if let Some(i) = args.iter().position(|a| a == "--precompile") {
        let out = &args[i + 1];
        std::fs::write(out, component.serialize()?)?;
        println!("precompiled={out}");
        return Ok(());
    }

    run(&engine, &component)
}

#[cfg(any(
    feature = "wasmtime-default",
    feature = "cranelift",
    feature = "winch",
    feature = "pulley"
))]
fn compile(engine: &Engine, guest: &str) -> wasmtime::Result<Component> {
    Component::from_file(engine, guest)
}

/// A build without a compiler can only load what another build precompiled.
#[cfg(not(any(
    feature = "wasmtime-default",
    feature = "cranelift",
    feature = "winch",
    feature = "pulley"
)))]
fn compile(_: &Engine, guest: &str) -> wasmtime::Result<Component> {
    Err(wasmtime::Error::msg(format!(
        "{guest}: this build has no compiler; pass a .cwasm"
    )))
}

fn run(engine: &Engine, component: &Component) -> wasmtime::Result<()> {
    let t = Instant::now();
    let mut linker = Linker::<Host>::new(engine);
    Plugin::add_to_linker::<Host, HasSelf<Host>>(&mut linker, |h| h)?;
    let pre = PluginPre::new(linker.instantiate_pre(component)?)?;
    spike::report("link", t.elapsed());

    let vault = Rc::new(spike::Vault::new("wit"));
    let new_store = || {
        Store::new(
            engine,
            Host {
                vault: vault.clone(),
                notices: Vec::new(),
            },
        )
    };

    let t = Instant::now();
    let mut store = new_store();
    arm(&mut store);
    let plugin = pre.instantiate(&mut store)?;
    spike::report("instantiate_first", t.elapsed());

    // Instantiation alone, against one store each, as a reload or a second vault would.
    {
        let mut stores = Vec::new();
        spike::sample("instantiate", 200, || {
            let mut s = new_store();
            arm(&mut s);
            pre.instantiate(&mut s).unwrap();
            stores.push(s);
        });
    }

    let t = Instant::now();
    let commands = plugin.call_commands(&mut store)?;
    spike::report("commands", t.elapsed());
    for c in &commands {
        println!(
            "command=spike:{} name={:?} check={}",
            c.id, c.name, c.has_check
        );
    }

    let active = Some(spike::NOTE.to_string());
    let checked = commands
        .iter()
        .find(|c| c.has_check)
        .expect("a command with a check");
    arm(&mut store);
    assert!(plugin.call_check(&mut store, &checked.id, active.as_deref())?);
    assert!(!plugin.call_check(&mut store, &checked.id, Some("x.canvas"))?);
    spike::sample("check", 100_000, || {
        arm(&mut store);
        plugin
            .call_check(&mut store, &checked.id, active.as_deref())
            .unwrap();
    });

    for c in &commands {
        arm(&mut store);
        let t = Instant::now();
        plugin
            .call_run(&mut store, &c.id, active.as_deref())?
            .map_err(wasmtime::Error::msg)?;
        spike::report(&format!("run_{}", c.id), t.elapsed());
    }
    for n in &store.data().notices {
        println!("notice={n:?}");
    }
    store.data().vault.check_appended();
    println!("ok");
    Ok(())
}
