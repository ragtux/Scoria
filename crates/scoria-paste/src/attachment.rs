//! The names a pasted or dropped attachment is saved under, before the
//! attachment folder numbers a taken one (`scoria_vault`'s
//! `available_path`: a space and the next free integer, the whole file name
//! compared).
//!
//! Observed in Obsidian 1.14.4 and 1.14.5
//! (`Specification#Observed attachment paste behavior, Obsidian 1.14.4`, and
//! its 2026-10-25 and 2026-11-15 passes):
//!
//! - An image on the system clipboard is always saved as a PNG named
//!   `Pasted image ` plus the paste's **local** time as fourteen digits,
//!   `yyyyMMddHHmmss`: `Pasted image 20261025103853.png`.
//! - A file — dragged in from a file manager, or picked — keeps its own
//!   name: `photo.png`, `notes.txt`, `Outside Note.md`.
//! - Several files dropped at once are embedded in the order they came,
//!   one blank line between two embeds (`![[a.png]]\n\n![[b.txt]]`).

/// A local civil time, to the second: what a pasted image is named after.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Civil {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// What came in, as far as its name goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Incoming<'a> {
    /// Image data on the clipboard, with no file behind it.
    ClipboardImage,
    /// A file, by its file name (`photo.png`).
    File(&'a str),
}

/// The stem every pasted image shares: `Pasted image 20261025103853`.
pub fn pasted_image_stem(at: Civil) -> String {
    format!(
        "Pasted image {:04}{:02}{:02}{:02}{:02}{:02}",
        at.year, at.month, at.day, at.hour, at.minute, at.second
    )
}

/// The stem and extension `item` is saved under, pasted or dropped at `at`.
pub fn saved_name(item: Incoming<'_>, at: Civil) -> (String, String) {
    match item {
        Incoming::ClipboardImage => (pasted_image_stem(at), "png".to_string()),
        Incoming::File(name) => split_name(name),
    }
}

/// A file name split at its last dot into stem and extension; a name with
/// no dot, or only a leading one (`.gitignore`), is all stem.
pub fn split_name(name: &str) -> (String, String) {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), ext.to_string()),
        _ => (name.to_string(), String::new()),
    }
}

/// The text a paste or a drop of several attachments writes: their embeds
/// in order, a blank line between two.
pub fn joined_embeds<S: AsRef<str>>(embeds: &[S]) -> String {
    let parts: Vec<&str> = embeds.iter().map(|e| e.as_ref()).collect();
    parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: Civil = Civil {
        year: 2026,
        month: 10,
        day: 25,
        hour: 10,
        minute: 38,
        second: 3,
    };

    #[test]
    fn a_clipboard_image_is_a_png_named_after_the_local_second() {
        assert_eq!(
            saved_name(Incoming::ClipboardImage, AT),
            ("Pasted image 20261025103803".to_string(), "png".to_string())
        );
        // Every field is zero-padded to its width.
        let early = Civil {
            year: 2027,
            month: 1,
            day: 2,
            hour: 3,
            minute: 4,
            second: 5,
        };
        assert_eq!(pasted_image_stem(early), "Pasted image 20270102030405");
    }

    #[test]
    fn a_file_keeps_its_own_name() {
        for (name, stem, ext) in [
            ("photo.png", "photo", "png"),
            ("second shot.png", "second shot", "png"),
            ("notes.txt", "notes", "txt"),
            ("Outside Note.md", "Outside Note", "md"),
            ("archive.tar.gz", "archive.tar", "gz"),
            ("README", "README", ""),
            (".gitignore", ".gitignore", ""),
        ] {
            assert_eq!(
                saved_name(Incoming::File(name), AT),
                (stem.to_string(), ext.to_string()),
                "{name}"
            );
        }
    }

    #[test]
    fn several_embeds_are_a_blank_line_apart() {
        assert_eq!(
            joined_embeds(&["![[photo 3.png]]", "![[notes 1.txt]]"]),
            "![[photo 3.png]]\n\n![[notes 1.txt]]"
        );
        assert_eq!(joined_embeds(&["![[one.png]]"]), "![[one.png]]");
        assert_eq!(joined_embeds::<&str>(&[]), "");
    }
}
