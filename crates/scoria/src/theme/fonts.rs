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

/// The sizes "Quick font size adjustment" steps between: the Font size
/// slider's own range. Ctrl+wheel stopped at 30 going up and 10 going down,
/// one pixel a notch (probed 2026-11-04).
pub const QUICK_MIN: f32 = 10.;
pub const QUICK_MAX: f32 = 30.;
/// Wheel pixels per step for a touchpad's fine-grained deltas: a delta of
/// 10 alone did not move Obsidian's size, a notch of 120 moved it one.
const QUICK_PIXEL_STEP: f32 = 50.;

/// The size Ctrl+wheel reached this session. Obsidian writes it nowhere —
/// not `baseFontSize`, not local storage, not even on a clean quit — so it
/// lives as long as the process, and only until the file's own
/// `baseFontSize` changes under it (the slider was moved, or the file was
/// written from outside), which is the size the reader asked for last.
#[derive(Debug, Default, Clone, Copy)]
pub struct QuickFontSize {
    /// The size reached, if the wheel has moved it.
    size: Option<f32>,
    /// The file's size when it did.
    from: f32,
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
        .copied()
        .unwrap_or_default();
    let steps = quick.steps(event.delta);
    if steps != 0 {
        let next = quick.step(current, steps);
        cx.global_mut::<super::Theme>().base_font_size = next;
    }
    cx.set_global(quick);
    true
}

impl QuickFontSize {
    /// `steps` pixels on from `current`, held between the clamps, and
    /// remembered as the session's size over the file's.
    fn step(&mut self, current: f32, steps: i32) -> f32 {
        let next = (current + steps as f32).clamp(QUICK_MIN, QUICK_MAX);
        if self.size.is_none() {
            self.from = current;
        }
        self.size = Some(next);
        next
    }
}

/// `app:increase-font-size` (`steps` 1) and `app:decrease-font-size` (-1):
/// the text size one pixel up or down between 10 and 30, the clamps and
/// the step Obsidian 1.15.0 showed, with or without "Quick font size
/// adjustment" on. The size is the same session size Ctrl+wheel moves.
/// Obsidian keeps the commands' size in the browser's local storage under
/// the vault's id, which outlives a relaunch; Scoria has nothing outside
/// the vault to put it in that Obsidian would read, and writing
/// `baseFontSize` would be a byte Obsidian never wrote, so it lasts the
/// session.
pub fn step_font_size(steps: i32, cx: &mut gpui::App) {
    let current = cx.global::<super::Theme>().base_font_size;
    let mut quick = cx
        .try_global::<QuickFontSize>()
        .copied()
        .unwrap_or_default();
    let next = quick.step(current, steps);
    cx.global_mut::<super::Theme>().base_font_size = next;
    cx.set_global(quick);
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

/// Carries the session's Ctrl+wheel size over to `theme`, rebuilt from the
/// file, unless the file's own size has changed since the wheel moved it.
pub fn keep_quick_size(theme: &mut super::Theme, cx: &mut gpui::App) {
    let Some(quick) = cx.try_global::<QuickFontSize>().copied() else {
        return;
    };
    match quick.size {
        Some(size) if quick.from == theme.base_font_size => theme.base_font_size = size,
        Some(_) => cx.set_global(QuickFontSize::default()),
        None => {}
    }
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
        let mut q = QuickFontSize::default();
        let mut size = 16.;
        size = q.step(size, 1);
        assert_eq!(size, 17.);
        for _ in 0..13 {
            size = q.step(size, 1);
        }
        assert_eq!(size, 30.);
        for _ in 0..11 {
            size = q.step(size, 1);
        }
        assert_eq!(size, 30., "held at the top");
        for run in 1..=25 {
            size = q.step(size, -1);
            if run == 20 {
                assert_eq!(size, 10.);
            }
        }
        assert_eq!(size, 10., "held at the bottom");
        // The session's size is over the file's 16, which is what
        // `keep_quick_size` carries across a reload of the theme.
        assert_eq!(q.size, Some(10.));
        assert_eq!(q.from, 16.);
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
