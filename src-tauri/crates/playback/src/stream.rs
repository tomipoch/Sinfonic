//! Stream resolution: local files vs HTTP URLs.
//!
//! `rodio::Decoder` requires `Read + Seek`, which means streaming an
//! HTTP body directly isn't possible. We bridge that gap by:
//!
//! - **Local files** (absolute paths) → open with `std::fs::File`,
//!   wrap in `BufReader`, decode directly. Cheap and supports seeking.
//! - **HTTP URLs** (`http://`, `https://`) → fetch the body with
//!   `reqwest::blocking::get` **on the tokio blocking pool**, wrap in
//!   a `Cursor`, decode from there. Slightly wasteful for long tracks
//!   but trivially robust and supports seeking.
//!
//! The blocking HTTP request is funneled through
//! [`tokio::task::spawn_blocking`] so the caller's async worker is not
//! stalled for the full download duration (typically 1-10 s for a
//! cached audio body on a LAN). Local files don't need the hop because
//! `File::open` is fast.
//!
//! The decoded source is then optionally piped through [`crate::eq::apply`]
//! so that the AudioPlayer doesn't need to know whether EQ is on.

use std::fs::File;
use std::io::{self, BufReader, Cursor, Read};
use std::path::Path;
use std::time::Duration;

use rodio::{Decoder, Source};
use thiserror::Error;

#[cfg(test)]
use std::path::PathBuf;

use crate::eq::{apply, SharedEqualizer};

/// Errors that can surface while opening a stream.
#[derive(Debug, Error)]
pub enum StreamError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("http error: {0}")]
    Http(String),
    #[error("decode error: {0}")]
    Decode(String),
    #[error("unsupported scheme: {0}")]
    UnsupportedScheme(String),
    #[error("invalid uri: {0}")]
    InvalidUri(String),
}

/// Decoded audio source, ready to append to a rodio `Sink`. The source
/// has been normalised to `f32` samples so callers don't have to worry
/// about the source's native format.
pub struct StreamHandle {
    pub source: Box<dyn Source<Item = f32> + Send>,
    /// The source's `total_duration()` if known, in seconds. `None` for
    /// live streams.
    pub duration_seconds: Option<u32>,
}

impl StreamHandle {
    /// Wrap the inner source with the project's EQ.
    pub fn with_eq(self, eq: SharedEqualizer) -> Self {
        Self {
            source: Box::new(apply(self.source, eq)),
            duration_seconds: self.duration_seconds,
        }
    }
}

/// Open a stream URI and produce a rodio `Source`.
///
/// `uri` is one of:
/// - An absolute filesystem path (e.g. `/music/track.flac`).
/// - A `file://` URI (e.g. `file:///music/track.flac`).
/// - An `http://` or `https://` URL.
///
/// The HTTP path is asynchronous: it offloads the blocking download
/// to `tokio::task::spawn_blocking` so the caller's worker thread is
/// not pinned for the duration of the network round-trip.
pub async fn open(uri: &str) -> Result<StreamHandle, StreamError> {
    let uri = uri.trim();
    if uri.is_empty() {
        return Err(StreamError::InvalidUri("empty uri".into()));
    }

    if uri.starts_with("http://") || uri.starts_with("https://") {
        return open_http(uri).await;
    }
    let path_str = uri.strip_prefix("file://").unwrap_or(uri);
    if Path::new(path_str).exists() {
        return open_local(path_str);
    }
    Err(StreamError::UnsupportedScheme(uri.to_string()))
}

fn open_local(path: &str) -> Result<StreamHandle, StreamError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let source = Decoder::new(reader)
        .map_err(|e| StreamError::Decode(e.to_string()))?
        .convert_samples::<f32>();
    let duration_seconds = source.total_duration().map(|d| d.as_secs() as u32);
    Ok(StreamHandle {
        source: Box::new(source),
        duration_seconds,
    })
}

async fn open_http(url: &str) -> Result<StreamHandle, StreamError> {
    // Move the blocking HTTP fetch off the async worker. The download
    // can take seconds for a remote track body; without this hop it
    // would stall one of tokio's worker threads for the full duration
    // and degrade responsiveness of every other IPC command.
    let bytes = tokio::task::spawn_blocking({
        let url = url.to_owned();
        move || download(&url)
    })
    .await
    .map_err(|e| StreamError::Http(format!("blocking pool join: {e}")))??;

    let cursor = Cursor::new(bytes);
    let source = Decoder::new(cursor)
        .map_err(|e| StreamError::Decode(e.to_string()))?
        .convert_samples::<f32>();
    let duration_seconds = source.total_duration().map(|d| d.as_secs() as u32);
    Ok(StreamHandle {
        source: Box::new(source),
        duration_seconds,
    })
}

/// Synchronous HTTP download. Returns the body as bytes. Used by
/// [`open_http`] (via `spawn_blocking`) to bridge from
/// `reqwest::async` (Tauri) to `rodio::Decoder` (which expects
/// `Read + Seek`).
fn download(url: &str) -> Result<Vec<u8>, StreamError> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| StreamError::Http(e.to_string()))?;
    let mut response = client
        .get(url)
        .send()
        .map_err(|e| StreamError::Http(e.to_string()))?;
    if !response.status().is_success() {
        return Err(StreamError::Http(format!(
            "GET {url} returned {}",
            response.status()
        )));
    }
    let mut buf = Vec::new();
    response
        .read_to_end(&mut buf)
        .map_err(StreamError::Io)?;
    Ok(buf)
}

/// Helper for tests: write a WAV file at `path` containing a short
/// generated tone. Returns the path on success.
#[cfg(test)]
pub(crate) fn write_test_wav(path: &Path, duration_seconds: u32) -> Result<PathBuf, StreamError> {
    use std::io::Error as IoError;
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|e| StreamError::Io(IoError::other(e.to_string())))?;
    let total_samples = spec.sample_rate * duration_seconds;
    for n in 0..total_samples {
        let t = n as f32 / spec.sample_rate as f32;
        let sample = (t * 440.0 * 2.0 * std::f32::consts::PI).sin();
        let amplitude = (i16::MAX as f32) * sample * 0.2;
        writer
            .write_sample(amplitude as i16)
            .map_err(|e| StreamError::Io(IoError::other(e.to_string())))?;
        writer
            .write_sample(amplitude as i16)
            .map_err(|e| StreamError::Io(IoError::other(e.to_string())))?;
    }
    writer
        .finalize()
        .map_err(|e| StreamError::Io(IoError::other(e.to_string())))?;
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_on<F: std::future::Future>(fut: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build runtime")
            .block_on(fut)
    }

    #[test]
    fn open_rejects_empty_uri() {
        assert!(matches!(
            block_on(open("")),
            Err(StreamError::InvalidUri(_))
        ));
    }

    #[test]
    fn open_unknown_path_returns_unsupported() {
        let bogus = "/tmp/does-not-exist-sinfonic-abc.flac";
        let result = block_on(open(bogus));
        // Falls through "not http" + "path doesn't exist" → unsupported
        // scheme error.
        assert!(matches!(result, Err(StreamError::UnsupportedScheme(_))));
    }

    #[test]
    fn test_wav_round_trip() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("sinfonic-test-{}.wav", std::process::id()));
        let written = write_test_wav(&path, 1).expect("write wav");
        let handle = block_on(open(written.to_str().unwrap())).expect("open wav");
        let mut source = handle.source;
        let mut count = 0_usize;
        for sample in source.by_ref() {
            count += 1;
            // Touch the sample so the optimiser can't elide the loop.
            let _ = sample.abs();
        }
        // 44_100 Hz × 2 channels × 1 s = 88_200 samples.
        assert!((88_000..=88_500).contains(&count), "got {count} samples");
        std::fs::remove_file(&written).ok();
    }

    #[test]
    fn open_file_uri_resolves_local_path() {
        // Regression for the local-files provider: stream URIs are
        // emitted as `file://...` and the open() dispatcher must strip
        // the scheme before handing off to open_local.
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "sinfonic-file-uri-{}-{}.wav",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        write_test_wav(&path, 1).expect("write wav");
        let uri = format!("file://{}", path.display());
        let handle = block_on(open(&uri)).expect("open file uri");
        assert!(
            handle.duration_seconds.unwrap_or(0) >= 1,
            "expected duration ~1s, got {:?}",
            handle.duration_seconds
        );
        std::fs::remove_file(&path).ok();
    }
}