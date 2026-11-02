//! Scoria's command line: everything the arguments decide before a window
//! exists.
//!
//! The app is a GPUI binary, and a GPUI binary is slow to build and
//! impossible to test without a display, so the argument handling lives
//! here instead: [`args`] parses the flags and writes the help text from
//! one table, [`target`] turns a path argument into the vault that holds
//! it, and [`instance`] is the part a desktop needs — a second `scoria` on
//! a vault that is already open hands its note, file or URI to that window
//! and raises it rather than starting a second copy.
//!
//! Obsidian itself has no command line to copy: its binary takes a URI and
//! Electron's own flags and nothing else (`obsidian --help` opens a
//! window). So this is Scoria's own surface, and the only behaviour it
//! borrows is the one a person would notice: one window per vault, raised
//! by the second launch, which is what Electron's single-instance lock
//! gives Obsidian.

pub mod args;
pub mod instance;
pub mod target;

pub use args::{parse, usage, version, Args, Flag, FLAGS};
pub use instance::{forward, socket_dir, socket_for, Forward};
pub use target::{resolve, Target};
