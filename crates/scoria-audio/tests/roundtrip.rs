//! The recorder's real path, without a microphone: a known signal fed in
//! the blocks an audio host delivers, encoded on the worker as it arrives,
//! written to a `.m4a` beside a folder of other files, and decoded again by
//! an independent decoder (symphonia) to check what came out.

use scoria_audio::capture::feed_from;
use scoria_audio::session::Destination;
use std::f64::consts::PI;

fn decode(path: &std::path::Path) -> (u32, u32, Vec<f32>) {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_AAC};
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;
    let file = std::fs::File::open(path).unwrap();
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("m4a");
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .expect("an MP4 symphonia reads");
    let mut format = probed.format;
    let track = format.default_track().unwrap().clone();
    assert_eq!(track.codec_params.codec, CODEC_TYPE_AAC);
    let rate = track.codec_params.sample_rate.unwrap();
    let mut channels = 0;
    let mut dec = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .unwrap();
    let mut out = Vec::new();
    while let Ok(packet) = format.next_packet() {
        let buf = dec.decode(&packet).expect("every packet decodes");
        channels = buf.spec().channels.count() as u32;
        let mut sb = SampleBuffer::<f32>::new(buf.capacity() as u64, *buf.spec());
        sb.copy_interleaved_ref(buf);
        out.extend_from_slice(sb.samples());
    }
    (rate, channels, out)
}

/// The best alignment of `decoded` against `reference` and the SNR there,
/// leaving out the edges.
fn snr(reference: &[f32], decoded: &[f32]) -> f64 {
    let edge = 4096;
    let corr = |lag: usize| -> f64 {
        (edge..reference.len() - edge)
            .step_by(5)
            .filter_map(|i| {
                decoded
                    .get(i + lag)
                    .map(|d| reference[i] as f64 * *d as f64)
            })
            .sum()
    };
    let lag = (0..=4096)
        .max_by(|&a, &b| corr(a).total_cmp(&corr(b)))
        .unwrap();
    let (mut s, mut e) = (0.0, 0.0);
    for i in edge..reference.len() - edge {
        let r = reference[i] as f64;
        let d = decoded[i + lag] as f64;
        s += r * r;
        e += (r - d) * (r - d);
    }
    10.0 * (s / e).log10()
}

fn crossings_per_second(x: &[f32], rate: u32) -> f64 {
    let a = x.len() / 2 - rate as usize / 4;
    let b = a + rate as usize / 2;
    let n = (a + 1..b)
        .filter(|&i| (x[i - 1] < 0.0) != (x[i] < 0.0))
        .count();
    n as f64 / 2.0 / 0.5
}

#[test]
fn a_tone_fed_in_blocks_comes_back_from_an_independent_decoder() {
    let rate = 48_000u32;
    let seconds = 1.5;
    let n = (rate as f64 * seconds) as usize;
    // The tone in both channels of a stereo device, as most microphones
    // are opened; the recording is mono.
    let mono: Vec<f32> = (0..n)
        .map(|i| (0.4 * (2.0 * PI * 1000.0 * i as f64 / rate as f64).sin()) as f32)
        .collect();
    let stereo: Vec<f32> = mono.iter().flat_map(|&s| [s, s]).collect();

    let dir = tempfile::tempdir().unwrap();
    let (session, feed) = feed_from(rate, 2).unwrap();
    // 480-frame blocks, a typical 10 ms host period.
    for block in stereo.chunks(480 * 2) {
        feed.send(block);
    }
    drop(feed);
    let done = session.finish(Destination {
        folder: dir.path().join("Attachments"),
        stem: "Recording 20261110175445".into(),
        creation: scoria_audio::session::mp4_time(1_794_000_000),
    });
    let finished = done.recv().unwrap().expect("the recording is written");
    assert_eq!(
        finished.path,
        dir.path().join("Attachments/Recording 20261110175445.m4a")
    );
    assert!((finished.seconds - seconds).abs() < 1e-9);
    // Only the file: the part file was renamed into place.
    let names: Vec<_> = std::fs::read_dir(dir.path().join("Attachments"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 1);

    let (got_rate, channels, decoded) = decode(&finished.path);
    assert_eq!((got_rate, channels), (rate, 1));
    // The encoder delay and the padded tail: a frame or two over.
    assert!(
        decoded.len() >= n && decoded.len() <= n + 3 * 1024,
        "{}",
        decoded.len()
    );
    let freq = crossings_per_second(&decoded, rate);
    assert!((freq - 1000.0).abs() < 2.0, "{freq} Hz");
    let db = snr(&mono, &decoded);
    assert!(db > 40.0, "SNR {db:.1} dB");
}
