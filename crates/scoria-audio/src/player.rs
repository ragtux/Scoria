//! The audio player: an audio file opened in a tab or embedded with
//! `![[x.mp3]]`, decoded here and played through the default output device.
//! Obsidian's is Chromium's `<audio controls>` (observed 2026-11-15, see
//! [[Specification#Observed canvas embeds, audio and video, Obsidian 1.15.0]]);
//! the controls are the app's, this is what they drive.
//!
//! - [`Source`]: a file opened for decoding. symphonia reads the container
//!   and decodes MP3, WAV, FLAC, Vorbis and AAC; Opus, which symphonia 0.5
//!   reads but cannot decode, goes to `opus-decoder`. [`probe`] reads the
//!   duration without decoding.
//! - [`Engine`]: the playback state between a decoding worker and the audio
//!   callback: the queue of samples ready to play, pause, seek, volume, the
//!   position, the end. It has no device in it, so the tests drive it with
//!   [`Engine::pump`] and [`Engine::fill`] the way the worker and the device
//!   would.
//! - [`Player`]: an engine with a worker thread decoding ahead and a cpal
//!   output stream pulling from it. Created when playback starts, so a note
//!   with ten embeds opens no device until one is played.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL, CODEC_TYPE_OPUS};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::{Time, TimeBase};

/// What a file holds, read from its headers.
#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    /// Seconds, when the container says (or a scan of its packets finds).
    pub duration: Option<f64>,
    pub sample_rate: u32,
    pub channels: usize,
}

/// The duration, rate and channels of the audio in `path`, without decoding
/// it. A container that records no length (an MP3 with no Xing header) has
/// its packets' durations added up, which reads the file but decodes nothing.
pub fn probe(path: &Path) -> Result<Info, String> {
    let mut source = Source::open(path)?;
    let mut info = source.info();
    // A container that does not say how many channels (AAC in MP4 leaves it
    // to the decoder) is asked by decoding its first block.
    if !source.channels_known {
        if let Ok(Some(b)) = source.next_block() {
            info.channels = b.channels;
        }
        source = Source::open(path)?;
    }
    if info.duration.is_none() {
        let mut ts = 0u64;
        loop {
            match source.format.next_packet() {
                Ok(p) if p.track_id() == source.track => ts = ts.max(p.ts() + p.dur()),
                Ok(_) => {}
                Err(_) => break,
            }
        }
        info.duration = source.seconds(ts);
    }
    Ok(info)
}

enum Codec {
    Symphonia(Box<dyn symphonia::core::codecs::Decoder>),
    Opus(Box<opus_decoder::OpusDecoder>, Vec<f32>),
}

/// One decoded block: interleaved samples and how many channels they have.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub samples: Vec<f32>,
    pub channels: usize,
}

/// A file opened for decoding.
pub struct Source {
    path: PathBuf,
    format: Box<dyn FormatReader>,
    codec: Codec,
    track: u32,
    pub sample_rate: u32,
    pub channels: usize,
    /// The container named the channels; otherwise `channels` is a guess
    /// until a block is decoded.
    channels_known: bool,
    time_base: Option<TimeBase>,
    n_frames: Option<u64>,
    /// The encoder's delay (Opus's pre-skip), dropped from the start.
    delay: u64,
    /// Frames still to drop from the head of what is decoded next: the
    /// delay at the start, the run-up to the target after a seek.
    skip: u64,
}

impl Source {
    pub fn open(path: &Path) -> Result<Source, String> {
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(&ext.to_lowercase());
        }
        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                mss,
                &FormatOptions {
                    enable_gapless: true,
                    ..Default::default()
                },
                &MetadataOptions::default(),
            )
            .map_err(|e| unsupported(&e))?;
        let format = probed.format;
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL && t.codec_params.sample_rate.is_some())
            .or_else(|| format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL))
            .ok_or_else(|| "The file holds no audio".to_string())?;
        let params = track.codec_params.clone();
        let id = track.id;
        let channels = params.channels.map(|c| c.count()).unwrap_or(2).max(1);
        let (codec, sample_rate) = if params.codec == CODEC_TYPE_OPUS {
            if channels > 2 {
                return Err("Opus with more than two channels is not supported".into());
            }
            let dec = opus_decoder::OpusDecoder::new(48_000, channels).map_err(|e| e.to_string())?;
            let max = dec.max_frame_size_per_channel() * channels;
            (Codec::Opus(Box::new(dec), vec![0.0; max]), 48_000)
        } else {
            let dec = symphonia::default::get_codecs()
                .make(&params, &DecoderOptions::default())
                .map_err(|e| unsupported(&e))?;
            (Codec::Symphonia(dec), params.sample_rate.unwrap_or(44_100))
        };
        // symphonia's gapless mode trims the delay of the codecs it decodes;
        // Opus is decoded here, so its pre-skip is dropped here.
        let delay = if params.codec == CODEC_TYPE_OPUS {
            params.delay.unwrap_or(0) as u64
        } else {
            0
        };
        Ok(Source {
            path: path.to_path_buf(),
            format,
            codec,
            track: id,
            sample_rate,
            channels,
            channels_known: params.channels.is_some(),
            time_base: params.time_base,
            n_frames: params.n_frames,
            delay,
            skip: delay,
        })
    }

    pub fn info(&self) -> Info {
        Info {
            duration: self
                .n_frames
                .and_then(|n| self.seconds(n.saturating_sub(self.delay))),
            sample_rate: self.sample_rate,
            channels: self.channels,
        }
    }

    /// A timestamp in the track's time base, in seconds.
    fn seconds(&self, ts: u64) -> Option<f64> {
        let tb = self
            .time_base
            .unwrap_or_else(|| TimeBase::new(1, self.sample_rate.max(1)));
        let t = tb.calc_time(ts);
        Some(t.seconds as f64 + t.frac)
    }

    /// The next decoded block, `None` at the end of the file. A packet that
    /// does not decode is skipped, as a browser skips a corrupt frame.
    pub fn next_block(&mut self) -> Result<Option<Block>, String> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Ok(None)
                }
                Err(SymError::ResetRequired) => return Ok(None),
                Err(e) => return Err(e.to_string()),
            };
            if packet.track_id() != self.track {
                continue;
            }
            let (mut samples, channels) = match &mut self.codec {
                Codec::Symphonia(dec) => match dec.decode(&packet) {
                    Ok(buf) => {
                        let spec = *buf.spec();
                        let mut sb = SampleBuffer::<f32>::new(buf.capacity() as u64, spec);
                        sb.copy_interleaved_ref(buf);
                        (sb.samples().to_vec(), spec.channels.count().max(1))
                    }
                    Err(SymError::DecodeError(_)) => continue,
                    Err(e) => return Err(e.to_string()),
                },
                // A third-party decoder that panics on a packet loses that
                // packet, not the player.
                Codec::Opus(dec, pcm) => {
                    let data = &packet.data;
                    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        dec.decode_float(data, pcm, false)
                    }));
                    match r {
                        Ok(Ok(n)) => (pcm[..n * self.channels].to_vec(), self.channels),
                        _ => continue,
                    }
                }
            };
            if self.skip > 0 {
                let frames = (samples.len() / channels) as u64;
                let drop = self.skip.min(frames);
                samples.drain(..drop as usize * channels);
                self.skip -= drop;
            }
            if samples.is_empty() {
                continue;
            }
            return Ok(Some(Block { samples, channels }));
        }
    }

    /// Moves to `seconds` (clamped at 0): the next block starts there. A
    /// container that cannot seek is read again from the start and decoded
    /// up to the point.
    pub fn seek(&mut self, seconds: f64) -> Result<(), String> {
        let seconds = seconds.max(0.0);
        let to = SeekTo::Time {
            time: Time::from(seconds),
            track_id: Some(self.track),
        };
        match self.format.seek(SeekMode::Accurate, to) {
            Ok(done) => {
                self.reset_codec();
                let ahead = done.required_ts.saturating_sub(done.actual_ts);
                let frames = self
                    .seconds(ahead)
                    .map(|s| (s * self.sample_rate as f64).round() as u64)
                    .unwrap_or(0);
                self.skip = frames + if done.required_ts == 0 { self.delay } else { 0 };
                Ok(())
            }
            Err(_) => {
                let mut fresh = Source::open(&self.path)?;
                fresh.skip += (seconds * fresh.sample_rate as f64).round() as u64;
                *self = fresh;
                Ok(())
            }
        }
    }

    fn reset_codec(&mut self) {
        match &mut self.codec {
            Codec::Symphonia(dec) => dec.reset(),
            Codec::Opus(dec, _) => dec.reset(),
        }
    }
}

fn unsupported(e: &SymError) -> String {
    match e {
        SymError::Unsupported(what) => format!("This file's format is not supported ({what})"),
        other => other.to_string(),
    }
}

// ---- conversion -----------------------------------------------------------

/// `samples` with `from` channels as `to` channels: equal counts copied, mono
/// spread to every output, many folded to mono by their mean, anything else
/// taken channel by channel and repeated round.
pub fn remix(samples: &[f32], from: usize, to: usize) -> Vec<f32> {
    if from == to {
        return samples.to_vec();
    }
    let mut out = Vec::with_capacity(samples.len() / from.max(1) * to);
    for f in samples.chunks_exact(from) {
        if to == 1 {
            out.push(f.iter().sum::<f32>() / from as f32);
        } else {
            for c in 0..to {
                out.push(f[c % from]);
            }
        }
    }
    out
}

/// A linear resampler that carries its phase from one block to the next,
/// so a stream cut into blocks resamples as the whole would.
#[derive(Debug, Clone)]
pub struct Resampler {
    from: u32,
    to: u32,
    channels: usize,
    /// Where the next output frame falls, in input frames from the start of
    /// the next block; down to -1, which is between the last frame of the
    /// previous block and the first of the next.
    t: f64,
    prev: Option<Vec<f32>>,
}

impl Resampler {
    pub fn new(from: u32, to: u32, channels: usize) -> Resampler {
        Resampler {
            from,
            to,
            channels: channels.max(1),
            t: 0.0,
            prev: None,
        }
    }

    pub fn reset(&mut self) {
        self.t = 0.0;
        self.prev = None;
    }

    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        let ch = self.channels;
        if self.from == self.to || self.from == 0 || self.to == 0 {
            return input.to_vec();
        }
        let n = input.len() / ch;
        if n == 0 {
            return Vec::new();
        }
        let step = self.from as f64 / self.to as f64;
        let mut out = Vec::with_capacity(((n as f64 / step) as usize + 2) * ch);
        let prev = self.prev.clone();
        let frame = |i: isize| -> &[f32] {
            if i < 0 {
                prev.as_deref().unwrap_or(&input[..ch])
            } else {
                &input[i as usize * ch..(i as usize + 1) * ch]
            }
        };
        while self.t + 1.0 < n as f64 {
            let i = self.t.floor();
            let f = (self.t - i) as f32;
            let (a, b) = (frame(i as isize), frame(i as isize + 1));
            for c in 0..ch {
                out.push(a[c] + (b[c] - a[c]) * f);
            }
            self.t += step;
        }
        self.t -= n as f64;
        self.prev = Some(input[(n - 1) * ch..n * ch].to_vec());
        out
    }
}

// ---- the engine -----------------------------------------------------------

/// The output a player plays into: its rate and channel count.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Output {
    pub rate: u32,
    pub channels: usize,
}

#[derive(Debug)]
struct State {
    /// Samples ready to play, interleaved, at the output's rate and channels.
    queue: VecDeque<f32>,
    /// The file position of the first sample queued since the last seek.
    base: f64,
    /// Output frames played since then.
    played: u64,
    paused: bool,
    volume: f32,
    muted: bool,
    /// The source has nothing more.
    exhausted: bool,
    /// Played to the end: paused there, the way `<audio>` stops.
    ended: bool,
    /// A seek the worker has not done yet.
    seek: Option<f64>,
    duration: Option<f64>,
    error: Option<String>,
    quit: bool,
}

/// The playback state shared by the decoding worker and the audio callback.
#[derive(Clone)]
pub struct Engine {
    inner: Arc<(Mutex<State>, Condvar)>,
    out: Output,
}

/// How much the worker decodes ahead, in seconds.
const AHEAD: f64 = 0.5;

impl Engine {
    pub fn new(out: Output, duration: Option<f64>) -> Engine {
        Engine {
            inner: Arc::new((
                Mutex::new(State {
                    queue: VecDeque::new(),
                    base: 0.0,
                    played: 0,
                    paused: true,
                    volume: 1.0,
                    muted: false,
                    exhausted: false,
                    ended: false,
                    seek: None,
                    duration,
                    error: None,
                    quit: false,
                }),
                Condvar::new(),
            )),
            out,
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.inner.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn wake(&self) {
        self.inner.1.notify_all();
    }

    pub fn output(&self) -> Output {
        self.out
    }

    /// Starts playing; from the start again when it had played to the end.
    pub fn play(&self) {
        let mut s = self.state();
        if s.ended {
            s.ended = false;
            s.seek = Some(0.0);
            s.queue.clear();
        }
        s.paused = false;
        drop(s);
        self.wake();
    }

    pub fn pause(&self) {
        self.state().paused = true;
    }

    pub fn is_playing(&self) -> bool {
        !self.state().paused
    }

    pub fn ended(&self) -> bool {
        self.state().ended
    }

    /// Moves to `seconds`, kept within the file. Playing or paused stays so;
    /// a seek away from the end is no longer ended.
    pub fn seek(&self, seconds: f64) {
        let mut s = self.state();
        let max = s.duration.unwrap_or(f64::INFINITY);
        let to = seconds.clamp(0.0, max.max(0.0));
        s.seek = Some(to);
        s.queue.clear();
        s.ended = false;
        drop(s);
        self.wake();
    }

    pub fn set_volume(&self, volume: f32) {
        self.state().volume = volume.clamp(0.0, 1.0);
    }

    pub fn volume(&self) -> f32 {
        self.state().volume
    }

    pub fn set_muted(&self, muted: bool) {
        self.state().muted = muted;
    }

    pub fn muted(&self) -> bool {
        self.state().muted
    }

    pub fn duration(&self) -> Option<f64> {
        self.state().duration
    }

    pub fn error(&self) -> Option<String> {
        self.state().error.clone()
    }

    /// Where playback is, in seconds: a pending seek's target, else what the
    /// device has played since the last one; the end once ended.
    pub fn position(&self) -> f64 {
        let s = self.state();
        if let Some(t) = s.seek {
            return t;
        }
        if s.ended {
            return s.duration.unwrap_or(s.base + s.played as f64 / self.out.rate as f64);
        }
        let p = s.base + s.played as f64 / self.out.rate as f64;
        match s.duration {
            Some(d) => p.min(d),
            None => p,
        }
    }

    fn quit(&self) {
        self.state().quit = true;
        self.wake();
    }

    /// One step of the worker: a pending seek, or one block decoded into the
    /// queue when it is short. `false` when there was nothing to do (or the
    /// player is closing), which is when the worker waits.
    pub fn pump(&self, source: &mut Source, resampler: &mut Resampler) -> bool {
        let target = (AHEAD * self.out.rate as f64) as usize * self.out.channels;
        let (seek, need) = {
            let s = self.state();
            if s.quit {
                return false;
            }
            (s.seek, !s.exhausted && s.queue.len() < target)
        };
        if let Some(t) = seek {
            let r = source.seek(t);
            resampler.reset();
            let mut s = self.state();
            if s.seek == Some(t) {
                s.seek = None;
                s.queue.clear();
                s.base = t;
                s.played = 0;
                s.exhausted = false;
                if let Err(e) = r {
                    s.error = Some(e);
                    s.exhausted = true;
                }
            }
            return true;
        }
        if !need {
            return false;
        }
        match source.next_block() {
            Ok(Some(block)) => {
                let mixed = remix(&block.samples, block.channels, self.out.channels);
                let out = resampler.process(&mixed);
                let mut s = self.state();
                if s.seek.is_none() {
                    s.queue.extend(out);
                }
            }
            Ok(None) => self.state().exhausted = true,
            Err(e) => {
                let mut s = self.state();
                s.error = Some(e);
                s.exhausted = true;
            }
        }
        true
    }

    /// What the device plays next: `out` filled from the queue at the
    /// volume, silence while paused, seeking or starved. Reaching the end of
    /// a finished source pauses there and marks it ended.
    pub fn fill(&self, out: &mut [f32]) {
        let mut s = self.state();
        if s.paused || s.seek.is_some() {
            out.fill(0.0);
            return;
        }
        let gain = if s.muted { 0.0 } else { s.volume };
        let n = out.len().min(s.queue.len());
        let n = n - n % self.out.channels;
        for (o, v) in out.iter_mut().zip(s.queue.drain(..n)) {
            *o = v * gain;
        }
        out[n..].fill(0.0);
        s.played += (n / self.out.channels) as u64;
        if s.queue.is_empty() && s.exhausted {
            s.paused = true;
            s.ended = true;
        }
        drop(s);
        self.wake();
    }

    /// The worker: decodes ahead until the player closes.
    fn run(&self, mut source: Source) {
        let mut resampler = Resampler::new(source.sample_rate, self.out.rate, self.out.channels);
        loop {
            if self.state().quit {
                return;
            }
            if !self.pump(&mut source, &mut resampler) {
                let s = self.state();
                if s.quit {
                    return;
                }
                let _ = self
                    .inner
                    .1
                    .wait_timeout(s, Duration::from_millis(50))
                    .map(|_| ());
            }
        }
    }
}

// ---- the device -----------------------------------------------------------

/// An [`Engine`] with its worker and its output stream: the playing half of
/// a player. Dropping it stops the sound and the worker.
pub struct Player {
    engine: Engine,
    _stream: cpal::Stream,
    worker: Option<JoinHandle<()>>,
}

impl Player {
    /// Opens `path` and the default output device, paused at the start.
    pub fn open(path: &Path) -> Result<Player, String> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        let source = Source::open(path)?;
        let duration = source.info().duration;
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "No audio output was found".to_string())?;
        let supported = device
            .default_output_config()
            .map_err(|e| format!("The audio output could not be opened: {e}"))?;
        let format = supported.sample_format();
        let config = supported.config();
        let out = Output {
            rate: config.sample_rate,
            channels: (config.channels as usize).max(1),
        };
        let engine = Engine::new(out, duration);
        let on_error = |e| eprintln!("audio player: {e}");
        let e = engine.clone();
        let stream = match format {
            cpal::SampleFormat::F32 => device.build_output_stream(
                config,
                move |data: &mut [f32], _: &_| e.fill(data),
                on_error,
                None,
            ),
            cpal::SampleFormat::I16 => {
                let mut buf = Vec::new();
                device.build_output_stream(
                    config,
                    move |data: &mut [i16], _: &_| {
                        buf.resize(data.len(), 0.0);
                        e.fill(&mut buf);
                        for (d, s) in data.iter_mut().zip(&buf) {
                            *d = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
                        }
                    },
                    on_error,
                    None,
                )
            }
            cpal::SampleFormat::I32 => {
                let mut buf = Vec::new();
                device.build_output_stream(
                    config,
                    move |data: &mut [i32], _: &_| {
                        buf.resize(data.len(), 0.0);
                        e.fill(&mut buf);
                        for (d, s) in data.iter_mut().zip(&buf) {
                            *d = (s.clamp(-1.0, 1.0) as f64 * 2_147_483_647.0) as i32;
                        }
                    },
                    on_error,
                    None,
                )
            }
            cpal::SampleFormat::U16 => {
                let mut buf = Vec::new();
                device.build_output_stream(
                    config,
                    move |data: &mut [u16], _: &_| {
                        buf.resize(data.len(), 0.0);
                        e.fill(&mut buf);
                        for (d, s) in data.iter_mut().zip(&buf) {
                            *d = ((s.clamp(-1.0, 1.0) + 1.0) * 32767.5) as u16;
                        }
                    },
                    on_error,
                    None,
                )
            }
            other => return Err(format!("The audio output's {other} samples are not supported")),
        }
        .map_err(|e| format!("The audio output could not be opened: {e}"))?;
        stream
            .play()
            .map_err(|e| format!("The audio output could not be started: {e}"))?;
        let worker_engine = engine.clone();
        let worker = std::thread::Builder::new()
            .name("audio player".into())
            .spawn(move || worker_engine.run(source))
            .map_err(|e| e.to_string())?;
        Ok(Player {
            engine,
            _stream: stream,
            worker: Some(worker),
        })
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.engine.quit();
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

/// `m:ss` (or `h:mm:ss`), as the player's time reads: `0:03`, `1:05:09`.
pub fn clock(seconds: f64) -> String {
    let s = if seconds.is_finite() && seconds > 0.0 {
        seconds.floor() as u64
    } else {
        0
    };
    let (h, m, s) = (s / 3600, (s / 60) % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mono 16-bit WAV of `tone` Hz at `rate`, `seconds` long.
    pub(crate) fn wav(path: &Path, rate: u32, seconds: f64, tone: f64) {
        let n = (rate as f64 * seconds) as usize;
        let mut data = Vec::with_capacity(n * 2);
        for i in 0..n {
            let v = (0.5 * (2.0 * std::f64::consts::PI * tone * i as f64 / rate as f64).sin()
                * 32767.0) as i16;
            data.extend_from_slice(&v.to_le_bytes());
        }
        let mut f = Vec::new();
        f.extend_from_slice(b"RIFF");
        f.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        f.extend_from_slice(b"WAVEfmt ");
        f.extend_from_slice(&16u32.to_le_bytes());
        f.extend_from_slice(&1u16.to_le_bytes());
        f.extend_from_slice(&1u16.to_le_bytes());
        f.extend_from_slice(&rate.to_le_bytes());
        f.extend_from_slice(&(rate * 2).to_le_bytes());
        f.extend_from_slice(&2u16.to_le_bytes());
        f.extend_from_slice(&16u16.to_le_bytes());
        f.extend_from_slice(b"data");
        f.extend_from_slice(&(data.len() as u32).to_le_bytes());
        f.extend_from_slice(&data);
        std::fs::write(path, f).unwrap();
    }

    fn crossings(x: &[f32], channels: usize, rate: u32) -> f64 {
        let mono: Vec<f32> = x.iter().step_by(channels).copied().collect();
        let n = mono.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
        n as f64 / 2.0 / (mono.len() as f64 / rate as f64)
    }

    #[test]
    fn the_clock_reads_like_the_players() {
        assert_eq!(clock(0.0), "0:00");
        assert_eq!(clock(3.0065), "0:03");
        assert_eq!(clock(65.9), "1:05");
        assert_eq!(clock(3909.0), "1:05:09");
        assert_eq!(clock(f64::NAN), "0:00");
    }

    #[test]
    fn a_wav_decodes_whole_and_seeks() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("tone.wav");
        wav(&p, 44_100, 2.0, 440.0);
        let info = probe(&p).unwrap();
        assert_eq!((info.sample_rate, info.channels), (44_100, 1));
        assert!((info.duration.unwrap() - 2.0).abs() < 1e-6);
        let mut s = Source::open(&p).unwrap();
        let mut all = Vec::new();
        while let Some(b) = s.next_block().unwrap() {
            assert_eq!(b.channels, 1);
            all.extend(b.samples);
        }
        assert_eq!(all.len(), 88_200);
        assert!((crossings(&all, 1, 44_100) - 440.0).abs() < 2.0);
        s.seek(1.5).unwrap();
        let mut rest = 0;
        while let Some(b) = s.next_block().unwrap() {
            rest += b.samples.len();
        }
        assert_eq!(rest, 22_050);
    }

    #[test]
    fn remixing_spreads_mono_and_folds_stereo() {
        assert_eq!(remix(&[1.0, 2.0], 1, 2), vec![1.0, 1.0, 2.0, 2.0]);
        assert_eq!(remix(&[1.0, 3.0, 2.0, 4.0], 2, 1), vec![2.0, 3.0]);
        assert_eq!(remix(&[1.0, 2.0, 3.0], 3, 2), vec![1.0, 2.0]);
    }

    #[test]
    fn resampling_in_blocks_keeps_the_pitch_and_the_length() {
        let from = 44_100;
        let tone: Vec<f32> = (0..from)
            .map(|i| (2.0 * std::f64::consts::PI * 1000.0 * i as f64 / from as f64).sin() as f32)
            .collect();
        let mut r = Resampler::new(from, 48_000, 1);
        let mut out = Vec::new();
        for block in tone.chunks(1152) {
            out.extend(r.process(block));
        }
        assert!((out.len() as i64 - 48_000).abs() <= 2, "{}", out.len());
        assert!((crossings(&out, 1, 48_000) - 1000.0).abs() < 3.0);
        // No step at a block boundary: the largest jump is the tone's own.
        let max_jump = out.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f32::max);
        assert!(max_jump < 0.14, "{max_jump}");
    }

    fn engine_on(p: &Path, out: Output) -> (Engine, Source, Resampler) {
        let s = Source::open(p).unwrap();
        let e = Engine::new(out, s.info().duration);
        let r = Resampler::new(s.sample_rate, out.rate, out.channels);
        (e, s, r)
    }

    fn pump_all(e: &Engine, s: &mut Source, r: &mut Resampler) {
        while e.pump(s, r) {}
    }

    #[test]
    fn the_engine_plays_pauses_seeks_and_stops_at_the_end() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("tone.wav");
        wav(&p, 48_000, 1.0, 500.0);
        let out = Output {
            rate: 48_000,
            channels: 2,
        };
        let (e, mut s, mut r) = engine_on(&p, out);
        pump_all(&e, &mut s, &mut r);
        // Paused at first: silence, and the position stays.
        let mut buf = vec![1.0; 960];
        e.fill(&mut buf);
        assert!(buf.iter().all(|&v| v == 0.0));
        assert_eq!(e.position(), 0.0);
        e.play();
        e.fill(&mut buf);
        assert!(buf.iter().any(|&v| v != 0.0));
        // Mono spread to both channels.
        assert!(buf.chunks(2).all(|f| f[0] == f[1]));
        assert!((e.position() - 0.01).abs() < 1e-9);
        // Volume is the gain, as `<audio>`'s is; mute is silence.
        let peak = |b: &[f32]| b.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let full = peak(&buf);
        e.set_volume(0.5);
        e.fill(&mut buf);
        assert!((peak(&buf) - full * 0.5).abs() < 0.02, "{}", peak(&buf));
        e.set_muted(true);
        e.fill(&mut buf);
        assert!(buf.iter().all(|&v| v == 0.0));
        e.set_muted(false);
        e.pause();
        let at = e.position();
        e.fill(&mut buf);
        assert_eq!(e.position(), at);
        // A seek: the position is the target at once, then advances from it.
        e.seek(0.75);
        assert_eq!(e.position(), 0.75);
        pump_all(&e, &mut s, &mut r);
        e.play();
        e.fill(&mut buf);
        assert!((e.position() - 0.76).abs() < 1e-9);
        // Play out: paused and ended at the end, position the duration.
        let mut big = vec![0.0; 48_000 * 2];
        while !e.ended() {
            pump_all(&e, &mut s, &mut r);
            e.fill(&mut big);
        }
        assert!(!e.is_playing());
        assert_eq!(e.position(), 1.0);
        // Play again starts over.
        e.play();
        assert_eq!(e.position(), 0.0);
        assert!(e.is_playing());
    }

    #[test]
    fn a_seek_is_kept_within_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("tone.wav");
        wav(&p, 8_000, 1.0, 500.0);
        let (e, _, _) = engine_on(&p, Output { rate: 8_000, channels: 1 });
        e.seek(-3.0);
        assert_eq!(e.position(), 0.0);
        e.seek(9.0);
        assert_eq!(e.position(), 1.0);
    }

    #[test]
    fn an_unreadable_file_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.mp3");
        std::fs::write(&p, b"not audio at all").unwrap();
        assert!(Source::open(&p).is_err());
        assert!(probe(&dir.path().join("gone.mp3")).is_err());
    }
}
