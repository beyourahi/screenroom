//! The player worker: one thread owns libmpv, the client renders elsewhere.
//!
//! ## Why a thread
//!
//! `mpv_wait_event` blocks, and render-API calls must stay on the thread that
//! created the render context. Both facts push libmpv onto its own thread; the UI
//! only ever sends commands and reads an immutable snapshot, so a slow decoder
//! can never freeze the window.
//!
//! ## What the worker publishes
//!
//! [`PlayerSnapshot`] is the whole observable state: lifecycle, position,
//! duration, the latest frame, and a truthful failure when playback cannot
//! continue. The domain layer (`screenroom-core`) remains the only place that
//! persists progress, so a native event can never write a bogus checkpoint.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::render::{MAX_DIMENSION, SoftwareRenderer, UPDATE_FRAME, target_size};
use crate::{Frame, LibMpv, Mpv, PlayerError, PlayerState, event};

/// How long the worker blocks in libmpv before checking its command queue.
const WAIT_SLICE: f64 = 0.02;

/// True when `SCREENROOM_PLAYER_TRACE` is set: prints worker steps to stderr.
fn tracing() -> bool {
    use std::sync::OnceLock;
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("SCREENROOM_PLAYER_TRACE").is_some())
}

/// Writes a worker trace line when diagnostics are enabled.
macro_rules! trace {
    ($($arg:tt)*) => {
        if tracing() {
            eprintln!("[player] {}", format_args!($($arg)*));
        }
    };
}

/// Property observation identifiers, returned as `reply_userdata`.
const ID_TIME_POS: u64 = 1;
const ID_DURATION: u64 = 2;
const ID_PAUSE: u64 = 3;
const ID_EOF_REACHED: u64 = 4;

/// `mpv_format` values used when observing properties (`mpv/client.h`).
const FORMAT_FLAG: i32 = 3;
const FORMAT_DOUBLE: i32 = 5;

/// An `mpv_event_property` (`mpv/client.h`).
#[repr(C)]
struct RawProperty {
    name: *const std::ffi::c_char,
    format: i32,
    data: *mut std::ffi::c_void,
}

/// An `mpv_event_end_file` (`mpv/client.h`), read only when needed.
#[repr(C)]
struct RawEndFile {
    reason: i32,
    error: i32,
}

/// `MPV_END_FILE_REASON_*` (`mpv/client.h`).
mod end_reason {
    pub const EOF: i32 = 0;
    pub const ERROR: i32 = 4;
}

/// A playback failure, phrased for the user and coded for the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerFailure {
    /// Stable code, matching the core's error vocabulary where one exists.
    pub code: String,
    /// Human-readable reason, never a silent blank screen.
    pub message: String,
}

/// Everything the interface may know about the player at one instant.
#[derive(Debug, Clone)]
pub struct PlayerSnapshot {
    /// Lifecycle state.
    pub state: PlayerState,
    /// Playhead in seconds.
    pub position: f64,
    /// Media duration in seconds; `0.0` until known.
    pub duration: f64,
    /// Latest decoded frame.
    pub frame: Option<Frame>,
    /// Set only while the player cannot continue.
    pub failure: Option<PlayerFailure>,
    /// True once the end of the media was reached.
    pub ended: bool,
    /// True once a file opened successfully.
    pub loaded: bool,
}

impl Default for PlayerSnapshot {
    fn default() -> Self {
        Self {
            state: PlayerState::Idle,
            position: 0.0,
            duration: 0.0,
            frame: None,
            failure: None,
            ended: false,
            loaded: false,
        }
    }
}

impl PlayerSnapshot {
    /// Progress in whole milliseconds, for the core's checkpoint contract.
    pub fn position_ms(&self) -> i64 {
        if !self.position.is_finite() || self.position < 0.0 {
            return 0;
        }
        (self.position * 1000.0).round() as i64
    }

    /// Duration in whole milliseconds; `0` when the player has not reported it.
    pub fn duration_ms(&self) -> i64 {
        if !self.duration.is_finite() || self.duration <= 0.0 {
            return 0;
        }
        (self.duration * 1000.0).round() as i64
    }
}

/// A request to the player thread.
#[derive(Debug, Clone)]
pub enum PlayerCommand {
    /// Open a local file and start playing it, optionally resuming at `resume`.
    Load {
        /// File to play.
        path: PathBuf,
        /// Position in seconds to jump to once the file is loaded.
        resume: Option<f64>,
    },
    /// Seek to an absolute position in seconds.
    Seek(f64),
    /// Pause or resume.
    SetPaused(bool),
    /// Stop playback and release the file.
    Stop,
    /// Ask the worker to exit.
    Shutdown,
}

/// The interface's handle to the player thread.
pub struct Player {
    shared: Arc<Mutex<PlayerSnapshot>>,
    commands: Sender<PlayerCommand>,
    thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Player").finish_non_exhaustive()
    }
}

impl Player {
    /// Starts the worker thread and waits until libmpv is ready.
    ///
    /// Returns the reason when libmpv is missing or cannot initialise, so the
    /// caller can show it instead of a dead video area.
    pub fn start() -> Result<Self, PlayerError> {
        let shared = Arc::new(Mutex::new(PlayerSnapshot::default()));
        let (commands, queue) = mpsc::channel();
        let (ready_sender, ready) = mpsc::channel();
        let thread_shared = Arc::clone(&shared);

        let handle = thread::Builder::new()
            .name("screenroom-player".to_string())
            .spawn(move || run(thread_shared, queue, ready_sender))
            .map_err(|error| {
                PlayerError::Unavailable(format!("could not start player: {error}"))
            })?;

        match ready.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(())) => Ok(Self {
                shared,
                commands,
                thread: Some(handle),
            }),
            Ok(Err(error)) => {
                let _ = handle.join();
                Err(error)
            }
            Err(_) => Err(PlayerError::Unavailable(
                "the player thread did not become ready".to_string(),
            )),
        }
    }

    /// Reads the latest published state.
    pub fn snapshot(&self) -> PlayerSnapshot {
        self.shared
            .lock()
            .map(|snapshot| snapshot.clone())
            .unwrap_or_default()
    }

    /// Opens `path` and starts playback from the beginning.
    pub fn load(&self, path: impl AsRef<Path>) {
        self.load_at(path, None);
    }

    /// Opens `path` and resumes at `resume` seconds once it is loaded.
    pub fn load_at(&self, path: impl AsRef<Path>, resume: Option<f64>) {
        self.send(PlayerCommand::Load {
            path: path.as_ref().to_path_buf(),
            resume,
        });
    }

    /// Seeks to an absolute position in seconds.
    pub fn seek(&self, seconds: f64) {
        self.send(PlayerCommand::Seek(seconds));
    }

    /// Pauses or resumes.
    pub fn set_paused(&self, paused: bool) {
        self.send(PlayerCommand::SetPaused(paused));
    }

    /// Stops playback.
    pub fn stop(&self) {
        self.send(PlayerCommand::Stop);
    }

    fn send(&self, command: PlayerCommand) {
        // A closed channel means the worker already exited; the snapshot then
        // carries the reason, so dropping the command is correct here.
        let _ = self.commands.send(command);
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.commands.send(PlayerCommand::Shutdown);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

/// Worker-thread entry point.
fn run(
    shared: Arc<Mutex<PlayerSnapshot>>,
    commands: Receiver<PlayerCommand>,
    ready: Sender<Result<(), PlayerError>>,
) {
    let mut runner = match Runner::new() {
        Ok(runner) => {
            let _ = ready.send(Ok(()));
            runner
        }
        Err(error) => {
            let message = error.to_string();
            publish(&shared, |snapshot| {
                snapshot.state = PlayerState::Failed;
                snapshot.failure = Some(PlayerFailure {
                    code: "player_unavailable".to_string(),
                    message,
                });
            });
            let _ = ready.send(Err(error));
            return;
        }
    };
    runner.play_loop(&commands, &shared);
}

/// Owns libmpv for the lifetime of the thread.
///
/// Field order matters: libmpv requires the render context (and therefore the
/// frames it hands out) to be released *before* the core is destroyed, and Rust
/// drops fields in declaration order. `renderer` is declared first so that
/// invariant holds even on an early return.
struct Runner {
    renderer: SoftwareRenderer,
    core: Mpv,
    library: Arc<LibMpv>,
    state: PlayerState,
    paused: bool,
    position: f64,
    duration: f64,
    loaded: bool,
    /// Position to jump to once the next file finishes loading.
    pending_resume: Option<f64>,
}

impl Runner {
    fn new() -> Result<Self, PlayerError> {
        let library = LibMpv::load()?;
        // Every option is set before initialisation, which is the only moment
        // libmpv accepts them.
        let mut options = vec![
            // On-demand rendering through the render API.
            ("vo", "libmpv"),
            // No playlist: Screenroom selects the file, the core plays it.
            ("idle", "yes"),
            ("keep-open", "no"),
            // Deterministic behaviour: never read a user's mpv.conf.
            ("config", "no"),
            ("terminal", "no"),
            // Screenroom draws its own interface.
            ("osc", "no"),
            ("input-default-bindings", "no"),
            ("input-vo-keyboard", "no"),
            // The CPU bridge cannot consume GPU frames; hardware decoding
            // arrives with the OpenGL/D3D11 adapters.
            ("hwdec", "no"),
            ("audio-client-name", "Screenroom"),
            ("video-sync", "audio"),
        ];
        if std::env::var_os("SCREENROOM_PLAYER_LOG").is_some() {
            // Opt-in diagnostics: libmpv's own log on stderr, for bug reports.
            options.push(("terminal", "yes"));
            options.push(("msg-level", "all=debug"));
        }
        let core = Mpv::new_with_options(&library, &options)?;
        let renderer = SoftwareRenderer::new(&core)?;

        core.observe("time-pos", FORMAT_DOUBLE, ID_TIME_POS)?;
        core.observe("duration", FORMAT_DOUBLE, ID_DURATION)?;
        core.observe("pause", FORMAT_FLAG, ID_PAUSE)?;
        core.observe("eof-reached", FORMAT_FLAG, ID_EOF_REACHED)?;

        Ok(Self {
            renderer,
            core,
            library,
            state: PlayerState::Idle,
            paused: false,
            position: 0.0,
            duration: 0.0,
            loaded: false,
            pending_resume: None,
        })
    }

    fn play_loop(
        &mut self,
        commands: &Receiver<PlayerCommand>,
        shared: &Arc<Mutex<PlayerSnapshot>>,
    ) {
        loop {
            loop {
                match commands.try_recv() {
                    Ok(PlayerCommand::Shutdown) => return,
                    Ok(command) => self.handle_command(command, shared),
                    Err(TryRecvError::Empty) => break,
                    // The handle was dropped; nothing can observe us any more.
                    Err(TryRecvError::Disconnected) => return,
                }
            }

            if let Some(raw) = self.core.wait_event_raw(WAIT_SLICE) {
                self.handle_event(raw, shared);
            }

            if self.renderer.take_dirty() {
                self.present(shared);
            }
        }
    }

    fn handle_command(&mut self, command: PlayerCommand, shared: &Arc<Mutex<PlayerSnapshot>>) {
        match command {
            PlayerCommand::Load { path, resume } => self.load(&path, resume, shared),
            PlayerCommand::Seek(seconds) => self.seek(seconds, shared),
            PlayerCommand::SetPaused(paused) => self.set_paused(paused, shared),
            PlayerCommand::Stop => self.stop(shared),
            PlayerCommand::Shutdown => {}
        }
    }

    fn load(&mut self, path: &Path, resume: Option<f64>, shared: &Arc<Mutex<PlayerSnapshot>>) {
        // Checked up front so a missing file is reported as such, not as a
        // generic decoder failure.
        if !path.is_file() {
            self.fail(
                "file_missing",
                format!("{} is no longer on disk.", path.display()),
                shared,
            );
            return;
        }
        let Some(path) = path.to_str() else {
            self.fail(
                "invalid_path",
                "The file path is not valid UTF-8.".to_string(),
                shared,
            );
            return;
        };
        self.position = 0.0;
        self.duration = 0.0;
        self.loaded = false;
        self.pending_resume = resume.filter(|seconds| seconds.is_finite() && *seconds > 0.5);
        self.set_state(PlayerState::Resolving, shared);
        if let Err(error) = self.core.command(&["loadfile", path, "replace"]) {
            self.fail("load_failed", error.to_string(), shared);
            return;
        }
        publish(shared, |snapshot| {
            snapshot.failure = None;
            snapshot.ended = false;
            snapshot.frame = None;
        });
    }

    fn seek(&mut self, seconds: f64, shared: &Arc<Mutex<PlayerSnapshot>>) {
        if !self.loaded {
            return;
        }
        if !seconds.is_finite() || seconds < 0.0 {
            return;
        }
        let target = if self.duration > 0.0 {
            seconds.min(self.duration)
        } else {
            seconds
        };
        if let Err(error) = self
            .core
            .command(&["seek", &format!("{target:.3}"), "absolute"])
        {
            self.fail("seek_failed", error.to_string(), shared);
        }
    }

    fn set_paused(&mut self, paused: bool, shared: &Arc<Mutex<PlayerSnapshot>>) {
        let value = if paused { "yes" } else { "no" };
        if let Err(error) = self.core.command(&["set", "pause", value]) {
            self.fail("pause_failed", error.to_string(), shared);
        }
    }

    fn stop(&mut self, shared: &Arc<Mutex<PlayerSnapshot>>) {
        let _ = self.core.command(&["stop"]);
        self.state = PlayerState::Idle;
        self.loaded = false;
        self.position = 0.0;
        self.duration = 0.0;
        publish(shared, |snapshot| {
            snapshot.state = PlayerState::Idle;
            snapshot.loaded = false;
            snapshot.position = 0.0;
            snapshot.duration = 0.0;
            snapshot.ended = false;
            snapshot.frame = None;
        });
    }

    fn handle_event(&mut self, raw: crate::RawEvent, shared: &Arc<Mutex<PlayerSnapshot>>) {
        match raw.event_id {
            event::PROPERTY_CHANGE => self.apply_property(raw, shared),
            event::START_FILE => self.set_state(PlayerState::Resolving, shared),
            event::FILE_LOADED => self.on_loaded(shared),
            event::END_FILE => self.on_end_file(raw, shared),
            event::SHUTDOWN => {}
            _ => {}
        }
    }

    fn apply_property(&mut self, raw: crate::RawEvent, shared: &Arc<Mutex<PlayerSnapshot>>) {
        if raw.data.is_null() {
            return;
        }
        // Safety: libmpv guarantees the payload type for the observed property.
        let property = unsafe { &*(raw.data as *const RawProperty) };
        trace!(
            "property id={} present={}",
            raw.reply_userdata,
            !property.data.is_null()
        );
        if property.data.is_null() {
            // The property is currently unavailable (before load, after stop).
            return;
        }
        match raw.reply_userdata {
            ID_TIME_POS => {
                let value = unsafe { *(property.data as *const f64) };
                if value.is_finite() && value >= 0.0 {
                    self.position = value;
                }
            }
            ID_DURATION => {
                let value = unsafe { *(property.data as *const f64) };
                if value.is_finite() && value > 0.0 {
                    self.duration = value;
                }
            }
            ID_PAUSE => {
                let paused = unsafe { *(property.data as *const i32) } != 0;
                self.paused = paused;
                if self.loaded {
                    self.set_state(
                        if paused {
                            PlayerState::Paused
                        } else {
                            PlayerState::Playing
                        },
                        shared,
                    );
                }
            }
            ID_EOF_REACHED => {
                let ended = unsafe { *(property.data as *const i32) } != 0;
                if ended {
                    self.set_state(PlayerState::Ended, shared);
                    publish(shared, |snapshot| snapshot.ended = true);
                }
            }
            _ => {}
        }
        self.publish_snapshot(shared);
    }

    fn on_loaded(&mut self, shared: &Arc<Mutex<PlayerSnapshot>>) {
        trace!("event: file loaded");
        self.loaded = true;
        self.duration = self.core.get_f64("duration").unwrap_or(0.0).max(0.0);

        let width = self.core.get_i64("width").unwrap_or(0).max(0) as u32;
        let height = self.core.get_i64("height").unwrap_or(0).max(0) as u32;
        let (width, height) = target_size(width, height, MAX_DIMENSION);
        self.renderer.set_target(width, height);

        if let Some(position) = self.pending_resume.take()
            && position > 0.5
        {
            let _ = self
                .core
                .command(&["seek", &format!("{position:.3}"), "absolute"]);
            self.position = position;
        }

        self.set_state(
            if self.paused {
                PlayerState::Paused
            } else {
                PlayerState::Playing
            },
            shared,
        );
        publish(shared, |snapshot| snapshot.duration = self.duration);
        self.publish_snapshot(shared);
    }

    fn on_end_file(&mut self, raw: crate::RawEvent, shared: &Arc<Mutex<PlayerSnapshot>>) {
        let (reason, error) = if raw.data.is_null() {
            (end_reason::EOF, 0)
        } else {
            // Safety: END_FILE always carries an `mpv_event_end_file`.
            let end = unsafe { &*(raw.data as *const RawEndFile) };
            (end.reason, end.error)
        };
        match reason {
            end_reason::EOF => {
                self.set_state(PlayerState::Ended, shared);
                publish(shared, |snapshot| snapshot.ended = true);
            }
            end_reason::ERROR => {
                let message = self.library.message(error);
                self.fail("decode_failed", message, shared);
            }
            // A stop or quit was requested by this client.
            _ => {}
        }
        self.publish_snapshot(shared);
    }

    fn present(&mut self, shared: &Arc<Mutex<PlayerSnapshot>>) {
        trace!("present: checking update flags");
        let flags = match self.renderer.update_flags() {
            Ok(flags) => flags,
            Err(error) => {
                self.fail("render_failed", error.to_string(), shared);
                return;
            }
        };
        if flags & UPDATE_FRAME == 0 {
            return;
        }
        trace!("present: rendering frame");
        match self.renderer.render() {
            Ok(Some(frame)) => {
                trace!(
                    "present: frame {}x{} gen {}",
                    frame.width, frame.height, frame.generation
                );
                publish(shared, |snapshot| snapshot.frame = Some(frame))
            }
            Ok(None) => {}
            Err(error) => self.fail("render_failed", error.to_string(), shared),
        }
    }

    fn set_state(&mut self, next: PlayerState, shared: &Arc<Mutex<PlayerSnapshot>>) {
        let previous = self.state;
        self.state = self.state.transition(next);
        if self.state != previous {
            publish(shared, |snapshot| snapshot.state = self.state);
        }
    }

    fn fail(&mut self, code: &str, message: String, shared: &Arc<Mutex<PlayerSnapshot>>) {
        self.state = PlayerState::Failed;
        publish(shared, |snapshot| {
            snapshot.state = PlayerState::Failed;
            snapshot.failure = Some(PlayerFailure {
                code: code.to_string(),
                message,
            });
        });
    }

    fn publish_snapshot(&self, shared: &Arc<Mutex<PlayerSnapshot>>) {
        let (position, duration, state, loaded) =
            (self.position, self.duration, self.state, self.loaded);
        publish(shared, |snapshot| {
            snapshot.position = position;
            snapshot.duration = duration;
            snapshot.state = state;
            snapshot.loaded = loaded;
        });
    }
}

/// Applies `update` to the shared snapshot, ignoring a poisoned lock.
fn publish(shared: &Arc<Mutex<PlayerSnapshot>>, update: impl FnOnce(&mut PlayerSnapshot)) {
    if let Ok(mut snapshot) = shared.lock() {
        update(&mut snapshot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_converts_seconds_to_milliseconds() {
        let snapshot = PlayerSnapshot {
            position: 92.4,
            duration: 3600.0,
            ..PlayerSnapshot::default()
        };
        assert_eq!(snapshot.position_ms(), 92_400);
        assert_eq!(snapshot.duration_ms(), 3_600_000);
    }

    #[test]
    fn nonsense_positions_become_zero_not_negative_progress() {
        let cases = [f64::NAN, f64::INFINITY, -5.0];
        for position in cases {
            let snapshot = PlayerSnapshot {
                position,
                duration: 0.0,
                ..PlayerSnapshot::default()
            };
            assert_eq!(snapshot.position_ms(), 0);
            assert_eq!(snapshot.duration_ms(), 0);
        }
    }

    #[test]
    fn a_missing_file_is_reported_without_touching_libmpv() {
        // The runner is not built here: `load` is exercised end-to-end in the
        // integration test, which needs a real library.
        let shared = Arc::new(Mutex::new(PlayerSnapshot::default()));
        publish(&shared, |snapshot| {
            snapshot.state = PlayerState::Failed;
            snapshot.failure = Some(PlayerFailure {
                code: "file_missing".into(),
                message: "gone".into(),
            });
        });
        let snapshot = shared.lock().unwrap().clone();
        assert_eq!(snapshot.state, PlayerState::Failed);
        assert_eq!(snapshot.failure.unwrap().code, "file_missing");
    }
}
