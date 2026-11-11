//! Vim key bindings over a [`TextBuffer`]: the modes, counts, motions,
//! operators, text objects, registers, search and repeat that `vimMode` in
//! `app.json` turns on.
//!
//! [`Vim`] is a state machine fed one [`Key`] at a time. It owns nothing of
//! the view: the editor translates keystrokes, hands over its buffer, and
//! afterwards reads what happened from the [`Outcome`]. Insert mode is the
//! editor's own typing path untouched (auto-pairing, the suggestion popup,
//! table cells), so the only key Vim takes there is the one that leaves it.
//!
//! The normal-mode cursor sits *on* a character, never past the last one of
//! a line; a visual selection includes the character under the cursor. Both
//! are kept here and written to the buffer as an ordinary selection.

use std::collections::HashMap;
use std::ops::Range;

use regex::RegexBuilder;

use crate::{EditKind, TextBuffer};

/// A keystroke, as far as Vim tells keys apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    /// Control and a letter or `[`, lower case.
    Ctrl(char),
    Esc,
    Enter,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    VisualLine,
}

/// The system clipboard, behind the `+` and `*` registers.
pub trait Host {
    fn clipboard(&mut self) -> Option<String>;
    fn set_clipboard(&mut self, text: &str);
}

/// A host with no clipboard, for tests and headless callers.
pub struct NoClipboard;

impl Host for NoClipboard {
    fn clipboard(&mut self) -> Option<String> {
        None
    }
    fn set_clipboard(&mut self, _: &str) {}
}

/// Something a key asks of the view rather than the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// `zz`: the cursor's row to the middle of the view.
    Center,
    /// `za`: fold or unfold at the cursor.
    ToggleFold,
    /// `zM`
    FoldAll,
    /// `zR`
    UnfoldAll,
}

/// What one key did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// False when the key is not Vim's and should go on to whatever else
    /// handles it (a typed character in insert mode, an unknown chord).
    pub handled: bool,
    /// The text changed.
    pub edited: bool,
    pub view: Option<View>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Delete,
    Change,
    Yank,
    Indent,
    Outdent,
    Lower,
    Upper,
    Toggle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    None,
    G,
    Z,
    Find { forward: bool, till: bool },
    Replace,
    Register,
    Object { around: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Span {
    Exclusive,
    Inclusive,
    Line,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mot {
    Left,
    Right,
    Down,
    Up,
    WordFwd(bool),
    WordBack(bool),
    WordEnd(bool),
    WordEndBack(bool),
    LineStart,
    FirstNonBlank,
    LineEnd,
    /// `_`
    LineFirst,
    DocStart,
    DocEnd,
    Find {
        c: char,
        forward: bool,
        till: bool,
        repeat: bool,
    },
    Match,
    ParaFwd,
    ParaBack,
    Search {
        reverse: bool,
    },
    NextLine,
    PrevLine,
    Column,
    Scroll {
        down: bool,
        half: bool,
    },
}

/// What an operator acts on. Charwise: `start..end`. Linewise: every line
/// from the one holding `start` to the one holding `end`.
#[derive(Debug, Clone, Copy)]
struct Target {
    start: usize,
    end: usize,
    linewise: bool,
}

#[derive(Debug, Clone, Default)]
struct Register {
    text: String,
    linewise: bool,
}

#[derive(Debug, Clone)]
struct Change {
    keys: Vec<Key>,
    inserted: String,
}

#[derive(Debug)]
struct Session {
    /// The undo depth when the command that entered insert mode began.
    depth: usize,
    /// Where typing started, to read back what was typed.
    start: usize,
    /// `3ix<Esc>` types `x` three times.
    repeat: usize,
    keys: Vec<Key>,
}

#[derive(Debug)]
struct Prompt {
    kind: char,
    text: String,
}

/// The Vim state of one editor.
#[derive(Debug)]
pub struct Vim {
    mode: Mode,
    count: Option<usize>,
    op: Option<Op>,
    op_count: Option<usize>,
    pending: Pending,
    register: Option<char>,
    register_fresh: bool,
    registers: HashMap<char, Register>,
    unnamed: Register,
    /// The column `j` and `k` aim for, in characters.
    goal: Option<usize>,
    vanchor: usize,
    vhead: usize,
    /// The selection last written to the buffer; a different one there
    /// means the mouse moved it.
    written: (usize, usize),
    last_find: Option<(char, bool, bool)>,
    search: Option<(String, bool)>,
    prompt: Option<Prompt>,
    keys: Vec<Key>,
    depth: usize,
    from_visual: bool,
    changed: bool,
    edited: bool,
    view: Option<View>,
    replaying: bool,
    last_change: Option<Change>,
    session: Option<Session>,
    /// Lines a `Ctrl-F` moves; the view sets it.
    pub page_lines: usize,
    /// `useTab` and `tabSize`, for `>` and `<`.
    pub use_tab: bool,
    pub tab_size: u8,
}

impl Default for Vim {
    fn default() -> Vim {
        Vim::new()
    }
}

// ----- text helpers ------------------------------------------------------------

fn ch(text: &str, i: usize) -> Option<char> {
    text.get(i..)?.chars().next()
}

fn next(text: &str, i: usize) -> usize {
    i + ch(text, i).map_or(0, char::len_utf8)
}

fn prev(text: &str, i: usize) -> usize {
    text[..i]
        .chars()
        .next_back()
        .map_or(i, |c| i - c.len_utf8())
}

/// 0 blank, 1 word, 2 punctuation; a WORD (`big`) is anything not blank.
fn cls(text: &str, i: usize, big: bool) -> u8 {
    match ch(text, i) {
        None => 0,
        Some(c) if c.is_whitespace() => 0,
        Some(c) if big || c.is_alphanumeric() || c == '_' => 1,
        Some(_) => 2,
    }
}

/// An empty line counts as a word to `w` and `b`.
fn empty_line(text: &str, i: usize) -> bool {
    let b = text.as_bytes();
    b.get(i) == Some(&b'\n') && (i == 0 || b[i - 1] == b'\n')
}

fn word_fwd(text: &str, pos: usize, big: bool) -> usize {
    let len = text.len();
    let mut i = pos;
    if i >= len {
        return len;
    }
    let c0 = cls(text, i, big);
    if c0 != 0 {
        while i < len && cls(text, i, big) == c0 {
            i = next(text, i);
        }
    }
    while i < len && cls(text, i, big) == 0 {
        if i != pos && empty_line(text, i) {
            break;
        }
        i = next(text, i);
    }
    i
}

fn word_back(text: &str, pos: usize, big: bool) -> usize {
    if pos == 0 {
        return 0;
    }
    let mut i = prev(text, pos);
    while i > 0 && cls(text, i, big) == 0 && !empty_line(text, i) {
        i = prev(text, i);
    }
    let c = cls(text, i, big);
    if c == 0 {
        return i;
    }
    while i > 0 && cls(text, prev(text, i), big) == c {
        i = prev(text, i);
    }
    i
}

fn word_end(text: &str, pos: usize, big: bool) -> usize {
    let len = text.len();
    if pos >= len {
        return pos;
    }
    let mut i = next(text, pos);
    while i < len && cls(text, i, big) == 0 {
        i = next(text, i);
    }
    if i >= len {
        return prev(text, len).max(pos);
    }
    let c = cls(text, i, big);
    while next(text, i) < len && cls(text, next(text, i), big) == c {
        i = next(text, i);
    }
    i
}

fn word_end_back(text: &str, pos: usize, big: bool) -> usize {
    let mut i = pos.min(text.len());
    let c0 = cls(text, i, big);
    if c0 != 0 {
        while i > 0 && cls(text, i, big) == c0 {
            i = prev(text, i);
        }
        if cls(text, i, big) == c0 {
            return 0;
        }
    } else if i > 0 {
        i = prev(text, i);
    }
    while i > 0 && cls(text, i, big) == 0 && !empty_line(text, i) {
        i = prev(text, i);
    }
    i
}

/// Where `cw` stops: the end of the word under the cursor, not the start
/// of the next one.
fn change_word_end(text: &str, from: usize, big: bool, count: usize) -> usize {
    let len = text.len();
    let mut i = from;
    for k in 0..count {
        if k > 0 {
            while i < len && cls(text, i, big) == 0 {
                i = next(text, i);
            }
        }
        let c = cls(text, i, big);
        if c == 0 {
            break;
        }
        while i < len && cls(text, i, big) == c {
            i = next(text, i);
        }
    }
    i
}

fn line_of(buf: &TextBuffer, pos: usize) -> usize {
    buf.cursor_at(pos.min(buf.len())).line
}

fn first_nonblank(buf: &TextBuffer, line: usize) -> usize {
    let t = buf.line_text(line);
    buf.line_range(line).start + (t.len() - t.trim_start().len())
}

fn last_char(buf: &TextBuffer, line: usize) -> usize {
    let r = buf.line_range(line);
    if r.is_empty() {
        r.start
    } else {
        buf.prev_grapheme_boundary(r.end).max(r.start)
    }
}

/// The nearest place a normal-mode cursor can be.
fn clamp(buf: &TextBuffer, pos: usize) -> usize {
    let pos = buf.clamp_offset(pos.min(buf.len()));
    pos.min(last_char(buf, line_of(buf, pos)))
}

fn col_of(buf: &TextBuffer, pos: usize) -> usize {
    let start = buf.line_range(line_of(buf, pos)).start;
    buf.text()[start..pos].chars().count()
}

fn at_col(buf: &TextBuffer, line: usize, col: usize) -> usize {
    let r = buf.line_range(line);
    match buf.line_text(line).char_indices().nth(col) {
        Some((i, _)) if col != usize::MAX => r.start + i,
        _ => r.end,
    }
}

/// The end of an inclusive range whose last character starts at `last`:
/// one character on, but never over the line's newline.
fn inclusive_end(buf: &TextBuffer, last: usize) -> usize {
    let end = buf.line_range(line_of(buf, last)).end;
    if last >= end {
        end
    } else {
        buf.next_grapheme_boundary(last)
    }
}

fn find_pattern(text: &str, pattern: &str, from: usize, forward: bool) -> Option<usize> {
    let build = |p: &str| {
        RegexBuilder::new(p)
            .case_insensitive(!pattern.chars().any(char::is_uppercase))
            .multi_line(true)
            .build()
    };
    let re = build(pattern)
        .or_else(|_| build(&regex::escape(pattern)))
        .ok()?;
    if forward {
        let start = next(text, from.min(text.len()));
        let hit = if start <= text.len() {
            re.find_at(text, start)
        } else {
            None
        };
        hit.or_else(|| re.find(text)).map(|m| m.start())
    } else {
        let mut before = None;
        let mut last = None;
        for m in re.find_iter(text) {
            if m.start() < from {
                before = Some(m.start());
            }
            last = Some(m.start());
        }
        before.or(last)
    }
}

fn toggle_case(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            if c.is_uppercase() {
                c.to_lowercase().collect::<Vec<_>>()
            } else {
                c.to_uppercase().collect::<Vec<_>>()
            }
        })
        .collect()
}

impl Vim {
    pub fn new() -> Vim {
        Vim {
            mode: Mode::Normal,
            count: None,
            op: None,
            op_count: None,
            pending: Pending::None,
            register: None,
            register_fresh: false,
            registers: HashMap::new(),
            unnamed: Register::default(),
            goal: None,
            vanchor: 0,
            vhead: 0,
            written: (0, 0),
            last_find: None,
            search: None,
            prompt: None,
            keys: Vec::new(),
            depth: 0,
            from_visual: false,
            changed: false,
            edited: false,
            view: None,
            replaying: false,
            last_change: None,
            session: None,
            page_lines: 20,
            use_tab: true,
            tab_size: 4,
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Back to normal mode with nothing pending, as for a newly opened
    /// file. Registers, the last search and the last change are kept.
    pub fn reset(&mut self) {
        self.clear();
        self.mode = Mode::Normal;
        self.session = None;
        self.keys.clear();
        self.changed = false;
    }

    /// The `/`, `?` or `:` line being typed, with its leading character.
    pub fn prompt(&self) -> Option<String> {
        self.prompt
            .as_ref()
            .map(|p| format!("{}{}", p.kind, p.text))
    }

    /// Where the block cursor is drawn; none in insert mode, which keeps
    /// the editor's bar.
    pub fn block_cursor(&self, buf: &TextBuffer) -> Option<usize> {
        match self.mode {
            Mode::Insert => None,
            Mode::Normal => Some(clamp(buf, buf.head)),
            Mode::Visual | Mode::VisualLine => {
                if (buf.anchor, buf.head) == self.written {
                    Some(clamp(buf, self.vhead))
                } else {
                    Some(clamp(buf, buf.head))
                }
            }
        }
    }

    /// Feeds one key.
    pub fn key(&mut self, buf: &mut TextBuffer, key: Key, host: &mut dyn Host) -> Outcome {
        self.edited = false;
        self.view = None;
        let handled = if self.mode == Mode::Insert {
            match key {
                Key::Esc | Key::Ctrl('[') => {
                    self.leave_insert(buf);
                    true
                }
                _ => false,
            }
        } else {
            self.feed(buf, key, host)
        };
        Outcome {
            handled,
            edited: self.edited,
            view: self.view.take(),
        }
    }

    fn feed(&mut self, buf: &mut TextBuffer, key: Key, host: &mut dyn Host) -> bool {
        self.sync_from_buffer(buf);
        if self.keys.is_empty() {
            self.depth = buf.undo_depth();
            self.from_visual = self.mode != Mode::Normal;
        }
        self.keys.push(key);
        self.register_fresh = false;
        let handled = self.command(buf, key, host);
        self.finish(buf);
        handled
    }

    /// Takes in what the mouse or another command did to the selection
    /// since the last key.
    fn sync_from_buffer(&mut self, buf: &mut TextBuffer) {
        if !buf.extra.is_empty() {
            buf.collapse_cursors();
        }
        let sel = buf.selection();
        match self.mode {
            Mode::Normal => {
                if sel.is_empty() {
                    let p = clamp(buf, buf.head);
                    if p != buf.head {
                        buf.set_cursor(p, None);
                    }
                } else {
                    self.adopt_selection(buf);
                }
            }
            Mode::Visual | Mode::VisualLine => {
                if (buf.anchor, buf.head) == self.written {
                } else if sel.is_empty() {
                    self.mode = Mode::Normal;
                    let p = clamp(buf, buf.head);
                    buf.set_cursor(p, None);
                } else {
                    self.adopt_selection(buf);
                }
            }
            Mode::Insert => {}
        }
    }

    /// A selection made outside Vim becomes a visual one.
    fn adopt_selection(&mut self, buf: &TextBuffer) {
        let sel = buf.selection();
        let last = buf.prev_grapheme_boundary(sel.end).max(sel.start);
        if buf.head < buf.anchor {
            self.vanchor = last;
            self.vhead = sel.start;
        } else {
            self.vanchor = sel.start;
            self.vhead = last;
        }
        self.mode = Mode::Visual;
        self.written = (buf.anchor, buf.head);
    }

    fn finish(&mut self, buf: &mut TextBuffer) {
        if matches!(self.mode, Mode::Visual | Mode::VisualLine) {
            self.write_visual(buf);
        }
        let idle = self.op.is_none()
            && self.pending == Pending::None
            && self.count.is_none()
            && self.op_count.is_none()
            && self.prompt.is_none()
            && !self.register_fresh;
        if !idle {
            return;
        }
        self.register = None;
        let keys = std::mem::take(&mut self.keys);
        let record = std::mem::take(&mut self.changed) && !self.replaying && !self.from_visual;
        if self.mode == Mode::Insert {
            if let (true, Some(session)) = (record, self.session.as_mut()) {
                session.keys = keys;
            }
        } else {
            buf.merge_undo_from(self.depth);
            if record {
                self.last_change = Some(Change {
                    keys,
                    inserted: String::new(),
                });
            }
        }
    }

    fn write_visual(&mut self, buf: &mut TextBuffer) {
        self.vanchor = clamp(buf, self.vanchor);
        self.vhead = clamp(buf, self.vhead);
        let (a, b) = (self.vanchor.min(self.vhead), self.vanchor.max(self.vhead));
        let (start, end) = if self.mode == Mode::VisualLine {
            let last = line_of(buf, b);
            let end = match buf.line_starts().get(last + 1) {
                Some(&next) => next,
                None => buf.len(),
            };
            (buf.line_range(line_of(buf, a)).start, end)
        } else {
            (a, buf.next_grapheme_boundary(b))
        };
        if self.vhead >= self.vanchor {
            buf.set_cursor(end, Some(start));
        } else {
            buf.set_cursor(start, Some(end));
        }
        self.written = (buf.anchor, buf.head);
    }

    fn clear(&mut self) {
        self.count = None;
        self.op = None;
        self.op_count = None;
        self.pending = Pending::None;
        self.register = None;
        self.register_fresh = false;
        self.prompt = None;
    }

    fn pos(&self, buf: &TextBuffer) -> usize {
        match self.mode {
            Mode::Visual | Mode::VisualLine => self.vhead,
            _ => buf.head,
        }
    }

    fn visual(&self) -> bool {
        matches!(self.mode, Mode::Visual | Mode::VisualLine)
    }

    fn take_count(&mut self) -> (usize, bool) {
        let a = self.op_count.take();
        let b = self.count.take();
        (
            (a.unwrap_or(1).saturating_mul(b.unwrap_or(1))).min(100_000),
            a.is_some() || b.is_some(),
        )
    }

    fn move_to(&mut self, buf: &mut TextBuffer, to: usize) {
        let p = clamp(buf, to);
        if self.visual() {
            self.vhead = p;
        } else {
            buf.set_cursor(p, None);
        }
    }

    fn exit_visual(&mut self, buf: &mut TextBuffer) {
        if self.visual() {
            self.mode = Mode::Normal;
            let p = clamp(buf, self.vhead);
            buf.set_cursor(p, None);
        }
    }

    fn edit(&mut self, buf: &mut TextBuffer, range: Range<usize>, insert: &str) {
        if range.is_empty() && insert.is_empty() {
            return;
        }
        buf.edit(range, insert, EditKind::Other);
        self.edited = true;
        self.changed = true;
    }

    // ----- dispatch ------------------------------------------------------------

    fn command(&mut self, buf: &mut TextBuffer, key: Key, host: &mut dyn Host) -> bool {
        if self.prompt.is_some() {
            self.prompt_key(buf, key, host);
            return true;
        }
        if matches!(key, Key::Esc | Key::Ctrl('[')) {
            self.clear();
            self.exit_visual(buf);
            return true;
        }
        match std::mem::replace(&mut self.pending, Pending::None) {
            Pending::None => {}
            Pending::Replace => {
                match key {
                    Key::Char(c) => self.replace(buf, c),
                    _ => self.clear(),
                }
                return true;
            }
            Pending::Register => {
                match key {
                    Key::Char(c) => {
                        self.register = Some(c);
                        self.register_fresh = true;
                    }
                    _ => self.clear(),
                }
                return true;
            }
            Pending::Find { forward, till } => {
                match key {
                    Key::Char(c) => {
                        self.last_find = Some((c, forward, till));
                        let m = Mot::Find {
                            c,
                            forward,
                            till,
                            repeat: false,
                        };
                        self.run_motion(buf, m, host);
                    }
                    _ => self.clear(),
                }
                return true;
            }
            Pending::Object { around } => {
                let target = match key {
                    Key::Char(c) => self.object(buf, c, around),
                    _ => None,
                };
                match target {
                    Some(t) => self.run_object(buf, t, host),
                    None => self.clear(),
                }
                return true;
            }
            Pending::G => {
                self.g_key(buf, key, host);
                return true;
            }
            Pending::Z => {
                self.view = match key {
                    Key::Char('z') | Key::Char('.') => Some(View::Center),
                    Key::Char('a') | Key::Char('o') | Key::Char('c') => Some(View::ToggleFold),
                    Key::Char('M') => Some(View::FoldAll),
                    Key::Char('R') => Some(View::UnfoldAll),
                    _ => None,
                };
                self.clear();
                return true;
            }
        }
        if let Key::Char(d) = key {
            if d.is_ascii_digit() && (d != '0' || self.count.is_some()) {
                let n = self.count.unwrap_or(0);
                self.count = Some((n.saturating_mul(10) + d as usize - '0' as usize).min(100_000));
                return true;
            }
        }
        if let Some(m) = self.motion_for(key) {
            if matches!(m, Mot::Scroll { .. }) && self.op.is_some() {
                self.clear();
            } else {
                self.run_motion(buf, m, host);
            }
            return true;
        }
        let c = match key {
            Key::Char(c) => c,
            Key::Delete => 'x',
            Key::Ctrl('r') => {
                self.clear();
                self.undo(buf, true);
                return true;
            }
            Key::Ctrl('v') if !self.visual() && self.op.is_none() => {
                // No block mode; the nearest thing is the charwise one.
                'v'
            }
            Key::Ctrl(_) => {
                return false;
            }
            _ => {
                self.clear();
                return true;
            }
        };
        self.char_command(buf, c, host);
        true
    }

    fn motion_for(&self, key: Key) -> Option<Mot> {
        let find = |reverse: bool| {
            self.last_find.map(|(c, forward, till)| Mot::Find {
                c,
                forward: forward != reverse,
                till,
                repeat: true,
            })
        };
        Some(match key {
            Key::Char('h') | Key::Left | Key::Backspace => Mot::Left,
            Key::Char('l') | Key::Right | Key::Char(' ') => Mot::Right,
            Key::Char('j') | Key::Down => Mot::Down,
            Key::Char('k') | Key::Up => Mot::Up,
            Key::Char('w') => Mot::WordFwd(false),
            Key::Char('W') => Mot::WordFwd(true),
            Key::Char('b') => Mot::WordBack(false),
            Key::Char('B') => Mot::WordBack(true),
            Key::Char('e') => Mot::WordEnd(false),
            Key::Char('E') => Mot::WordEnd(true),
            Key::Char('0') | Key::Home => Mot::LineStart,
            Key::Char('^') => Mot::FirstNonBlank,
            Key::Char('$') | Key::End => Mot::LineEnd,
            Key::Char('_') => Mot::LineFirst,
            Key::Char('G') => Mot::DocEnd,
            Key::Char(';') => find(false)?,
            Key::Char(',') => find(true)?,
            Key::Char('%') => Mot::Match,
            Key::Char('}') => Mot::ParaFwd,
            Key::Char('{') => Mot::ParaBack,
            Key::Char('n') => Mot::Search { reverse: false },
            Key::Char('N') => Mot::Search { reverse: true },
            Key::Char('+') | Key::Enter => Mot::NextLine,
            Key::Char('-') => Mot::PrevLine,
            Key::Char('|') => Mot::Column,
            Key::Ctrl('d') => Mot::Scroll {
                down: true,
                half: true,
            },
            Key::Ctrl('u') => Mot::Scroll {
                down: false,
                half: true,
            },
            Key::Ctrl('f') => Mot::Scroll {
                down: true,
                half: false,
            },
            Key::Ctrl('b') => Mot::Scroll {
                down: false,
                half: false,
            },
            _ => return None,
        })
    }

    fn char_command(&mut self, buf: &mut TextBuffer, c: char, host: &mut dyn Host) {
        // Operators, alone, doubled, or over a visual selection.
        let op = match c {
            'd' => Some(Op::Delete),
            'c' => Some(Op::Change),
            'y' => Some(Op::Yank),
            '>' => Some(Op::Indent),
            '<' => Some(Op::Outdent),
            _ => None,
        };
        let doubled = match (self.op, c) {
            (Some(Op::Lower), 'u') | (Some(Op::Upper), 'U') | (Some(Op::Toggle), '~') => self.op,
            (Some(a), _) if Some(a) == op => self.op,
            _ => None,
        };
        if let Some(op) = doubled {
            self.op = None;
            self.lines_op(buf, op, host);
            return;
        }
        if self.op.is_some() {
            match c {
                'i' | 'a' => self.pending = Pending::Object { around: c == 'a' },
                'f' | 'F' | 't' | 'T' => self.pending = find_pending(c),
                'g' => self.pending = Pending::G,
                '/' | '?' => self.open_prompt(c),
                '*' | '#' => self.word_search(buf, c == '*', host),
                _ => self.clear(),
            }
            return;
        }
        if let Some(op) = op {
            if self.visual() {
                self.visual_op(buf, op, false, host);
            } else {
                self.op = Some(op);
                self.op_count = self.count.take();
            }
            return;
        }
        match c {
            'f' | 'F' | 't' | 'T' => self.pending = find_pending(c),
            'g' => self.pending = Pending::G,
            'z' => self.pending = Pending::Z,
            '"' => self.pending = Pending::Register,
            '/' | '?' | ':' => self.open_prompt(c),
            '*' | '#' => self.word_search(buf, c == '*', host),
            _ if self.visual() => self.visual_command(buf, c, host),
            _ => self.normal_command(buf, c, host),
        }
    }

    fn normal_command(&mut self, buf: &mut TextBuffer, c: char, host: &mut dyn Host) {
        let pos = buf.head;
        let line = line_of(buf, pos);
        let range = buf.line_range(line);
        match c {
            'x' => {
                self.op = Some(Op::Delete);
                self.run_motion(buf, Mot::Right, host);
            }
            'X' => {
                self.op = Some(Op::Delete);
                self.run_motion(buf, Mot::Left, host);
            }
            's' => {
                self.op = Some(Op::Change);
                self.run_motion(buf, Mot::Right, host);
                if self.mode != Mode::Insert {
                    // An empty line has nothing to take, and still opens.
                    self.enter_insert(buf, pos, 1);
                }
            }
            'D' | 'C' => {
                self.take_count();
                let op = if c == 'D' { Op::Delete } else { Op::Change };
                let target = Target {
                    start: pos,
                    end: range.end,
                    linewise: false,
                };
                self.apply(buf, op, target, pos, host);
            }
            'S' => self.lines_op(buf, Op::Change, host),
            'Y' => self.lines_op(buf, Op::Yank, host),
            'p' | 'P' => self.paste(buf, c == 'P', host),
            'r' => self.pending = Pending::Replace,
            'J' => {
                let (n, _) = self.take_count();
                let last = (line + n.max(2) - 1).min(buf.line_count() - 1);
                self.join(buf, line, last);
            }
            '~' => {
                let (n, _) = self.take_count();
                let mut end = pos;
                for _ in 0..n {
                    if end < range.end {
                        end = buf.next_grapheme_boundary(end);
                    }
                }
                let new = toggle_case(&buf.text()[pos..end]);
                self.edit(buf, pos..end, &new);
                let p = clamp(buf, pos + new.len());
                buf.set_cursor(p, None);
            }
            'u' => self.undo(buf, false),
            '.' => self.repeat(buf, host),
            'i' | 'a' | 'I' | 'A' => {
                let (n, _) = self.take_count();
                let at = match c {
                    'i' => pos,
                    'a' if pos < range.end => buf.next_grapheme_boundary(pos),
                    'a' => pos,
                    'I' => first_nonblank(buf, line),
                    _ => range.end,
                };
                self.changed = true;
                self.enter_insert(buf, at, n);
            }
            'o' | 'O' => {
                self.take_count();
                let text = buf.line_text(line);
                let indent = text[..text.len() - text.trim_start().len()].to_string();
                let at = if c == 'o' {
                    self.edit(buf, range.end..range.end, &format!("\n{indent}"));
                    range.end + 1 + indent.len()
                } else {
                    self.edit(buf, range.start..range.start, &format!("{indent}\n"));
                    range.start + indent.len()
                };
                self.enter_insert(buf, at, 1);
            }
            'v' | 'V' => {
                self.take_count();
                self.vanchor = pos;
                self.vhead = pos;
                self.mode = if c == 'v' {
                    Mode::Visual
                } else {
                    Mode::VisualLine
                };
            }
            _ => self.clear(),
        }
    }

    fn visual_command(&mut self, buf: &mut TextBuffer, c: char, host: &mut dyn Host) {
        match c {
            'x' => self.visual_op(buf, Op::Delete, false, host),
            'X' | 'D' => self.visual_op(buf, Op::Delete, true, host),
            's' => self.visual_op(buf, Op::Change, false, host),
            'S' | 'C' | 'R' => self.visual_op(buf, Op::Change, true, host),
            'Y' => self.visual_op(buf, Op::Yank, true, host),
            'u' => self.visual_op(buf, Op::Lower, false, host),
            'U' => self.visual_op(buf, Op::Upper, false, host),
            '~' => self.visual_op(buf, Op::Toggle, false, host),
            'J' => {
                let (a, b) = (self.vanchor.min(self.vhead), self.vanchor.max(self.vhead));
                let (first, last) = (line_of(buf, a), line_of(buf, b));
                self.mode = Mode::Normal;
                buf.set_cursor(a, None);
                self.join(buf, first, last.max(first + 1).min(buf.line_count() - 1));
            }
            'p' | 'P' => self.visual_paste(buf, host),
            'r' => self.pending = Pending::Replace,
            'o' | 'O' => std::mem::swap(&mut self.vanchor, &mut self.vhead),
            'i' | 'a' => self.pending = Pending::Object { around: c == 'a' },
            'v' | 'V' => {
                let mode = if c == 'v' {
                    Mode::Visual
                } else {
                    Mode::VisualLine
                };
                if self.mode == mode {
                    self.exit_visual(buf);
                } else {
                    self.mode = mode;
                }
            }
            'I' | 'A' => {
                let (a, b) = (self.vanchor.min(self.vhead), self.vanchor.max(self.vhead));
                let at = if c == 'I' { a } else { inclusive_end(buf, b) };
                self.mode = Mode::Normal;
                self.enter_insert(buf, at, 1);
            }
            _ => self.clear(),
        }
    }

    fn g_key(&mut self, buf: &mut TextBuffer, key: Key, host: &mut dyn Host) {
        let Key::Char(c) = key else {
            self.clear();
            return;
        };
        let case = match c {
            'u' => Some(Op::Lower),
            'U' => Some(Op::Upper),
            '~' => Some(Op::Toggle),
            _ => None,
        };
        match c {
            'g' => self.run_motion(buf, Mot::DocStart, host),
            'e' => self.run_motion(buf, Mot::WordEndBack(false), host),
            'E' => self.run_motion(buf, Mot::WordEndBack(true), host),
            'j' => self.run_motion(buf, Mot::Down, host),
            'k' => self.run_motion(buf, Mot::Up, host),
            _ if case.is_some() && self.visual() => {
                self.visual_op(buf, case.unwrap(), false, host);
            }
            _ if case.is_some() && self.op.is_none() => {
                self.op = case;
                self.op_count = self.count.take();
            }
            // `gugu` is `guu`.
            _ if case.is_some() && self.op == case => {
                self.op = None;
                self.lines_op(buf, case.unwrap(), host);
            }
            _ => self.clear(),
        }
    }

    // ----- the prompt ----------------------------------------------------------

    fn open_prompt(&mut self, kind: char) {
        if kind == ':' {
            self.count = None;
        }
        self.prompt = Some(Prompt {
            kind,
            text: String::new(),
        });
    }

    fn prompt_key(&mut self, buf: &mut TextBuffer, key: Key, host: &mut dyn Host) {
        let Some(prompt) = self.prompt.as_mut() else {
            return;
        };
        match key {
            Key::Char(c) => prompt.text.push(c),
            Key::Backspace => {
                // Backspace on an empty prompt closes it, as in Vim.
                let was_empty = prompt.text.pop().is_none();
                if was_empty {
                    self.clear();
                }
            }
            Key::Enter => {
                let Prompt { kind, text } = self.prompt.take().expect("checked");
                if kind == ':' {
                    self.ex(buf, text.trim(), host);
                } else {
                    if !text.is_empty() {
                        self.search = Some((text, kind == '/'));
                    }
                    self.run_motion(buf, Mot::Search { reverse: false }, host);
                }
            }
            Key::Esc | Key::Ctrl('[') => self.clear(),
            _ => {}
        }
    }

    /// The command line knows line numbers and little else; a file is
    /// saved by the app, not by `:w`.
    fn ex(&mut self, buf: &mut TextBuffer, line: &str, host: &mut dyn Host) {
        self.clear();
        if let Ok(n) = line.parse::<usize>() {
            self.count = Some(n.max(1));
            self.run_motion(buf, Mot::DocEnd, host);
        } else if line == "$" {
            self.run_motion(buf, Mot::DocEnd, host);
        } else if line == "noh" || line == "nohlsearch" {
            self.search = None;
        }
    }

    fn word_search(&mut self, buf: &mut TextBuffer, forward: bool, host: &mut dyn Host) {
        let pos = self.pos(buf);
        let text = buf.text();
        let end = buf.line_range(line_of(buf, pos)).end;
        let mut s = pos;
        while s < end && cls(text, s, false) != 1 {
            s = next(text, s);
        }
        if s >= end {
            self.clear();
            return;
        }
        while s > 0 && cls(text, prev(text, s), false) == 1 {
            s = prev(text, s);
        }
        let mut e = s;
        while e < end && cls(text, e, false) == 1 {
            e = next(text, e);
        }
        let pattern = format!(r"\b{}\b", regex::escape(&text[s..e]));
        self.search = Some((pattern, forward));
        // From the word's start, so `#` does not find the word it is on.
        if !forward {
            self.move_to(buf, s);
        }
        self.run_motion(buf, Mot::Search { reverse: false }, host);
    }

    // ----- motions -------------------------------------------------------------

    fn run_motion(&mut self, buf: &mut TextBuffer, m: Mot, host: &mut dyn Host) {
        let (count, had) = self.take_count();
        let from = self.pos(buf);
        let op = self.op.take();
        if !matches!(m, Mot::Down | Mot::Up | Mot::Scroll { .. }) {
            self.goal = None;
        }
        // `cw` on a word changes to its end, leaving the blank after it.
        if let (Some(Op::Change), Mot::WordFwd(big)) = (op, m) {
            if cls(buf.text(), from, big) != 0 {
                let end = change_word_end(buf.text(), from, big, count);
                let target = Target {
                    start: from,
                    end,
                    linewise: false,
                };
                self.apply(buf, Op::Change, target, from, host);
                return;
            }
        }
        let Some((to, span)) = self.motion(buf, from, m, count, had, op.is_some()) else {
            self.clear();
            return;
        };
        let Some(op) = op else {
            self.move_to(buf, to);
            return;
        };
        let (s, mut e) = (from.min(to), from.max(to));
        let target = match span {
            Span::Line => Target {
                start: s,
                end: e,
                linewise: true,
            },
            Span::Inclusive => Target {
                start: s,
                end: inclusive_end(buf, e),
                linewise: false,
            },
            Span::Exclusive => {
                let mut linewise = false;
                let (ls, le) = (line_of(buf, s), line_of(buf, e));
                if le > ls {
                    let line_start = buf.line_range(le).start;
                    if matches!(m, Mot::WordFwd(_)) {
                        // The last word moved over ends its line: stop there.
                        if buf.text()[line_start..e].trim().is_empty() {
                            let back = buf.line_range(le - 1).end;
                            if back > s {
                                e = back;
                            }
                        }
                    } else if e == line_start {
                        e = buf.line_range(le - 1).end;
                        linewise = s <= first_nonblank(buf, ls);
                    }
                }
                Target {
                    start: s,
                    end: e,
                    linewise,
                }
            }
        };
        self.apply(buf, op, target, from, host);
    }

    fn motion(
        &mut self,
        buf: &TextBuffer,
        from: usize,
        m: Mot,
        n: usize,
        had: bool,
        op: bool,
    ) -> Option<(usize, Span)> {
        let text = buf.text();
        let line = line_of(buf, from);
        let range = buf.line_range(line);
        let last_line = buf.line_count() - 1;
        let vertical = |this: &mut Vim, to: usize| {
            if to == line {
                return None;
            }
            let col = *this.goal.get_or_insert_with(|| col_of(buf, from));
            Some((at_col(buf, to, col), Span::Line))
        };
        Some(match m {
            Mot::Left => {
                let mut p = from;
                for _ in 0..n {
                    if p > range.start {
                        p = buf.prev_grapheme_boundary(p).max(range.start);
                    }
                }
                (p, Span::Exclusive)
            }
            Mot::Right => {
                let limit = if op { range.end } else { last_char(buf, line) };
                let mut p = from;
                for _ in 0..n {
                    if p < limit {
                        p = buf.next_grapheme_boundary(p).min(limit);
                    }
                }
                (p, Span::Exclusive)
            }
            Mot::Down => vertical(self, line.saturating_add(n).min(last_line))?,
            Mot::Up => vertical(self, line.saturating_sub(n))?,
            Mot::Scroll { down, half } => {
                let page = self.page_lines.max(2);
                let step = if half { page / 2 } else { page }.saturating_mul(n);
                let to = if down {
                    line.saturating_add(step).min(last_line)
                } else {
                    line.saturating_sub(step)
                };
                vertical(self, to)?
            }
            Mot::WordFwd(big) => {
                let mut p = from;
                for _ in 0..n {
                    p = word_fwd(text, p, big);
                }
                (p, Span::Exclusive)
            }
            Mot::WordBack(big) => {
                let mut p = from;
                for _ in 0..n {
                    p = word_back(text, p, big);
                }
                (p, Span::Exclusive)
            }
            Mot::WordEnd(big) => {
                let mut p = from;
                for _ in 0..n {
                    p = word_end(text, p, big);
                }
                (p, Span::Inclusive)
            }
            Mot::WordEndBack(big) => {
                let mut p = from;
                for _ in 0..n {
                    p = word_end_back(text, p, big);
                }
                (p, Span::Inclusive)
            }
            Mot::LineStart => (range.start, Span::Exclusive),
            Mot::FirstNonBlank => (first_nonblank(buf, line), Span::Exclusive),
            Mot::LineEnd => {
                let to = (line + n - 1).min(last_line);
                self.goal = Some(usize::MAX);
                (last_char(buf, to), Span::Inclusive)
            }
            Mot::LineFirst => {
                let to = (line + n - 1).min(last_line);
                (first_nonblank(buf, to), Span::Line)
            }
            Mot::DocStart => {
                let to = if had { (n - 1).min(last_line) } else { 0 };
                (first_nonblank(buf, to), Span::Line)
            }
            Mot::DocEnd => {
                let to = if had {
                    (n - 1).min(last_line)
                } else {
                    last_line
                };
                (first_nonblank(buf, to), Span::Line)
            }
            Mot::NextLine => {
                if line == last_line {
                    return None;
                }
                let to = line.saturating_add(n).min(last_line);
                (first_nonblank(buf, to), Span::Line)
            }
            Mot::PrevLine => {
                if line == 0 {
                    return None;
                }
                (first_nonblank(buf, line.saturating_sub(n)), Span::Line)
            }
            Mot::Column => (at_col(buf, line, n - 1), Span::Exclusive),
            Mot::Find {
                c,
                forward,
                till,
                repeat,
            } => {
                if forward {
                    let mut start = next(text, from);
                    if repeat && till {
                        start = next(text, start);
                    }
                    if start > range.end {
                        return None;
                    }
                    let (i, _) = text[start..range.end]
                        .char_indices()
                        .filter(|&(_, x)| x == c)
                        .nth(n - 1)?;
                    let hit = start + i;
                    (if till { prev(text, hit) } else { hit }, Span::Inclusive)
                } else {
                    let mut end = from;
                    if repeat && till && end > range.start {
                        end = prev(text, end);
                    }
                    let (i, x) = text[range.start..end]
                        .char_indices()
                        .rev()
                        .filter(|&(_, x)| x == c)
                        .nth(n - 1)?;
                    let hit = range.start + i;
                    (if till { hit + x.len_utf8() } else { hit }, Span::Exclusive)
                }
            }
            Mot::Match => (match_bracket(text, from, range.end)?, Span::Inclusive),
            Mot::ParaFwd => {
                let empty = |l: usize| buf.line_range(l).is_empty();
                let mut l = line;
                for _ in 0..n {
                    while l < last_line && empty(l) {
                        l += 1;
                    }
                    while l < last_line && !empty(l) {
                        l += 1;
                    }
                }
                if empty(l) {
                    (buf.line_range(l).start, Span::Exclusive)
                } else if op {
                    (buf.line_range(l).end, Span::Exclusive)
                } else {
                    (last_char(buf, l), Span::Inclusive)
                }
            }
            Mot::ParaBack => {
                let empty = |l: usize| buf.line_range(l).is_empty();
                let mut l = line;
                for _ in 0..n {
                    while l > 0 && empty(l) {
                        l -= 1;
                    }
                    while l > 0 && !empty(l) {
                        l -= 1;
                    }
                }
                (buf.line_range(l).start, Span::Exclusive)
            }
            Mot::Search { reverse } => {
                let (pattern, forward) = self.search.clone()?;
                let mut p = from;
                for _ in 0..n {
                    p = find_pattern(text, &pattern, p, forward != reverse)?;
                }
                (p, Span::Exclusive)
            }
        })
    }

    // ----- text objects --------------------------------------------------------

    fn object(&self, buf: &TextBuffer, c: char, around: bool) -> Option<Target> {
        let pos = self.pos(buf);
        match c {
            'w' => word_object(buf, pos, around, false),
            'W' => word_object(buf, pos, around, true),
            '"' | '\'' | '`' => quote_object(buf, pos, c, around),
            '(' | ')' | 'b' => bracket_object(buf, pos, '(', ')', around),
            '[' | ']' => bracket_object(buf, pos, '[', ']', around),
            '{' | '}' | 'B' => bracket_object(buf, pos, '{', '}', around),
            '<' | '>' => bracket_object(buf, pos, '<', '>', around),
            'p' => Some(paragraph_object(buf, pos, around)),
            _ => None,
        }
    }

    fn run_object(&mut self, buf: &mut TextBuffer, t: Target, host: &mut dyn Host) {
        self.take_count();
        if let Some(op) = self.op.take() {
            let from = self.pos(buf);
            self.apply(buf, op, t, from, host);
        } else if self.visual() {
            if t.linewise {
                self.mode = Mode::VisualLine;
                self.vanchor = t.start;
                self.vhead = t.end;
            } else if t.end > t.start {
                self.vanchor = t.start;
                self.vhead = buf.prev_grapheme_boundary(t.end).max(t.start);
            }
        }
    }

    // ----- operators -----------------------------------------------------------

    /// `dd`, `cc`, `yy`, `>>` and the rest: the operator over `count` lines.
    fn lines_op(&mut self, buf: &mut TextBuffer, op: Op, host: &mut dyn Host) {
        let (n, _) = self.take_count();
        let from = buf.head;
        let line = line_of(buf, from);
        let last = (line + n - 1).min(buf.line_count() - 1);
        let target = Target {
            start: from,
            end: buf.line_range(last).start,
            linewise: true,
        };
        self.apply(buf, op, target, from, host);
    }

    fn visual_target(&self, buf: &TextBuffer, linewise: bool) -> Target {
        let (a, b) = (self.vanchor.min(self.vhead), self.vanchor.max(self.vhead));
        if linewise || self.mode == Mode::VisualLine {
            Target {
                start: a,
                end: b,
                linewise: true,
            }
        } else {
            Target {
                start: a,
                end: buf.next_grapheme_boundary(b),
                linewise: false,
            }
        }
    }

    fn visual_op(&mut self, buf: &mut TextBuffer, op: Op, linewise: bool, host: &mut dyn Host) {
        self.take_count();
        let target = self.visual_target(buf, linewise);
        self.mode = Mode::Normal;
        buf.set_cursor(target.start, None);
        self.apply(buf, op, target, target.start, host);
        if op == Op::Yank {
            let p = if target.linewise {
                buf.line_range(line_of(buf, target.start)).start
            } else {
                target.start
            };
            let p = clamp(buf, p);
            buf.set_cursor(p, None);
        }
    }

    fn apply(&mut self, buf: &mut TextBuffer, op: Op, t: Target, from: usize, host: &mut dyn Host) {
        let (l0, l1) = (line_of(buf, t.start), line_of(buf, t.end));
        // A linewise target as bytes: the lines without the last newline.
        let (s, e) = if t.linewise {
            (buf.line_range(l0).start, buf.line_range(l1).end)
        } else {
            (t.start, t.end)
        };
        match op {
            Op::Yank => {
                let mut text = buf.text()[s..e].to_string();
                if t.linewise {
                    text.push('\n');
                }
                self.store(host, text, t.linewise, true);
                let p = if !t.linewise {
                    s
                } else if l0 < line_of(buf, from) {
                    at_col(buf, l0, col_of(buf, from))
                } else {
                    from
                };
                let p = clamp(buf, p);
                buf.set_cursor(p, None);
            }
            Op::Delete if t.linewise => {
                let text = format!("{}\n", &buf.text()[s..e]);
                self.store(host, text, true, false);
                let range = if l1 + 1 < buf.line_count() {
                    s..e + 1
                } else if s > 0 {
                    s - 1..e
                } else {
                    s..e
                };
                self.edit(buf, range, "");
                let line = l0.min(buf.line_count() - 1);
                let p = clamp(buf, first_nonblank(buf, line));
                buf.set_cursor(p, None);
            }
            Op::Change if t.linewise => {
                let text = format!("{}\n", &buf.text()[s..e]);
                self.store(host, text, true, false);
                // The first line keeps its indentation.
                let s = first_nonblank(buf, l0).min(e);
                self.edit(buf, s..e, "");
                self.changed = true;
                self.enter_insert(buf, s, 1);
            }
            Op::Delete | Op::Change => {
                if e > s {
                    let text = buf.text()[s..e].to_string();
                    self.store(host, text, false, false);
                    self.edit(buf, s..e, "");
                } else {
                    self.register = None;
                }
                if op == Op::Change {
                    self.changed = true;
                    self.enter_insert(buf, s, 1);
                } else {
                    let p = clamp(buf, s);
                    buf.set_cursor(p, None);
                }
            }
            Op::Indent | Op::Outdent => {
                let unit = if self.use_tab {
                    "\t".to_string()
                } else {
                    " ".repeat(self.tab_size.max(1) as usize)
                };
                let (s, e) = (buf.line_range(l0).start, buf.line_range(l1).end);
                let width = self.tab_size.max(1) as usize;
                let new = buf.text()[s..e]
                    .split('\n')
                    .map(|line| {
                        if op == Op::Indent {
                            if line.is_empty() {
                                String::new()
                            } else {
                                format!("{unit}{line}")
                            }
                        } else if let Some(rest) = line.strip_prefix('\t') {
                            rest.to_string()
                        } else {
                            let spaces = line.len() - line.trim_start_matches(' ').len();
                            line[spaces.min(width)..].to_string()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if new != buf.text()[s..e] {
                    self.edit(buf, s..e, &new);
                }
                let p = clamp(buf, first_nonblank(buf, l0));
                buf.set_cursor(p, None);
            }
            Op::Lower | Op::Upper | Op::Toggle => {
                let old = &buf.text()[s..e];
                let new = match op {
                    Op::Lower => old.to_lowercase(),
                    Op::Upper => old.to_uppercase(),
                    _ => toggle_case(old),
                };
                if new != old {
                    self.edit(buf, s..e, &new);
                }
                let p = clamp(buf, s);
                buf.set_cursor(p, None);
            }
        }
    }

    fn store(&mut self, host: &mut dyn Host, text: String, linewise: bool, yank: bool) {
        let reg = Register { text, linewise };
        match self.register.take() {
            Some('_') => return,
            Some('+') | Some('*') => host.set_clipboard(&reg.text),
            Some(c) if c.is_ascii_alphabetic() => {
                self.registers.insert(c.to_ascii_lowercase(), reg.clone());
            }
            _ => {}
        }
        if yank {
            self.registers.insert('0', reg.clone());
        }
        self.unnamed = reg;
    }

    fn fetch(&mut self, host: &mut dyn Host) -> Option<Register> {
        match self.register.take() {
            Some('+') | Some('*') => host.clipboard().map(|text| Register {
                linewise: text.ends_with('\n'),
                text,
            }),
            Some(c) => self.registers.get(&c.to_ascii_lowercase()).cloned(),
            None => Some(self.unnamed.clone()),
        }
    }

    fn paste(&mut self, buf: &mut TextBuffer, before: bool, host: &mut dyn Host) {
        let (n, _) = self.take_count();
        let Some(reg) = self.fetch(host).filter(|r| !r.text.is_empty()) else {
            return;
        };
        let pos = buf.head;
        let line = line_of(buf, pos);
        let range = buf.line_range(line);
        if reg.linewise {
            let body = reg.text.strip_suffix('\n').unwrap_or(&reg.text);
            let block = format!("{body}\n").repeat(n);
            let at = if before {
                self.edit(buf, range.start..range.start, &block);
                range.start
            } else if line + 1 < buf.line_count() {
                self.edit(buf, range.end + 1..range.end + 1, &block);
                range.end + 1
            } else {
                let block = format!("\n{}", &block[..block.len() - 1]);
                self.edit(buf, range.end..range.end, &block);
                range.end + 1
            };
            let p = clamp(buf, first_nonblank(buf, line_of(buf, at)));
            buf.set_cursor(p, None);
        } else {
            let text = reg.text.repeat(n);
            let at = if before || range.is_empty() {
                pos
            } else {
                buf.next_grapheme_boundary(pos)
            };
            self.edit(buf, at..at, &text);
            let p = if text.contains('\n') {
                at
            } else {
                buf.prev_grapheme_boundary(at + text.len())
            };
            let p = clamp(buf, p);
            buf.set_cursor(p, None);
        }
    }

    /// `p` over a selection: the register replaces it, and what it
    /// replaced becomes the unnamed register.
    fn visual_paste(&mut self, buf: &mut TextBuffer, host: &mut dyn Host) {
        self.take_count();
        let Some(reg) = self.fetch(host) else {
            return;
        };
        let t = self.visual_target(buf, false);
        self.mode = Mode::Normal;
        let (s, e) = if t.linewise {
            (
                buf.line_range(line_of(buf, t.start)).start,
                buf.line_range(line_of(buf, t.end)).end,
            )
        } else {
            (t.start, t.end)
        };
        let mut old = buf.text()[s..e].to_string();
        if t.linewise {
            old.push('\n');
        }
        let body = reg.text.strip_suffix('\n').unwrap_or(&reg.text);
        let new = match (t.linewise, reg.linewise) {
            (false, true) => format!("\n{body}\n"),
            (true, _) => body.to_string(),
            (false, false) => reg.text.clone(),
        };
        self.edit(buf, s..e, &new);
        let p = if t.linewise || reg.linewise || new.is_empty() {
            s
        } else {
            buf.prev_grapheme_boundary(s + new.len())
        };
        let p = clamp(buf, p);
        buf.set_cursor(p, None);
        self.unnamed = Register {
            text: old,
            linewise: t.linewise,
        };
    }

    fn replace(&mut self, buf: &mut TextBuffer, c: char) {
        let (n, _) = self.take_count();
        if self.visual() {
            let t = self.visual_target(buf, false);
            let (s, e) = if t.linewise {
                (
                    buf.line_range(line_of(buf, t.start)).start,
                    buf.line_range(line_of(buf, t.end)).end,
                )
            } else {
                (t.start, t.end)
            };
            let new: String = buf.text()[s..e]
                .chars()
                .map(|x| if x == '\n' { x } else { c })
                .collect();
            self.mode = Mode::Normal;
            self.edit(buf, s..e, &new);
            let p = clamp(buf, s);
            buf.set_cursor(p, None);
            return;
        }
        let pos = buf.head;
        let end = buf.line_range(line_of(buf, pos)).end;
        let mut e = pos;
        for _ in 0..n {
            if e >= end {
                // Fewer characters than the count: nothing is replaced.
                return;
            }
            e = buf.next_grapheme_boundary(e);
        }
        let new = c.to_string().repeat(n);
        self.edit(buf, pos..e, &new);
        let p = pos + new.len() - c.len_utf8();
        buf.set_cursor(p, None);
    }

    fn join(&mut self, buf: &mut TextBuffer, first: usize, last: usize) {
        if last <= first {
            return;
        }
        let (s, e) = (buf.line_range(first).start, buf.line_range(last).end);
        let mut out = String::new();
        let mut at = 0;
        for (i, line) in buf.text()[s..e].split('\n').enumerate() {
            if i == 0 {
                out.push_str(line);
                continue;
            }
            let rest = line.trim_start();
            at = out.len();
            if !rest.is_empty() {
                if !out.is_empty() && !out.ends_with(char::is_whitespace) && !rest.starts_with(')')
                {
                    out.push(' ');
                }
                out.push_str(rest);
            }
        }
        self.edit(buf, s..e, &out);
        let p = clamp(buf, s + at);
        buf.set_cursor(p, None);
    }

    fn undo(&mut self, buf: &mut TextBuffer, redo: bool) {
        let (n, _) = self.take_count();
        for _ in 0..n {
            let done = if redo { buf.redo() } else { buf.undo() };
            if !done {
                break;
            }
            self.edited = true;
        }
        let p = clamp(buf, buf.selection().start);
        buf.set_cursor(p, None);
    }

    // ----- insert mode and repeat ----------------------------------------------

    fn enter_insert(&mut self, buf: &mut TextBuffer, at: usize, repeat: usize) {
        buf.set_cursor(at, None);
        self.mode = Mode::Insert;
        self.session = Some(Session {
            depth: self.depth,
            start: buf.head,
            repeat,
            keys: Vec::new(),
        });
    }

    fn leave_insert(&mut self, buf: &mut TextBuffer) {
        self.mode = Mode::Normal;
        self.clear();
        let Some(session) = self.session.take() else {
            return;
        };
        buf.collapse_cursors();
        let head = buf.head;
        let inserted = if head >= session.start && buf.text().is_char_boundary(session.start) {
            buf.text()[session.start..head].to_string()
        } else {
            String::new()
        };
        if !inserted.is_empty() {
            for _ in 1..session.repeat {
                let at = buf.head;
                buf.edit(at..at, &inserted, EditKind::Other);
                self.edited = true;
            }
        }
        buf.merge_undo_from(session.depth);
        let start = buf.line_range(line_of(buf, buf.head)).start;
        let p = if buf.head > start {
            buf.prev_grapheme_boundary(buf.head)
        } else {
            buf.head
        };
        buf.set_cursor(p, None);
        if !self.replaying && !session.keys.is_empty() {
            self.last_change = Some(Change {
                keys: session.keys,
                inserted,
            });
        }
    }

    /// `.`: the last change's keys again, and the text typed after them.
    fn repeat(&mut self, buf: &mut TextBuffer, host: &mut dyn Host) {
        let Some(change) = self.last_change.clone() else {
            return;
        };
        self.clear();
        self.keys.clear();
        self.replaying = true;
        let depth = buf.undo_depth();
        for &key in &change.keys {
            self.feed(buf, key, host);
        }
        if self.mode == Mode::Insert {
            if let Some(session) = self.session.as_mut() {
                session.depth = depth;
            }
            if !change.inserted.is_empty() {
                let at = buf.head;
                buf.edit(at..at, &change.inserted, EditKind::Other);
                self.edited = true;
            }
            self.leave_insert(buf);
        }
        self.replaying = false;
        self.depth = depth;
        self.keys.clear();
        self.changed = false;
    }
}

fn find_pending(c: char) -> Pending {
    Pending::Find {
        forward: c == 'f' || c == 't',
        till: c == 't' || c == 'T',
    }
}

/// `%`: from the first bracket at or after `from` on its line to its match.
fn match_bracket(text: &str, from: usize, line_end: usize) -> Option<usize> {
    const PAIRS: [(char, char); 3] = [('(', ')'), ('[', ']'), ('{', '}')];
    let (i, c) = text[from..line_end]
        .char_indices()
        .find(|&(_, c)| PAIRS.iter().any(|&(o, cl)| c == o || c == cl))?;
    let at = from + i;
    let &(open, close) = PAIRS.iter().find(|&&(o, cl)| c == o || c == cl)?;
    let mut depth = 0usize;
    if c == open {
        for (j, x) in text[at..].char_indices() {
            if x == open {
                depth += 1;
            } else if x == close {
                depth -= 1;
                if depth == 0 {
                    return Some(at + j);
                }
            }
        }
    } else {
        for (j, x) in text[..=at].char_indices().rev() {
            if x == close {
                depth += 1;
            } else if x == open {
                depth -= 1;
                if depth == 0 {
                    return Some(j);
                }
            }
        }
    }
    None
}

fn word_object(buf: &TextBuffer, pos: usize, around: bool, big: bool) -> Option<Target> {
    let text = buf.text();
    let r = buf.line_range(line_of(buf, pos));
    if r.is_empty() {
        return None;
    }
    let c = cls(text, pos, big);
    let mut s = pos;
    while s > r.start && cls(text, prev(text, s), big) == c {
        s = prev(text, s);
    }
    let mut e = pos;
    while e < r.end && cls(text, e, big) == c {
        e = next(text, e);
    }
    if around {
        if c == 0 {
            let c2 = cls(text, e, big);
            while e < r.end && cls(text, e, big) == c2 {
                e = next(text, e);
            }
        } else {
            let e0 = e;
            while e < r.end && cls(text, e, big) == 0 {
                e = next(text, e);
            }
            if e == e0 {
                while s > r.start && cls(text, prev(text, s), big) == 0 {
                    s = prev(text, s);
                }
            }
        }
    }
    Some(Target {
        start: s,
        end: e,
        linewise: false,
    })
}

fn quote_object(buf: &TextBuffer, pos: usize, q: char, around: bool) -> Option<Target> {
    let text = buf.text();
    let r = buf.line_range(line_of(buf, pos));
    let line = &text[r.clone()];
    let mut quotes = Vec::new();
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        if c == q && !escaped {
            quotes.push(r.start + i);
        }
        escaped = c == '\\' && !escaped;
    }
    let pairs = || quotes.chunks_exact(2).map(|p| (p[0], p[1]));
    let (a, b) = pairs()
        .find(|&(a, b)| a <= pos && pos <= b)
        .or_else(|| pairs().find(|&(a, _)| a > pos))?;
    if !around {
        return Some(Target {
            start: a + 1,
            end: b,
            linewise: false,
        });
    }
    let (mut s, mut e) = (a, b + 1);
    let e0 = e;
    while e < r.end && cls(text, e, false) == 0 {
        e = next(text, e);
    }
    if e == e0 {
        while s > r.start && cls(text, prev(text, s), false) == 0 {
            s = prev(text, s);
        }
    }
    Some(Target {
        start: s,
        end: e,
        linewise: false,
    })
}

fn bracket_object(
    buf: &TextBuffer,
    pos: usize,
    open: char,
    close: char,
    around: bool,
) -> Option<Target> {
    let text = buf.text();
    if pos >= text.len() {
        return None;
    }
    let here = next(text, pos);
    let mut depth = 0usize;
    let mut start = None;
    for (i, c) in text[..here].char_indices().rev() {
        if c == close && i != pos {
            depth += 1;
        } else if c == open {
            if depth == 0 {
                start = Some(i);
                break;
            }
            depth -= 1;
        }
    }
    let s = start?;
    let inner = s + open.len_utf8();
    let mut depth = 0usize;
    let mut end = None;
    for (i, c) in text[inner..].char_indices() {
        if c == open {
            depth += 1;
        } else if c == close {
            if depth == 0 {
                end = Some(inner + i);
                break;
            }
            depth -= 1;
        }
    }
    let e = end?;
    Some(if around {
        Target {
            start: s,
            end: e + close.len_utf8(),
            linewise: false,
        }
    } else {
        Target {
            start: inner,
            end: e,
            linewise: false,
        }
    })
}

fn paragraph_object(buf: &TextBuffer, pos: usize, around: bool) -> Target {
    let empty = |l: usize| buf.line_range(l).is_empty();
    let last = buf.line_count() - 1;
    let line = line_of(buf, pos);
    let kind = empty(line);
    let (mut l0, mut l1) = (line, line);
    while l0 > 0 && empty(l0 - 1) == kind {
        l0 -= 1;
    }
    while l1 < last && empty(l1 + 1) == kind {
        l1 += 1;
    }
    if around {
        let before = l1;
        while l1 < last && empty(l1 + 1) != kind {
            l1 += 1;
        }
        if l1 == before && !kind {
            while l0 > 0 && empty(l0 - 1) {
                l0 -= 1;
            }
        }
    }
    Target {
        start: buf.line_range(l0).start,
        end: buf.line_range(l1).start,
        linewise: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `keys` over `text` with the cursor at `|`, and gives the text
    /// back with the cursor marked the same way. `<Esc>`, `<CR>`, `<BS>`
    /// and `<C-x>` name the keys that are not characters; in insert mode
    /// characters are typed into the buffer the way the editor would.
    fn run_with(vim: &mut Vim, buf: &mut TextBuffer, keys: &str) {
        let mut rest = keys;
        while let Some(c) = rest.chars().next() {
            let key = if c == '<' && rest.contains('>') && rest.len() > 2 {
                let end = rest.find('>').unwrap();
                let name = &rest[1..end];
                let key = match name {
                    "Esc" => Some(Key::Esc),
                    "CR" => Some(Key::Enter),
                    "BS" => Some(Key::Backspace),
                    _ if name.starts_with("C-") => Some(Key::Ctrl(name.chars().nth(2).unwrap())),
                    _ => None,
                };
                match key {
                    Some(key) => {
                        rest = &rest[end + 1..];
                        key
                    }
                    None => {
                        rest = &rest[1..];
                        Key::Char(c)
                    }
                }
            } else {
                rest = &rest[c.len_utf8()..];
                Key::Char(c)
            };
            let out = vim.key(buf, key, &mut NoClipboard);
            if !out.handled {
                match key {
                    Key::Char(c) => {
                        buf.insert_all(&c.to_string(), EditKind::Typing);
                    }
                    Key::Enter => {
                        buf.insert_all("\n", EditKind::Other);
                    }
                    Key::Backspace => {
                        buf.backspace();
                    }
                    _ => {}
                }
            }
        }
    }

    fn load(text: &str) -> TextBuffer {
        let at = text.find('|').expect("a cursor");
        let mut buf = TextBuffer::from_file(&text.replace('|', ""));
        buf.set_cursor(at, None);
        buf
    }

    fn show(vim: &Vim, buf: &TextBuffer) -> String {
        let mut out = buf.text().to_string();
        if vim.visual() {
            let sel = buf.selection();
            out.insert(sel.end, ']');
            out.insert(sel.start, '[');
        } else {
            out.insert(buf.head, '|');
        }
        out
    }

    fn run(text: &str, keys: &str) -> String {
        let mut vim = Vim::new();
        let mut buf = load(text);
        run_with(&mut vim, &mut buf, keys);
        show(&vim, &buf)
    }

    #[test]
    fn hjkl_stay_on_characters() {
        assert_eq!(run("|abc\nde", "l"), "a|bc\nde");
        assert_eq!(run("|abc\nde", "5l"), "ab|c\nde");
        assert_eq!(run("ab|c\nde", "j"), "abc\nd|e");
        assert_eq!(run("ab|c\nd\nxyz", "jj"), "abc\nd\nxy|z");
        assert_eq!(run("abc\nd|e", "kh"), "|abc\nde");
        assert_eq!(run("a|bc\n\nxyz", "$jj"), "abc\n\nxy|z");
    }

    #[test]
    fn word_motions() {
        assert_eq!(run("|foo bar.baz", "w"), "foo |bar.baz");
        assert_eq!(run("|foo bar.baz", "ww"), "foo bar|.baz");
        assert_eq!(run("|foo bar.baz", "2W"), "foo bar.ba|z");
        assert_eq!(run("foo bar.ba|z", "b"), "foo bar.|baz");
        assert_eq!(run("foo bar.ba|z", "B"), "foo |bar.baz");
        assert_eq!(run("|foo bar", "e"), "fo|o bar");
        assert_eq!(run("fo|o bar", "e"), "foo ba|r");
        assert_eq!(run("foo b|ar", "ge"), "fo|o bar");
        assert_eq!(run("|foo\n\nbar", "w"), "foo\n|\nbar");
        assert_eq!(run("foo\n\n|bar", "b"), "foo\n|\nbar");
    }

    #[test]
    fn line_and_document_motions() {
        assert_eq!(run("  a|bc", "0"), "|  abc");
        assert_eq!(run("  ab|c", "^"), "  |abc");
        assert_eq!(run("|abc", "$"), "ab|c");
        assert_eq!(run("|a\nb\n  c", "G"), "a\nb\n  |c");
        assert_eq!(run("a\nb\n  |c", "gg"), "|a\nb\n  c");
        assert_eq!(run("|a\nb\nc", "2G"), "a\n|b\nc");
        assert_eq!(run("|a\nb\nc", ":3<CR>"), "a\nb\n|c");
    }

    #[test]
    fn find_and_repeat() {
        assert_eq!(run("|a,b,c,d", "f,"), "a|,b,c,d");
        assert_eq!(run("|a,b,c,d", "f,;"), "a,b|,c,d");
        assert_eq!(run("|a,b,c,d", "2f,"), "a,b|,c,d");
        assert_eq!(run("|a,b,c,d", "t,;"), "a,|b,c,d");
        assert_eq!(run("a,b,c,|d", "F,,"), "a,b,c|,d");
        assert_eq!(run("a,b,c,|d", "T,"), "a,b,c,|d");
        assert_eq!(run("a,b,c|,d", "Tb"), "a,b|,c,d");
    }

    #[test]
    fn bracket_match_and_paragraphs() {
        assert_eq!(run("|f(a[1], b)", "%"), "f(a[1], b|)");
        assert_eq!(run("f(a[1], b|)", "%"), "f|(a[1], b)");
        assert_eq!(run("|a\nb\n\nc", "}"), "a\nb\n|\nc");
        assert_eq!(run("a\nb\n\n|c", "{"), "a\nb\n|\nc");
    }

    #[test]
    fn delete_with_motions() {
        assert_eq!(run("|foo bar", "dw"), "|bar");
        assert_eq!(run("foo |bar\nbaz", "dw"), "foo| \nbaz");
        assert_eq!(run("|foo bar baz", "d2w"), "|baz");
        assert_eq!(run("|foo bar baz", "2dw"), "|baz");
        assert_eq!(run("foo b|ar", "d$"), "foo |b");
        assert_eq!(run("foo b|ar", "D"), "foo |b");
        assert_eq!(run("foo b|ar", "d0"), "|ar");
        assert_eq!(run("a|bcd", "x"), "a|cd");
        assert_eq!(run("abc|d", "x"), "ab|c");
        assert_eq!(run("a|bcd", "3x"), "|a");
        assert_eq!(run("ab|cd", "X"), "a|cd");
        assert_eq!(run("|a,b", "df,"), "|b");
        assert_eq!(run("|a,b", "dt,"), "|,b");
        assert_eq!(run("|", "x"), "|");
        assert_eq!(run("a\n|\nb", "D"), "a\n|\nb");
    }

    #[test]
    fn delete_lines() {
        assert_eq!(run("a\n|b\nc", "dd"), "a\n|c");
        assert_eq!(run("a\nb\n|c", "dd"), "a\n|b");
        assert_eq!(run("|a", "dd"), "|");
        assert_eq!(run("|a\nb\nc", "2dd"), "|c");
        assert_eq!(run("|a\nb\nc", "dj"), "|c");
        assert_eq!(run("a\nb\n|c", "dk"), "|a");
        assert_eq!(run("a\n|b\nc", "dG"), "|a");
        assert_eq!(run("a\n|b\nc", "dgg"), "|c");
        assert_eq!(run("|a\nb\n\nc", "d}"), "|\nc");
    }

    #[test]
    fn change_and_insert() {
        assert_eq!(run("|foo bar", "cwxy<Esc>"), "x|y bar");
        assert_eq!(run("foo |bar baz", "cexy<Esc>"), "foo x|y baz");
        assert_eq!(run("  |foo\nbar", "ccx<Esc>"), "  |x\nbar");
        assert_eq!(run("a|bc", "sx<Esc>"), "a|xc");
        assert_eq!(run("a|bc", "Cx<Esc>"), "a|x");
        assert_eq!(run("a|bc", "ix<Esc>"), "a|xbc");
        assert_eq!(run("a|bc", "ax<Esc>"), "ab|xc");
        assert_eq!(run("  a|bc", "Ix<Esc>"), "  |xabc");
        assert_eq!(run("a|bc", "Ax<Esc>"), "abc|x");
        assert_eq!(run("  a|bc\nd", "ox<Esc>"), "  abc\n  |x\nd");
        assert_eq!(run("  a|bc\nd", "Ox<Esc>"), "  |x\n  abc\nd");
        assert_eq!(run("|a", "3ix<Esc>"), "xx|xa");
        assert_eq!(run("|", "ihello<Esc>"), "hell|o");
    }

    #[test]
    fn text_objects() {
        assert_eq!(run("foo b|ar baz", "diw"), "foo | baz");
        assert_eq!(run("foo b|ar baz", "daw"), "foo |baz");
        assert_eq!(run("foo ba|r", "daw"), "fo|o");
        assert_eq!(run("say \"he|llo\" now", "di\""), "say \"|\" now");
        assert_eq!(run("say \"he|llo\" now", "da\""), "say |now");
        assert_eq!(run("|say \"hello\"", "ci\"x<Esc>"), "say \"|x\"");
        assert_eq!(run("f(a, (b|), c)", "di("), "f(a, (|), c)");
        assert_eq!(run("f(a, (b|), c)", "da("), "f(a, |, c)");
        assert_eq!(run("f(a, (b), |c)", "dib"), "f(|)");
        assert_eq!(run("[[No|te]]", "ci[x<Esc>"), "[[|x]]");
        assert_eq!(run("a\n\nb|\nc\n\nd", "dip"), "a\n\n|\nd");
        assert_eq!(run("a\n\nb|\nc\n\nd", "dap"), "a\n\n|d");
    }

    #[test]
    fn yank_and_put() {
        assert_eq!(run("|foo bar", "yiwwP"), "foo fo|obar");
        assert_eq!(run("|foo bar", "yiwwp"), "foo bfo|oar");
        assert_eq!(run("|a\nb", "yyp"), "a\n|a\nb");
        assert_eq!(run("a\n|b", "yyp"), "a\nb\n|b");
        assert_eq!(run("a\n|b", "yyP"), "a\n|b\nb");
        assert_eq!(run("|a\nb", "ddp"), "b\n|a");
        assert_eq!(run("|ab", "xp"), "b|a");
        assert_eq!(run("|a\nb", "yy2p"), "a\n|a\na\nb");
        assert_eq!(run("|a b", "\"ayiww\"byiw0\"ap\"bp"), "aa|b b");
        // A yank survives a delete in register 0.
        assert_eq!(run("|a b", "yiwwdiw\"0P"), "a|a ");
        // The black hole register keeps the unnamed one.
        assert_eq!(run("|a b", "yiww\"_diwP"), "a|a ");
    }

    #[test]
    fn visual_modes() {
        assert_eq!(run("a|bcd", "vl"), "a[bc]d");
        assert_eq!(run("a|bcd", "vld"), "a|d");
        assert_eq!(run("ab|cd", "vhhd"), "|d");
        assert_eq!(run("a\n|b\nc", "Vd"), "a\n|c");
        assert_eq!(run("|a\nb\nc", "Vjd"), "|c");
        assert_eq!(run("|foo bar", "viwcx<Esc>"), "|x bar");
        assert_eq!(run("|foo bar", "veU"), "|FOO bar");
        assert_eq!(run("|foo bar", "wviwyP"), "foo ba|rbar");
        assert_eq!(run("|foo bar", "yiwwviwp"), "foo fo|o");
        assert_eq!(run("a|bcd", "vlo"), "a[bc]d");
        assert_eq!(run("a|bcd", "vloh"), "[abc]d");
        assert_eq!(run("a|bcd", "v<Esc>"), "a|bcd");
        assert_eq!(run("a|bc\nd", "vlrx"), "a|xx\nd");
        assert_eq!(run("a\n|b\nc", "Vj>"), "a\n\t|b\n\tc");
    }

    #[test]
    fn a_mouse_selection_is_a_visual_one() {
        let mut vim = Vim::new();
        let mut buf = TextBuffer::from_file("hello world");
        buf.set_cursor(5, Some(0));
        run_with(&mut vim, &mut buf, "d");
        assert_eq!(show(&vim, &buf), "| world");
    }

    #[test]
    fn replace_join_case_indent() {
        assert_eq!(run("a|bc", "rx"), "a|xc");
        assert_eq!(run("a|bcd", "2rx"), "ax|xd");
        assert_eq!(run("ab|c", "2rx"), "ab|c");
        assert_eq!(run("|a\n  b\nc", "J"), "a| b\nc");
        assert_eq!(run("|a\n  b\nc", "3J"), "a b| c");
        assert_eq!(run("|aBc", "~~"), "Ab|c");
        assert_eq!(run("|foo bar", "gUiw"), "|FOO bar");
        assert_eq!(run("FOO |BAR", "guu"), "|foo bar");
        assert_eq!(run("|a\nb", ">>"), "\t|a\nb");
        assert_eq!(run("|a\nb", ">j"), "\t|a\n\tb");
        assert_eq!(run("\t|a", "<<"), "|a");
        let mut vim = Vim::new();
        vim.use_tab = false;
        vim.tab_size = 2;
        let mut buf = load("|a");
        run_with(&mut vim, &mut buf, ">>");
        assert_eq!(show(&vim, &buf), "  |a");
    }

    #[test]
    fn undo_takes_a_whole_command() {
        assert_eq!(run("|foo bar", "cwxyz<Esc>u"), "|foo bar");
        assert_eq!(run("|foo bar", "dwu"), "|foo bar");
        assert_eq!(run("|a\nb", "ox<Esc>u"), "|a\nb");
        assert_eq!(run("|foo bar", "dwu<C-r>"), "|bar");
        assert_eq!(run("|a b c", "dwdwuu"), "|a b c");
        assert_eq!(run("|a", "3ix<Esc>u"), "|a");
    }

    #[test]
    fn dot_repeats_the_last_change() {
        assert_eq!(run("|a b c d", "dw."), "|c d");
        assert_eq!(run("|a b c", "cwx<Esc>w."), "x |x c");
        assert_eq!(run("|a\nb\nc", "dd."), "|c");
        assert_eq!(run("|a\nb", "A!<Esc>j."), "a!\nb|!");
        assert_eq!(run("|abc", "x.."), "|");
        assert_eq!(run("|a\nb", "ox<Esc>."), "a\nx\n|x\nb");
        // One undo step for one repeat.
        assert_eq!(run("|a b c", "cwx<Esc>w.u"), "x |b c");
        // A motion in between is not a change.
        assert_eq!(run("|a b c d", "dww."), "b |d");
    }

    #[test]
    fn search() {
        assert_eq!(run("|one two one two", "/two<CR>"), "one |two one two");
        assert_eq!(run("|one two one two", "/two<CR>n"), "one two one |two");
        assert_eq!(run("|one two one two", "/two<CR>nn"), "one |two one two");
        assert_eq!(run("|one two one two", "/two<CR>nN"), "one |two one two");
        assert_eq!(run("one two one |two", "?one<CR>"), "one two |one two");
        assert_eq!(run("|one Two", "/two<CR>"), "one |Two");
        assert_eq!(run("|one two", "/Two<CR>"), "|one two");
        assert_eq!(run("|one two one", "*"), "one two |one");
        assert_eq!(run("one two |one", "#"), "|one two one");
        assert_eq!(run("|a b c", "d/c<CR>"), "|c");
        assert_eq!(run("|one two", "/tx<BS>wo<CR>"), "one |two");
        assert_eq!(run("|one two", "/two<Esc>x"), "|ne two");
        // An invalid pattern is searched for as it stands.
        assert_eq!(run("|a (b", "/(b<CR>"), "a |(b");
    }

    #[test]
    fn escape_cancels_what_is_pending() {
        assert_eq!(run("|a b", "d<Esc>w"), "a |b");
        assert_eq!(run("|a b", "2<Esc>x"), "| b");
        assert_eq!(run("|abc", "dzx"), "|bc");
    }

    #[test]
    fn views() {
        let mut vim = Vim::new();
        let mut buf = load("|a");
        let mut out = Outcome::default();
        for c in "zz".chars() {
            out = vim.key(&mut buf, Key::Char(c), &mut NoClipboard);
        }
        assert_eq!(out.view, Some(View::Center));
        assert!(!out.edited);
        let out = vim.key(&mut buf, Key::Char('x'), &mut NoClipboard);
        assert!(out.edited && out.handled);
        vim.key(&mut buf, Key::Char('i'), &mut NoClipboard);
        let out = vim.key(&mut buf, Key::Char('q'), &mut NoClipboard);
        assert!(!out.handled, "insert mode leaves typing to the editor");
        assert_eq!(vim.mode(), Mode::Insert);
    }

    #[test]
    fn multibyte_text() {
        assert_eq!(run("|é日本 x", "x"), "|日本 x");
        assert_eq!(run("|é日本 x", "dw"), "|x");
        assert_eq!(run("|é日本 x", "$"), "é日本 |x");
        assert_eq!(run("é日|本", "ix<Esc>"), "é日|x本");
        assert_eq!(run("|äb", "~"), "Ä|b");
    }

    #[test]
    fn the_clipboard_registers() {
        struct Clip(String);
        impl Host for Clip {
            fn clipboard(&mut self) -> Option<String> {
                Some(self.0.clone())
            }
            fn set_clipboard(&mut self, text: &str) {
                self.0 = text.to_string();
            }
        }
        let mut clip = Clip("zz".to_string());
        let mut vim = Vim::new();
        let mut buf = load("|foo bar");
        for c in "\"+p".chars() {
            vim.key(&mut buf, Key::Char(c), &mut clip);
        }
        assert_eq!(show(&vim, &buf), "fz|zoo bar");
        for c in "\"+yy".chars() {
            vim.key(&mut buf, Key::Char(c), &mut clip);
        }
        assert_eq!(clip.0, "fzzoo bar\n");
    }
}
