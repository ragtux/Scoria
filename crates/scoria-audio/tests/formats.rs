//! Every format the player decodes, from real files: half a second of a
//! 1 kHz tone written by ffmpeg 8.1.3 into `tests/data` (the commands are in
//! the 2026-11-15 section of `docs/scoria/Development.md`), and the
//! recorder's own `.m4a` made on the spot. Each is opened, its length read
//! without decoding, decoded whole, and its pitch counted.

use scoria_audio::player::{probe, Source};
use std::path::{Path, PathBuf};

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

fn decode(path: &Path) -> (u32, usize, Vec<f32>) {
    let mut s = Source::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut out = Vec::new();
    let mut channels = s.channels;
    while let Some(b) = s.next_block().unwrap() {
        channels = b.channels;
        out.extend(b.samples);
    }
    (s.sample_rate, channels, out)
}

/// Zero crossings of the first channel over the middle of the signal, as a
/// frequency.
fn pitch(samples: &[f32], channels: usize, rate: u32) -> f64 {
    let mono: Vec<f32> = samples.iter().step_by(channels).copied().collect();
    let (a, b) = (mono.len() / 4, mono.len() * 3 / 4);
    let n = mono[a..b]
        .windows(2)
        .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
        .count();
    n as f64 / 2.0 / ((b - a) as f64 / rate as f64)
}

#[test]
fn every_audio_format_decodes_to_the_tone() {
    for (name, rate) in [
        ("tone.mp3", 22_050),
        ("tone.flac", 16_000),
        ("tone.ogg", 22_050),
        ("tone.opus", 48_000),
        ("tone.3gp", 16_000),
        ("tone.webm", 48_000),
        ("tone.wav", 8_000),
    ] {
        let p = data(name);
        let info = probe(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(info.sample_rate, rate, "{name}");
        let d = info.duration.unwrap_or_else(|| panic!("{name}: no duration"));
        // AAC in MP4 counts the encoder's priming and padding, which the
        // file's edit list would trim and symphonia does not read: 0.564 s
        // for the 3GP.
        assert!((d - 0.5).abs() < 0.07, "{name}: {d} s");
        let (got_rate, channels, samples) = decode(&p);
        assert_eq!(got_rate, rate, "{name}");
        let seconds = samples.len() as f64 / channels as f64 / rate as f64;
        assert!((seconds - 0.5).abs() < 0.08, "{name}: decoded {seconds} s");
        let hz = pitch(&samples, channels, rate);
        assert!((hz - 1000.0).abs() < 15.0, "{name}: {hz} Hz");
    }
}

#[test]
fn a_seek_into_each_format_lands_near_its_target() {
    for name in ["tone.mp3", "tone.flac", "tone.ogg", "tone.opus", "tone.3gp", "tone.wav"] {
        let p = data(name);
        let (rate, channels, whole) = decode(&p);
        let total = whole.len() / channels;
        let mut s = Source::open(&p).unwrap();
        s.seek(0.25).unwrap();
        let mut rest = 0;
        while let Some(b) = s.next_block().unwrap() {
            rest += b.samples.len() / b.channels;
        }
        let expect = total as f64 - 0.25 * rate as f64;
        // Within 40 ms: an accurate seek, or a coarse one decoded forward
        // (Vorbis loses its first block after a seek to the overlap, 34 ms
        // here).
        assert!(
            (rest as f64 - expect).abs() < 0.04 * rate as f64,
            "{name}: {rest} frames after the seek, {expect} expected"
        );
    }
}

#[test]
fn the_recorders_own_m4a_plays() {
    use scoria_audio::capture::feed_from;
    use scoria_audio::session::Destination;
    let rate = 48_000u32;
    let tone: Vec<f32> = (0..rate as usize)
        .map(|i| (0.4 * (2.0 * std::f64::consts::PI * 1000.0 * i as f64 / rate as f64).sin()) as f32)
        .collect();
    let dir = tempfile::tempdir().unwrap();
    let (session, feed) = feed_from(rate, 1).unwrap();
    for block in tone.chunks(480) {
        feed.send(block);
    }
    drop(feed);
    let done = session.finish(Destination {
        folder: dir.path().to_path_buf(),
        stem: "Recording 20261115120000".into(),
        creation: scoria_audio::session::mp4_time(1_794_000_000),
    });
    let file = done.recv().unwrap().expect("the recording is written").path;
    let info = probe(&file).unwrap();
    assert_eq!((info.sample_rate, info.channels), (rate, 1));
    assert!((info.duration.unwrap() - 1.0).abs() < 0.05, "{:?}", info.duration);
    let (_, channels, samples) = decode(&file);
    assert!((pitch(&samples, channels, rate) - 1000.0).abs() < 5.0);
}

#[test]
fn the_video_probe_reads_ffmpegs_files() {
    use scoria_audio::video::probe;
    let mp4 = probe(&data("clip.mp4")).unwrap();
    assert_eq!((mp4.width, mp4.height), (Some(64), Some(48)));
    assert!((mp4.duration.unwrap() - 0.5).abs() < 0.01, "{:?}", mp4.duration);
    assert_eq!(mp4.video_codec.as_deref(), Some("avc1"));
    let webm = probe(&data("clip.webm")).unwrap();
    assert_eq!((webm.width, webm.height), (Some(64), Some(48)));
    assert!((webm.duration.unwrap() - 0.5).abs() < 0.01, "{:?}", webm.duration);
    assert_eq!(webm.video_codec.as_deref(), Some("V_VP9"));
    let ogv = probe(&data("clip.ogv")).unwrap();
    assert_eq!((ogv.width, ogv.height), (Some(64), Some(48)));
    assert_eq!(ogv.video_codec.as_deref(), Some("theora"));
    // An audio file is not a video: no picture.
    let audio = probe(&data("tone.3gp")).unwrap();
    assert_eq!((audio.width, audio.video_codec), (None, None));
}
