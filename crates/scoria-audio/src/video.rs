//! What a video file holds, without decoding a frame: its picture size, its
//! length and its codecs, for the card Scoria draws where Obsidian has a
//! `<video controls>` (Scoria has no video decoder; see
//! [[Development#Canvas embeds, the audio player and video, 2026-11-15]]
//! for why).
//!
//! Two container families cover the extensions Obsidian opens as video
//! (`mp4 webm ogv mov mkv`):
//!
//! - ISO base media (MP4, QuickTime `.mov`, `.m4v`): the `moov` box's `mvhd`
//!   gives the length, each `trak`'s `tkhd` the picture size (turned by its
//!   matrix, as a phone's portrait clip is), `hdlr` whether it is video or
//!   sound and `stsd` the codec's four characters. `mdat` is skipped over,
//!   never read, wherever it sits.
//! - Matroska and WebM (EBML): `Info`'s `TimestampScale` and `Duration`,
//!   and each `TrackEntry`'s `TrackType`, `CodecID` and `PixelWidth` /
//!   `PixelHeight`. Clusters are skipped.
//!
//! An Ogg file (`.ogv`) has its Theora picture size read from the first
//! header, and no length.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// What a video file says about itself. Anything the file does not record
/// is `None`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VideoInfo {
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Seconds.
    pub duration: Option<f64>,
    /// The video track's codec as the container names it: `avc1`, `hvc1`,
    /// `V_VP9`, `theora`.
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
}

/// A codec's everyday name, from the name its container gives it: `avc1`
/// and `V_MPEG4/ISO/AVC` are both H.264. An unknown one is shown as written.
pub fn codec_name(codec: &str) -> String {
    let named = match codec {
        "avc1" | "avc3" | "V_MPEG4/ISO/AVC" => "H.264",
        "hvc1" | "hev1" | "V_MPEGH/ISO/HEVC" => "HEVC",
        "av01" | "V_AV1" => "AV1",
        "vp09" | "V_VP9" => "VP9",
        "vp08" | "V_VP8" => "VP8",
        "mp4v" => "MPEG-4",
        "theora" | "V_THEORA" => "Theora",
        "mp4a" | "A_AAC" => "AAC",
        "Opus" | "opus" | "A_OPUS" => "Opus",
        "vorbis" | "A_VORBIS" => "Vorbis",
        ".mp3" | "A_MPEG/L3" => "MP3",
        "ac-3" | "A_AC3" => "AC-3",
        "ec-3" | "A_EAC3" => "E-AC-3",
        "flac" | "fLaC" | "A_FLAC" => "FLAC",
        other => return other.to_string(),
    };
    named.to_string()
}

/// Reads `path`'s header. An unknown container is an error; a known one
/// missing a field leaves it `None`.
pub fn probe(path: &Path) -> Result<VideoInfo, String> {
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    let len = f.metadata().map_err(|e| e.to_string())?.len();
    let mut head = [0u8; 12];
    let n = read_up_to(&mut f, &mut head).map_err(|e| e.to_string())?;
    f.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    if n >= 4 && head[..4] == [0x1A, 0x45, 0xDF, 0xA3] {
        return matroska(&mut f, len);
    }
    if n >= 4 && &head[..4] == b"OggS" {
        return ogg(&mut f);
    }
    if n >= 8 && is_box_type(&head[4..8]) {
        return iso(&mut f, len);
    }
    Err("This is not a video file Scoria can read".into())
}

fn read_up_to(f: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut got = 0;
    while got < buf.len() {
        match f.read(&mut buf[got..])? {
            0 => break,
            n => got += n,
        }
    }
    Ok(got)
}

fn is_box_type(t: &[u8]) -> bool {
    matches!(
        t,
        b"ftyp" | b"moov" | b"mdat" | b"wide" | b"free" | b"skip" | b"pnot"
    )
}

// ---- ISO base media -------------------------------------------------------

/// The boxes worth descending into on the way to the track headers.
const CONTAINERS: [&[u8; 4]; 5] = [b"moov", b"trak", b"mdia", b"minf", b"stbl"];

#[derive(Default)]
struct Track {
    width: f64,
    height: f64,
    turned: bool,
    handler: [u8; 4],
    codec: Option<String>,
}

fn iso(f: &mut File, len: u64) -> Result<VideoInfo, String> {
    let mut info = VideoInfo::default();
    let mut tracks: Vec<Track> = Vec::new();
    walk_boxes(f, 0, len, &mut info, &mut tracks, 0).map_err(|e| e.to_string())?;
    if let Some(v) = tracks.iter().find(|t| &t.handler == b"vide") {
        let (w, h) = if v.turned {
            (v.height, v.width)
        } else {
            (v.width, v.height)
        };
        if w > 0.0 && h > 0.0 {
            info.width = Some(w.round() as u32);
            info.height = Some(h.round() as u32);
        }
        info.video_codec = v.codec.clone();
    }
    info.audio_codec = tracks
        .iter()
        .find(|t| &t.handler == b"soun")
        .and_then(|t| t.codec.clone());
    Ok(info)
}

fn walk_boxes(
    f: &mut File,
    start: u64,
    end: u64,
    info: &mut VideoInfo,
    tracks: &mut Vec<Track>,
    depth: usize,
) -> std::io::Result<()> {
    let mut at = start;
    while at + 8 <= end && depth < 8 {
        f.seek(SeekFrom::Start(at))?;
        let mut h = [0u8; 8];
        if read_up_to(f, &mut h)? < 8 {
            break;
        }
        let mut size = u32::from_be_bytes([h[0], h[1], h[2], h[3]]) as u64;
        let kind: [u8; 4] = [h[4], h[5], h[6], h[7]];
        let mut header = 8;
        if size == 1 {
            let mut big = [0u8; 8];
            read_up_to(f, &mut big)?;
            size = u64::from_be_bytes(big);
            header = 16;
        } else if size == 0 {
            size = end - at;
        }
        if size < header || at + size > end {
            break;
        }
        let body = at + header;
        let body_len = size - header;
        if CONTAINERS.contains(&&kind) {
            if &kind == b"trak" {
                tracks.push(Track::default());
            }
            walk_boxes(f, body, body + body_len, info, tracks, depth + 1)?;
        } else if matches!(&kind, b"mvhd" | b"tkhd" | b"hdlr" | b"stsd") {
            let mut b = vec![0u8; body_len.min(256) as usize];
            read_up_to(f, &mut b)?;
            match &kind {
                b"mvhd" => info.duration = mvhd(&b),
                b"tkhd" => {
                    if let Some(t) = tracks.last_mut() {
                        tkhd(&b, t);
                    }
                }
                // The media's handler is `mdia`'s, the first in the track:
                // QuickTime puts a data handler (`alis`) in `minf` after it.
                b"hdlr" => {
                    if let (Some(t), Some(h)) = (tracks.last_mut(), b.get(8..12)) {
                        if t.handler == [0; 4] {
                            t.handler.copy_from_slice(h);
                        }
                    }
                }
                _ => {
                    // stsd: version and flags, the entry count, then the
                    // first entry's size and its four-character type.
                    if let (Some(t), Some(c)) = (tracks.last_mut(), b.get(12..16)) {
                        t.codec = Some(String::from_utf8_lossy(c).trim().to_string());
                    }
                }
            }
        }
        at += size;
    }
    Ok(())
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

fn be64(b: &[u8], at: usize) -> Option<u64> {
    b.get(at..at + 8).map(|s| {
        let mut a = [0u8; 8];
        a.copy_from_slice(s);
        u64::from_be_bytes(a)
    })
}

/// `mvhd`: the movie's length, in its time scale.
fn mvhd(b: &[u8]) -> Option<f64> {
    let (scale, duration) = if b.first() == Some(&1) {
        (be32(b, 20)?, be64(b, 24)?)
    } else {
        (be32(b, 12)?, be32(b, 16)? as u64)
    };
    // All ones is "unknown".
    if scale == 0 || duration == u32::MAX as u64 || duration == u64::MAX {
        return None;
    }
    Some(duration as f64 / scale as f64)
}

/// `tkhd`: the picture size (16.16 fixed) and whether the matrix turns it a
/// quarter.
fn tkhd(b: &[u8], t: &mut Track) {
    let base = if b.first() == Some(&1) {
        4 + 32
    } else {
        4 + 20
    };
    // reserved 8, layer 2, alternate group 2, volume 2, reserved 2.
    let matrix = base + 16;
    let size = matrix + 36;
    if let (Some(a), Some(d), Some(w), Some(h)) = (
        be32(b, matrix),
        be32(b, matrix + 16),
        be32(b, size),
        be32(b, size + 4),
    ) {
        t.width = w as f64 / 65536.0;
        t.height = h as f64 / 65536.0;
        t.turned = a == 0 && d == 0;
    }
}

// ---- Matroska / WebM ------------------------------------------------------

const SEGMENT: u32 = 0x1853_8067;
const INFO: u32 = 0x1549_A966;
const TIMESTAMP_SCALE: u32 = 0x2A_D7B1;
const DURATION: u32 = 0x4489;
const TRACKS: u32 = 0x1654_AE6B;
const TRACK_ENTRY: u32 = 0xAE;
const TRACK_TYPE: u32 = 0x83;
const CODEC_ID: u32 = 0x86;
const VIDEO: u32 = 0xE0;
const PIXEL_WIDTH: u32 = 0xB0;
const PIXEL_HEIGHT: u32 = 0xBA;
const CLUSTER: u32 = 0x1F43_B675;

/// An EBML variable-length integer: (value, its length). The ID form keeps
/// its marker bit; the size form drops it, and all ones means unknown.
fn vint(f: &mut impl Read, keep_marker: bool) -> std::io::Result<Option<(u64, u32, bool)>> {
    let mut first = [0u8; 1];
    if read_up_to(f, &mut first)? == 0 {
        return Ok(None);
    }
    let len = first[0].leading_zeros() + 1;
    if len > 8 {
        return Ok(None);
    }
    // The value bits of the first byte: those after the length's marker.
    let mask = (0xFFu64 >> len) as u8;
    let mut v = if keep_marker {
        first[0] as u64
    } else {
        (first[0] & mask) as u64
    };
    let mut all_ones = v == mask as u64;
    for _ in 1..len {
        let mut b = [0u8; 1];
        if read_up_to(f, &mut b)? == 0 {
            return Ok(None);
        }
        all_ones &= b[0] == 0xFF;
        v = (v << 8) | b[0] as u64;
    }
    Ok(Some((v, len, all_ones && !keep_marker)))
}

struct MkvTrack {
    kind: u64,
    codec: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
}

fn matroska(f: &mut File, len: u64) -> Result<VideoInfo, String> {
    let mut scale = 1_000_000u64;
    let mut duration = None;
    let mut tracks: Vec<MkvTrack> = Vec::new();
    mkv_walk(f, 0, len, &mut scale, &mut duration, &mut tracks, 0).map_err(|e| e.to_string())?;
    let mut info = VideoInfo {
        duration: duration.map(|d: f64| d * scale as f64 / 1e9),
        ..Default::default()
    };
    if let Some(v) = tracks.iter().find(|t| t.kind == 1) {
        info.width = v.width;
        info.height = v.height;
        info.video_codec = v.codec.clone();
    }
    info.audio_codec = tracks
        .iter()
        .find(|t| t.kind == 2)
        .and_then(|t| t.codec.clone());
    Ok(info)
}

fn read_uint(f: &mut impl Read, size: u64) -> std::io::Result<u64> {
    let mut v = 0u64;
    for _ in 0..size.min(8) {
        let mut b = [0u8; 1];
        read_up_to(f, &mut b)?;
        v = (v << 8) | b[0] as u64;
    }
    Ok(v)
}

fn mkv_walk(
    f: &mut File,
    start: u64,
    end: u64,
    scale: &mut u64,
    duration: &mut Option<f64>,
    tracks: &mut Vec<MkvTrack>,
    depth: usize,
) -> std::io::Result<()> {
    let mut at = start;
    while at < end && depth < 6 {
        f.seek(SeekFrom::Start(at))?;
        let Some((id, id_len, _)) = vint(f, true)? else {
            break;
        };
        let Some((size, size_len, unknown)) = vint(f, false)? else {
            break;
        };
        let body = at + id_len as u64 + size_len as u64;
        let id = id as u32;
        // An unknown size runs to the end of its parent; only a segment or
        // a cluster is written that way in practice.
        let size = if unknown { end - body } else { size };
        let next = body.saturating_add(size).min(end);
        match id {
            SEGMENT | INFO | TRACKS | VIDEO => {
                mkv_walk(f, body, next, scale, duration, tracks, depth + 1)?;
            }
            TRACK_ENTRY => {
                tracks.push(MkvTrack {
                    kind: 0,
                    codec: None,
                    width: None,
                    height: None,
                });
                mkv_walk(f, body, next, scale, duration, tracks, depth + 1)?;
            }
            TIMESTAMP_SCALE => *scale = read_uint(f, size)?.max(1),
            DURATION => {
                let mut b = vec![0u8; size.min(8) as usize];
                read_up_to(f, &mut b)?;
                *duration = match b.len() {
                    4 => Some(f32::from_be_bytes([b[0], b[1], b[2], b[3]]) as f64),
                    8 => Some(f64::from_be_bytes([
                        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
                    ])),
                    _ => None,
                };
            }
            TRACK_TYPE => {
                let v = read_uint(f, size)?;
                if let Some(t) = tracks.last_mut() {
                    t.kind = v;
                }
            }
            CODEC_ID => {
                let mut b = vec![0u8; size.min(64) as usize];
                read_up_to(f, &mut b)?;
                if let Some(t) = tracks.last_mut() {
                    t.codec = Some(
                        String::from_utf8_lossy(&b)
                            .trim_end_matches('\0')
                            .to_string(),
                    );
                }
            }
            PIXEL_WIDTH | PIXEL_HEIGHT => {
                let v = read_uint(f, size)? as u32;
                if let Some(t) = tracks.last_mut() {
                    if id == PIXEL_WIDTH {
                        t.width = Some(v);
                    } else {
                        t.height = Some(v);
                    }
                }
            }
            // The media itself: nothing past the first cluster is needed
            // once the tracks are read, and an unknown-sized cluster has
            // no end to skip to.
            CLUSTER if !tracks.is_empty() => break,
            _ => {}
        }
        if unknown && id != SEGMENT {
            break;
        }
        at = next;
    }
    Ok(())
}

// ---- Ogg ------------------------------------------------------------------

fn ogg(f: &mut File) -> Result<VideoInfo, String> {
    let mut head = vec![0u8; 8192];
    let n = read_up_to(f, &mut head).map_err(|e| e.to_string())?;
    head.truncate(n);
    let mut info = VideoInfo::default();
    if let Some(at) = head.windows(7).position(|w| w == b"\x80theora") {
        // Identification header: version (3), frame size in macroblocks
        // (2 + 2), then the picture's width and height (24 bits each).
        let p = at + 7 + 3 + 4;
        if let Some(b) = head.get(p..p + 6) {
            let w = u32::from_be_bytes([0, b[0], b[1], b[2]]);
            let h = u32::from_be_bytes([0, b[3], b[4], b[5]]);
            info.width = Some(w);
            info.height = Some(h);
        }
        info.video_codec = Some("theora".into());
    }
    if head.windows(7).any(|w| w == b"\x01vorbis") {
        info.audio_codec = Some("vorbis".into());
    } else if head.windows(8).any(|w| w == b"OpusHead") {
        info.audio_codec = Some("opus".into());
    }
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(kind);
        v.extend_from_slice(body);
        v
    }

    fn tkhd_body(w: u32, h: u32, turned: bool) -> Vec<u8> {
        let mut b = vec![0u8; 4 + 20 + 16];
        let (a, d, bb, c) = if turned {
            (0u32, 0u32, 0x0001_0000u32, 0xFFFF_0000u32)
        } else {
            (0x0001_0000, 0x0001_0000, 0, 0)
        };
        for v in [a, bb, 0, c, d, 0, 0, 0, 0x4000_0000] {
            b.extend_from_slice(&v.to_be_bytes());
        }
        b.extend_from_slice(&(w << 16).to_be_bytes());
        b.extend_from_slice(&(h << 16).to_be_bytes());
        b
    }

    fn mp4(turned: bool) -> Vec<u8> {
        let mut mvhd = vec![0u8; 12];
        mvhd.extend_from_slice(&1000u32.to_be_bytes());
        mvhd.extend_from_slice(&3000u32.to_be_bytes());
        mvhd.extend(vec![0u8; 80]);
        let hdlr = |t: &[u8; 4]| {
            let mut b = vec![0u8; 8];
            b.extend_from_slice(t);
            b.extend(vec![0u8; 13]);
            b
        };
        let stsd = |c: &[u8; 4]| {
            let mut b = vec![0u8; 4];
            b.extend_from_slice(&1u32.to_be_bytes());
            b.extend_from_slice(&16u32.to_be_bytes());
            b.extend_from_slice(c);
            b.extend(vec![0u8; 8]);
            b
        };
        let video = bx(
            b"trak",
            &[
                bx(b"tkhd", &tkhd_body(320, 240, turned)),
                bx(
                    b"mdia",
                    &[
                        bx(b"hdlr", &hdlr(b"vide")),
                        // QuickTime's data handler, after the media's.
                        bx(
                            b"minf",
                            &[
                                bx(b"hdlr", &hdlr(b"alis")),
                                bx(b"stbl", &bx(b"stsd", &stsd(b"avc1"))),
                            ]
                            .concat(),
                        ),
                    ]
                    .concat(),
                ),
            ]
            .concat(),
        );
        let sound = bx(
            b"trak",
            &[
                bx(b"tkhd", &tkhd_body(0, 0, false)),
                bx(
                    b"mdia",
                    &[
                        bx(b"hdlr", &hdlr(b"soun")),
                        bx(b"minf", &bx(b"stbl", &bx(b"stsd", &stsd(b"mp4a")))),
                    ]
                    .concat(),
                ),
            ]
            .concat(),
        );
        // mdat first, the way a file that was never "fast-started" is.
        [
            bx(b"ftyp", b"isom\0\0\x02\0isomiso2avc1mp41"),
            bx(b"mdat", &vec![0xAB; 5000]),
            bx(b"moov", &[bx(b"mvhd", &mvhd), video, sound].concat()),
        ]
        .concat()
    }

    #[test]
    fn codecs_go_by_their_everyday_names() {
        assert_eq!(codec_name("avc1"), "H.264");
        assert_eq!(codec_name("V_MPEG4/ISO/AVC"), "H.264");
        assert_eq!(codec_name("V_VP9"), "VP9");
        assert_eq!(codec_name("hvc1"), "HEVC");
        assert_eq!(codec_name("mp4a"), "AAC");
        assert_eq!(codec_name("xyz9"), "xyz9");
    }

    #[test]
    fn an_mp4s_size_length_and_codecs_come_from_its_boxes() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("clip.mp4");
        std::fs::write(&p, mp4(false)).unwrap();
        assert_eq!(
            probe(&p).unwrap(),
            VideoInfo {
                width: Some(320),
                height: Some(240),
                duration: Some(3.0),
                video_codec: Some("avc1".into()),
                audio_codec: Some("mp4a".into()),
            }
        );
        // A clip turned a quarter by its matrix is as tall as it is wide.
        std::fs::write(&p, mp4(true)).unwrap();
        let turned = probe(&p).unwrap();
        assert_eq!((turned.width, turned.height), (Some(240), Some(320)));
    }

    fn el(id: u32, body: &[u8]) -> Vec<u8> {
        let idb: Vec<u8> = id
            .to_be_bytes()
            .iter()
            .copied()
            .skip_while(|&b| b == 0)
            .collect();
        let mut v = idb;
        // An 8-byte size, as some muxers write every size.
        v.push(0x01);
        v.extend_from_slice(&(body.len() as u64).to_be_bytes()[1..]);
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn a_webms_size_length_and_codecs_come_from_its_elements() {
        let info = [
            el(TIMESTAMP_SCALE, &[0x0F, 0x42, 0x40]),
            el(DURATION, &3008.0f64.to_be_bytes()),
        ]
        .concat();
        let video = el(
            TRACK_ENTRY,
            &[
                el(TRACK_TYPE, &[1]),
                el(CODEC_ID, b"V_VP9"),
                el(
                    VIDEO,
                    &[el(PIXEL_WIDTH, &[0x01, 0x40]), el(PIXEL_HEIGHT, &[0xF0])].concat(),
                ),
            ]
            .concat(),
        );
        let audio = el(
            TRACK_ENTRY,
            &[el(TRACK_TYPE, &[2]), el(CODEC_ID, b"A_OPUS")].concat(),
        );
        let mut segment_body = [el(INFO, &info), el(TRACKS, &[video, audio].concat())].concat();
        // A cluster of unknown size, as a live recording writes one.
        segment_body.extend_from_slice(&[0x1F, 0x43, 0xB6, 0x75, 0xFF, 0xA3, 0x81, 0x00]);
        let file = [
            el(0x1A45_DFA3, &el(0x4282, b"webm")),
            el(SEGMENT, &segment_body),
        ]
        .concat();
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("clip.webm");
        std::fs::write(&p, file).unwrap();
        let got = probe(&p).unwrap();
        assert_eq!((got.width, got.height), (Some(320), Some(240)));
        assert!((got.duration.unwrap() - 3.008).abs() < 1e-9);
        assert_eq!(got.video_codec.as_deref(), Some("V_VP9"));
        assert_eq!(got.audio_codec.as_deref(), Some("A_OPUS"));
    }

    #[test]
    fn an_audio_only_m4a_has_no_picture_and_junk_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.mp4");
        std::fs::write(&p, b"hello, not a video").unwrap();
        assert!(probe(&p).is_err());
        // The recorder's own file: sound only.
        let mut m = mp4(false);
        // Turn the video track's handler into a second sound track.
        let at = m.windows(4).position(|w| w == b"vide").unwrap();
        m[at..at + 4].copy_from_slice(b"soun");
        std::fs::write(&p, m).unwrap();
        let got = probe(&p).unwrap();
        assert_eq!((got.width, got.height, got.video_codec), (None, None, None));
        assert_eq!(got.duration, Some(3.0));
    }
}
