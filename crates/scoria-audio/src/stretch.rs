//! Playback speed with the pitch kept: a streaming WSOLA (waveform-similarity
//! overlap-add) time stretcher.
//!
//! Chromium's `<audio>` keeps the pitch when its playback rate changes
//! (`preservesPitch` is true by default, and it was in the 2026-11-16 probe,
//! see [[Specification#Observed audio player extras, Obsidian 1.15.1]]), so
//! the player cannot simply resample. The input is cut into Hann-windowed
//! frames 40 ms long, read every `hop × rate` frames and laid down every
//! `hop` frames with half of each overlapping the last; each frame's start
//! is moved, within ±10 ms of where the rate puts it, to where it best
//! continues the frame before (the most correlated offset), so the overlap
//! adds waveforms in phase rather than beating. At rate 1 the samples pass
//! through untouched.

/// A time stretcher for interleaved samples with `channels` channels.
#[derive(Debug, Clone)]
pub struct Stretch {
    channels: usize,
    rate: f64,
    /// Frame length and output hop, in frames.
    win: usize,
    hop: usize,
    /// How far a frame's start may move from its nominal place, in frames.
    search: usize,
    window: Vec<f32>,
    /// Input not yet consumed, interleaved; `input[0]` is input frame `origin`.
    input: Vec<f32>,
    origin: usize,
    /// Where the next frame nominally starts, in input frames.
    next: f64,
    /// Where the previous frame actually started.
    prev: Option<usize>,
    /// The overlap still to be completed, `win` frames interleaved.
    acc: Vec<f32>,
}

impl Stretch {
    /// A stretcher at `rate` (2.0 plays twice as fast) for audio at
    /// `sample_rate` frames a second.
    pub fn new(sample_rate: u32, channels: usize, rate: f64) -> Stretch {
        let channels = channels.max(1);
        let hop = ((sample_rate as f64 * 0.02) as usize).max(16);
        let win = hop * 2;
        // A periodic Hann window: two of them half a window apart sum to one.
        let window = (0..win)
            .map(|i| {
                let x = std::f64::consts::PI * i as f64 / win as f64;
                (x.sin() * x.sin()) as f32
            })
            .collect();
        Stretch {
            channels,
            rate: if rate.is_finite() && rate > 0.0 {
                rate
            } else {
                1.0
            },
            win,
            hop,
            search: hop / 2,
            window,
            input: Vec::new(),
            origin: 0,
            next: 0.0,
            prev: None,
            acc: vec![0.0; win * channels],
        }
    }

    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// Whether the samples pass through as they are.
    pub fn is_identity(&self) -> bool {
        self.rate == 1.0
    }

    /// Forgets everything buffered, as after a seek.
    pub fn reset(&mut self) {
        self.input.clear();
        self.origin = 0;
        self.next = 0.0;
        self.prev = None;
        self.acc.iter_mut().for_each(|v| *v = 0.0);
    }

    /// Takes `input` and returns what is ready of the stretched output.
    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        if self.is_identity() {
            return input.to_vec();
        }
        let ch = self.channels;
        self.input.extend_from_slice(input);
        let mut out = Vec::new();
        loop {
            let have = self.origin + self.input.len() / ch;
            let nominal = self.next.round() as usize;
            let lo = nominal.saturating_sub(self.search).max(self.origin);
            let hi = nominal + self.search;
            let start = match self.prev {
                None => {
                    if nominal.max(self.origin) + self.win > have {
                        break;
                    }
                    nominal.max(self.origin)
                }
                Some(prev) => {
                    // The template is how the previous frame would have
                    // gone on; it must be in the buffer, and so must every
                    // candidate frame whole.
                    let template = prev + self.hop;
                    if hi + self.win > have || template + self.hop > have {
                        break;
                    }
                    self.best(template, lo, hi)
                }
            };
            let first = self.prev.is_none();
            let at = (start - self.origin) * ch;
            for i in 0..self.win {
                // The first frame has nothing before it to overlap, so its
                // rising half is taken whole rather than faded in.
                let w = if first && i < self.hop {
                    1.0
                } else {
                    self.window[i]
                };
                for c in 0..ch {
                    self.acc[i * ch + c] += self.input[at + i * ch + c] * w;
                }
            }
            out.extend_from_slice(&self.acc[..self.hop * ch]);
            self.acc.copy_within(self.hop * ch.., 0);
            let n = self.acc.len();
            self.acc[n - self.hop * ch..]
                .iter_mut()
                .for_each(|v| *v = 0.0);
            self.prev = Some(start);
            self.next += self.hop as f64 * self.rate;
            // Drop what no later frame or template can reach.
            let keep = (self.next.round() as usize)
                .saturating_sub(self.search)
                .min(start + self.hop);
            if keep > self.origin {
                let drop = ((keep - self.origin) * ch).min(self.input.len());
                self.input.drain(..drop);
                self.origin += drop / ch;
            }
        }
        out
    }

    /// What is left in the overlap at the end of the stream.
    pub fn flush(&mut self) -> Vec<f32> {
        if self.is_identity() || self.prev.is_none() {
            self.reset();
            return Vec::new();
        }
        let out = self.acc[..self.hop * self.channels].to_vec();
        self.reset();
        out
    }

    /// The start in `lo..=hi` whose overlap best matches the template at
    /// `template`: a coarse pass over every other offset and sample, then
    /// the neighbours of the best one at full resolution.
    fn best(&self, template: usize, lo: usize, hi: usize) -> usize {
        let n = self.hop;
        let score = |cand: usize, step: usize| -> f32 {
            let (mut xy, mut yy) = (0.0f32, 0.0f32);
            let mut i = 0;
            while i < n {
                let x = self.mono(template + i);
                let y = self.mono(cand + i);
                xy += x * y;
                yy += y * y;
                i += step;
            }
            if yy <= 1e-12 {
                0.0
            } else {
                xy / yy.sqrt()
            }
        };
        let mut best = (lo, f32::MIN);
        let mut c = lo;
        while c <= hi {
            let s = score(c, 2);
            if s > best.1 {
                best = (c, s);
            }
            c += 2;
        }
        let (centre, _) = best;
        let mut fine = (centre, f32::MIN);
        for c in centre.saturating_sub(2).max(lo)..=(centre + 2).min(hi) {
            let s = score(c, 1);
            if s > fine.1 {
                fine = (c, s);
            }
        }
        fine.0
    }

    fn mono(&self, frame: usize) -> f32 {
        let ch = self.channels;
        let at = (frame - self.origin) * ch;
        self.input[at..at + ch].iter().sum::<f32>() / ch as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, seconds: f64, hz: f64) -> Vec<f32> {
        (0..(rate as f64 * seconds) as usize)
            .map(|i| {
                (0.5 * (2.0 * std::f64::consts::PI * hz * i as f64 / rate as f64).sin()) as f32
            })
            .collect()
    }

    fn crossings(x: &[f32], rate: u32) -> f64 {
        let n = x
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count();
        n as f64 / 2.0 / (x.len() as f64 / rate as f64)
    }

    fn stretched(input: &[f32], rate: u32, speed: f64, block: usize) -> Vec<f32> {
        let mut s = Stretch::new(rate, 1, speed);
        let mut out = Vec::new();
        for b in input.chunks(block) {
            out.extend(s.process(b));
        }
        out.extend(s.flush());
        out
    }

    #[test]
    fn rate_one_passes_the_samples_through() {
        let x = tone(8_000, 0.3, 440.0);
        assert_eq!(stretched(&x, 8_000, 1.0, 100), x);
    }

    #[test]
    fn a_faster_or_slower_rate_keeps_the_pitch_and_scales_the_length() {
        let rate = 24_000;
        let x = tone(rate, 1.5, 1000.0);
        for speed in [0.25, 0.5, 0.75, 1.25, 1.5, 1.75, 2.0] {
            let y = stretched(&x, rate, speed, 1152);
            let want = x.len() as f64 / speed;
            // Off by at most the input's last frame, its search and a hop
            // (80 ms), which no frame reached, stretched.
            assert!(
                (y.len() as f64 - want).abs() < rate as f64 * 0.08 / speed,
                "{speed}: {} vs {want}",
                y.len()
            );
            assert!(
                (crossings(&y, rate) - 1000.0).abs() < 10.0,
                "{speed}: {}",
                crossings(&y, rate)
            );
            // In phase: no frame boundary leaves a dip or a step. The
            // steady part keeps the tone's own amplitude.
            let mid = &y[y.len() / 4..y.len() * 3 / 4];
            let peak = mid.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            let jump = mid
                .windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .fold(0.0, f32::max);
            assert!((peak - 0.5).abs() < 0.03, "{speed}: peak {peak}");
            // The tone's own largest step between samples, and a little.
            let own = (0.5 * 2.0 * std::f64::consts::PI * 1000.0 / rate as f64) as f32;
            assert!(jump < own * 1.1, "{speed}: jump {jump} over {own}");
        }
    }

    #[test]
    fn blocks_of_any_size_stretch_alike() {
        let rate = 16_000;
        let x = tone(rate, 1.0, 300.0);
        let a = stretched(&x, rate, 1.5, 7);
        let b = stretched(&x, rate, 1.5, 4096);
        assert_eq!(a.len(), b.len());
        assert!(a.iter().zip(&b).all(|(p, q)| (p - q).abs() < 1e-6));
    }

    #[test]
    fn stereo_channels_stay_apart() {
        let rate = 16_000;
        let l = tone(rate, 1.0, 500.0);
        let mut x = Vec::new();
        for v in &l {
            x.push(*v);
            x.push(0.0);
        }
        let mut s = Stretch::new(rate, 2, 2.0);
        let y = s.process(&x);
        assert!(!y.is_empty() && y.len().is_multiple_of(2));
        assert!(y.chunks(2).all(|f| f[1] == 0.0));
        let left: Vec<f32> = y.chunks(2).map(|f| f[0]).collect();
        assert!((crossings(&left[..left.len() - 200], rate) - 500.0).abs() < 10.0);
    }
}
