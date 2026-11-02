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
//! shows one vault per window and does not switch under a URI. The scheme
//! is **not** registered with the desktop -- Obsidian owns `obsidian://` on
//! a machine that has it -- so a URI reaches Scoria as a command-line
//! argument or over the control socket.

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

/// Parses an `obsidian://` URI, or says why it could not.
pub fn parse(uri: &str) -> Result<Uri, String> {
    let rest = uri
        .trim()
        .strip_prefix("obsidian://")
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(parse("https://example.invalid").is_err());
        assert!(names_vault(None, "Any"));
        assert!(names_vault(Some("demo"), "Demo"));
        assert!(!names_vault(Some("other"), "Demo"));
    }
}
