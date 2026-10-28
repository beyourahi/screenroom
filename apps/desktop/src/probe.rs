//! Container facts read with ffprobe: duration, and whether a cover is embedded.
//!
//! A scan only reads names and sizes, so a fresh library knows no runtimes. This
//! module asks ffprobe for the two facts the catalogue needs and nothing more.
//! It reads a container's header — about 80 ms a file on a USB SSD — and never
//! writes anywhere, so it is safe on a read-only drive.

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use screenroom_core::Library;

/// Most bytes of ffprobe output accepted. A file with hundreds of streams still
/// fits; anything larger is not a file this module should trust.
const MAX_PROBE_BYTES: usize = 64 * 1024;

/// Files asked of the library per page during a duration pass.
const PASS_PAGE: i64 = 100;

/// How often a finished batch is reported, so runtimes appear while the pass runs.
const REPORT_EVERY: usize = 50;

/// What one probe learned.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Facts {
    /// Container duration in milliseconds; `0` when the container does not say.
    pub duration_ms: i64,
    /// Stream index of an embedded cover image, if the file carries one.
    pub cover_stream: Option<u32>,
}

/// The command one probe runs. Arguments are separate values, never a shell line.
pub fn command_for(path: &Path) -> Command {
    let mut command = Command::new(crate::artwork::tool("ffprobe"));
    command
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration:stream=index,codec_type:stream_disposition=attached_pic")
        .arg("-of")
        .arg("flat")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    command
}

/// Probes one file under the same deadline and output cap as poster extraction.
pub fn probe(path: &Path) -> Result<Facts, String> {
    let bytes = crate::artwork::capture(command_for(path), MAX_PROBE_BYTES, "Probing the file")?;
    Ok(parse(&String::from_utf8_lossy(&bytes)))
}

/// Reads ffprobe's `flat` output.
///
/// Lines look like `format.duration="6731.751000"` and
/// `streams.stream.8.disposition.attached_pic=1`. Anything unexpected is
/// ignored, so a malformed or hostile container can only produce "unknown".
pub fn parse(output: &str) -> Facts {
    let mut facts = Facts::default();
    let mut video = Vec::new();
    let mut covers = Vec::new();
    for line in output.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"');
        if key == "format.duration" {
            if let Ok(seconds) = value.parse::<f64>()
                && seconds.is_finite()
                && seconds > 0.0
            {
                facts.duration_ms = (seconds * 1_000.0).round() as i64;
            }
            continue;
        }
        let Some(rest) = key.strip_prefix("streams.stream.") else {
            continue;
        };
        let Some((index, field)) = rest.split_once('.') else {
            continue;
        };
        let Ok(index) = index.parse::<u32>() else {
            continue;
        };
        match (field, value) {
            ("codec_type", "video") => video.push(index),
            ("disposition.attached_pic", "1") => covers.push(index),
            _ => {}
        }
    }
    // The flat writer numbers streams by their own index, so the key's number is
    // the stream to map. A cover must also be a video stream to be decodable.
    facts.cover_stream = covers.into_iter().find(|index| video.contains(index));
    facts
}

/// A running duration pass, reporting how many durations it has stored.
pub struct DurationPass {
    /// Counts reported as batches finish; the channel closes when the pass ends.
    pub reports: Receiver<usize>,
}

/// Walks every available file without a known duration and probes it.
///
/// Sequential, one ffprobe at a time, and it waits while `paused` is set — the
/// app sets it while a file is playing, so a viewing never shares the drive with
/// the pass. A file that fails is passed over (the walk is keyset-paged), and
/// the next launch tries it again; nothing is ever written to the media.
pub fn spawn_duration_pass(library: Arc<Library>, paused: Arc<AtomicBool>) -> DurationPass {
    let (sender, reports) = channel();
    std::thread::spawn(move || {
        let mut after: Option<String> = None;
        let mut stored = 0_usize;
        let mut unreported = 0_usize;
        loop {
            let page = match library.unprobed_files(after.as_deref(), PASS_PAGE) {
                Ok(page) if !page.is_empty() => page,
                _ => break,
            };
            for file in page {
                while paused.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(500));
                }
                if let Ok(facts) = probe(&file.path)
                    && library
                        .record_duration(&file.file_id, facts.duration_ms)
                        .unwrap_or(false)
                {
                    stored += 1;
                    unreported += 1;
                }
                after = Some(file.file_id);
                if unreported >= REPORT_EVERY {
                    unreported = 0;
                    if sender.send(stored).is_err() {
                        return;
                    }
                }
            }
        }
        let _ = sender.send(stored);
    });
    DurationPass { reports }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_and_cover_are_read_from_flat_output() {
        let output = "streams.stream.0.index=0\n\
            streams.stream.0.codec_type=\"video\"\n\
            streams.stream.0.disposition.attached_pic=0\n\
            streams.stream.1.codec_type=\"audio\"\n\
            streams.stream.1.disposition.attached_pic=0\n\
            streams.stream.8.index=8\n\
            streams.stream.8.codec_type=\"video\"\n\
            streams.stream.8.disposition.attached_pic=1\n\
            format.duration=\"6731.751000\"\n";
        assert_eq!(
            parse(output),
            Facts {
                duration_ms: 6_731_751,
                cover_stream: Some(8),
            }
        );
    }

    #[test]
    fn missing_or_hostile_values_mean_unknown() {
        assert_eq!(parse(""), Facts::default());
        assert_eq!(parse("format.duration=\"N/A\"\n"), Facts::default());
        assert_eq!(
            parse("format.duration=\"-3\"\nformat.duration=inf\n"),
            Facts::default()
        );
        // An attached_pic flag on a non-video stream is not a decodable cover.
        let audio_flag = "streams.stream.2.codec_type=\"audio\"\n\
            streams.stream.2.disposition.attached_pic=1\n";
        assert_eq!(parse(audio_flag).cover_stream, None);
        assert_eq!(
            parse("streams.stream.x.codec_type=\"video\"\n"),
            Facts::default()
        );
    }

    #[test]
    fn the_probe_passes_the_path_as_one_argument() {
        let command = command_for(Path::new("/media/a film; rm -rf /.mkv"));
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();
        assert_eq!(
            args.last().map(String::as_str),
            Some("/media/a film; rm -rf /.mkv")
        );
        assert!(command.get_program().to_string_lossy().ends_with("ffprobe"));
    }
}
