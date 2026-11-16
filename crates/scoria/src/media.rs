//! Audio and video files: `![[x.mp3]]` and `![[x.mp4]]` in a note, and the
//! same files opened in a tab (which is the embed alone, the way an image's
//! tab is).
//!
//! **Audio** is a player drawn after Chromium's `<audio controls>`, which is
//! what Obsidian shows (observed 2026-11-15 and 2026-11-16,
//! [[Specification#Observed canvas embeds, audio and video, Obsidian 1.15.0]]
//! and [[Specification#Observed audio player extras, Obsidian 1.15.1]]):
//! play and pause, the time and the length, a seek bar, the volume button
//! with its slider, and the overflow (⋮) button with its one row, Playback
//! speed, and that row's eight speeds. `.internal-embed.media-embed.audio-embed`
//! is the note's full width, the control 42 px tall and 6 px of line under
//! it; a tab's `.audio-container` is the same at the top of the view, 12 px
//! in; `|300` changes nothing.
//!
//! - **The volume slider** opens while the pointer is over the volume
//!   button (or the slider beside it, or while it is dragged) and closes as
//!   soon as it leaves: 52 px to the button's left, its thumb 12 px wide, so
//!   a press 6 px in is 0 and 6 px from the right end is 1. Moving it
//!   unmutes; at 0 the button shows muted though the player is not.
//! - **The overflow menu** stands over the ⋮ button, its bottom right 4 px
//!   in from the button's, 200 px wide: Playback speed, then Options (back)
//!   over 0.25, 0.5, 0.75, Normal, 1.25, 1.5, 1.75 and 2, the current one
//!   checked, 250 px tall at most. A pick closes it; Escape or a press
//!   anywhere else closes it too. The speed keeps the pitch
//!   (`scoria_audio::stretch`).
//! - **Space** plays and pauses once the player has the focus, which a click
//!   on its play button, its time or its background gives it — not before
//!   (the reading view and Live Preview leave Space to the page and the
//!   editor). With the seek bar focused, the arrows step 1 % of the length
//!   and Space does nothing; with the volume slider focused, the arrows
//!   step 1 % of the volume, Home is 0 and End is 1.
//! - **One player per embed.** The decoding and the output are
//!   `scoria_audio::player`'s; this module keeps them in an app-wide
//!   registry ([`Players`]) keyed by the view drawing the note, the note,
//!   the file and the embed's ordinal among the note's embeds of that file
//!   — so two `![[tone.mp3]]` are two players, a note redrawn sixty times a
//!   second keeps its own, and a tab left for another keeps playing, as
//!   Obsidian's do. No device is opened until something is played, and a
//!   paused player not drawn for a minute lets its device go, keeping its
//!   place.
//!
//! **Video** Scoria cannot decode (see
//! [[Development#Canvas embeds, the audio player and video, 2026-11-15]]):
//! where Obsidian has `<video controls preload="metadata">` at the file's
//! own size, Scoria draws a card with the name, the picture size, the
//! length, the codecs and the file size read from the container
//! (`scoria_audio::video`), and "Open in default app".
//!
//! Differences, all Scoria's: the controls are drawn in the theme's colours
//! rather than Chromium's own grey; a playing embed scrolled far out of view
//! keeps playing (Obsidian's reading view and Live Preview drop the element,
//! which pauses it).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::prelude::FluentBuilder;
use gpui::{
    anchored, canvas, deferred, div, point, px, Anchor, AnyElement, App, Bounds, Element,
    ElementId, EntityId, FocusHandle, FontWeight, Global, GlobalElementId, InspectorElementId,
    InteractiveElement, IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement, Pixels, SharedString, StatefulInteractiveElement, Styled, Window,
};
use scoria_audio::player::{clock, probe, Info, Player};
use scoria_audio::video::{codec_name, VideoInfo};

use crate::assets::icon;
use crate::reading::model::{EmbedKind, RBlock, Segment};
use crate::theme::Theme;

/// An audio or video file embedded in a note: where it is, nothing read.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaEmbed {
    /// Unique within one model, for element ids.
    pub id: usize,
    /// The vault path.
    pub path: String,
    pub absolute: PathBuf,
    /// The note the embed is written in.
    pub owner: String,
    /// Which of that note's embeds of this file it is, from 0.
    pub nth: usize,
}

/// The control's height, and the line left under it.
pub const PLAYER_HEIGHT: f32 = 42.0;
pub const DESCENT: f32 = 6.0;

/// The volume slider's width when open, and its thumb's.
pub const SLIDER_WIDTH: f32 = 52.0;
pub const THUMB: f32 = 12.0;

/// The overflow menu's width, row height and tallest list.
const MENU_WIDTH: f32 = 200.0;
const MENU_ROW: f32 = 48.0;
const MENU_MAX: f32 = 250.0;

/// The speeds Chromium's Playback speed list offers, as it labels them.
pub const SPEEDS: [(f64, &str); 8] = [
    (0.25, "0.25"),
    (0.5, "0.5"),
    (0.75, "0.75"),
    (1.0, "Normal"),
    (1.25, "1.25"),
    (1.5, "1.5"),
    (1.75, "1.75"),
    (2.0, "2"),
];

/// A paused player not drawn for this long closes its output device.
const IDLE: Duration = Duration::from_secs(60);

/// The player's key context: its bindings sit beneath the editor's and the
/// reading view's, so they are the ones a focused player's keys reach.
const KEY_CONTEXT: &str = "AudioPlayer";

gpui::actions!(
    audio_player,
    [Space, Left, Right, Up, Down, Home, End, Escape]
);

/// The player's keys; called once at startup.
pub fn bind_keys(cx: &mut App) {
    let ctx = Some(KEY_CONTEXT);
    cx.bind_keys([
        gpui::KeyBinding::new("space", Space, ctx),
        gpui::KeyBinding::new("left", Left, ctx),
        gpui::KeyBinding::new("right", Right, ctx),
        gpui::KeyBinding::new("up", Up, ctx),
        gpui::KeyBinding::new("down", Down, ctx),
        gpui::KeyBinding::new("home", Home, ctx),
        gpui::KeyBinding::new("end", End, ctx),
        gpui::KeyBinding::new("escape", Escape, ctx),
    ]);
}

/// Whether a Live Preview widget is a paragraph that is one audio embed,
/// which stands on its line with no paragraph gap under it.
pub fn is_audio_widget(block: &RBlock) -> bool {
    matches!(block, RBlock::Paragraph(segments)
        if segments.len() == 1
            && matches!(&segments[0], Segment::Embed(e) if matches!(e.kind, EmbedKind::Audio(_))))
}

// ---- what the controls do, without a window -------------------------------

/// The slider's value under the pointer at `x`, for a slider at `left`
/// `width` wide: its thumb's centre runs from 6 px in to 6 px from the end.
pub fn slider_value(x: f32, left: f32, width: f32) -> f32 {
    let run = width - THUMB;
    if run <= 0.0 {
        return 0.0;
    }
    ((x - left - THUMB / 2.0) / run).clamp(0.0, 1.0)
}

/// The volume button's icon: muted, or at volume 0, is the struck speaker.
pub fn volume_icon(volume: f32, muted: bool) -> &'static str {
    if muted || volume <= 0.0 {
        "volume-x"
    } else {
        "volume-2"
    }
}

/// What the slider shows: 0 while muted, else the volume.
pub fn slider_shows(volume: f32, muted: bool) -> f32 {
    if muted {
        0.0
    } else {
        volume
    }
}

/// The part of a player that has the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Part {
    /// The play button, the time or the background: Space plays and pauses.
    #[default]
    Button,
    /// The seek bar: the arrows step through the file.
    Timeline,
    /// The volume slider: the arrows step the volume.
    Volume,
}

/// What a key does to a focused player.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyAction {
    Toggle,
    /// Seek by this fraction of the length.
    SeekBy(f64),
    VolumeBy(f32),
    VolumeTo(f32),
    CloseMenu,
}

/// The key `key` (gpui's name) pressed with `part` focused; `None` leaves it
/// to whatever holds the player.
pub fn key_action(part: Part, key: &str, menu_open: bool) -> Option<KeyAction> {
    if menu_open && key == "escape" {
        return Some(KeyAction::CloseMenu);
    }
    match (part, key) {
        (Part::Button, "space") => Some(KeyAction::Toggle),
        (Part::Timeline, "right" | "up") => Some(KeyAction::SeekBy(0.01)),
        (Part::Timeline, "left" | "down") => Some(KeyAction::SeekBy(-0.01)),
        (Part::Volume, "right" | "up") => Some(KeyAction::VolumeBy(0.01)),
        (Part::Volume, "left" | "down") => Some(KeyAction::VolumeBy(-0.01)),
        (Part::Volume, "home") => Some(KeyAction::VolumeTo(0.0)),
        (Part::Volume, "end") => Some(KeyAction::VolumeTo(1.0)),
        _ => None,
    }
}

/// The overflow menu's page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Menu {
    #[default]
    Closed,
    Options,
    Speed,
}

// ---- the registry ---------------------------------------------------------

#[derive(Clone)]
enum Probe {
    Loading,
    Audio(Info),
    Video(VideoInfo, u64),
    Failed(String),
}

/// One embed's player: the view drawing it, the note, the file (both
/// lowercased) and the ordinal.
pub type Key = (Option<EntityId>, String, String, usize);

pub fn key_of(embed: &MediaEmbed, host: Option<EntityId>) -> Key {
    (
        host,
        embed.owner.to_lowercase(),
        embed.path.to_lowercase(),
        embed.nth,
    )
}

/// The settings a player carries: what `<audio>` keeps on its element.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    pub volume: f32,
    pub muted: bool,
    pub rate: f64,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            volume: 1.0,
            muted: false,
            rate: 1.0,
        }
    }
}

impl Settings {
    /// A volume from the slider or its keys: moving it unmutes.
    pub fn slide_to(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        self.muted = false;
    }
}

struct Slot {
    absolute: PathBuf,
    player: Option<Player>,
    error: Option<String>,
    settings: Settings,
    /// Where a player that was let go of stood; the next one starts there.
    resume: f64,
    focus: Option<FocusHandle>,
    part: Part,
    menu: Menu,
    volume_hover: bool,
    volume_drag: bool,
    /// Where the ⋮ button was last painted.
    overflow: Option<Bounds<Pixels>>,
    drawn: Instant,
}

impl Slot {
    fn new(absolute: PathBuf) -> Slot {
        Slot {
            absolute,
            player: None,
            error: None,
            settings: Settings::default(),
            resume: 0.0,
            focus: None,
            part: Part::Button,
            menu: Menu::Closed,
            volume_hover: false,
            volume_drag: false,
            overflow: None,
            drawn: Instant::now(),
        }
    }

    /// The player, opened (at the place a let-go one stood, with the
    /// settings) if it was not.
    fn player(&mut self) -> Option<&Player> {
        if self.player.is_none() {
            match Player::open(&self.absolute) {
                Ok(p) => {
                    let e = p.engine();
                    e.set_volume(self.settings.volume);
                    e.set_muted(self.settings.muted);
                    e.set_rate(self.settings.rate);
                    if self.resume > 0.0 {
                        e.seek(self.resume);
                    }
                    self.player = Some(p);
                    self.error = None;
                }
                Err(e) => {
                    self.error = Some(e);
                    return None;
                }
            }
        }
        self.player.as_ref()
    }

    fn playing(&self) -> bool {
        self.player
            .as_ref()
            .is_some_and(|p| p.engine().is_playing())
    }

    fn apply(&self) {
        if let Some(p) = &self.player {
            let e = p.engine();
            e.set_volume(self.settings.volume);
            e.set_muted(self.settings.muted);
            e.set_rate(self.settings.rate);
        }
    }
}

#[derive(Default)]
struct Inner {
    probes: HashMap<PathBuf, Probe>,
    slots: HashMap<Key, Slot>,
    /// A timer is redrawing the windows while something plays.
    ticking: bool,
}

/// Every audio and video file the app has drawn: what its header says and,
/// for each audio embed, its player and its settings.
#[derive(Clone, Default)]
pub struct Players {
    inner: Rc<RefCell<Inner>>,
}

impl Global for Players {}

/// Every audio embed's player as the control socket reports it.
pub fn describe(cx: &App) -> Vec<serde_json::Value> {
    cx.try_global::<Players>()
        .map(Players::describe)
        .unwrap_or_default()
}

/// The app's registry, created on first use.
pub fn players(cx: &mut App) -> Players {
    cx.default_global::<Players>().clone()
}

/// What the controls show for one embed.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioState {
    pub loading: bool,
    pub playing: bool,
    pub position: f64,
    pub duration: Option<f64>,
    pub settings: Settings,
    pub menu: Menu,
    pub volume_open: bool,
    pub error: Option<String>,
}

impl Players {
    /// The file's header, read on the background executor the first time it
    /// is asked for; `Loading` until it lands, when the windows are redrawn.
    fn probe(&self, absolute: &Path, video: bool, cx: &mut App) -> Probe {
        if let Some(p) = self.inner.borrow().probes.get(absolute) {
            return p.clone();
        }
        self.inner
            .borrow_mut()
            .probes
            .insert(absolute.to_path_buf(), Probe::Loading);
        let this = self.clone();
        let path = absolute.to_path_buf();
        cx.spawn(async move |cx| {
            let read = path.clone();
            let got = cx
                .background_executor()
                .spawn(async move {
                    if video {
                        let size = std::fs::metadata(&read).map(|m| m.len()).unwrap_or(0);
                        match scoria_audio::video::probe(&read) {
                            Ok(v) => Probe::Video(v, size),
                            // A container Scoria cannot read still gets a
                            // card, with what the file system knows.
                            Err(_) => Probe::Video(VideoInfo::default(), size),
                        }
                    } else {
                        match probe(&read) {
                            Ok(i) => Probe::Audio(i),
                            Err(e) => Probe::Failed(e),
                        }
                    }
                })
                .await;
            if let Some(p) = this.inner.borrow_mut().probes.get_mut(&path) {
                *p = got;
            }
            cx.update(|cx| cx.refresh_windows());
        })
        .detach();
        Probe::Loading
    }

    /// Runs `f` on the embed's slot, made if it is new.
    fn with<R>(&self, key: &Key, absolute: &Path, f: impl FnOnce(&mut Slot) -> R) -> R {
        let mut inner = self.inner.borrow_mut();
        let slot = inner
            .slots
            .entry(key.clone())
            .or_insert_with(|| Slot::new(absolute.to_path_buf()));
        f(slot)
    }

    pub fn audio_state(&self, key: &Key, absolute: &Path, cx: &mut App) -> AudioState {
        let probe = self.probe(absolute, false, cx);
        self.let_go_of_idle(key);
        let (duration, loading, probe_error) = match &probe {
            Probe::Audio(i) => (i.duration, false, None),
            Probe::Failed(e) => (None, false, Some(e.clone())),
            _ => (None, true, None),
        };
        self.with(key, absolute, |slot| {
            slot.drawn = Instant::now();
            let mut state = AudioState {
                loading,
                playing: false,
                position: slot.resume,
                duration,
                settings: slot.settings,
                menu: slot.menu,
                volume_open: slot.volume_hover || slot.volume_drag,
                error: probe_error,
            };
            if let Some(p) = &slot.player {
                let e = p.engine();
                state.playing = e.is_playing();
                state.position = e.position();
                state.duration = e.duration().or(duration);
                if state.error.is_none() {
                    state.error = e.error();
                }
            }
            if state.error.is_none() {
                state.error = slot.error.clone();
            }
            state
        })
    }

    /// Closes the output of every paused player not drawn for a while,
    /// keeping its place for the next play.
    fn let_go_of_idle(&self, except: &Key) {
        let mut inner = self.inner.borrow_mut();
        for (key, slot) in inner.slots.iter_mut() {
            if key == except || slot.player.is_none() || slot.playing() {
                continue;
            }
            if slot.drawn.elapsed() >= IDLE {
                let p = slot.player.take().expect("checked");
                slot.resume = if p.engine().ended() {
                    0.0
                } else {
                    p.engine().position()
                };
            }
        }
    }

    /// Play or pause. The first play opens the file and the output device.
    pub fn toggle(&self, key: &Key, absolute: &Path, cx: &mut App) {
        self.with(key, absolute, |slot| {
            if let Some(p) = slot.player() {
                if p.engine().is_playing() {
                    p.engine().pause();
                } else {
                    p.engine().play();
                }
            }
        });
        self.tick(cx);
        cx.refresh_windows();
    }

    /// Moves to `fraction` of the way through. Before the first play this
    /// opens the player, paused, so the position has somewhere to live.
    pub fn seek(&self, key: &Key, absolute: &Path, fraction: f64, cx: &mut App) {
        let duration = match self.inner.borrow().probes.get(absolute) {
            Some(Probe::Audio(i)) => i.duration,
            _ => None,
        };
        self.with(key, absolute, |slot| {
            if let (Some(p), Some(d)) = (slot.player(), duration) {
                p.engine().seek(fraction.clamp(0.0, 1.0) * d);
            }
        });
        cx.refresh_windows();
    }

    /// Seeks by `by` of the length from where playback is.
    fn seek_by(&self, key: &Key, absolute: &Path, by: f64, cx: &mut App) {
        let duration = match self.inner.borrow().probes.get(absolute) {
            Some(Probe::Audio(i)) => i.duration,
            _ => None,
        };
        self.with(key, absolute, |slot| {
            if let (Some(p), Some(d)) = (slot.player(), duration) {
                let e = p.engine();
                e.seek(e.position() + by * d);
            }
        });
        cx.refresh_windows();
    }

    /// Changes the settings and hands them to the player, if it is open.
    pub fn update(&self, key: &Key, absolute: &Path, f: impl FnOnce(&mut Settings), cx: &mut App) {
        self.with(key, absolute, |slot| {
            f(&mut slot.settings);
            slot.apply();
        });
        cx.refresh_windows();
    }

    fn set_menu(&self, key: &Key, absolute: &Path, menu: Menu, cx: &mut App) {
        self.with(key, absolute, |slot| slot.menu = menu);
        cx.refresh_windows();
    }

    /// Forgets what was read from a file that changed, and every player of
    /// it: what they were playing is not the file now.
    pub fn invalidate(&self, absolute: &Path) {
        let mut inner = self.inner.borrow_mut();
        inner.probes.remove(absolute);
        inner.slots.retain(|_, s| s.absolute != absolute);
    }

    fn any_playing(&self) -> bool {
        self.inner.borrow().slots.values().any(Slot::playing)
    }

    /// Each embed's player and settings, for the control socket.
    pub fn describe(&self) -> Vec<serde_json::Value> {
        let inner = self.inner.borrow();
        let mut out: Vec<_> = inner
            .slots
            .iter()
            .map(|(key, slot)| {
                let (playing, position) = match &slot.player {
                    Some(p) => (p.engine().is_playing(), p.engine().position()),
                    None => (false, slot.resume),
                };
                serde_json::json!({
                    "note": key.1,
                    "file": key.2,
                    "nth": key.3,
                    "hosted": key.0.is_some(),
                    "open": slot.player.is_some(),
                    "playing": playing,
                    "position": (position * 1000.0).round() / 1000.0,
                    "volume": slot.settings.volume,
                    "muted": slot.settings.muted,
                    "rate": slot.settings.rate,
                    "menu": format!("{:?}", slot.menu),
                    "volume_open": slot.volume_hover || slot.volume_drag,
                    "focus": format!("{:?}", slot.part),
                })
            })
            .collect();
        out.sort_by(|a, b| {
            (a["note"].as_str(), a["file"].as_str(), a["nth"].as_u64()).cmp(&(
                b["note"].as_str(),
                b["file"].as_str(),
                b["nth"].as_u64(),
            ))
        });
        out
    }

    /// Redraws the windows four times a second while anything plays, so the
    /// time and the bar move, and once more when the last one stops.
    fn tick(&self, cx: &mut App) {
        if self.inner.borrow().ticking {
            return;
        }
        self.inner.borrow_mut().ticking = true;
        let this = self.clone();
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let playing = this.any_playing();
                cx.update(|cx| cx.refresh_windows());
                if !playing {
                    break;
                }
            }
            this.inner.borrow_mut().ticking = false;
        })
        .detach();
    }
}

// ---- drawing --------------------------------------------------------------

/// An element whose content is built where gpui hands it an `App`: the
/// reading renderer builds embeds without one, and the controls show the
/// registry's state.
type Build = Box<dyn FnOnce(&mut Window, &mut App) -> AnyElement>;

struct Deferred {
    id: ElementId,
    build: Option<Build>,
    child: Option<AnyElement>,
}

impl IntoElement for Deferred {
    type Element = Deferred;
    fn into_element(self) -> Deferred {
        self
    }
}

impl Element for Deferred {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let build = self.build.take().expect("laid out once");
        let mut child = build(window, cx);
        let layout = child.request_layout(window, cx);
        self.child = Some(child);
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(c) = self.child.as_mut() {
            c.prepaint(window, cx);
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(c) = self.child.as_mut() {
            c.paint(window, cx);
        }
    }
}

/// The audio player for `embed`, as wide as it is given. `host` is the view
/// drawing the note, which with the embed names its player.
pub fn audio(embed: &MediaEmbed, host: Option<EntityId>, theme: &Theme) -> AnyElement {
    let embed = embed.clone();
    let theme = theme.clone();
    Deferred {
        id: ElementId::from(("audio-embed", embed.id)),
        build: Some(Box::new(move |window, cx| {
            audio_controls(&embed, host, &theme, window, cx)
        })),
        child: None,
    }
    .into_any_element()
}

/// Everything a control's handler needs to reach its player.
#[derive(Clone)]
struct Target {
    reg: Players,
    key: Key,
    path: PathBuf,
    focus: FocusHandle,
}

impl Target {
    /// Gives the player the keyboard, with `part` the part that has it.
    fn focus(&self, part: Part, window: &mut Window, cx: &mut App) {
        self.reg.with(&self.key, &self.path, |s| s.part = part);
        window.focus(&self.focus, cx);
    }

    fn slot<R>(&self, f: impl FnOnce(&mut Slot) -> R) -> R {
        self.reg.with(&self.key, &self.path, f)
    }
}

fn audio_controls(
    embed: &MediaEmbed,
    host: Option<EntityId>,
    t: &Theme,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let reg = players(cx);
    let key = key_of(embed, host);
    let state = reg.audio_state(&key, &embed.absolute, cx);
    let focus = reg.with(&key, &embed.absolute, |s| s.focus.clone());
    let focus = match focus {
        Some(f) => f,
        None => {
            let f = cx.focus_handle();
            reg.with(&key, &embed.absolute, |s| s.focus = Some(f.clone()));
            f
        }
    };
    let focused = focus.is_focused(window);
    let target = Target {
        reg: reg.clone(),
        key: key.clone(),
        path: embed.absolute.clone(),
        focus: focus.clone(),
    };
    let id = embed.id;
    let ground = t.tokens.background_secondary;
    let border = t.tokens.background_modifier_border;
    let fg = t.tokens.text_normal;
    let muted_fg = t.tokens.text_muted;
    let hover = t.tokens.background_modifier_hover;
    let button = |name: &'static str, key: &'static str| {
        div()
            .id((key, id))
            .flex_none()
            .w(px(32.))
            .h(px(32.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(16.))
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .child(icon(name, 16., fg))
    };
    let duration = state.duration.unwrap_or(0.0);
    let time = format!("{} / {}", clock(state.position), clock(duration));
    let fraction = if duration > 0.0 {
        (state.position / duration).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };

    let play = {
        let target = target.clone();
        button(if state.playing { "pause" } else { "play" }, "audio-play").on_click(
            move |_, window, cx| {
                cx.stop_propagation();
                target.focus(Part::Button, window, cx);
                target.reg.toggle(&target.key, &target.path, cx);
            },
        )
    };

    // A 4 px track filled to `value`, laid on a row `h` tall; where it was
    // painted goes to `bounds` so a press can be turned into a value.
    let track = |value: f32, bounds: Rc<Cell<Option<Bounds<Pixels>>>>| {
        div()
            .relative()
            .w_full()
            .h(px(4.))
            .rounded(px(2.))
            .bg(muted_fg.opacity(0.5))
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .h_full()
                    .w(gpui::relative(value))
                    .rounded(px(2.))
                    .bg(fg),
            )
            .child(
                canvas(move |b, _, _| bounds.set(Some(b)), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
    };

    // The seek bar.
    let track_bounds: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::new(Cell::new(None));
    let seek_to = {
        let target = target.clone();
        let tb = track_bounds.clone();
        move |x: Pixels, cx: &mut App| {
            if let Some(b) = tb.get() {
                let w = f32::from(b.size.width);
                if w > 0.0 {
                    let f = (f32::from(x - b.origin.x) / w) as f64;
                    target.reg.seek(&target.key, &target.path, f, cx);
                }
            }
        }
    };
    let seek_down = seek_to.clone();
    let bar = {
        let target = target.clone();
        div()
            .id(("audio-seek", id))
            .flex_1()
            .min_w(px(0.))
            .h(px(32.))
            .flex()
            .items_center()
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                target.focus(Part::Timeline, window, cx);
                seek_down(e.position.x, cx);
            })
            .on_mouse_move(move |e: &MouseMoveEvent, _, cx| {
                if e.pressed_button == Some(MouseButton::Left) {
                    seek_to(e.position.x, cx);
                }
            })
            .child(track(fraction, track_bounds))
    };

    // The volume button and, while the pointer is over either, its slider.
    let volume = {
        let s = state.settings;
        let mute = {
            let target = target.clone();
            button(volume_icon(s.volume, s.muted), "audio-mute").on_click(move |_, window, cx| {
                cx.stop_propagation();
                target.focus(Part::Button, window, cx);
                target
                    .reg
                    .update(&target.key, &target.path, |s| s.muted = !s.muted, cx);
            })
        };
        let slider_bounds: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::new(Cell::new(None));
        let slide = {
            let target = target.clone();
            let sb = slider_bounds.clone();
            move |x: Pixels, cx: &mut App| {
                // The track's own bounds, widened by the thumb's half on
                // each side: the slider is the track plus its run-out.
                if let Some(b) = sb.get() {
                    let v =
                        slider_value(f32::from(x), f32::from(b.origin.x), f32::from(b.size.width));
                    target
                        .reg
                        .update(&target.key, &target.path, |s| s.slide_to(v), cx);
                }
            }
        };
        let slide_down = slide.clone();
        let slider = {
            let target = target.clone();
            let target_up = target.clone();
            let sb = slider_bounds.clone();
            div()
                .id(("audio-volume", id))
                .relative()
                .flex_none()
                .w(px(SLIDER_WIDTH))
                .h(px(32.))
                .flex()
                .items_center()
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    target.focus(Part::Volume, window, cx);
                    target.slot(|s| s.volume_drag = true);
                    slide_down(e.position.x, cx);
                })
                .on_mouse_move(move |e: &MouseMoveEvent, _, cx| {
                    if e.pressed_button == Some(MouseButton::Left) {
                        slide(e.position.x, cx);
                    }
                })
                .on_mouse_up_out(MouseButton::Left, move |_: &MouseUpEvent, _, cx| {
                    target_up.slot(|s| s.volume_drag = false);
                    cx.refresh_windows();
                })
                .child(div().w_full().px(px(THUMB / 2.)).child(track(
                    slider_shows(s.volume, s.muted),
                    Rc::new(Cell::new(None)),
                )))
                // The thumb: its centre runs from 6 px in to 6 px from the end.
                .child(
                    div()
                        .absolute()
                        .left(px(slider_shows(s.volume, s.muted) * (SLIDER_WIDTH - THUMB)))
                        .top(px((32. - THUMB) / 2.))
                        .size(px(THUMB))
                        .rounded_full()
                        .bg(fg),
                )
                .child(
                    canvas(move |b, _, _| sb.set(Some(b)), |_, _, _, _| {})
                        .absolute()
                        .size_full(),
                )
        };
        let target_hover = target.clone();
        let target_up = target.clone();
        div()
            .id(("audio-volume-control", id))
            .relative()
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .on_hover(move |hovered: &bool, _, cx| {
                let h = *hovered;
                target_hover.slot(|s| s.volume_hover = h);
                cx.refresh_windows();
            })
            .on_mouse_up(MouseButton::Left, move |_: &MouseUpEvent, _, cx| {
                target_up.slot(|s| s.volume_drag = false);
                cx.refresh_windows();
            })
            .when(state.volume_open, |d| d.pl(px(16.)).child(slider))
            .child(mute)
    };

    // The ⋮ button and its menu.
    let overflow = {
        let target = target.clone();
        let target_paint = target.clone();
        let menu = state.menu;
        button("ellipsis-vertical", "audio-overflow")
            .relative()
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                target.focus(Part::Button, window, cx);
                let next = if menu == Menu::Closed {
                    Menu::Options
                } else {
                    Menu::Closed
                };
                target.reg.set_menu(&target.key, &target.path, next, cx);
            })
            .child(
                canvas(
                    move |b, _, _| target_paint.slot(|s| s.overflow = Some(b)),
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
    };
    let menu = (state.menu != Menu::Closed)
        .then(|| {
            let at = target.slot(|s| s.overflow)?;
            Some(overflow_menu(
                &target,
                state.menu,
                state.settings.rate,
                at,
                t,
                id,
            ))
        })
        .flatten();

    let mut row = div()
        .id(("audio-player", id))
        .track_focus(&focus)
        .w_full()
        .h(px(PLAYER_HEIGHT))
        .px(px(8.))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.))
        .bg(ground)
        .border_1()
        .border_color(if focused {
            t.tokens.background_modifier_border_focus
        } else {
            border
        })
        .rounded(px(4.))
        .text_size(px(13.))
        .text_color(fg);
    {
        // A press on the background or the time gives the player the
        // keyboard, as a click on Chromium's control does.
        let target = target.clone();
        row = row.on_mouse_down(MouseButton::Left, move |_, window, cx| {
            cx.stop_propagation();
            target.focus(Part::Button, window, cx);
            cx.refresh_windows();
        });
    }
    {
        let target = target.clone();
        // The keys are actions in the player's own context, so they win
        // over the editor's bindings for the same keys (the arrows, Home,
        // End) that a press inside the editor would otherwise reach.
        let on = |key: &'static str| {
            let target = target.clone();
            move |window: &mut Window, cx: &mut App| on_key(&target, key, window, cx)
        };
        let (space, left, right, up, down, home, end, escape) = (
            on("space"),
            on("left"),
            on("right"),
            on("up"),
            on("down"),
            on("home"),
            on("end"),
            on("escape"),
        );
        row = row
            .key_context(KEY_CONTEXT)
            .on_action(move |_: &Space, w, cx| space(w, cx))
            .on_action(move |_: &Left, w, cx| left(w, cx))
            .on_action(move |_: &Right, w, cx| right(w, cx))
            .on_action(move |_: &Up, w, cx| up(w, cx))
            .on_action(move |_: &Down, w, cx| down(w, cx))
            .on_action(move |_: &Home, w, cx| home(w, cx))
            .on_action(move |_: &End, w, cx| end(w, cx))
            .on_action(move |_: &Escape, w, cx| escape(w, cx));
    }
    match &state.error {
        Some(err) => {
            row = row
                .child(
                    div()
                        .flex_none()
                        .w(px(32.))
                        .flex()
                        .justify_center()
                        .child(icon("triangle-alert", 16., muted_fg)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_color(muted_fg)
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(SharedString::from(err.clone())),
                );
        }
        None => {
            row = row
                .child(play)
                .child(
                    div()
                        .flex_none()
                        .px(px(4.))
                        .whitespace_nowrap()
                        .child(SharedString::from(time)),
                )
                .child(bar)
                .child(volume)
                .child(overflow);
        }
    }
    div()
        .w_full()
        .pb(px(DESCENT))
        .child(row)
        .children(menu)
        .into_any_element()
}

/// A key pressed while the player has the focus, by the part that has it.
fn on_key(target: &Target, key: &str, _: &mut Window, cx: &mut App) {
    let (menu_open, part) = target.slot(|s| (s.menu != Menu::Closed, s.part));
    let Some(action) = key_action(part, key, menu_open) else {
        // Escape with no menu open is the window's (a modal, a dock); the
        // other keys stop here, so the editor around the player does not
        // move its caret under a player that has the keyboard.
        if key == "escape" {
            cx.propagate();
        }
        return;
    };
    let (reg, k, p) = (&target.reg, &target.key, &target.path);
    match action {
        KeyAction::Toggle => reg.toggle(k, p, cx),
        KeyAction::SeekBy(by) => reg.seek_by(k, p, by, cx),
        KeyAction::VolumeBy(by) => reg.update(k, p, |s| s.slide_to(s.volume + by), cx),
        KeyAction::VolumeTo(v) => reg.update(k, p, |s| s.slide_to(v), cx),
        KeyAction::CloseMenu => reg.set_menu(k, p, Menu::Closed, cx),
    }
}

/// The overflow menu, its bottom right 4 px in from the ⋮ button's: the
/// Playback speed row, or the speeds under Options.
fn overflow_menu(
    target: &Target,
    menu: Menu,
    rate: f64,
    button: Bounds<Pixels>,
    t: &Theme,
    id: usize,
) -> AnyElement {
    let at = point(
        button.origin.x + button.size.width - px(4.),
        button.origin.y + button.size.height - px(4.),
    );
    let fg = t.tokens.text_normal;
    let muted_fg = t.tokens.text_muted;
    let hover = t.tokens.background_modifier_hover;
    let row = |key: &'static str, ix: usize| {
        div()
            .id((key, id * 16 + ix))
            .flex_none()
            .h(px(MENU_ROW))
            .px(px(16.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
    };
    let close = {
        let target = target.clone();
        move |_: &MouseDownEvent, _: &mut Window, cx: &mut App| {
            target
                .reg
                .set_menu(&target.key, &target.path, Menu::Closed, cx);
        }
    };
    let body = match menu {
        Menu::Speed => {
            let back = {
                let target = target.clone();
                row("audio-speed-back", 0)
                    .border_b_1()
                    .border_color(t.tokens.background_modifier_border)
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        target
                            .reg
                            .set_menu(&target.key, &target.path, Menu::Options, cx);
                    })
                    .child(icon("chevron-left", 16., muted_fg))
                    .child("Options")
            };
            let rows = SPEEDS.iter().enumerate().map(|(i, &(speed, label))| {
                let target = target.clone();
                row("audio-speed", i + 1)
                    .justify_between()
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        target
                            .reg
                            .update(&target.key, &target.path, |s| s.rate = speed, cx);
                        target
                            .reg
                            .set_menu(&target.key, &target.path, Menu::Closed, cx);
                    })
                    .child(label)
                    .when(speed == rate, |r| {
                        r.child(icon("check", 16., t.tokens.interactive_accent))
                    })
            });
            div()
                .id(("audio-speed-list", id))
                .max_h(px(MENU_MAX))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .child(back)
                .children(rows)
                .into_any_element()
        }
        _ => {
            let target = target.clone();
            row("audio-speed-row", 0)
                .h(px(46.))
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    target
                        .reg
                        .set_menu(&target.key, &target.path, Menu::Speed, cx);
                })
                .child(icon("circle-play", 20., muted_fg))
                .child("Playback speed")
                .into_any_element()
        }
    };
    deferred(
        anchored()
            .position(at)
            .anchor(Anchor::BottomRight)
            .snap_to_window_with_margin(px(8.))
            .child(
                div()
                    .id(("audio-menu", id))
                    .w(px(MENU_WIDTH))
                    .py(px(4.))
                    .bg(t.tokens.menu_background)
                    .border_1()
                    .border_color(t.tokens.menu_border_color)
                    .rounded(px(6.))
                    .shadow_lg()
                    .text_size(px(13.))
                    .text_color(fg)
                    .occlude()
                    .on_mouse_down_out(close)
                    .child(body),
            ),
    )
    .with_priority(1)
    .into_any_element()
}

/// The card Scoria draws for a video, with "Open in default app".
pub fn video(embed: &MediaEmbed, theme: &Theme) -> AnyElement {
    let embed = embed.clone();
    let theme = theme.clone();
    Deferred {
        id: ElementId::from(("video-embed", embed.id)),
        build: Some(Box::new(move |_, cx| video_card(&embed, &theme, cx))),
        child: None,
    }
    .into_any_element()
}

/// `45 KB`, `1.2 MB`: the size the way a file manager writes it.
pub fn file_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let mut v = bytes as f64 / 1000.0;
    let mut unit = 0;
    while v >= 1000.0 && unit < UNITS.len() - 1 {
        v /= 1000.0;
        unit += 1;
    }
    if v < 10.0 {
        format!("{v:.1} {}", UNITS[unit])
    } else {
        format!("{v:.0} {}", UNITS[unit])
    }
}

/// The card's second line: picture size, length, codecs and file size, each
/// left out when unknown.
pub fn video_facts(info: &VideoInfo, bytes: u64) -> String {
    let mut parts = Vec::new();
    if let (Some(w), Some(h)) = (info.width, info.height) {
        parts.push(format!("{w} × {h}"));
    }
    if let Some(d) = info.duration {
        parts.push(clock(d.round()));
    }
    match (&info.video_codec, &info.audio_codec) {
        (Some(v), Some(a)) => parts.push(format!("{} / {}", codec_name(v), codec_name(a))),
        (Some(v), None) => parts.push(codec_name(v)),
        (None, Some(a)) => parts.push(codec_name(a)),
        (None, None) => {}
    }
    if bytes > 0 {
        parts.push(file_size(bytes));
    }
    parts.join(" · ")
}

fn video_card(embed: &MediaEmbed, t: &Theme, cx: &mut App) -> AnyElement {
    let reg = players(cx);
    let probe = reg.probe(&embed.absolute, true, cx);
    let name = embed
        .path
        .rsplit('/')
        .next()
        .unwrap_or(&embed.path)
        .to_string();
    let facts = match &probe {
        Probe::Video(info, bytes) => video_facts(info, *bytes),
        _ => String::new(),
    };
    let absolute = embed.absolute.clone();
    div()
        .w_full()
        .pb(px(DESCENT))
        .child(
            div()
                .w_full()
                .p(px(12.))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .bg(t.tokens.background_secondary)
                .border_1()
                .border_color(t.tokens.background_modifier_border)
                .rounded(px(4.))
                .child(
                    div()
                        .flex_none()
                        .child(icon("film", 24., t.tokens.text_muted)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(t.tokens.text_normal)
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .child(SharedString::from(name)),
                        )
                        .child(
                            div()
                                .text_size(px(13.))
                                .text_color(t.tokens.text_muted)
                                .child(SharedString::from(if facts.is_empty() {
                                    "Video".to_string()
                                } else {
                                    facts
                                })),
                        ),
                )
                .child(
                    div()
                        .id(("video-open", embed.id))
                        .flex_none()
                        .px(px(12.))
                        .py(px(6.))
                        .rounded(px(4.))
                        .text_size(px(13.))
                        .bg(t.tokens.interactive_normal)
                        .hover(|s| s.bg(t.tokens.interactive_hover))
                        .cursor_pointer()
                        .text_color(t.tokens.text_normal)
                        .child("Open in default app")
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            cx.open_with_system(&absolute);
                        }),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_volume_slider_runs_between_its_thumbs_half_widths() {
        // The probe's two presses on the 52 px slider at x 1461: a quarter
        // and three quarters of the way along read 0.175 and 0.825.
        assert!((slider_value(1461.0 + 13.0, 1461.0, SLIDER_WIDTH) - 0.175).abs() < 1e-6);
        assert!((slider_value(1461.0 + 39.0, 1461.0, SLIDER_WIDTH) - 0.825).abs() < 1e-6);
        assert_eq!(slider_value(1461.0, 1461.0, SLIDER_WIDTH), 0.0);
        assert_eq!(slider_value(1600.0, 1461.0, SLIDER_WIDTH), 1.0);
        assert_eq!(slider_value(10.0, 0.0, 0.0), 0.0);
    }

    #[test]
    fn mute_and_the_slider_behave_as_chromiums() {
        let mut s = Settings::default();
        assert_eq!((s.volume, s.muted, s.rate), (1.0, false, 1.0));
        // Muted: the slider shows 0 and the button the struck speaker, the
        // volume kept underneath.
        s.muted = true;
        assert_eq!(slider_shows(s.volume, s.muted), 0.0);
        assert_eq!(volume_icon(s.volume, s.muted), "volume-x");
        // Moving the slider unmutes.
        s.slide_to(0.5);
        assert_eq!((s.volume, s.muted), (0.5, false));
        assert_eq!(volume_icon(s.volume, s.muted), "volume-2");
        // At 0 the button shows muted though the player is not.
        s.slide_to(0.0);
        assert_eq!((s.volume, s.muted), (0.0, false));
        assert_eq!(volume_icon(s.volume, s.muted), "volume-x");
        s.slide_to(7.0);
        assert_eq!(s.volume, 1.0);
    }

    #[test]
    fn keys_go_by_the_part_that_has_the_focus() {
        use KeyAction::*;
        // Space plays and pauses from the button, not from the seek bar.
        assert_eq!(key_action(Part::Button, "space", false), Some(Toggle));
        assert_eq!(key_action(Part::Timeline, "space", false), None);
        // The arrows on the seek bar step 1 % of the length (0.6 s of 60).
        assert_eq!(
            key_action(Part::Timeline, "right", false),
            Some(SeekBy(0.01))
        );
        assert_eq!(
            key_action(Part::Timeline, "left", false),
            Some(SeekBy(-0.01))
        );
        // On the button they do nothing, as the 2026-11-15 probe saw.
        assert_eq!(key_action(Part::Button, "right", false), None);
        assert_eq!(key_action(Part::Button, "k", false), None);
        assert_eq!(key_action(Part::Button, "m", false), None);
        // The volume slider: 1 % a step, Home and End its ends.
        assert_eq!(key_action(Part::Volume, "up", false), Some(VolumeBy(0.01)));
        assert_eq!(
            key_action(Part::Volume, "down", false),
            Some(VolumeBy(-0.01))
        );
        assert_eq!(key_action(Part::Volume, "home", false), Some(VolumeTo(0.0)));
        assert_eq!(key_action(Part::Volume, "end", false), Some(VolumeTo(1.0)));
        // Escape closes an open menu and is left alone otherwise.
        assert_eq!(key_action(Part::Button, "escape", true), Some(CloseMenu));
        assert_eq!(key_action(Part::Button, "escape", false), None);
    }

    #[test]
    fn the_speed_list_is_chromiums() {
        let labels: Vec<_> = SPEEDS.iter().map(|s| s.1).collect();
        assert_eq!(
            labels,
            ["0.25", "0.5", "0.75", "Normal", "1.25", "1.5", "1.75", "2"]
        );
        assert_eq!(SPEEDS[3].0, 1.0);
    }

    #[test]
    fn two_embeds_of_one_file_are_two_players() {
        let embed = |owner: &str, nth| MediaEmbed {
            id: 1,
            path: "Audio/Tone.mp3".into(),
            absolute: PathBuf::from("/v/Audio/Tone.mp3"),
            owner: owner.into(),
            nth,
        };
        let a = key_of(&embed("Note.md", 0), None);
        let b = key_of(&embed("Note.md", 1), None);
        assert_ne!(a, b);
        // The same embed is the same player however it is spelled...
        assert_eq!(a, key_of(&embed("note.MD", 0), None));
        // ...and another note's is not.
        assert_ne!(a, key_of(&embed("Other.md", 0), None));
        let reg = Players::default();
        let file = Path::new("/v/Audio/Tone.mp3");
        reg.with(&a, file, |s| s.settings.rate = 1.5);
        assert_eq!(reg.with(&b, file, |s| s.settings.rate), 1.0);
        assert_eq!(reg.with(&a, file, |s| s.settings.rate), 1.5);
        assert_eq!(reg.inner.borrow().slots.len(), 2);
        // A change to the file forgets both.
        reg.invalidate(file);
        assert!(reg.inner.borrow().slots.is_empty());
    }

    #[test]
    fn sizes_read_like_a_file_managers() {
        assert_eq!(file_size(512), "512 B");
        assert_eq!(file_size(45_659), "46 KB");
        assert_eq!(file_size(2_424), "2.4 KB");
        assert_eq!(file_size(1_234_567), "1.2 MB");
        assert_eq!(file_size(3_400_000_000), "3.4 GB");
    }

    #[test]
    fn the_video_card_says_what_the_container_does() {
        let info = VideoInfo {
            width: Some(320),
            height: Some(240),
            duration: Some(3.008),
            video_codec: Some("V_VP9".into()),
            audio_codec: Some("A_OPUS".into()),
        };
        assert_eq!(
            video_facts(&info, 52_928),
            "320 × 240 · 0:03 · VP9 / Opus · 53 KB"
        );
        assert_eq!(video_facts(&VideoInfo::default(), 0), "");
        let silent = VideoInfo {
            video_codec: Some("avc1".into()),
            ..Default::default()
        };
        assert_eq!(video_facts(&silent, 900), "H.264 · 900 B");
    }
}
