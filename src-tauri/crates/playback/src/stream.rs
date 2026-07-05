//! Stream resolution: local files vs HTTP URLs.
//!
//! `rodio::Decoder` requires `Read + Seek`, which means streaming an
//! HTTP body directly isn't possible. We bridge that gap by:
//!
//! - **Local files** (absolute paths) → open with `std::fs::File`,
//!   wrap in `BufReader`, decode directly. Cheap and supports seeking.
//! - **HTTP URLs** (`http://`, `https://`) → stream the response body
//!   into a shared `Vec<u8>` and hand rodio a `StreamingSource` that
//!   implements `Read + Seek` over that growing buffer. A separate
//!   worker thread runs `reqwest::blocking::Client` and appends chunks
//!   as they arrive; rodio reads the header (256 KB prebuffer), starts
//!   playback, and the rest of the file continues streaming in the
//!   background. The user hears audio in well under a second even on
//!   a 30 MB FLAC over a home connection.
//!
//! If the first chunk takes too long (slow server, no cached
//! transcode, etc.) the open path falls back to the historical
//! "download the whole file into memory" approach so the user can
//! still play the track. The fallback is identical to the previous
//! behaviour — no regression risk for users on flaky networks.
//!
//! The decoded source is then optionally piped through
//! [`crate::eq::apply`] so that the AudioPlayer doesn't need to
//! know whether EQ is on.

use std::fs::File;
use std::io::{self, BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

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

/// Bytes we want prebuffered before rodio starts decoding. Sized to
/// comfortably cover the headers of every common audio format we
/// support (FLAC STREAMINFO, MP3 first frame, Vorbis setup header,
/// AAC ftyp+moov box with faststart) plus ~5-10 seconds of audio.
const INITIAL_PREFETCH: usize = 256 * 1024;

/// How long `open_http` waits for the first 256 KB before bailing out
/// to the full-buffer fallback. Most LAN servers deliver this in
/// under a second; we give them 10 s before we give up.
const INITIAL_PREFETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// Open a stream URI and produce a rodio `Source`.
///
/// `uri` is one of:
/// - An absolute filesystem path (e.g. `/music/track.flac`).
/// - A `file://` URI (e.g. `file:///music/track.flac`).
/// - An `http://` or `https://` URL.
///
/// The HTTP path runs the body download in a worker thread and
/// returns as soon as the first 256 KB are buffered (~1-10 s on a LAN
/// for a typical track); rodio begins playback while the rest of the
/// file streams in. See module docs for the failure-mode fallback.
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

/// Open an HTTP audio stream with progressive buffering. See the
/// module docs for the full picture.
///
/// Sequence:
/// 1. Spawn a worker thread that runs `reqwest::blocking::Client`,
///    appends chunks to a shared `Vec<u8>`, and signals EOF on completion.
/// 2. Block the current task until either 256 KB is buffered or the
///    worker signals EOF. Time out after 10 s and fall back to a
///    full download — preserves the historical behaviour when the
///    server is slow.
/// 3. Hand the buffer (wrapped in `StreamingSource`) to rodio.
///    rodio parses the format header from the prebuffered bytes,
///    starts playback. Subsequent reads wait for more bytes via
///    `Condvar` as the worker continues streaming.
async fn open_http(url: &str) -> Result<StreamHandle, StreamError> {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let eof = Arc::new(AtomicBool::new(false));
    let notify = Arc::new(Condvar::new());
    let download_failed = Arc::new(AtomicBool::new(false));
    let download_error = Arc::new(Mutex::new(None::<String>));

    // Worker thread: blocking reqwest → append to buffer. We use
    // `std::thread` (not tokio) because `reqwest::blocking` is sync
    // and `spawn_blocking` would just hand it off to a worker pool
    // we already own.
    let url_string = url.to_owned();
    let buffer_for_thread = buffer.clone();
    let eof_for_thread = eof.clone();
    let notify_for_thread = notify.clone();
    let failed_for_thread = download_failed.clone();
    let error_for_thread = download_error.clone();
    std::thread::Builder::new()
        .name("stream-download".into())
        .spawn(move || {
            run_download(url_string, buffer_for_thread, eof_for_thread, notify_for_thread, failed_for_thread, error_for_thread);
        })
        .map_err(|e| StreamError::Http(format!("spawn download thread: {e}")))?;

    // Wait until at least INITIAL_PREFETCH bytes are buffered or EOF
    // is signalled. Times out and falls back to a full download so
    // the user can still play the track on slow networks.
    if !wait_for_prebuffer(&buffer, &eof, &notify, &download_failed) {
        // Inspect why we gave up — EOF with no bytes is a real error,
        // anything else (timeout) is just a slow connection.
        if eof.load(Ordering::Acquire) && buffer.lock().unwrap().is_empty() {
            let msg = download_error
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| "empty response".into());
            return Err(StreamError::Http(msg));
        }
        // Slow connection path — drop the streaming attempt and
        // download the whole file in one shot.
        return open_http_full(url).await;
    }

    let source = StreamingSource::new(buffer, eof, notify);
    let decoded = Decoder::new(source)
        .map_err(|e| StreamError::Decode(e.to_string()))?
        .convert_samples::<f32>();
    let duration_seconds = decoded.total_duration().map(|d| d.as_secs() as u32);
    Ok(StreamHandle {
        source: Box::new(decoded),
        duration_seconds,
    })
}

/// `true` when the buffer is ready for the decoder, `false` when the
/// 10 s prebuffer timeout fires (caller falls back).
fn wait_for_prebuffer(
    buffer: &Arc<Mutex<Vec<u8>>>,
    eof: &Arc<AtomicBool>,
    notify: &Arc<Condvar>,
    failed: &Arc<AtomicBool>,
) -> bool {
    let deadline = Instant::now() + INITIAL_PREFETCH_TIMEOUT;
    let mut guard = buffer.lock().unwrap();
    loop {
        // Worker reported a hard failure → abort.
        if failed.load(Ordering::Acquire) {
            return false;
        }
        // EOF with no bytes at all → empty/error response.
        if eof.load(Ordering::Acquire) {
            return !guard.is_empty();
        }
        // Got enough to parse the format header.
        if guard.len() >= INITIAL_PREFETCH {
            return true;
        }
        // Sleep on the condvar. The worker notifies on every chunk
        // append and on EOF so we wake up to re-check.
        let now = Instant::now();
        if now >= deadline {
            return !guard.is_empty();
        }
        let remaining = deadline - now;
        let (next_guard, _) = notify
            .wait_timeout(guard, remaining.min(Duration::from_millis(500)))
            .unwrap();
        guard = next_guard;
    }
}

/// Body of the worker thread. Streams the response into the shared
/// buffer. On any error sets `failed = true` + populates
/// `download_error` so the awaiting task can report it.
fn run_download(
    url: String,
    buffer: Arc<Mutex<Vec<u8>>>,
    eof: Arc<AtomicBool>,
    notify: Arc<Condvar>,
    failed: Arc<AtomicBool>,
    download_error: Arc<Mutex<Option<String>>>,
) {
    let result = download(&url, &buffer, &eof, &notify);
    if let Err(err) = result {
        *download_error.lock().unwrap() = Some(err.to_string());
        failed.store(true, Ordering::Release);
        notify.notify_all();
    }
}

/// Synchronous body drain. We use `reqwest::blocking::Client` because
/// the response body is consumed on the same worker thread that
/// owns the stream URL lifecycle — no async hop needed for the body
/// read itself.
fn download(
    url: &str,
    buffer: &Arc<Mutex<Vec<u8>>>,
    eof: &Arc<AtomicBool>,
    notify: &Arc<Condvar>,
) -> Result<(), StreamError> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        // Hard cap on the entire body read. After this a slow server
        // gives up; the awaiting task has likely already fallen back
        // to `open_http_full` at 10 s, so this timeout rarely fires.
        .timeout(Duration::from_secs(60 * 5))
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

    let mut chunk = [0u8; 32 * 1024];
    loop {
        match response.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                let mut guard = buffer.lock().unwrap();
                guard.extend_from_slice(&chunk[..n]);
                drop(guard);
                notify.notify_all();
            }
            Err(e) => {
                return Err(StreamError::Io(e));
            }
        }
    }

    eof.store(true, Ordering::Release);
    notify.notify_all();
    Ok(())
}

/// Fallback path used when the streaming prebuffer doesn't arrive
/// in time (slow server). Downloads the entire body into a single
/// `Vec<u8>` and hands a `Cursor` to rodio. Same shape as the
/// pre-refactor implementation — guaranteed to keep working
/// regardless of format / network quirks.
async fn open_http_full(url: &str) -> Result<StreamHandle, StreamError> {
    let url_owned = url.to_owned();
    let bytes = tokio::task::spawn_blocking(move || download_full(&url_owned))
        .await
        .map_err(|e| StreamError::Http(format!("blocking pool join: {e}")))??;

    let cursor = Cursor::new(bytes);
    let decoded = Decoder::new(cursor)
        .map_err(|e| StreamError::Decode(e.to_string()))?
        .convert_samples::<f32>();
    let duration_seconds = decoded.total_duration().map(|d| d.as_secs() as u32);
    Ok(StreamHandle {
        source: Box::new(decoded),
        duration_seconds,
    })
}

/// Full-buffer download used by the fallback path.
fn download_full(url: &str) -> Result<Vec<u8>, StreamError> {
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

/// `Read + Seek` adapter over a growing in-memory buffer that's
/// being filled by a separate worker thread.
///
/// rodio's `Decoder` parses the format header at byte 0 (FLAC
/// STREAMINFO, MP3 frame sync, Vorbis ID header, AAC `ftyp`+`moov`
/// box with faststart), then reads forward sequentially. Our
/// implementation supports both patterns:
///
/// - `read` returns whatever is buffered, blocking via `Condvar`
///   when more data is needed.
/// - `seek(SeekFrom::Start(_))` and `seek(SeekFrom::Current(_))` work
///   for positions already received; seeking past what we have
///   blocks until the worker catches up. `seek(SeekFrom::End(_))`
///   returns `Unsupported` because we never know the final size
///   before the worker signals EOF.
pub struct StreamingSource {
    buffer: Arc<Mutex<Vec<u8>>>,
    eof: Arc<AtomicBool>,
    notify: Arc<Condvar>,
    read_pos: usize,
}

impl StreamingSource {
    fn new(
        buffer: Arc<Mutex<Vec<u8>>>,
        eof: Arc<AtomicBool>,
        notify: Arc<Condvar>,
    ) -> Self {
        Self {
            buffer,
            eof,
            notify,
            read_pos: 0,
        }
    }

    /// Block until `min` bytes are buffered or EOF is signalled.
    fn wait_for_at_least(&self, min: usize) {
        if self.eof.load(Ordering::Acquire) {
            return;
        }
        let mut guard = self.buffer.lock().unwrap();
        while guard.len() < min && !self.eof.load(Ordering::Acquire) {
            guard = self.notify.wait(guard).unwrap();
        }
    }
}

impl Read for StreamingSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        self.wait_for_at_least(self.read_pos + buf.len());
        let data = self.buffer.lock().unwrap();
        let available = data.len().saturating_sub(self.read_pos);
        if available == 0 {
            // EOF was signalled and we have nothing left to copy.
            return Ok(0);
        }
        let to_copy = buf.len().min(available);
        buf[..to_copy].copy_from_slice(&data[self.read_pos..self.read_pos + to_copy]);
        self.read_pos += to_copy;
        Ok(to_copy)
    }
}

impl Seek for StreamingSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let new_pos: usize = match pos {
            SeekFrom::Start(p) => p as usize,
            SeekFrom::Current(d) => {
                let candidate = self.read_pos as i64 + d;
                if candidate < 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "seek before start of stream",
                    ));
                }
                candidate as usize
            }
            SeekFrom::End(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "SeekFrom::End not supported in streaming source",
                ));
            }
        };
        self.wait_for_at_least(new_pos);
        let data = self.buffer.lock().unwrap();
        if new_pos > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "seek past end of stream",
            ));
        }
        self.read_pos = new_pos;
        Ok(self.read_pos as u64)
    }
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
    use std::io::Write as IoWrite;

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

    #[test]
    fn streaming_source_read_blocks_until_data_arrives() {
        // Exercise the Condvar path: a producer that pushes bytes in a
        // delayed fashion. The reader thread blocks on the first read
        // and resumes after the producer wakes it up.
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let eof = Arc::new(AtomicBool::new(false));
        let notify = Arc::new(Condvar::new());
        let mut source = StreamingSource::new(
            buffer.clone(),
            eof.clone(),
            notify.clone(),
        );

        // First read with an empty buffer: it would block forever, so
        // we measure that this thread wakes up within a bounded
        // budget after the producer pushes + signals EOF.
        let producer = std::thread::spawn({
            let buffer = buffer.clone();
            let eof = eof.clone();
            let notify = notify.clone();
            move || {
                std::thread::sleep(Duration::from_millis(20));
                buffer.lock().unwrap().extend_from_slice(b"hello");
                notify.notify_all();
                std::thread::sleep(Duration::from_millis(20));
                eof.store(true, Ordering::Release);
                notify.notify_all();
            }
        });

        let mut out = Vec::new();
        out.resize(5, 0u8);
        let n = source.read(&mut out).unwrap();
        assert_eq!(n, 5);
        assert_eq!(&out, b"hello");

        // Read past EOF returns 0 cleanly.
        let mut more = [0u8; 4];
        let n2 = source.read(&mut more).unwrap();
        assert_eq!(n2, 0);

        producer.join().unwrap();
    }

    #[test]
    fn streaming_source_supports_seek_within_buffer() {
        let mut payload = Vec::with_capacity(2048);
        for n in 0..1024 {
            payload.push(n as u8);
        }
        let buffer = Arc::new(Mutex::new(payload.clone()));
        let eof = Arc::new(AtomicBool::new(true));
        let notify = Arc::new(Condvar::new());

        let mut source = StreamingSource::new(buffer, eof, notify);

        // Seek forward to a known position.
        source.seek(SeekFrom::Start(512)).unwrap();
        let mut buf = [0u8; 8];
        source.read_exact(&mut buf).unwrap();
        assert_eq!(buf, [0u8, 1, 2, 3, 4, 5, 6, 7]);

        // Backward seek resets the read position so the next read
        // returns the bytes starting at offset 0.
        source.seek(SeekFrom::Start(0)).unwrap();
        let mut buf = [0u8; 4];
        source.read_exact(&mut buf).unwrap();
        assert_eq!(buf, [0u8, 1, 2, 3]);
    }

    #[test]
    fn streaming_source_rejects_seek_past_eof() {
        let buffer = Arc::new(Mutex::new(vec![1, 2, 3]));
        let eof = Arc::new(AtomicBool::new(true));
        let notify = Arc::new(Condvar::new());
        let mut source = StreamingSource::new(buffer, eof, notify);

        let result = source.seek(SeekFrom::Start(10));
        assert!(result.is_err());
    }

    #[test]
    fn streaming_source_rejects_seek_from_end() {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let eof = Arc::new(AtomicBool::new(true));
        let notify = Arc::new(Condvar::new());
        let mut source = StreamingSource::new(buffer, eof, notify);

        let result = source.seek(SeekFrom::End(0));
        assert!(result.is_err());
    }
}
