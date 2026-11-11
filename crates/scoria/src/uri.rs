//! `obsidian://` URIs: the four actions Obsidian documents, parsed into
//! what the workspace does for each.
//!
//! Implemented from Obsidian's public help ("Obsidian URI"), which is the
//! whole specification here; no URI was driven against a running Obsidian.
//! The scheme is `obsidian://<action>?<params>`, every parameter value
//! percent-encoded:
//!
//! - `open` — `vault` (name or id), and either `file` (a note's name, the
//!   way a wikilink names it, `.md` implied) or `path` (vault-relative, the
//!   extension included);
//! - `new` — `vault`, the note as `name` or `file` (`.md` implied) or
//!   `path`, `content`, and the flags `silent` (do not open it), `append`,
//!   `prepend` and `overwrite` for a note that exists;
//! - `search` — `vault` and `query`;
//! - `daily` — `vault`: today's daily note.
//!
//! A `vault` naming another vault is refused rather than opened: Scoria
//! shows one vault per window and does not switch under a URI. What the
//! desktop entry registers is `scoria://`, the same grammar under Scoria's
//! own name, because Obsidian owns `obsidian://` on a machine that has it
//! (`tools/linux-desktop/install.sh --obsidian-scheme` claims the other one
//! on a machine that has not). Either way the URI reaches Scoria as a
//! command-line argument, which a second run hands to the window already
//! open, or over the control socket's `uri` method.

/// One parsed URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Uri {
    Open {
        vault: Option<String>,
        file: Option<String>,
        path: Option<String>,
    },
    New {
        vault: Option<String>,
        /// `name` or `file`: a note's name, `.md` implied.
        name: Option<String>,
        /// `path`: vault-relative, the extension included.
        path: Option<String>,
        content: Option<String>,
        silent: bool,
        append: bool,
        prepend: bool,
        overwrite: bool,
    },
    Search {
        vault: Option<String>,
        query: Option<String>,
    },
    Daily {
        vault: Option<String>,
    },
}

impl Uri {
    /// The `vault` parameter, which names a vault by its folder name or by
    /// its id in the machine's list. A second `scoria` reads it to decide
    /// which window to hand the URI to.
    pub fn vault(&self) -> Option<&str> {
        match self {
            Uri::Open { vault, .. }
            | Uri::New { vault, .. }
            | Uri::Search { vault, .. }
            | Uri::Daily { vault } => vault.as_deref(),
        }
    }
}

/// Parses an `obsidian://` (or `scoria://`) URI, or says why it could not.
pub fn parse(uri: &str) -> Result<Uri, String> {
    let trimmed = uri.trim();
    let rest = trimmed
        .strip_prefix("obsidian://")
        .or_else(|| trimmed.strip_prefix("scoria://"))
        .ok_or_else(|| format!("not an obsidian:// URI: {uri}"))?;
    let (action, query) = match rest.split_once('?') {
        Some((action, query)) => (action, query),
        None => (rest, ""),
    };
    let action = action.trim_end_matches('/').to_ascii_lowercase();
    let params: Vec<(String, String)> = query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (decode(k).to_ascii_lowercase(), decode(v)),
            None => (decode(p).to_ascii_lowercase(), String::new()),
        })
        .collect();
    let get = |name: &str| -> Option<String> {
        params
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .filter(|v| !v.is_empty())
    };
    let flag = |name: &str| -> bool { params.iter().any(|(k, _)| k == name) };
    Ok(match action.as_str() {
        "open" => Uri::Open {
            vault: get("vault"),
            file: get("file"),
            path: get("path"),
        },
        "new" => Uri::New {
            vault: get("vault"),
            name: get("name").or_else(|| get("file")),
            path: get("path"),
            content: get("content"),
            silent: flag("silent"),
            append: flag("append"),
            prepend: flag("prepend"),
            overwrite: flag("overwrite"),
        },
        "search" => Uri::Search {
            vault: get("vault"),
            query: get("query"),
        },
        "daily" => Uri::Daily {
            vault: get("vault"),
        },
        other => return Err(format!("unknown obsidian:// action: {other}")),
    })
}

/// Percent-decoding, with `+` left alone: Obsidian's help encodes a space
/// as `%20`, and a `+` in a note's name is a `+`.
fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = &s[i + 1..i + 3];
            if let Ok(b) = u8::from_str_radix(hex, 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Whether `vault` names this vault: its folder name, compared without
/// regard to case the way every other name is.
pub fn names_vault(vault: Option<&str>, vault_name: &str) -> bool {
    match vault {
        None => true,
        Some(v) => v.eq_ignore_ascii_case(vault_name),
    }
}

/// What `workspace:copy-url` ("Copy Obsidian URL for current file") puts
/// on the clipboard for the vault-relative `path` in the vault folder
/// `vault_name`: `obsidian://open?vault=<name>&file=<path>`, both values
/// encoded the way JavaScript's `encodeURIComponent` encodes them, and the
/// path's `.md` dropped while any other extension stays (Obsidian 1.15.0,
/// `tools/obsidian-probe/results/2026-11-09-app-window-commands.json`).
/// The scheme is Obsidian's own: the URL is for pasting where Obsidian, or
/// a Scoria registered for the scheme, will open it.
pub fn open_url(vault_name: &str, path: &str) -> String {
    let file = path.strip_suffix(".md").unwrap_or(path);
    format!(
        "obsidian://open?vault={}&file={}",
        encode_component(vault_name),
        encode_component(file)
    )
}

/// `encodeURIComponent`: every byte of the UTF-8 text escaped as `%XX` in
/// upper-case hex, bar the letters, the digits and `-_.!~*'()`.
pub fn encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The probe's three files in the vault folder `Probe Vault`.
    #[test]
    fn the_copied_url_encodes_both_parts_and_drops_only_md() {
        assert_eq!(
            open_url("Probe Vault", "Sub/My Note.md"),
            "obsidian://open?vault=Probe%20Vault&file=Sub%2FMy%20Note"
        );
        assert_eq!(
            open_url("Probe Vault", "Sub/Caf\u{e9} & Bar #2.md"),
            "obsidian://open?vault=Probe%20Vault&file=Sub%2FCaf%C3%A9%20%26%20Bar%20%232"
        );
        assert_eq!(
            open_url("Probe Vault", "Note A.md"),
            "obsidian://open?vault=Probe%20Vault&file=Note%20A"
        );
        assert_eq!(
            open_url("Probe Vault", "img/pic.png"),
            "obsidian://open?vault=Probe%20Vault&file=img%2Fpic.png"
        );
    }

    #[test]
    fn encoding_a_component_leaves_what_encode_uri_component_leaves() {
        assert_eq!(encode_component("aZ09-_.!~*'()"), "aZ09-_.!~*'()");
        assert_eq!(encode_component("a+b=c?d/e%f"), "a%2Bb%3Dc%3Fd%2Fe%25f");
        assert_eq!(encode_component("\u{1f525}"), "%F0%9F%94%A5");
        // What the URL is built from reads back through the parser.
        let url = open_url("my vault", "Daily/2026-11-09 #3.md");
        assert_eq!(
            parse(&url).unwrap(),
            Uri::Open {
                vault: Some("my vault".into()),
                file: Some("Daily/2026-11-09 #3".into()),
                path: None,
            }
        );
    }

    #[test]
    fn open_takes_a_file_or_a_path_and_decodes_percent_escapes() {
        assert_eq!(
            parse("obsidian://open?vault=my%20vault&file=Daily%20notes%2F2026-11-02").unwrap(),
            Uri::Open {
                vault: Some("my vault".into()),
                file: Some("Daily notes/2026-11-02".into()),
                path: None,
            }
        );
        assert_eq!(
            parse("obsidian://open?path=Rocks%2FBasalt.md").unwrap(),
            Uri::Open {
                vault: None,
                file: None,
                path: Some("Rocks/Basalt.md".into()),
            }
        );
        // A `+` is itself, and an empty value is no value.
        assert_eq!(
            parse("obsidian://open?file=a+b&vault=").unwrap(),
            Uri::Open {
                vault: None,
                file: Some("a+b".into()),
                path: None,
            }
        );
    }

    #[test]
    fn new_carries_its_content_and_flags() {
        let uri =
            parse("obsidian://new?vault=v&name=Ideas&content=Hello%0Aworld&append&silent").unwrap();
        assert_eq!(
            uri,
            Uri::New {
                vault: Some("v".into()),
                name: Some("Ideas".into()),
                path: None,
                content: Some("Hello\nworld".into()),
                silent: true,
                append: true,
                prepend: false,
                overwrite: false,
            }
        );
        // `file` is the older spelling of `name`.
        assert!(matches!(
            parse("obsidian://new?file=Later").unwrap(),
            Uri::New { name: Some(n), .. } if n == "Later"
        ));
    }

    #[test]
    fn search_daily_and_the_refusals() {
        assert_eq!(
            parse("obsidian://search?query=tag%3A%23rock").unwrap(),
            Uri::Search {
                vault: None,
                query: Some("tag:#rock".into()),
            }
        );
        assert_eq!(
            parse("obsidian://daily?vault=x").unwrap(),
            Uri::Daily {
                vault: Some("x".into())
            }
        );
        assert!(parse("obsidian://hook-get-address").is_err());
    }

    #[test]
    fn scoria_is_the_same_scheme_under_its_own_name() {
        assert_eq!(
            parse("scoria://open?vault=rocks&file=Basalt").unwrap(),
            parse("obsidian://open?vault=rocks&file=Basalt").unwrap()
        );
        assert!(parse("https://obsidian.md/open?file=Basalt").is_err());
    }

    #[test]
    fn the_vault_parameter_is_readable_for_routing() {
        assert_eq!(
            parse("obsidian://open?vault=rocks&file=Basalt")
                .unwrap()
                .vault(),
            Some("rocks")
        );
        assert_eq!(
            parse("obsidian://daily?vault=x").unwrap().vault(),
            Some("x")
        );
        assert_eq!(parse("obsidian://open?file=Basalt").unwrap().vault(), None);
        assert!(parse("https://example.invalid").is_err());
        assert!(names_vault(None, "Any"));
        assert!(names_vault(Some("demo"), "Demo"));
        assert!(!names_vault(Some("other"), "Demo"));
    }
}
