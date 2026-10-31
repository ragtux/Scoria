//! `scoriagen` - the command line in front of the generator.
//!
//! Hand-rolled argument parsing, because a dependency-free crate stays
//! dependency-free all the way out to the binary.

use std::path::PathBuf;
use std::time::Instant;

use anya_scoria_gen::body;
use anya_scoria_gen::params::Params;

/// What the command line asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Cli {
    seed: Option<u64>,
    count: u32,
    out: Option<PathBuf>,
    preset: String,
    sets: Vec<(String, String)>,
    stdout: bool,
    list_params: bool,
    time: bool,
    help: bool,
}

impl Default for Cli {
    fn default() -> Self {
        Cli {
            seed: None,
            count: 1,
            // The artist's drawing is the thing this generator aims at, so it
            // is what you get when you ask for nothing.
            preset: "anya".to_string(),
            out: None,
            sets: Vec::new(),
            stdout: false,
            list_params: false,
            time: false,
            help: false,
        }
    }
}

const USAGE: &str = "\
scoriagen - procedurally generated scoria logos

USAGE:
  scoriagen [OPTIONS]

OPTIONS:
  --seed N            Seed for the first logo (default 0)
  --count N           Generate N logos, seeds N..N+count-1 (needs --out DIR)
  --out PATH          File to write (count 1) or directory to fill (count > 1)
  --preset NAME       anya (default), tame, feral
  --set key=value     Override one parameter; repeatable, applied after --preset
  --stdout            Write the document to stdout (the default for count 1)
  --list-params       Print the parameter table and exit
  --time              Print per-stage microseconds to stderr
  --help              This

EXAMPLES:
  scoriagen --seed 7 > rock.svg
  scoriagen --preset feral --set vesicle_count=12 --out rock.svg
  scoriagen --count 500 --out ./batch --time
";

fn main() {
    if let Err(e) = run() {
        eprintln!("scoriagen: {e}");
        std::process::exit(1);
    }
}

/// Parse the command line. Split out so it can be tested.
fn parse(args: &[String]) -> Result<Cli, String> {
    let mut cli = Cli::default();
    let mut i = 0;
    // Both `--flag value` and `--flag=value` are accepted; the second is what
    // people reach for after --set teaches them the habit.
    while i < args.len() {
        let arg = args[i].as_str();
        let (name, inline) = match arg.split_once('=') {
            Some((n, v)) => (n, Some(v.to_string())),
            None => (arg, None),
        };
        let mut next = |what: &str| -> Result<String, String> {
            if let Some(v) = inline.clone() {
                return Ok(v);
            }
            i += 1;
            args.get(i)
                .cloned()
                .ok_or_else(|| format!("{what} needs a value"))
        };
        match name {
            "--seed" => {
                let v = next("--seed")?;
                cli.seed = Some(
                    v.parse::<u64>()
                        .map_err(|_| format!("--seed: `{v}` is not a u64"))?,
                );
            }
            "--count" => {
                let v = next("--count")?;
                cli.count = v
                    .parse::<u32>()
                    .map_err(|_| format!("--count: `{v}` is not a count"))?;
            }
            "--out" => cli.out = Some(PathBuf::from(next("--out")?)),
            "--preset" => cli.preset = next("--preset")?,
            "--set" => {
                let kv = next("--set")?;
                let (k, v) = kv
                    .split_once('=')
                    .ok_or_else(|| format!("--set wants key=value, got `{kv}`"))?;
                cli.sets.push((k.to_string(), v.to_string()));
            }
            "--stdout" => cli.stdout = true,
            "--list-params" => cli.list_params = true,
            "--time" => cli.time = true,
            "--help" | "-h" => cli.help = true,
            other => return Err(format!("unknown option `{other}` (try --help)")),
        }
        i += 1;
    }
    Ok(cli)
}

/// Turn a parsed command line into parameters.
fn params_from(cli: &Cli) -> Result<Params, String> {
    let mut p = Params::preset(&cli.preset).ok_or_else(|| {
        format!(
            "unknown preset `{}` (have {})",
            cli.preset,
            Params::PRESETS.join(", ")
        )
    })?;
    for (k, v) in &cli.sets {
        p.set(k, v)?;
    }
    if let Some(s) = cli.seed {
        p.seed = s;
    }
    // The dial only runs if the caller actually turned it; otherwise the
    // measured defaults stand exactly as they are.
    if p.is_explicit("tameness") {
        p.apply_tameness();
    }
    p.validate()?;
    Ok(p)
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = parse(&args)?;
    if cli.help {
        print!("{USAGE}");
        return Ok(());
    }
    if cli.list_params {
        list_params();
        return Ok(());
    }
    let base = params_from(&cli)?;

    if cli.count <= 1 {
        let (doc, t) = body::generate_timed(&base).map_err(|e| e.to_string())?;
        match (&cli.out, cli.stdout) {
            (Some(path), false) => {
                std::fs::write(path, &doc)
                    .map_err(|e| format!("writing {}: {e}", path.display()))?;
            }
            _ => print!("{doc}"),
        }
        if cli.time {
            eprintln!(
                "seed {} in {} us: field {}, ring {}, facets {}, vesicles {}, gates {}, svg {} ({} attempt(s))",
                base.seed, t.total_us, t.field_us, t.ring_us, t.facet_us, t.vesicle_us,
                t.quality_us, t.svg_us, t.attempts
            );
        }
        return Ok(());
    }

    let dir = cli
        .out
        .clone()
        .ok_or_else(|| "--count above 1 needs --out DIR".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    let start = Instant::now();
    let mut rerolls = 0u64;
    let mut failed = 0u64;
    for k in 0..cli.count as u64 {
        let p = Params {
            seed: base.seed.wrapping_add(k),
            ..base.clone()
        };
        match body::generate_timed(&p) {
            Ok((doc, t)) => {
                rerolls += (t.attempts - 1) as u64;
                let path = dir.join(format!("scoria-{}.svg", p.seed));
                std::fs::write(&path, &doc)
                    .map_err(|e| format!("writing {}: {e}", path.display()))?;
            }
            Err(e) => {
                failed += 1;
                eprintln!("scoriagen: seed {}: {e}", p.seed);
            }
        }
    }
    let us = start.elapsed().as_micros().max(1);
    println!(
        "{} logos in {}.{:03} s ({:.2} ms each, {} re-roll(s), {} failed) -> {}",
        cli.count as u64 - failed,
        us / 1_000_000,
        (us / 1_000) % 1_000,
        us as f64 / 1000.0 / cli.count as f64,
        rerolls,
        failed,
        dir.display()
    );
    if failed > 0 {
        return Err(format!("{failed} seed(s) could not be generated"));
    }
    Ok(())
}

/// Print the parameter table.
fn list_params() {
    let rows = Params::describe();
    let wn = rows.iter().map(|r| r.name.len()).max().unwrap_or(4);
    let wd = rows.iter().map(|r| r.default.len()).max().unwrap_or(7);
    let wr = rows.iter().map(|r| r.range.len()).max().unwrap_or(5);
    println!(
        "{:<wn$}  {:<5}  {:<wd$}  {:<wr$}  effect",
        "name",
        "type",
        "default",
        "range",
        wn = wn,
        wd = wd,
        wr = wr
    );
    for r in rows {
        println!(
            "{:<wn$}  {:<5}  {:<wd$}  {:<wr$}  {}",
            r.name,
            r.kind.as_str(),
            r.default,
            r.range,
            r.effect,
            wn = wn,
            wd = wd,
            wr = wr
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn empty_args_are_the_anya_preset() {
        let cli = parse(&a(&[])).expect("parses");
        assert_eq!(cli.preset, "anya");
        assert_eq!(cli.count, 1);
        assert!(cli.seed.is_none());
    }

    #[test]
    fn both_flag_forms_work() {
        let one = parse(&a(&["--seed", "7", "--set", "aspect=1.9"])).expect("parses");
        let two = parse(&a(&["--seed=7", "--set=aspect=1.9"])).expect("parses");
        assert_eq!(one, two);
        assert_eq!(one.seed, Some(7));
        assert_eq!(one.sets, vec![("aspect".to_string(), "1.9".to_string())]);
    }

    #[test]
    fn sets_are_applied_after_the_preset() {
        let cli = parse(&a(&["--preset", "tame", "--set", "lump_amp=0.31"])).expect("parses");
        let p = params_from(&cli).expect("valid");
        assert_eq!(p.lump_amp, 0.31, "--set must win over the preset");
    }

    #[test]
    fn a_bad_parameter_is_an_error_not_a_panic() {
        let cli = parse(&a(&["--set", "aspect=9"])).expect("parses");
        assert!(params_from(&cli).is_err(), "9 is outside the aspect range");
        let cli = parse(&a(&["--set", "nope=1"])).expect("parses");
        assert!(params_from(&cli).is_err());
        assert!(parse(&a(&["--frobnicate"])).is_err());
        assert!(parse(&a(&["--seed"])).is_err());
    }

    #[test]
    fn tameness_only_runs_when_asked_for() {
        let plain = params_from(&parse(&a(&[])).expect("parses")).expect("valid");
        assert_eq!(plain.lump_amp, Params::default().lump_amp);
        let dialed =
            params_from(&parse(&a(&["--set", "tameness=1.0"])).expect("parses")).expect("valid");
        assert!(
            dialed.lump_amp < plain.lump_amp,
            "the dial must have moved it"
        );
    }

    #[test]
    fn every_preset_name_resolves() {
        for name in Params::PRESETS {
            let cli = parse(&a(&["--preset", name])).expect("parses");
            params_from(&cli).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        let cli = parse(&a(&["--preset", "obsidian"])).expect("parses");
        assert!(params_from(&cli).is_err());
    }
}
