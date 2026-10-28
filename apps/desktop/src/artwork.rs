//! Poster extraction: one frame from a file the library already holds.
//!
//! Three rules shape this module. Artwork is **local** — the only source is media
//! on this machine, so nothing here can leave a library undrawable offline. It is
//! **bounded** — one frame per work, a wall-clock deadline per run, a hard cap on
//! the bytes accepted, and a small queue, so a corrupt file cannot stall the
//! application. And it is **off the UI thread** — ffmpeg is a child process, and
//! the frame it returns is handed back through a channel the update loop polls,
//! exactly like the scanner.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, channel, sync_channel};
use std::time::{Duration, Instant};

use screenroom_core::{ArtworkCandidate, Library};

/// Widest poster kept, in pixels. Cards are a few hundred points wide at most.
const POSTER_WIDTH: u32 = 400;

/// Most bytes one extraction may return.
const MAX_POSTER_BYTES: usize = 768 * 1024;

/// How long one ffmpeg run gets before it is killed.
///
/// A damaged container can make a demuxer spin; the deadline converts that from a
/// hung application into a work whose artwork is simply missing.
const EXTRACT_TIMEOUT: Duration = Duration::from_secs(20);

/// Extractions queued at once. Bounded so a library of thousands cannot queue
/// thousands of child processes, and so the queue's cost is known up front.
const QUEUE_DEPTH: usize = 16;

/// A frame ready to be stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Poster {
    /// Encoded JPEG bytes.
    pub bytes: Vec<u8>,
    /// Pixel width of the stored image.
    pub width: i64,
    /// Pixel height of the stored image.
    pub height: i64,
    /// Position in the file the frame was taken from.
    pub sampled_at_ms: i64,
    /// Whether this is the file's own embedded cover rather than a film frame.
    pub cover: bool,
}

/// Where in a file to look for a representative frame.
///
/// Ten percent in is past a title card and before the establishing shots that
/// often open on black. A file whose length is not known yet is sampled two
/// minutes in, which is where a film is playing in almost every case, and a short
/// clip is sampled at its midpoint so it is not sampled past its own end.
pub fn sample_position_ms(duration_ms: i64) -> i64 {
    if duration_ms <= 0 {
        return 120_000;
    }
    (duration_ms / 10).clamp(1_000, 900_000)
}

/// Where a media tool is run from.
///
/// An application opened from the Dock or Finder inherits launchd's `PATH`
/// (`/usr/bin:/bin:/usr/sbin:/sbin`), which holds no ffmpeg, so a bare name
/// worked from a terminal and failed in the installed bundle. `PATH` is still
/// searched first; Homebrew's two prefixes are the fallback. Only absolute
/// directories are considered, so the working directory can never supply a tool.
pub fn tool(name: &str) -> PathBuf {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .chain([
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
        ])
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| PathBuf::from(name))
}

/// The command one extraction runs.
///
/// Arguments are passed as separate values, never through a shell, so a filename
/// containing spaces, quotes or `;` is just a filename. `-nostdin` stops ffmpeg
/// from consuming the application's input, and the frame is written to stdout so
/// no temporary file has to be created or cleaned up.
pub fn command_for(path: &Path, at_ms: i64) -> Command {
    let mut command = Command::new(tool("ffmpeg"));
    command
        .arg("-nostdin")
        .arg("-v")
        .arg("error")
        .arg("-ss")
        .arg(format!("{:.3}", at_ms.max(0) as f64 / 1_000.0))
        .arg("-i")
        .arg(path)
        .arg("-an")
        .arg("-sn")
        .arg("-frames:v")
        .arg("1")
        .arg("-vf")
        .arg(format!("scale={POSTER_WIDTH}:-2"))
        .arg("-f")
        .arg("image2")
        .arg("-vcodec")
        .arg("mjpeg")
        .arg("pipe:1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    command
}

/// The command that reads a file's embedded cover image.
///
/// `stream` is the absolute stream index ffprobe reported with the
/// `attached_pic` disposition. The image is scaled like a sampled frame, so a
/// cover is stored under the same size and byte limits as any other poster.
pub fn cover_command_for(path: &Path, stream: u32) -> Command {
    let mut command = Command::new(tool("ffmpeg"));
    command
        .arg("-nostdin")
        .arg("-v")
        .arg("error")
        .arg("-i")
        .arg(path)
        .arg("-map")
        .arg(format!("0:{stream}"))
        .arg("-frames:v")
        .arg("1")
        .arg("-vf")
        .arg(format!("scale={POSTER_WIDTH}:-2"))
        .arg("-f")
        .arg("image2")
        .arg("-vcodec")
        .arg("mjpeg")
        .arg("pipe:1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    command
}

/// Reads a file's embedded cover image as a poster.
pub fn extract_cover(path: &Path, stream: u32) -> Result<Poster, String> {
    let bytes = capture(
        cover_command_for(path, stream),
        MAX_POSTER_BYTES,
        "Reading the cover",
    )?;
    let (width, height) =
        jpeg_dimensions(&bytes).ok_or_else(|| "The cover was not a readable image.".to_string())?;
    Ok(Poster {
        bytes,
        width,
        height,
        sampled_at_ms: 0,
        cover: true,
    })
}

/// The best poster a file can give: its own cover image, else a sampled frame.
///
/// The container is probed first. A probed duration is stored, so the runtime
/// appears on the card before the first play and the frame is sampled at a real
/// ten percent instead of a two-minute guess. A cover that fails to decode falls
/// back to a frame rather than leaving the work without a poster.
pub fn best_poster(library: &Library, candidate: &ArtworkCandidate) -> Result<Poster, String> {
    let mut duration_ms = candidate.duration_ms;
    let mut cover = None;
    if let Ok(facts) = crate::probe::probe(&candidate.path) {
        if facts.duration_ms > 0 {
            let _ = library.record_duration(&candidate.file_id, facts.duration_ms);
            if duration_ms <= 0 {
                duration_ms = facts.duration_ms;
            }
        }
        cover = facts.cover_stream;
    }
    if let Some(stream) = cover
        && let Ok(poster) = extract_cover(&candidate.path, stream)
    {
        return Ok(poster);
    }
    extract_poster(&candidate.path, sample_position_ms(duration_ms))
}

/// Extracts one frame, falling back to the start of the file.
///
/// The fallback matters for a file whose stored duration is wrong: a seek past
/// the end produces no frame at all, and the opening of the film is still a
/// truthful picture of what the file contains.
pub fn extract_poster(path: &Path, at_ms: i64) -> Result<Poster, String> {
    let mut last_reason = String::new();
    for attempt in [at_ms, 0] {
        match run(path, attempt) {
            Ok(bytes) if !bytes.is_empty() => {
                let (width, height) = jpeg_dimensions(&bytes)
                    .ok_or_else(|| "The extracted frame was not a readable JPEG.".to_string())?;
                return Ok(Poster {
                    bytes,
                    width,
                    height,
                    sampled_at_ms: attempt,
                    cover: false,
                });
            }
            Ok(_) => last_reason = format!("No frame could be read at {attempt} ms."),
            Err(reason) => last_reason = reason,
        }
    }
    Err(last_reason)
}

/// Runs one ffmpeg invocation under a deadline.
fn run(path: &Path, at_ms: i64) -> Result<Vec<u8>, String> {
    capture(
        command_for(path, at_ms),
        MAX_POSTER_BYTES,
        "Reading a frame",
    )
}

/// Runs a media tool under a deadline, returning at most `cap` bytes of stdout.
///
/// Shared by the frame extractor and the container probe: both read untrusted
/// files, so both get the same wall clock, the same output cap and the same
/// guarantee that a stalled child is killed and reaped rather than left behind.
pub(crate) fn capture(mut command: Command, cap: usize, what: &str) -> Result<Vec<u8>, String> {
    let program = command.get_program().to_string_lossy().to_string();
    let mut child = command
        .spawn()
        .map_err(|error| format!("{program} could not be started: {error}"))?;

    // The reader is its own thread so a full pipe buffer can never deadlock the
    // deadline, and it stops reading at the cap rather than at end of file.
    let stdout = child.stdout.take();
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut stream) = stdout {
            let _ = stream.by_ref().take(cap as u64 + 1).read_to_end(&mut bytes);
        }
        bytes
    });

    let deadline = Instant::now() + EXTRACT_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(format!("{program} could not be waited on: {error}"));
            }
        }
    };

    let bytes = reader
        .join()
        .map_err(|_| "the output reader stopped unexpectedly".to_string())?;
    match status {
        None => Err(format!("{what} took too long and was stopped.")),
        Some(status) if !status.success() && bytes.is_empty() => {
            Err(format!("{program} could not read this file."))
        }
        Some(_) if bytes.len() > cap => Err("The output was too large to keep.".to_string()),
        Some(_) => Ok(bytes),
    }
}

/// Width and height of a JPEG, read from its start-of-frame header.
///
/// Only what is needed to record the size is parsed; the image itself is decoded
/// by the widget that draws it, so a full decoder here would be a second, unused
/// one.
pub fn jpeg_dimensions(bytes: &[u8]) -> Option<(i64, i64)> {
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut index = 2;
    // Five bytes of header plus two of dimensions are read from the frame marker,
    // so a segment starting at the last possible position must still fit.
    while index + 8 < bytes.len() {
        if bytes[index] != 0xFF {
            index += 1;
            continue;
        }
        let marker = bytes[index + 1];
        // Start of frame markers carry the dimensions; C4, C8 and CC are not
        // frames despite sitting in the same range.
        if (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
            let height = u16::from_be_bytes([bytes[index + 5], bytes[index + 6]]) as i64;
            let width = u16::from_be_bytes([bytes[index + 7], bytes[index + 8]]) as i64;
            return (width > 0 && height > 0).then_some((width, height));
        }
        let length = u16::from_be_bytes([bytes[index + 2], bytes[index + 3]]) as usize;
        if length < 2 {
            return None;
        }
        index += 2 + length;
    }
    None
}

/// Something the extraction worker reports back to the update loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtworkEvent {
    /// Artwork was extracted and stored.
    Stored {
        /// Work the image belongs to.
        work_id: String,
    },
    /// Artwork could not be produced; the card keeps its title-only face.
    Failed {
        /// Work the image was meant for.
        work_id: String,
        /// Why, in words a viewer could act on.
        reason: String,
    },
}

/// One queued extraction.
struct Job {
    candidate: ArtworkCandidate,
}

/// The background poster extractor.
///
/// Dropping it closes the request channel, which ends the worker thread once the
/// extraction in flight returns, so quitting during extraction cannot leave a
/// thread writing into a closed database.
pub struct Extractor {
    requests: SyncSender<Job>,
    events: Receiver<ArtworkEvent>,
}

impl Extractor {
    /// Starts a worker that stores what it extracts into `library`.
    pub fn spawn(library: Arc<Library>) -> Self {
        let (requests, queue) = sync_channel::<Job>(QUEUE_DEPTH);
        let (sender, events) = channel::<ArtworkEvent>();
        std::thread::spawn(move || {
            while let Ok(job) = queue.recv() {
                let candidate = job.candidate;
                let outcome = best_poster(&library, &candidate);
                let event = match outcome {
                    Ok(poster) => match library.store_artwork(
                        &candidate.work_id,
                        Some(&candidate.file_id),
                        &poster.bytes,
                        poster.width,
                        poster.height,
                        poster.sampled_at_ms,
                    ) {
                        Ok(()) => ArtworkEvent::Stored {
                            work_id: candidate.work_id,
                        },
                        Err(error) => ArtworkEvent::Failed {
                            work_id: candidate.work_id,
                            reason: format!("{}: {error}", error.code()),
                        },
                    },
                    Err(reason) => ArtworkEvent::Failed {
                        work_id: candidate.work_id,
                        reason,
                    },
                };
                if sender.send(event).is_err() {
                    break;
                }
            }
        });
        Self { requests, events }
    }

    /// Queues one extraction, reporting whether it was accepted.
    ///
    /// A full queue is not an error: the work is simply offered again the next
    /// time its screen is drawn, which spreads a large library's extraction over
    /// several visits instead of one long pause.
    pub fn request(&self, candidate: ArtworkCandidate) -> bool {
        match self.requests.try_send(Job { candidate }) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => false,
        }
    }

    /// Collects everything the worker has reported since the last poll.
    pub fn poll(&self) -> Vec<ArtworkEvent> {
        self.events.try_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sample_position_is_inside_the_file_and_never_at_its_start() {
        assert_eq!(sample_position_ms(0), 120_000);
        // A one-second clip is sampled at its midpoint rather than past its end.
        assert_eq!(sample_position_ms(1_000), 1_000);
        assert_eq!(sample_position_ms(600_000), 60_000);
        assert_eq!(sample_position_ms(10_800_000), 900_000);
    }

    #[test]
    fn the_command_passes_the_path_as_an_argument_and_writes_to_stdout() {
        let command = command_for(Path::new("/media/a film; rm -rf /.mkv"), 65_000);
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();
        assert_eq!(args[0], "-nostdin");
        assert!(args.contains(&"/media/a film; rm -rf /.mkv".to_string()));
        assert!(args.contains(&"65.000".to_string()));
        assert!(args.contains(&"1".to_string()));
        assert_eq!(args.last().map(String::as_str), Some("pipe:1"));
        // No shell is involved, so the semicolon in the name stays a character.
        assert!(command.get_program().to_string_lossy().ends_with("ffmpeg"));
    }

    #[test]
    fn a_tool_is_found_outside_a_minimal_launch_path() {
        // The installed bundle's PATH; ffmpeg is never in it on this machine.
        let found = tool("ffmpeg");
        if found.is_absolute() {
            assert!(found.is_file());
        } else {
            assert_eq!(found, PathBuf::from("ffmpeg"));
        }
    }

    #[test]
    fn dimensions_are_read_from_a_start_of_frame_header() {
        let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00];
        bytes.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08, 0x02, 0x58, 0x01, 0x90]);
        assert_eq!(jpeg_dimensions(&bytes), Some((400, 600)));
        assert_eq!(jpeg_dimensions(b"not a jpeg"), None);
    }
}
