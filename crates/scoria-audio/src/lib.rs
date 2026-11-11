//! The Audio recorder (`audio-recorder:start`, `audio-recorder:stop`), as
//! Obsidian 1.15.0 behaved for the 2026-11-10 probe
//! ([[Specification#Audio recorder]]): `Recording YYYYMMDDHHmmss.m4a`
//! (local time at the stop) in the attachment folder, AAC in an MP4
//! container, `\n![[…]]\n` over the selection of a note being edited.
//!
//! - [`capture`]: the default microphone through cpal, its blocks handed
//!   to a worker thread over a channel the audio callback never waits on.
//! - [`session`]: that worker, encoding each hop as it arrives, and at the
//!   stop draining what is left and writing the file under a temporary
//!   name before renaming it into place. [`session::wait_pending`] lets an
//!   app that is quitting finish every recording first.
//! - [`encode`], [`m4a`]: the encoder around oxideav-aac and the file.
//! - [`file_stem`], [`embed_insertion`]: the name and the note's edit.

pub mod capture;
pub mod encode;
pub mod m4a;
pub mod session;

use chrono::{Datelike, NaiveDateTime, Timelike};

/// The recording's file name without its extension, from the local time
/// at the stop: `Recording 20261110175445`.
pub fn file_stem(at: NaiveDateTime) -> String {
    format!(
        "Recording {:04}{:02}{:02}{:02}{:02}{:02}",
        at.year(),
        at.month(),
        at.day(),
        at.hour(),
        at.minute(),
        at.second()
    )
}

/// The recording's extension.
pub const EXTENSION: &str = "m4a";

/// What replaces the selection of the note the recording was started in:
/// the embed on a line of its own, a newline either side, the caret after
/// the second. `One` + `\n![[Recording ….m4a]]\n` + ` line.` as observed.
pub fn embed_insertion(embed: &str) -> String {
    format!("\n{embed}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_is_the_local_time_at_the_stop() {
        let at = chrono::NaiveDate::from_ymd_opt(2026, 11, 10)
            .unwrap()
            .and_hms_opt(17, 54, 45)
            .unwrap();
        assert_eq!(file_stem(at), "Recording 20261110175445");
        let early = chrono::NaiveDate::from_ymd_opt(2026, 1, 2)
            .unwrap()
            .and_hms_opt(3, 4, 5)
            .unwrap();
        assert_eq!(file_stem(early), "Recording 20260102030405");
    }

    #[test]
    fn the_embed_goes_on_a_line_of_its_own() {
        let link = "![[Recording 20261110175445.m4a]]";
        let text = "One line.";
        let at = 3;
        let out = format!("{}{}{}", &text[..at], embed_insertion(link), &text[at..]);
        assert_eq!(out, "One\n![[Recording 20261110175445.m4a]]\n line.");
    }
}
