//! Attachments that come into the vault from outside it: the names
//! `editor:download-attachments` ("Download attachments for current file")
//! gives what it fetches, which of a note's links it offers, and the text it
//! writes in their place.
//!
//! Every rule here was watched in Obsidian 1.15.0 by
//! `tools/obsidian-probe/attachments-app-info.mjs` (results
//! `2026-11-09-attachments-app-info.json`, `s03`, `s03b`, `s03c`), recorded in
//! `Specification#Observed attachment commands, Obsidian 1.15.0`:
//!
//! - Offered: each Markdown image `![alt](url)` whose destination is `http:`
//!   or `https:`, wherever the parser finds one (list items, quotes, table
//!   cells), once per distinct URL, in document order. Not offered: a plain
//!   link `[x](url)`, `<img src>`, `![[url]]`, an image inside a code span.
//! - Named: the URL's last path segment, query and fragment dropped and its
//!   percent-escapes kept as they are (`two%20words.png`), the extension
//!   lowercased (`UPPER.PNG` → `UPPER.png`) and otherwise kept whatever the
//!   server says the type is (`mismatch.png` served as JPEG stays `.png`).
//!   A segment with no extension (or none at all, `…/dir/`) is `Image` with
//!   the extension of the response's type: `png`, `jpg` for JPEG, `gif`,
//!   `webp`, `svg`; any other type (`text/html`, `application/octet-stream`)
//!   gave `jpg`.
//! - Written: each embed replaced by an embed of the downloaded file, the
//!   alt text before any `|` kept as the display (`![sized|100](…)` →
//!   `![[pic.jpg|sized]]`, an empty alt → `![[photo.png]]`), the title
//!   dropped.
//!
//! Left out because it was not settled: a URL that is the whole value of a
//! frontmatter property is downloaded too in Obsidian (`img: http://…/x.png`
//! became `img: "[[photo.png]]"`), but which values qualify (a `.png` and a
//! `.jpg` did, a `.html` and a list item did not) was not pinned down, so
//! Scoria offers only the body's embeds.

use scoria_markdown::{InlineKind, LinkKind};
use std::ops::Range;

/// The notice when the note has nothing to download.
pub const NOTHING_TO_DOWNLOAD: &str = "There are no external attachments in this note.";

/// The notice after a download that finished: `Downloaded 1 attachment.`,
/// `Downloaded 3 attachments.`
pub fn downloaded_notice(count: usize) -> String {
    if count == 1 {
        "Downloaded 1 attachment.".to_string()
    } else {
        format!("Downloaded {count} attachments.")
    }
}

/// One `![alt](url)` with a remote destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalEmbed {
    /// The bytes of the whole image, `![` to `)`.
    pub range: Range<usize>,
    pub url: String,
    /// The alt text up to its first `|`: what the new embed displays.
    pub alt: String,
}

/// Whether a destination is fetched: `http:` or `https:`, scheme case
/// ignored.
fn is_remote(destination: &str) -> bool {
    let d = destination.trim_start();
    ["http://", "https://"].iter().any(|scheme| {
        d.get(..scheme.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(scheme))
    })
}

/// Every Markdown image in `text` whose destination is remote, in document
/// order, from the one parser.
pub fn external_embeds(text: &str) -> Vec<ExternalEmbed> {
    let doc = scoria_markdown::parse(text);
    let mut out = Vec::new();
    for run in scoria_markdown::inline_runs(&doc.blocks) {
        scoria_markdown::walk_inlines(run, &mut |inline| {
            let InlineKind::Image {
                destination,
                kind: LinkKind::Inline,
                children,
                ..
            } = &inline.kind
            else {
                return;
            };
            if !is_remote(destination) {
                return;
            }
            let alt = scoria_markdown::plain_text(children);
            let alt = alt.split('|').next().unwrap_or("").trim().to_string();
            out.push(ExternalEmbed {
                range: inline.span.start..inline.span.end,
                url: destination.trim().to_string(),
                alt,
            });
        });
    }
    out.sort_by_key(|e| e.range.start);
    out
}

/// The distinct URLs of `embeds`, first appearance first: what the modal
/// offers and the order they are fetched and numbered in.
pub fn distinct_urls(embeds: &[ExternalEmbed]) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    for e in embeds {
        if !urls.contains(&e.url) {
            urls.push(e.url.clone());
        }
    }
    urls
}

/// The stem and extension a download of `url` is saved under, before the
/// attachment folder numbers a taken name; `content_type` is the
/// response's `Content-Type`.
pub fn downloaded_name(url: &str, content_type: Option<&str>) -> (String, String) {
    let after_scheme = url.split_once("://").map_or(url, |(_, rest)| rest);
    let path = after_scheme
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .split_once('/')
        .map_or("", |(_, path)| path);
    let segment = path.rsplit('/').next().unwrap_or("");
    match segment.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => {
            (stem.to_string(), ext.to_ascii_lowercase())
        }
        _ => ("Image".to_string(), extension_for(content_type).to_string()),
    }
}

/// The extension a response's type gives a name that has none.
fn extension_for(content_type: Option<&str>) -> &'static str {
    let mime = content_type
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match mime.as_str() {
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        // JPEG, and every type that is not one of the above.
        _ => "jpg",
    }
}

/// `link` (an embed as autocompletion writes it, `![[x.png]]` or
/// `![](x.png)`) with `alt` as its display text.
pub fn with_alt(link: &str, alt: &str) -> String {
    if let Some(inner) = link
        .strip_prefix("![[")
        .and_then(|rest| rest.strip_suffix("]]"))
    {
        let target = inner.split('|').next().unwrap_or(inner);
        return if alt.is_empty() {
            format!("![[{target}]]")
        } else {
            format!("![[{target}|{alt}]]")
        };
    }
    if let Some(rest) = link.strip_prefix("![") {
        if let Some(close) = rest.find("](") {
            return format!("![{alt}]({}", &rest[close + 2..]);
        }
    }
    link.to_string()
}

/// `text` with every embed of a downloaded URL replaced: `links` maps a URL
/// to the embed that now stands for it (display text not yet added).
/// Embeds of URLs not in `links` keep their bytes.
pub fn rewrite(text: &str, links: &[(String, String)]) -> (String, Vec<(Range<usize>, String)>) {
    let edits: Vec<(Range<usize>, String)> = external_embeds(text)
        .into_iter()
        .filter_map(|e| {
            links
                .iter()
                .find(|(url, _)| *url == e.url)
                .map(|(_, link)| (e.range, with_alt(link, &e.alt)))
        })
        .collect();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for (range, new) in &edits {
        out.push_str(&text[at..range.start]);
        out.push_str(new);
        at = range.end;
    }
    out.push_str(&text[at..]);
    (out, edits)
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: &str = "http://127.0.0.1:9343";

    /// The probe's note, `s03` `root`: what is offered, and in what order.
    #[test]
    fn only_remote_markdown_images_are_offered_once_each() {
        let text = format!(
            "a ![]({M}/img/photo.png) b\n![alt]({M}/img/two%20words.png)\n[link]({M}/img/pic.jpg)\n\
             ![x]({M}/png-noext)\n![j]({M}/jpeg-noext)\n![q]({M}/img/photo.png?size=2)\n\
             ![sized|100]({M}/img/pic.jpg)\n![t]({M}/img/again/photo.png \"a title\")\n\
             <img src=\"{M}/img/pic.jpg\">\n![[{M}/img/photo.png]]\n![pdf]({M}/doc.pdf)\n"
        );
        let embeds = external_embeds(&text);
        let urls: Vec<String> = distinct_urls(&embeds)
            .into_iter()
            .map(|u| u.replace(M, "M"))
            .collect();
        assert_eq!(
            urls,
            [
                "M/img/photo.png",
                "M/img/two%20words.png",
                "M/png-noext",
                "M/jpeg-noext",
                "M/img/photo.png?size=2",
                "M/img/pic.jpg",
                "M/img/again/photo.png",
                "M/doc.pdf"
            ]
        );
        let alts: Vec<&str> = embeds.iter().map(|e| e.alt.as_str()).collect();
        assert_eq!(alts, ["", "alt", "x", "j", "q", "sized", "t", "pdf"]);
    }

    /// `s03b` `contexts`: list items, quotes and table cells count; a code
    /// span does not.
    #[test]
    fn embeds_in_lists_quotes_and_tables_count_and_code_does_not() {
        let text = format!(
            "- ![a]({M}/img/photo.png)\n> ![b]({M}/img/pic.jpg)\n| ![c]({M}/png-noext) |\n| --- |\n`![d]({M}/img/photo.png)`\n"
        );
        let alts: Vec<String> = external_embeds(&text).into_iter().map(|e| e.alt).collect();
        assert_eq!(alts, ["a", "b", "c"]);
        assert!(external_embeds(&format!(
            "no images here [x]({M}/img/pic.jpg) ![[local.png]]\n"
        ))
        .is_empty());
    }

    /// Names, from `s03`, `s03b` and `s03c`.
    #[test]
    fn names_come_from_the_url_and_the_type_only_without_an_extension() {
        let n = |url: &str, ct: &str| {
            let (stem, ext) = downloaded_name(url, Some(ct));
            format!("{stem}.{ext}")
        };
        assert_eq!(n(&format!("{M}/img/photo.png"), "image/png"), "photo.png");
        assert_eq!(
            n(&format!("{M}/img/photo.png?size=2"), "image/png"),
            "photo.png"
        );
        assert_eq!(
            n(&format!("{M}/img/photo.png#frag"), "image/png"),
            "photo.png"
        );
        assert_eq!(
            n(&format!("{M}/img/two%20words.png"), "image/png"),
            "two%20words.png"
        );
        assert_eq!(n(&format!("{M}/img/a%23b.png"), "image/png"), "a%23b.png");
        assert_eq!(
            n(&format!("{M}/img/Grand%C3%A9.gif"), "image/gif"),
            "Grand%C3%A9.gif"
        );
        assert_eq!(n(&format!("{M}/img/UPPER.PNG"), "image/png"), "UPPER.png");
        assert_eq!(
            n(&format!("{M}/img/dots.min.png"), "image/png"),
            "dots.min.png"
        );
        assert_eq!(n(&format!("{M}/img/name.jpeg"), "image/jpeg"), "name.jpeg");
        assert_eq!(
            n(&format!("{M}/img/mismatch.png"), "image/jpeg"),
            "mismatch.png"
        );
        assert_eq!(n(&format!("{M}/img/x.webp"), "text/html"), "x.webp");
        assert_eq!(n(&format!("{M}/doc.pdf"), "application/pdf"), "doc.pdf");
        assert_eq!(n(&format!("{M}/png-noext"), "image/png"), "Image.png");
        assert_eq!(n(&format!("{M}/jpeg-noext"), "image/jpeg"), "Image.jpg");
        assert_eq!(n(&format!("{M}/gif-noext"), "image/gif"), "Image.gif");
        assert_eq!(n(&format!("{M}/webp-noext"), "image/webp"), "Image.webp");
        assert_eq!(n(&format!("{M}/svg-noext"), "image/svg+xml"), "Image.svg");
        assert_eq!(n(&format!("{M}/html-noext"), "text/html"), "Image.jpg");
        assert_eq!(
            n(&format!("{M}/octet-noext"), "application/octet-stream"),
            "Image.jpg"
        );
        assert_eq!(n(&format!("{M}/img/dir/"), "image/png"), "Image.png");
    }

    #[test]
    fn the_display_is_the_alt_text() {
        assert_eq!(with_alt("![[photo.png]]", ""), "![[photo.png]]");
        assert_eq!(with_alt("![[photo.png]]", "alt"), "![[photo.png|alt]]");
        assert_eq!(with_alt("![](photo%201.png)", "q"), "![q](photo%201.png)");
        assert_eq!(with_alt("![photo](photo.png)", ""), "![](photo.png)");
    }

    /// `s03` `root`'s bytes after: every embed of a fetched URL rewritten,
    /// the rest untouched.
    #[test]
    fn the_rewrite_matches_obsidians_bytes() {
        let text = format!(
            "a ![]({M}/img/photo.png) b\n![alt]({M}/img/two%20words.png)\n[link]({M}/img/pic.jpg)\n\
             ![x]({M}/png-noext)\n![q]({M}/img/photo.png?size=2)\n![sized|100]({M}/img/pic.jpg)\n\
             ![t]({M}/img/again/photo.png \"a title\")\n<img src=\"{M}/img/pic.jpg\">\n![[{M}/img/photo.png]]\n"
        );
        let link = |name: &str| format!("![[{name}]]");
        let links = vec![
            (format!("{M}/img/photo.png"), link("photo.png")),
            (format!("{M}/img/two%20words.png"), link("two%20words.png")),
            (format!("{M}/png-noext"), link("Image.png")),
            (format!("{M}/img/photo.png?size=2"), link("photo 1.png")),
            (format!("{M}/img/pic.jpg"), link("pic.jpg")),
            (format!("{M}/img/again/photo.png"), link("photo 2.png")),
        ];
        let (out, edits) = rewrite(&text, &links);
        assert_eq!(edits.len(), 6);
        assert_eq!(
            out,
            format!(
                "a ![[photo.png]] b\n![[two%20words.png|alt]]\n[link]({M}/img/pic.jpg)\n\
                 ![[Image.png|x]]\n![[photo 1.png|q]]\n![[pic.jpg|sized]]\n\
                 ![[photo 2.png|t]]\n<img src=\"{M}/img/pic.jpg\">\n![[{M}/img/photo.png]]\n"
            )
        );
        // The same URL twice is one download and two rewritten embeds.
        let twice = format!("![a]({M}/img/photo.png)\n![b]({M}/img/photo.png)\n");
        let (out, _) = rewrite(&twice, &[(format!("{M}/img/photo.png"), link("photo.png"))]);
        assert_eq!(out, "![[photo.png|a]]\n![[photo.png|b]]\n");
        // A deselected URL keeps its bytes.
        let two = format!("![a]({M}/img/photo.png)\n![b]({M}/img/two%20words.png)\n");
        let (out, _) = rewrite(&two, &[(format!("{M}/img/photo.png"), link("photo.png"))]);
        assert_eq!(
            out,
            format!("![[photo.png|a]]\n![b]({M}/img/two%20words.png)\n")
        );
    }

    #[test]
    fn the_notices_are_obsidians() {
        assert_eq!(downloaded_notice(1), "Downloaded 1 attachment.");
        assert_eq!(downloaded_notice(3), "Downloaded 3 attachments.");
        assert_eq!(
            NOTHING_TO_DOWNLOAD,
            "There are no external attachments in this note."
        );
    }
}
