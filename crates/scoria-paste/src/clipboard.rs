//! The clipboard formats gpui does not read, as the app gets them as bytes
//! from the desktop: a file manager's `text/uri-list`, and `text/html`.

/// The local files a `text/uri-list` names, in order, each path's bytes
/// percent-decoded: `file:///home/a/My%20Pic.png` is `/home/a/My Pic.png`,
/// `file://localhost/x` is `/x`. Comments (`#`), blank lines and any URI
/// that is not a local `file:` one are left out.
pub fn uri_list_paths(list: &str) -> Vec<Vec<u8>> {
    list.lines()
        .map(|l| l.trim_end_matches('\r').trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|uri| {
            let scheme = uri.get(..7)?;
            if !scheme.eq_ignore_ascii_case("file://") {
                return None;
            }
            let rest = &uri[7..];
            // The authority: empty, or `localhost` for this machine.
            let slash = rest.find('/')?;
            let host = &rest[..slash];
            if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
                return None;
            }
            Some(percent_decode(&rest[slash..]))
        })
        .collect()
}

fn percent_decode(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = |b: u8| (b as char).to_digit(16);
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

/// Clipboard text as a string: UTF-16 when it opens with a byte-order mark
/// (Firefox has offered `text/html` that way on X11), UTF-8 otherwise, a
/// bad sequence replaced rather than refused.
pub fn decode_text(bytes: &[u8]) -> String {
    let utf16 = |be: bool| {
        let (pairs, _) = bytes[2..].as_chunks::<2>();
        let units: Vec<u16> = pairs
            .iter()
            .map(|&pair| {
                if be {
                    u16::from_be_bytes(pair)
                } else {
                    u16::from_le_bytes(pair)
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xff, 0xfe, ..] => utf16(false),
        [0xfe, 0xff, ..] => utf16(true),
        [0xef, 0xbb, 0xbf, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_uri_list_gives_its_local_files_decoded() {
        let list = "# copied by a file manager\r\nfile:///home/a/My%20Pic.png\r\n\
                    file://localhost/tmp/notes.txt\r\nhttps://example.com/x.png\r\n\
                    file://otherhost/share/x\r\n\r\nfile:///tmp/caf%C3%A9.md\n";
        let paths: Vec<String> = uri_list_paths(list)
            .into_iter()
            .map(|p| String::from_utf8(p).unwrap())
            .collect();
        assert_eq!(
            paths,
            ["/home/a/My Pic.png", "/tmp/notes.txt", "/tmp/café.md"]
        );
    }

    #[test]
    fn a_stray_percent_is_kept() {
        assert_eq!(uri_list_paths("file:///a%2"), vec![b"/a%2".to_vec()]);
        assert_eq!(uri_list_paths("file:///a%zz"), vec![b"/a%zz".to_vec()]);
        assert_eq!(uri_list_paths("file:///50%25"), vec![b"/50%".to_vec()]);
    }

    #[test]
    fn clipboard_text_decodes_by_its_mark() {
        assert_eq!(decode_text(b"<b>x</b>"), "<b>x</b>");
        assert_eq!(decode_text(b"\xef\xbb\xbf<b>x</b>"), "<b>x</b>");
        assert_eq!(decode_text(b"\xff\xfe<\x00b\x00>\x00"), "<b>");
        assert_eq!(decode_text(b"\xfe\xff\x00<\x00b\x00>"), "<b>");
    }
}
