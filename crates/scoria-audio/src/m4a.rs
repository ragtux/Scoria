//! The `.m4a` file: one AAC-LC track in an ISO base media file (ISO/IEC
//! 14496-12 and -14), written in-crate. `ftyp` (`M4A `, compatible with
//! `M4A `, `isom`, `mp42`), then `moov` with one sound track whose sample
//! entry is `mp4a` carrying an `esds` with the two-byte AudioSpecificConfig,
//! one chunk holding every frame, and `mdat` last. ffprobe reads it as
//! `aac (LC)` in `mov,mp4,m4a,3gp,3g2,mj2`.

/// The ADTS / MPEG-4 sampling frequency table.
pub const RATES: [u32; 13] = [
    96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350,
];

/// The two-byte AudioSpecificConfig for AAC-LC.
pub fn audio_specific_config(sample_rate: u32, channels: u16) -> Option<[u8; 2]> {
    let ix = RATES.iter().position(|&r| r == sample_rate)? as u16;
    let v: u16 = (2 << 11) | (ix << 7) | ((channels & 0xf) << 3);
    Some(v.to_be_bytes())
}

fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 8);
    out.extend_from_slice(&((body.len() + 8) as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    out
}

fn full(kind: &[u8; 4], version: u8, flags: u32, body: &[u8]) -> Vec<u8> {
    let mut b = vec![version];
    b.extend_from_slice(&flags.to_be_bytes()[1..]);
    b.extend_from_slice(body);
    bx(kind, &b)
}

fn descriptor(tag: u8, body: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let n = body.len();
    // Four-byte length form, as most muxers write it.
    out.extend_from_slice(&[
        0x80 | ((n >> 21) & 0x7f) as u8,
        0x80 | ((n >> 14) & 0x7f) as u8,
        0x80 | ((n >> 7) & 0x7f) as u8,
        (n & 0x7f) as u8,
    ]);
    out.extend_from_slice(body);
    out
}

/// The file: `frames` are raw AAC access units (no ADTS), 1024 samples each.
pub fn write(
    sample_rate: u32,
    channels: u16,
    frames: &[Vec<u8>],
    creation: u32,
) -> Option<Vec<u8>> {
    let asc = audio_specific_config(sample_rate, channels)?;
    let n = frames.len() as u32;
    let duration = n as u64 * 1024;
    let total: u64 = frames.iter().map(|f| f.len() as u64).sum();
    let secs = (duration as f64 / sample_rate as f64).max(1e-9);
    let avg_bitrate = (total as f64 * 8.0 / secs) as u32;
    let max_frame = frames.iter().map(|f| f.len()).max().unwrap_or(0) as u32;
    let max_bitrate = (max_frame as f64 * 8.0 * sample_rate as f64 / 1024.0) as u32;

    let ftyp = bx(
        b"ftyp",
        &[
            b"M4A ".as_slice(),
            &0u32.to_be_bytes(),
            b"M4A ",
            b"isom",
            b"mp42",
        ]
        .concat(),
    );

    let mut mvhd = Vec::new();
    mvhd.extend_from_slice(&creation.to_be_bytes());
    mvhd.extend_from_slice(&creation.to_be_bytes());
    mvhd.extend_from_slice(&sample_rate.to_be_bytes());
    mvhd.extend_from_slice(&(duration as u32).to_be_bytes());
    mvhd.extend_from_slice(&0x0001_0000u32.to_be_bytes()); // rate 1.0
    mvhd.extend_from_slice(&0x0100u16.to_be_bytes()); // volume 1.0
    mvhd.extend_from_slice(&[0; 10]);
    for v in [0x0001_0000u32, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000] {
        mvhd.extend_from_slice(&v.to_be_bytes());
    }
    mvhd.extend_from_slice(&[0; 24]);
    mvhd.extend_from_slice(&2u32.to_be_bytes()); // next track id
    let mvhd = full(b"mvhd", 0, 0, &mvhd);

    let mut tkhd = Vec::new();
    tkhd.extend_from_slice(&creation.to_be_bytes());
    tkhd.extend_from_slice(&creation.to_be_bytes());
    tkhd.extend_from_slice(&1u32.to_be_bytes());
    tkhd.extend_from_slice(&0u32.to_be_bytes());
    tkhd.extend_from_slice(&(duration as u32).to_be_bytes());
    tkhd.extend_from_slice(&[0; 8]);
    tkhd.extend_from_slice(&0u16.to_be_bytes()); // layer
    tkhd.extend_from_slice(&0u16.to_be_bytes()); // alternate group
    tkhd.extend_from_slice(&0x0100u16.to_be_bytes()); // volume
    tkhd.extend_from_slice(&[0; 2]);
    for v in [0x0001_0000u32, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000] {
        tkhd.extend_from_slice(&v.to_be_bytes());
    }
    tkhd.extend_from_slice(&[0; 8]); // width, height
    let tkhd = full(b"tkhd", 0, 3, &tkhd);

    let mut mdhd = Vec::new();
    mdhd.extend_from_slice(&creation.to_be_bytes());
    mdhd.extend_from_slice(&creation.to_be_bytes());
    mdhd.extend_from_slice(&sample_rate.to_be_bytes());
    mdhd.extend_from_slice(&(duration as u32).to_be_bytes());
    mdhd.extend_from_slice(&0x55c4u16.to_be_bytes()); // "und"
    mdhd.extend_from_slice(&[0; 2]);
    let mdhd = full(b"mdhd", 0, 0, &mdhd);

    let mut hdlr = vec![0; 4];
    hdlr.extend_from_slice(b"soun");
    hdlr.extend_from_slice(&[0; 12]);
    hdlr.extend_from_slice(b"SoundHandler\0");
    let hdlr = full(b"hdlr", 0, 0, &hdlr);

    let smhd = full(b"smhd", 0, 0, &[0; 4]);
    let dref = full(
        b"dref",
        0,
        0,
        &[&1u32.to_be_bytes()[..], &full(b"url ", 0, 1, &[])].concat(),
    );
    let dinf = bx(b"dinf", &dref);

    // esds: ES_Descriptor > DecoderConfigDescriptor > DecoderSpecificInfo, SLConfig.
    let mut dcd = vec![0x40, 0x15]; // MPEG-4 audio, audio stream
    dcd.extend_from_slice(&[0, 0, 0]); // buffer size
    dcd.extend_from_slice(&max_bitrate.to_be_bytes());
    dcd.extend_from_slice(&avg_bitrate.to_be_bytes());
    dcd.extend_from_slice(&descriptor(0x05, &asc));
    let mut es = vec![0, 1, 0]; // ES_ID 1, flags 0
    es.extend_from_slice(&descriptor(0x04, &dcd));
    es.extend_from_slice(&descriptor(0x06, &[0x02]));
    let esds = full(b"esds", 0, 0, &descriptor(0x03, &es));

    let mut mp4a = vec![0; 6];
    mp4a.extend_from_slice(&1u16.to_be_bytes()); // data reference index
    mp4a.extend_from_slice(&[0; 8]);
    mp4a.extend_from_slice(&channels.to_be_bytes());
    mp4a.extend_from_slice(&16u16.to_be_bytes());
    mp4a.extend_from_slice(&[0; 4]);
    mp4a.extend_from_slice(&((sample_rate.min(65535)) << 16).to_be_bytes());
    mp4a.extend_from_slice(&esds);
    let mp4a = bx(b"mp4a", &mp4a);
    let stsd = full(b"stsd", 0, 0, &[&1u32.to_be_bytes()[..], &mp4a].concat());

    let stts = full(
        b"stts",
        0,
        0,
        &[
            &1u32.to_be_bytes()[..],
            &n.to_be_bytes(),
            &1024u32.to_be_bytes(),
        ]
        .concat(),
    );
    let stsc = full(
        b"stsc",
        0,
        0,
        &[
            &1u32.to_be_bytes()[..],
            &1u32.to_be_bytes(),
            &n.to_be_bytes(),
            &1u32.to_be_bytes(),
        ]
        .concat(),
    );
    let mut stsz = vec![0; 4];
    stsz.extend_from_slice(&n.to_be_bytes());
    for f in frames {
        stsz.extend_from_slice(&(f.len() as u32).to_be_bytes());
    }
    let stsz = full(b"stsz", 0, 0, &stsz);

    // The chunk offset depends on the moov's size, which stco is part of:
    // build with a placeholder, then patch.
    let build = |offset: u32| -> Vec<u8> {
        let stco = full(
            b"stco",
            0,
            0,
            &[&1u32.to_be_bytes()[..], &offset.to_be_bytes()].concat(),
        );
        let stbl = bx(
            b"stbl",
            &[stsd.clone(), stts.clone(), stsc.clone(), stsz.clone(), stco].concat(),
        );
        let minf = bx(b"minf", &[smhd.clone(), dinf.clone(), stbl].concat());
        let mdia = bx(b"mdia", &[mdhd.clone(), hdlr.clone(), minf].concat());
        let trak = bx(b"trak", &[tkhd.clone(), mdia].concat());
        bx(b"moov", &[mvhd.clone(), trak].concat())
    };
    let moov_len = build(0).len();
    let offset = (ftyp.len() + moov_len + 8) as u32;
    let moov = build(offset);
    let mut out = ftyp;
    out.extend_from_slice(&moov);
    out.extend_from_slice(&((total + 8) as u32).to_be_bytes());
    out.extend_from_slice(b"mdat");
    for f in frames {
        out.extend_from_slice(f);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The boxes at the top level and the first frame where `stco` says.
    #[test]
    fn the_boxes_and_the_chunk_offset() {
        let frames = vec![vec![1u8, 2, 3], vec![4u8, 5]];
        let file = write(48_000, 1, &frames, 0).unwrap();
        let mut at = 0;
        let mut kinds = Vec::new();
        while at < file.len() {
            let size = u32::from_be_bytes(file[at..at + 4].try_into().unwrap()) as usize;
            kinds.push(String::from_utf8_lossy(&file[at + 4..at + 8]).to_string());
            at += size;
        }
        assert_eq!(at, file.len());
        assert_eq!(kinds, ["ftyp", "moov", "mdat"]);
        let stco = file.windows(4).position(|w| w == b"stco").unwrap();
        let offset = u32::from_be_bytes(file[stco + 12..stco + 16].try_into().unwrap()) as usize;
        assert_eq!(&file[offset..offset + 5], &[1, 2, 3, 4, 5]);
        // stsz: two samples, 3 and 2 bytes.
        let stsz = file.windows(4).position(|w| w == b"stsz").unwrap();
        assert_eq!(
            &file[stsz + 12..stsz + 24],
            &[0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 2]
        );
    }

    #[test]
    fn the_audio_specific_config() {
        // AAC-LC (2), 48 kHz (index 3), mono: 00010 0011 0001 000.
        assert_eq!(audio_specific_config(48_000, 1), Some([0x11, 0x88]));
        // 44.1 kHz (index 4), mono.
        assert_eq!(audio_specific_config(44_100, 1), Some([0x12, 0x08]));
        assert_eq!(audio_specific_config(192_000, 1), None);
    }
}
