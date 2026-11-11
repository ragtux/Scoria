//! The default microphone, through cpal: ALSA on Linux (PipeWire and
//! PulseAudio through their ALSA plugins), CoreAudio on macOS, WASAPI on
//! Windows. Each block the host delivers goes to the recording's worker
//! as `f32` and the callback returns at once.

use crate::session::{Feed, Session};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};

/// A recording that is capturing. Dropping it stops the microphone and
/// abandons the recording; [`Capture::stop`] keeps it.
pub struct Capture {
    stream: Stream,
    session: Session,
    pub sample_rate: u32,
    pub channels: usize,
    /// The input device's name, for the log.
    pub device: String,
    /// The samples it delivers (`f32`, `i16`, …), for the log.
    pub format: String,
}

impl Capture {
    /// Opens the default input device in its default configuration and
    /// starts recording from it.
    pub fn start() -> Result<Capture, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "No microphone was found".to_string())?;
        let name = device
            .description()
            .map(|d| d.to_string())
            .unwrap_or_else(|_| "input".into());
        let supported = device
            .default_input_config()
            .map_err(|e| format!("The microphone could not be opened: {e}"))?;
        let format = supported.sample_format();
        let config = supported.config();
        let sample_rate = config.sample_rate;
        let channels = config.channels as usize;
        let (session, feed) = Session::start(sample_rate, channels)?;
        let on_error = |e| eprintln!("audio recorder: {e}");
        let stream = match format {
            SampleFormat::F32 => device.build_input_stream(
                config,
                move |data: &[f32], _: &_| feed.send(data),
                on_error,
                None,
            ),
            SampleFormat::I16 => device.build_input_stream(
                config,
                move |data: &[i16], _: &_| feed.send(&convert(data, |s| s as f32 / 32768.0)),
                on_error,
                None,
            ),
            SampleFormat::I32 => device.build_input_stream(
                config,
                move |data: &[i32], _: &_| {
                    feed.send(&convert(data, |s| s as f32 / 2_147_483_648.0))
                },
                on_error,
                None,
            ),
            SampleFormat::U16 => device.build_input_stream(
                config,
                move |data: &[u16], _: &_| {
                    feed.send(&convert(data, |s| (s as f32 - 32768.0) / 32768.0))
                },
                on_error,
                None,
            ),
            other => {
                return Err(format!(
                    "The microphone's {other} samples are not supported"
                ))
            }
        }
        .map_err(|e| format!("The microphone could not be opened: {e}"))?;
        stream
            .play()
            .map_err(|e| format!("The microphone could not be started: {e}"))?;
        Ok(Capture {
            stream,
            session,
            sample_rate,
            channels,
            device: name,
            format: format.to_string(),
        })
    }

    /// Stops the microphone; the session finishes what was captured.
    pub fn stop(self) -> Session {
        // Dropping the stream ends the callbacks and drops their feed, so
        // every block that will ever come is already queued.
        drop(self.stream);
        self.session
    }
}

fn convert<T: Copy>(data: &[T], f: impl Fn(T) -> f32) -> Vec<f32> {
    data.iter().map(|&s| f(s)).collect()
}

/// Feeds a recording from a source other than a microphone: the tests,
/// and anything that wants the real encoding path without a device.
pub fn feed_from(sample_rate: u32, channels: usize) -> Result<(Session, Feed), String> {
    Session::start(sample_rate, channels)
}
