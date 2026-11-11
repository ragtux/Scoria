//! PCM in, AAC-LC access units out, frame by frame as the samples arrive.
//!
//! The microphone hands over interleaved samples in whatever blocks the
//! audio host chooses; [`Framer`] downmixes them to mono and cuts them into
//! the 1024-sample hops AAC codes, and [`Encoding`] runs oxideav-aac's
//! `StreamEncoder` on each hop at once, keeping only the encoded frames
//! (about 6 KB a second), never the recording's PCM.

use crate::m4a;
use oxideav_aac::encoder::{EncoderConfig, StreamEncoder};

/// Samples per AAC frame.
pub const FRAME_LEN: usize = 1024;

/// The bitrate asked of the encoder. oxideav-aac's rate loop treats it as
/// a ceiling and lands well under it: about 46-48 kbps for a tone or a sweep
/// at 44.1 and 48 kHz (measured 2026-11-11).
pub const BITRATE: u32 = 96_000;

/// Cuts interleaved PCM of any block size into mono hops of [`FRAME_LEN`].
#[derive(Debug, Default)]
pub struct Framer {
    pending: Vec<i16>,
}

impl Framer {
    /// Adds interleaved `f32` samples of `channels` channels, averaged to
    /// one, and returns every hop that is now whole.
    pub fn push(&mut self, interleaved: &[f32], channels: usize) -> Vec<Vec<i16>> {
        let channels = channels.max(1);
        for frame in interleaved.chunks_exact(channels) {
            let mono = frame.iter().sum::<f32>() / channels as f32;
            self.pending.push(to_i16(mono));
        }
        let whole = self.pending.len() / FRAME_LEN * FRAME_LEN;
        let hops = self.pending[..whole]
            .chunks(FRAME_LEN)
            .map(<[i16]>::to_vec)
            .collect();
        self.pending.drain(..whole);
        hops
    }

    /// The samples short of a whole hop, at the end of the recording.
    pub fn rest(&mut self) -> Vec<i16> {
        std::mem::take(&mut self.pending)
    }
}

fn to_i16(x: f32) -> i16 {
    if x.is_nan() {
        return 0;
    }
    (x.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

/// One recording being encoded.
pub struct Encoding {
    sample_rate: u32,
    encoder: StreamEncoder,
    framer: Framer,
    /// Raw access units, ADTS headers stripped, in order.
    frames: Vec<Vec<u8>>,
    /// Samples taken in, for the duration.
    samples: u64,
}

impl Encoding {
    /// An encoder for mono at `sample_rate`, which must be one of AAC's
    /// rates (8 kHz to 96 kHz; a microphone's 44.1 or 48 kHz always is).
    pub fn new(sample_rate: u32) -> Result<Encoding, String> {
        if m4a::audio_specific_config(sample_rate, 1).is_none() {
            return Err(format!("{sample_rate} Hz cannot be recorded as AAC"));
        }
        let encoder = StreamEncoder::new(EncoderConfig {
            sample_rate,
            channels: 1,
            bitrate: BITRATE,
        })
        .map_err(|e| format!("AAC encoder: {e:?}"))?;
        Ok(Encoding {
            sample_rate,
            encoder,
            framer: Framer::default(),
            frames: Vec::new(),
            samples: 0,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Seconds of audio taken in so far.
    pub fn seconds(&self) -> f64 {
        self.samples as f64 / self.sample_rate as f64
    }

    /// Encodes every whole hop `interleaved` completes.
    pub fn push(&mut self, interleaved: &[f32], channels: usize) -> Result<(), String> {
        self.samples += (interleaved.len() / channels.max(1)) as u64;
        for hop in self.framer.push(interleaved, channels) {
            self.encode(&hop)?;
        }
        Ok(())
    }

    fn encode(&mut self, hop: &[i16]) -> Result<(), String> {
        let adts = self
            .encoder
            .encode_frame(hop)
            .map_err(|e| format!("AAC encoder: {e:?}"))?;
        self.frames.push(strip_adts(adts));
        Ok(())
    }

    /// The last partial hop and the encoder's overlap flushed, and the
    /// whole `.m4a`.
    pub fn finish(mut self, creation: u32) -> Result<Vec<u8>, String> {
        let rest = self.framer.rest();
        if !rest.is_empty() {
            self.encode(&rest)?;
        }
        let tail = self
            .encoder
            .finish()
            .map_err(|e| format!("AAC encoder: {e:?}"))?;
        self.frames.push(strip_adts(tail));
        m4a::write(self.sample_rate, 1, &self.frames, creation)
            .ok_or_else(|| "the recording could not be written".to_string())
    }
}

/// An ADTS frame's payload: the 7-byte header gone, or 9 with its CRC.
fn strip_adts(mut adts: Vec<u8>) -> Vec<u8> {
    let header = if adts.len() > 1 && adts[1] & 1 == 0 {
        9
    } else {
        7
    };
    adts.drain(..header.min(adts.len()));
    adts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_framer_cuts_any_blocks_into_mono_hops() {
        let mut f = Framer::default();
        // Stereo blocks of 700 frames: L = 0.5, R = -0.5 averages to 0.
        let block: Vec<f32> = (0..1400)
            .map(|i| if i % 2 == 0 { 0.5 } else { -0.5 })
            .collect();
        assert!(f.push(&block, 2).is_empty());
        let hops = f.push(&block, 2);
        assert_eq!(hops.len(), 1);
        assert_eq!(hops[0].len(), FRAME_LEN);
        assert!(hops[0].iter().all(|&s| s == 0));
        assert_eq!(f.rest().len(), 1400 - FRAME_LEN);
        assert!(f.rest().is_empty());
    }

    #[test]
    fn samples_are_clamped_and_nan_is_silence() {
        assert_eq!(to_i16(2.0), 32767);
        assert_eq!(to_i16(-2.0), -32767);
        assert_eq!(to_i16(f32::NAN), 0);
    }

    #[test]
    fn an_odd_rate_is_refused() {
        assert!(Encoding::new(44_100).is_ok());
        assert!(Encoding::new(192_000).is_err());
    }
}
