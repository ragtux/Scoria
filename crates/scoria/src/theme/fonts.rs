//! The three font fields of `appearance.json` and the families the machine
//! has to fill them with.
//!
//! Obsidian's Interface, Text and Monospace font rows each open a page that
//! builds a *list* of families by picking: one pick writes the family, the
//! next appends it after a bare `,`, the drag handle reorders, and the
//! Delete button takes one out, so `interfaceFontFamily` reads
//! `Segoe UI,Arial,Courier New` — no space after the comma and no quotes,
//! even round a name with a space in it. Removing the last one writes `""`.
//! The page says "The first font from this list that is available on your
//! system will be applied", and that is what [`first_installed`] does
//! (probed 2026-11-04, `tools/obsidian-probe/font-pages.mjs`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The families of a font field, in its order. A hand-written CSS list is
/// read too: each entry is trimmed and loses one pair of surrounding
/// quotes, and empty entries are dropped.
pub fn font_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|f| {
            let f = f.trim();
            let unquoted = f
                .strip_prefix('"')
                .and_then(|f| f.strip_suffix('"'))
                .or_else(|| f.strip_prefix('\'').and_then(|f| f.strip_suffix('\'')))
                .unwrap_or(f);
            unquoted.trim().to_string()
        })
        .filter(|f| !f.is_empty())
        .collect()
}

/// The field's value for `list`, as Obsidian writes it: joined by a bare
/// `,`, nothing quoted.
pub fn join_fonts(list: &[String]) -> String {
    list.join(",")
}

/// What the font row on the Appearance tab reads before its chevron:
/// nothing for an empty list, the family for one, "*N* fonts" for more.
pub fn summary(list: &[String]) -> String {
    match list {
        [] => String::new(),
        [one] => one.clone(),
        many => format!("{} fonts", many.len()),
    }
}

/// The families the machine has, sorted, read once from gpui's text
/// system at startup by [`set_installed`].
static INSTALLED: OnceLock<Installed> = OnceLock::new();

struct Installed {
    sorted: Vec<String>,
    lower: HashSet<String>,
}

/// Records the families the font pages list and [`first_installed`] tests
/// against. Only the first call counts.
pub fn set_installed(families: Vec<String>) {
    let lower = families.iter().map(|f| f.to_lowercase()).collect();
    INSTALLED.get_or_init(|| Installed {
        sorted: families,
        lower,
    });
}

/// The installed families, sorted; empty before [`set_installed`].
pub fn installed() -> &'static [String] {
    INSTALLED.get().map(|i| i.sorted.as_slice()).unwrap_or(&[])
}

/// Whether `family` is installed, compared without case. `true` before
/// [`set_installed`], so nothing is flagged as missing when nothing is
/// known.
pub fn is_installed(family: &str) -> bool {
    INSTALLED
        .get()
        .is_none_or(|i| i.lower.contains(&family.to_lowercase()))
}

/// The family a font field applies: the first of its list the machine
/// has, `None` when it has none of them (or the list is empty), which
/// leaves the theme's default.
pub fn first_installed(value: Option<&str>) -> Option<String> {
    first_of(&font_list(value?), is_installed)
}

fn first_of(list: &[String], has: impl Fn(&str) -> bool) -> Option<String> {
    list.iter().find(|f| has(f)).cloned()
}

/// The families gpui reports, cut down to what a person can pick: no
/// hidden `.`-prefixed names (gpui's `.SystemUIFont`, `.ZedSans`), no
/// duplicates, sorted without case the way Obsidian's list is, and
/// `drop` filtering whatever else the caller knows is not a real choice.
pub fn pickable(names: Vec<String>, drop: impl Fn(&str) -> bool) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out: Vec<String> = names
        .into_iter()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty() && !n.starts_with('.') && !drop(n))
        .filter(|n| seen.insert(n.to_lowercase()))
        .collect();
    out.sort_by_key(|n| n.to_lowercase());
    out
}

/// The sizes the font size steps between: the Font size slider's own
/// range. Ctrl+wheel and the two commands stopped at 30 going up and 10
/// going down, one pixel a step (probed 2026-11-04 and 2026-11-16).
pub const QUICK_MIN: f32 = 10.;
pub const QUICK_MAX: f32 = 30.;
/// Wheel pixels per step for a touchpad's fine-grained deltas: a delta of
/// 10 alone did not move Obsidian's size, a notch of 120 moved it one.
const QUICK_PIXEL_STEP: f32 = 50.;

/// The key of the vault's window file the text size is kept under.
///
/// Obsidian 1.15.0 keeps the size in the browser's local storage as
/// `<vault id>-base-font-size`, a bare number string, and nowhere in the
/// vault: the two font-size commands, Ctrl+wheel, the Font size slider and
/// its Restore default all write that key and leave `appearance.json`
/// alone. The first read with no key copies the file's `baseFontSize` into
/// it, and from then on the key alone decides: an `appearance.json`
/// rewritten from outside with another size changed nothing, and a
/// relaunch drew the key's size over the file's
/// (`tools/obsidian-probe/font-size-backlinks-block.mjs`, 2026-11-16).
/// Local storage belongs to the data directory, outside the vault, so
/// Scoria keeps the size beside the window zoom in its own
/// `window/<vault id>.json`, under the local storage key less its vault id.
pub const STORE_KEY: &str = "base-font-size";

type SizeCache = std::sync::Mutex<std::collections::HashMap<PathBuf, Option<f32>>>;

/// The sizes read from or written to the window files this run, by vault
/// root, so the Font size slider can read its value on every frame without
/// touching the disk. `None` is a file with no size in it.
fn cache() -> &'static SizeCache {
    static CACHE: OnceLock<SizeCache> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// The size recorded in the window file at `path`: a number, or the
/// number string local storage holds.
fn read_stored(path: &Path) -> Option<f32> {
    let bytes = std::fs::read(path).ok()?;
    let json = serde_json::from_slice::<serde_json::Value>(&bytes).ok()?;
    let value = json.get(STORE_KEY)?;
    let size = value
        .as_f64()
        .or_else(|| value.as_str().and_then(|s| s.trim().parse::<f64>().ok()))?;
    // Only a zero or a negative is kept out of the layout: Obsidian copies
    // the file's value in as it is.
    (size.is_finite() && size >= 1.).then_some(size as f32)
}

/// Records `size` in the window file at `path`, keeping its other keys
/// (the zoom). Whole sizes are written as integers.
fn write_stored(path: &Path, size: f32) {
    let mut json = std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}));
    json[STORE_KEY] = if size.fract() == 0.0 {
        serde_json::json!(size as i64)
    } else {
        serde_json::json!(size as f64)
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, json.to_string());
}

/// The size kept for the vault at `root`, if one is.
pub fn stored_size(root: &Path) -> Option<f32> {
    let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(size) = cache.get(root) {
        return *size;
    }
    let size = crate::zoom::window_file(root).and_then(|p| read_stored(&p));
    cache.insert(root.to_path_buf(), size);
    size
}

/// Keeps `size` for the vault at `root`. A failure to write costs only the
/// next launch's size.
pub fn save_size(root: &Path, size: f32) {
    if let Some(path) = crate::zoom::window_file(root) {
        write_stored(&path, size);
    }
    cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(root.to_path_buf(), Some(size));
}

/// The size the vault at `root` draws at: the kept one, else the file's
/// `baseFontSize`, else 16. With `seed`, a file's size is copied into the
/// window file the first time it is read, the way Obsidian's first read
/// copies it into local storage, after which the file no longer decides.
pub fn vault_font_size(root: &Path, file_size: Option<u32>, seed: bool) -> f32 {
    let (size, copy) = resolve_size(stored_size(root), file_size);
    if seed && copy {
        save_size(root, size);
    }
    size
}

/// The size drawn from what is kept and the file's `baseFontSize`, and
/// whether the file's size is to be copied in: the kept size wins, a file
/// with no key leaves 16 and nothing to copy.
fn resolve_size(stored: Option<f32>, file_size: Option<u32>) -> (f32, bool) {
    match (stored, file_size) {
        (Some(size), _) => (size, false),
        (None, Some(size)) => (size.max(1) as f32, true),
        (None, None) => (16., false),
    }
}

/// The vault the font size is kept for, and the touchpad pixels not yet
/// worth a step.
#[derive(Debug, Default, Clone)]
pub struct QuickFontSize {
    /// The open vault's root, set when its theme is loaded.
    root: Option<PathBuf>,
    /// Touchpad pixels not yet worth a step.
    pending: f32,
}

impl gpui::Global for QuickFontSize {}

impl QuickFontSize {
    /// Steps for a wheel event: a mouse notch (lines) is one step its way,
    /// a touchpad's pixels add up until they are worth one. Positive is
    /// larger: gpui's delta is positive for a wheel turned away from the
    /// reader, the way that scrolls toward the top.
    fn steps(&mut self, delta: gpui::ScrollDelta) -> i32 {
        match delta {
            gpui::ScrollDelta::Lines(p) if p.y != 0. => p.y.signum() as i32,
            gpui::ScrollDelta::Lines(_) => 0,
            gpui::ScrollDelta::Pixels(p) => {
                self.pending += f32::from(p.y);
                let steps = (self.pending / QUICK_PIXEL_STEP).trunc();
                self.pending -= steps * QUICK_PIXEL_STEP;
                steps as i32
            }
        }
    }
}

/// `steps` pixels on from `current`, held between the clamps.
fn stepped(current: f32, steps: i32) -> f32 {
    (current + steps as f32).clamp(QUICK_MIN, QUICK_MAX)
}

/// Sets the text size to `size` and keeps it for the open vault: what each
/// of the two commands, Ctrl+wheel and the Font size slider does.
pub fn set_font_size(size: f32, cx: &mut gpui::App) {
    cx.global_mut::<super::Theme>().base_font_size = size;
    if let Some(root) = cx
        .try_global::<QuickFontSize>()
        .and_then(|q| q.root.clone())
    {
        save_size(&root, size);
    }
}

/// Ctrl+wheel over a note with "Quick font size adjustment" on: steps the
/// theme's size and says so, so the caller stops the wheel from scrolling
/// too. `false` (nothing done) when the setting is off or Ctrl is not held.
pub fn quick_wheel(event: &gpui::ScrollWheelEvent, cx: &mut gpui::App) -> bool {
    let theme = cx.global::<super::Theme>();
    if !theme.quick_font_size || !event.modifiers.control {
        return false;
    }
    let current = theme.base_font_size;
    let mut quick = cx
        .try_global::<QuickFontSize>()
        .cloned()
        .unwrap_or_default();
    let steps = quick.steps(event.delta);
    cx.set_global(quick);
    if steps != 0 {
        set_font_size(stepped(current, steps), cx);
    }
    true
}

/// `app:increase-font-size` (`steps` 1) and `app:decrease-font-size` (-1):
/// the text size one pixel up or down between 10 and 30, the clamps and
/// the step Obsidian 1.15.0 showed, with or without "Quick font size
/// adjustment" on, and kept across launches (see [`STORE_KEY`]).
pub fn step_font_size(steps: i32, cx: &mut gpui::App) {
    let current = cx.global::<super::Theme>().base_font_size;
    set_font_size(stepped(current, steps), cx);
}

/// A layer over a note's view that takes Ctrl+wheel for the font size when
/// "Quick font size adjustment" is on. It listens in the capture phase so
/// the wheel never reaches the scrolling underneath, and lets every other
/// wheel through untouched. The parent must be `relative`.
pub fn quick_size_layer() -> impl gpui::IntoElement {
    use gpui::Styled;
    gpui::canvas(
        |bounds, window, _| window.insert_hitbox(bounds, gpui::HitboxBehavior::Normal),
        |_, hitbox, window, _| {
            window.on_mouse_event(move |event: &gpui::ScrollWheelEvent, phase, window, cx| {
                if phase == gpui::DispatchPhase::Capture
                    && hitbox.should_handle_scroll(window)
                    && quick_wheel(event, cx)
                {
                    cx.stop_propagation();
                    window.refresh();
                }
            });
        },
    )
    .absolute()
    .inset_0()
}

/// Puts the size kept for the vault at `root` on `theme`, just rebuilt
/// from `appearance.json` whose `baseFontSize` is `file_size`, seeding the
/// window file from the file on the first read; and remembers the vault,
/// so a later step is kept for it.
pub fn keep_vault_size(
    theme: &mut super::Theme,
    root: &Path,
    file_size: Option<u32>,
    cx: &mut gpui::App,
) {
    theme.base_font_size = vault_font_size(root, file_size, true);
    let mut quick = cx
        .try_global::<QuickFontSize>()
        .cloned()
        .unwrap_or_default();
    quick.root = Some(root.to_path_buf());
    cx.set_global(quick);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_notch_is_one_step_and_pixels_add_up() {
        let mut q = QuickFontSize::default();
        let lines = |y: f32| gpui::ScrollDelta::Lines(gpui::point(0., y));
        let pixels = |y: f32| gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.), gpui::px(y)));
        assert_eq!(q.steps(lines(3.)), 1);
        assert_eq!(q.steps(lines(-1.)), -1);
        assert_eq!(q.steps(lines(0.)), 0);
        assert_eq!(q.steps(pixels(10.)), 0);
        assert_eq!(q.steps(pixels(30.)), 0);
        assert_eq!(q.steps(pixels(15.)), 1);
        assert_eq!(q.steps(pixels(-120.)), -2);
    }

    /// The probe's runs: 16 to 17 on the first Increase, 30 reached on the
    /// fourteenth and held there, 10 reached from 30 on the twentieth.
    #[test]
    fn the_font_size_commands_step_one_pixel_between_10_and_30() {
        let mut size = 16.;
        size = stepped(size, 1);
        assert_eq!(size, 17.);
        for _ in 0..13 {
            size = stepped(size, 1);
        }
        assert_eq!(size, 30.);
        for _ in 0..11 {
            size = stepped(size, 1);
        }
        assert_eq!(size, 30., "held at the top");
        for run in 1..=25 {
            size = stepped(size, -1);
            if run == 20 {
                assert_eq!(size, 10.);
            }
        }
        assert_eq!(size, 10., "held at the bottom");
    }

    /// The 2026-11-16 run: with `baseFontSize: 20` in the file and no key
    /// the boot drew 20 and copied it in; once kept, a file rewritten with
    /// 24 changed nothing and a relaunch drew the kept 18; a file with no
    /// size and nothing kept is 16, with nothing to copy.
    #[test]
    fn the_kept_size_wins_over_the_file_once_the_file_is_copied_in() {
        assert_eq!(resolve_size(None, Some(20)), (20., true));
        assert_eq!(resolve_size(Some(16.), Some(24)), (16., false));
        assert_eq!(resolve_size(Some(18.), Some(24)), (18., false));
        assert_eq!(resolve_size(None, None), (16., false));
        // The file's own value is honoured as it is, bar a zero.
        assert_eq!(resolve_size(None, Some(99)), (99., true));
        assert_eq!(resolve_size(None, Some(0)), (1., true));
    }

    /// The size goes in beside the zoom in the window file, which keeps
    /// the zoom; it reads back, from a number and from local storage's
    /// number string, and a file without it reads as nothing kept.
    #[test]
    fn the_size_is_kept_beside_the_zoom_in_the_window_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("window").join("0123456789abcdef.json");
        assert_eq!(read_stored(&path), None);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{"zoom":1}"#).unwrap();
        assert_eq!(read_stored(&path), None);
        write_stored(&path, 17.);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            r#"{"zoom":1,"base-font-size":17}"#
        );
        assert_eq!(read_stored(&path), Some(17.));
        write_stored(&path, 21.);
        assert_eq!(read_stored(&path), Some(21.));
        std::fs::write(&path, r#"{"base-font-size":"19"}"#).unwrap();
        assert_eq!(read_stored(&path), Some(19.));
        std::fs::write(&path, r#"{"base-font-size":0}"#).unwrap();
        assert_eq!(read_stored(&path), None);
    }

    #[test]
    fn a_picked_list_round_trips_as_obsidian_writes_it() {
        let list = font_list("Segoe UI,Arial,Courier New");
        assert_eq!(list, ["Segoe UI", "Arial", "Courier New"]);
        assert_eq!(join_fonts(&list), "Segoe UI,Arial,Courier New");
        assert_eq!(join_fonts(&[]), "");
    }

    #[test]
    fn a_hand_written_css_list_is_read_too() {
        assert_eq!(
            font_list(r#" "Inter", 'Fira Sans' , sans-serif,, "#),
            ["Inter", "Fira Sans", "sans-serif"]
        );
        assert!(font_list("").is_empty());
        assert!(font_list(" , ").is_empty());
    }

    #[test]
    fn the_row_reads_nothing_one_name_or_a_count() {
        assert_eq!(summary(&[]), "");
        assert_eq!(summary(&["Consolas".into()]), "Consolas");
        assert_eq!(
            summary(&["Consolas".into(), "Cascadia Mono".into()]),
            "2 fonts"
        );
    }

    #[test]
    fn the_first_family_the_machine_has_is_the_one_applied() {
        let list = font_list("Probe Sans,Segoe UI,Arial");
        let has = |f: &str| f == "Segoe UI" || f == "Arial";
        assert_eq!(first_of(&list, has).as_deref(), Some("Segoe UI"));
        assert_eq!(first_of(&list, |_| false), None);
        assert_eq!(first_of(&[], |_| true), None);
    }

    #[test]
    fn the_pickable_list_drops_hidden_names_and_sorts_without_case() {
        let names = vec![
            "Verdana".to_string(),
            ".SystemUIFont".into(),
            "arial".into(),
            "Bahnschrift".into(),
            "Arial".into(),
            "Rubik".into(),
        ];
        assert_eq!(
            pickable(names, |n| n == "Rubik"),
            ["arial", "Bahnschrift", "Verdana"]
        );
    }
}
