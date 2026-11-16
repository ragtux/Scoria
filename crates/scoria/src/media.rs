//! Audio and video files: `![[x.mp3]]` and `![[x.mp4]]` in a note, and the
//! same files opened in a tab (which is the embed alone, the way an image's
//! tab is).
//!
//! **Audio** is a player: play and pause, the time and the length, a seek
//! bar, and mute, drawn after Chromium's `<audio controls>`, which is what
//! Obsidian shows (observed 2026-11-15,
//! [[Specification#Observed canvas embeds, audio and video, Obsidian 1.15.0]]:
//! `.internal-embed.media-embed.audio-embed`, the note's full width, the
//! control 42 px tall and 6 px of line under it; a tab's `.audio-container`
//! is the same at the top of the view, 12 px in; `|300` changes nothing;
//! Space on a focused player plays and pauses, and the arrows did nothing).
//! The decoding and the output are `scoria_audio::player`'s; this module
//! keeps one player per file in an app-wide registry ([`Players`]), so a
//! note redrawn sixty times a second, or the same file embedded twice,
//! drives one player. No device is opened until something is played.
//!
//! **Video** Scoria cannot decode (see
//! [[Development#Canvas embeds, the audio player and video, 2026-11-15]]):
//! where Obsidian has `<video controls preload="metadata">` at the file's
//! own size, Scoria draws a card with the name, the picture size, the
//! length, the codecs and the file size read from the container
//! (`scoria_audio::video`), and "Open in default app".
//!
//! Differences, all Scoria's: the overflow menu (Chromium's playback speed)
//! and the volume slider are not drawn, mute is the volume button's click;
//! the keyboard does nothing; two embeds of one file share a player.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    canvas, div, px, AnyElement, App, Bounds, Element, ElementId, FontWeight, Global,
    GlobalElementId, InspectorElementId, InteractiveElement, IntoElement, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, ParentElement, Pixels, SharedString,
    StatefulInteractiveElement, Styled, Window,
};
use scoria_audio::player::{clock, probe, Info, Player};
use scoria_audio::video::{codec_name, VideoInfo};

use crate::assets::icon;
use crate::theme::Theme;

/// An audio or video file embedded in a note: where it is, nothing read.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaEmbed {
    /// Unique within one model, for element ids.
    pub id: usize,
    /// The vault path.
    pub path: String,
    pub absolute: PathBuf,
}

/// The control's height, and the line left under it.
pub const PLAYER_HEIGHT: f32 = 42.0;
pub const DESCENT: f32 = 6.0;

// ---- the registry ---------------------------------------------------------

#[derive(Clone)]
enum Probe {
    Loading,
    Audio(Info),
    Video(VideoInfo, u64),
    Failed(String),
}

struct Slot {
    probe: Probe,
    player: Option<Player>,
    error: Option<String>,
    muted: bool,
}

#[derive(Default)]
struct Inner {
    slots: HashMap<PathBuf, Slot>,
    /// A timer is redrawing the windows while something plays.
    ticking: bool,
}

/// Every audio and video file the app has drawn: what its header says and,
/// for audio that has been played, its player.
#[derive(Clone, Default)]
pub struct Players {
    inner: Rc<RefCell<Inner>>,
}

impl Global for Players {}

/// The app's registry, created on first use.
pub fn players(cx: &mut App) -> Players {
    cx.default_global::<Players>().clone()
}

/// What the controls show for one file.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioState {
    pub loading: bool,
    pub playing: bool,
    pub position: f64,
    pub duration: Option<f64>,
    pub muted: bool,
    pub error: Option<String>,
}

impl Players {
    /// The file's header, read on the background executor the first time it
    /// is asked for; `Loading` until it lands, when the windows are redrawn.
    fn probe(&self, absolute: &Path, video: bool, cx: &mut App) -> Probe {
        if let Some(slot) = self.inner.borrow().slots.get(absolute) {
            return slot.probe.clone();
        }
        self.inner.borrow_mut().slots.insert(
            absolute.to_path_buf(),
            Slot {
                probe: Probe::Loading,
                player: None,
                error: None,
                muted: false,
            },
        );
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
            if let Some(slot) = this.inner.borrow_mut().slots.get_mut(&path) {
                slot.probe = got;
            }
            cx.update(|cx| cx.refresh_windows());
        })
        .detach();
        Probe::Loading
    }

    pub fn audio_state(&self, absolute: &Path, cx: &mut App) -> AudioState {
        let probe = self.probe(absolute, false, cx);
        let inner = self.inner.borrow();
        let slot = inner.slots.get(absolute);
        let (duration, loading, mut error) = match &probe {
            Probe::Audio(i) => (i.duration, false, None),
            Probe::Failed(e) => (None, false, Some(e.clone())),
            _ => (None, true, None),
        };
        let mut state = AudioState {
            loading,
            playing: false,
            position: 0.0,
            duration,
            muted: slot.is_some_and(|s| s.muted),
            error: None,
        };
        if let Some(slot) = slot {
            if let Some(p) = &slot.player {
                let e = p.engine();
                state.playing = e.is_playing();
                state.position = e.position();
                state.duration = e.duration().or(duration);
                if error.is_none() {
                    error = e.error();
                }
            }
            if error.is_none() {
                error = slot.error.clone();
            }
        }
        state.error = error;
        state
    }

    /// Play or pause. The first play opens the file and the output device.
    pub fn toggle(&self, absolute: &Path, cx: &mut App) {
        {
            let mut inner = self.inner.borrow_mut();
            let Some(slot) = inner.slots.get_mut(absolute) else {
                return;
            };
            if slot.player.is_none() {
                match Player::open(absolute) {
                    Ok(p) => {
                        p.engine().set_muted(slot.muted);
                        slot.player = Some(p);
                        slot.error = None;
                    }
                    Err(e) => {
                        slot.error = Some(e);
                        return;
                    }
                }
            }
            if let Some(p) = &slot.player {
                if p.engine().is_playing() {
                    p.engine().pause();
                } else {
                    p.engine().play();
                }
            }
        }
        self.tick(cx);
    }

    /// Moves to `fraction` of the way through. Before the first play this
    /// opens the player, paused, so the position has somewhere to live.
    pub fn seek(&self, absolute: &Path, fraction: f64, cx: &mut App) {
        let mut inner = self.inner.borrow_mut();
        let Some(slot) = inner.slots.get_mut(absolute) else {
            return;
        };
        let duration = match &slot.probe {
            Probe::Audio(i) => i.duration,
            _ => None,
        };
        if slot.player.is_none() {
            match Player::open(absolute) {
                Ok(p) => {
                    p.engine().set_muted(slot.muted);
                    slot.player = Some(p);
                }
                Err(e) => {
                    slot.error = Some(e);
                    return;
                }
            }
        }
        if let (Some(p), Some(d)) = (&slot.player, duration) {
            p.engine().seek(fraction.clamp(0.0, 1.0) * d);
        }
        drop(inner);
        cx.refresh_windows();
    }

    pub fn toggle_mute(&self, absolute: &Path, cx: &mut App) {
        if let Some(slot) = self.inner.borrow_mut().slots.get_mut(absolute) {
            slot.muted = !slot.muted;
            if let Some(p) = &slot.player {
                p.engine().set_muted(slot.muted);
            }
        }
        cx.refresh_windows();
    }

    /// Forgets what was read from a file that changed, and its player.
    pub fn invalidate(&self, absolute: &Path) {
        self.inner.borrow_mut().slots.remove(absolute);
    }

    fn any_playing(&self) -> bool {
        self.inner
            .borrow()
            .slots
            .values()
            .any(|s| s.player.as_ref().is_some_and(|p| p.engine().is_playing()))
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

/// The audio player for `embed`, as wide as it is given.
pub fn audio(embed: &MediaEmbed, theme: &Theme) -> AnyElement {
    let embed = embed.clone();
    let theme = theme.clone();
    Deferred {
        id: ElementId::from(("audio-embed", embed.id)),
        build: Some(Box::new(move |_, cx| audio_controls(&embed, &theme, cx))),
        child: None,
    }
    .into_any_element()
}

fn audio_controls(embed: &MediaEmbed, t: &Theme, cx: &mut App) -> AnyElement {
    let reg = players(cx);
    let state = reg.audio_state(&embed.absolute, cx);
    let id = embed.id;
    let ground = t.tokens.background_secondary;
    let border = t.tokens.background_modifier_border;
    let fg = t.tokens.text_normal;
    let muted_fg = t.tokens.text_muted;
    let button = |name: &'static str, key: &'static str, hover| {
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
    let hover = t.tokens.background_modifier_hover;

    let play = {
        let reg = reg.clone();
        let path = embed.absolute.clone();
        button(
            if state.playing { "pause" } else { "play" },
            "audio-play",
            hover,
        )
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            reg.toggle(&path, cx);
        })
    };
    let mute = {
        let reg = reg.clone();
        let path = embed.absolute.clone();
        button(
            if state.muted { "volume-x" } else { "volume-2" },
            "audio-mute",
            hover,
        )
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            reg.toggle_mute(&path, cx);
        })
    };
    // The seek bar: a 4 px track filled to the position. Where it was
    // painted is kept so a press can be turned into a fraction.
    let track_bounds: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::new(Cell::new(None));
    let seek_to = {
        let reg = reg.clone();
        let path = embed.absolute.clone();
        let tb = track_bounds.clone();
        move |x: Pixels, cx: &mut App| {
            if let Some(b) = tb.get() {
                let w = f32::from(b.size.width);
                if w > 0.0 {
                    let f = (f32::from(x - b.origin.x) / w) as f64;
                    reg.seek(&path, f, cx);
                }
            }
        }
    };
    let seek_down = seek_to.clone();
    let bar = div()
        .id(("audio-seek", id))
        .flex_1()
        .min_w(px(0.))
        .h(px(32.))
        .flex()
        .items_center()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, _, cx| {
            cx.stop_propagation();
            seek_down(e.position.x, cx);
        })
        .on_mouse_move(move |e: &MouseMoveEvent, _, cx| {
            if e.pressed_button == Some(MouseButton::Left) {
                seek_to(e.position.x, cx);
            }
        })
        .child(
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
                        .w(gpui::relative(fraction))
                        .rounded(px(2.))
                        .bg(fg),
                )
                .child({
                    let tb = track_bounds.clone();
                    canvas(move |bounds, _, _| tb.set(Some(bounds)), |_, _, _, _| {})
                        .absolute()
                        .size_full()
                }),
        );
    let mut row = div()
        .w_full()
        .h(px(PLAYER_HEIGHT))
        .px(px(8.))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .bg(ground)
        .border_1()
        .border_color(border)
        .rounded(px(4.))
        .text_size(px(13.))
        .text_color(fg);
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
                        .whitespace_nowrap()
                        .child(SharedString::from(time)),
                )
                .child(bar)
                .child(mute);
        }
    }
    div().w_full().pb(px(DESCENT)).child(row).into_any_element()
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
