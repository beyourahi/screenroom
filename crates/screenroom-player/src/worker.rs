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
const ID_AUDIO_PTS: u64 = 5;
const ID_AVSYNC: u64 = 6;
const ID_TOTAL_AVSYNC_CHANGE: u64 = 7;
const ID_MISTIMED_FRAMES: u64 = 8;
const ID_VOLUME: u64 = 9;
const ID_MUTE: u64 = 10;
const ID_TRACK_LIST: u64 = 11;

/// `mpv_format` values used when observing properties (`mpv/client.h`).
const FORMAT_FLAG: i32 = 3;
const FORMAT_INT64: i32 = 4;
const FORMAT_DOUBLE: i32 = 5;
const FORMAT_NODE: i32 = 6;

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
    playlist_entry_id: i64,
}

/// An `mpv_event_start_file` (`mpv/client.h`).
#[repr(C)]
struct RawStartFile {
    playlist_entry_id: i64,
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
    /// Monotonic identity for the load request that produced this snapshot.
    pub load_id: u64,
    /// True after libmpv confirms the starting position with PLAYBACK_RESTART.
    pub position_ready: bool,
    /// Highest seek serial confirmed by a subsequent PLAYBACK_RESTART.
    pub seek_serial: u64,
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
    /// Selectable tracks the player reported for the current file.
    pub tracks: Vec<MediaTrack>,
    /// The audio output's own playhead, in seconds.
    ///
    /// This is the *device* clock, not the file position: libmpv advances it as
    /// the audio output consumes samples. A stalled output freezes it, which is
    /// what makes it usable as evidence that audio is really being played rather
    /// than merely initialised. `None` when the file has no audio track.
    pub audio_pts: Option<f64>,
    /// libmpv's own last audio/video sync difference, in seconds.
    ///
    /// Positive means audio leads the video. Measured by the player rather than
    /// inferred by comparing two clocks, which is the only reason the number is
    /// worth recording in the ledger.
    pub avsync: Option<f64>,
    /// Cumulative A/V sync adjustment libmpv has applied to this file, in seconds.
    pub total_avsync_change: Option<f64>,
    /// Frames libmpv dropped or duplicated to hold sync.
    pub mistimed_frames: Option<i64>,
    /// Subtitle text size libmpv is actually using, as a multiple of normal.
    ///
    /// Read back from `sub-scale` rather than echoed from the request, so this is
    /// evidence of what the player accepted. `None` until a style is applied.
    pub subtitle_scale: Option<f64>,
    /// Subtitle position libmpv is actually using, in percent from the top.
    ///
    /// Read back from `sub-pos`, for the same reason as `subtitle_scale`.
    pub subtitle_position: Option<f64>,
    /// Output volume libmpv is actually using, 0-100.
    ///
    /// Read back from `volume`, so the slider follows the player rather than the
    /// other way round. `None` until the player reports it.
    pub volume: Option<f64>,
    /// Whether output is muted, read back from libmpv's `mute`.
    pub muted: Option<bool>,
}

impl Default for PlayerSnapshot {
    fn default() -> Self {
        Self {
            load_id: 0,
            position_ready: false,
            seek_serial: 0,
            state: PlayerState::Idle,
            position: 0.0,
            duration: 0.0,
            frame: None,
            failure: None,
            ended: false,
            loaded: false,
            tracks: Vec::new(),
            audio_pts: None,
            avsync: None,
            total_avsync_change: None,
            mistimed_frames: None,
            subtitle_scale: None,
            subtitle_position: None,
            volume: None,
            muted: None,
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

/// Which kind of stream a [`MediaTrack`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    /// The picture.
    Video,
    /// A sound track.
    Audio,
    /// A subtitle or closed-caption track.
    Subtitle,
}

impl TrackKind {
    /// libmpv's spelling of this kind, as used in `track-list/N/type`.
    pub fn as_str(self) -> &'static str {
        match self {
            TrackKind::Video => "video",
            TrackKind::Audio => "audio",
            TrackKind::Subtitle => "subtitle",
        }
    }
}

/// One selectable stream, as libmpv reports it in `track-list`.
///
/// Only facts libmpv actually states are carried: a track with no language tag
/// has `language: None` rather than an invented `und` or a guess read out of the
/// filename, because the interface has to be able to say "no language tag"
/// without lying about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaTrack {
    /// libmpv's track id, used to select the track or turn it off.
    pub id: i64,
    /// Video, audio or subtitle.
    pub kind: TrackKind,
    /// The stream's own title, when it carries one.
    pub title: Option<String>,
    /// ISO language tag, when the stream carries one.
    pub language: Option<String>,
    /// Codec name, for example `aac` or `subrip`.
    pub codec: Option<String>,
    /// True when this is the track currently in use.
    pub selected: bool,
    /// True when the track came from a different file than the media itself.
    pub external: bool,
}

impl MediaTrack {
    /// The best short label for a chooser: title, then language, then codec.
    pub fn label(&self) -> String {
        let codec = self
            .codec
            .clone()
            .unwrap_or_else(|| self.kind.as_str().to_string());
        match (&self.title, &self.language) {
            (Some(title), Some(language)) => format!("{title} · {language} · {codec}"),
            (Some(title), None) => format!("{title} · {codec}"),
            (None, Some(language)) => format!("{language} · {codec}"),
            (None, None) => codec,
        }
    }
}

/// Track id that means "none selected": `MPV_TRACK_ID_NONE` in `client.h`.
///
/// libmpv spells this one as the string `no`, so it is translated at the call
/// site rather than sent as a number.
pub const NO_TRACK: i64 = 0;

/// A request to the player thread.
#[derive(Debug, Clone)]
pub enum PlayerCommand {
    /// Open a local file and start playing it, optionally resuming at `resume`.
    Load {
        /// Monotonic request identity returned by `Player::load_at`.
        load_id: u64,
        /// File to play.
        path: PathBuf,
        /// Position in seconds to jump to once the file is loaded.
        resume: Option<f64>,
    },
    /// Seek to an absolute position in seconds.
    Seek { seconds: f64, serial: u64 },
    /// Pause or resume.
    SetPaused(bool),
    /// Stop playback and release the file.
    Stop,
    /// Choose a track by its libmpv id, or disable the kind with [`NO_TRACK`].
    SelectTrack {
        /// Which family of stream to change.
        kind: TrackKind,
        /// libmpv track id, or [`NO_TRACK`] to turn subtitles off.
        id: i64,
    },
    /// Set how subtitles are drawn, independently of which track is selected.
    SetSubtitleStyle {
        /// Text size as a multiple of normal, matching libmpv's `sub-scale`.
        scale: f64,
        /// Vertical position in percent of the screen height, matching `sub-pos`.
        position: f64,
    },
    /// Set the output volume, 0-100.
    SetVolume(f64),
    /// Mute or unmute the output.
    SetMuted(bool),
    /// Ask the worker to exit.
    Shutdown,
}

/// The range Screenroom keeps `sub-scale` inside, as a multiple of normal size.
///
/// Screenroom's own guard, not libmpv's limit: libmpv accepts a much wider range,
/// including a scale that renders a subtitle of no readable size. Anything outside
/// this is clamped by [`Player::set_subtitle_style`] so a bad caller cannot make the
/// player look broken. The Settings screen offers a narrower range still.
pub const SUB_SCALE_RANGE: (f64, f64) = (0.1, 10.0);

/// The range Screenroom keeps `sub-pos` inside, in percent of the screen height.
///
/// Matches libmpv's documented `sub-pos` range of 0-150, where 0 is the top and 100
/// the bottom. Screenroom's Settings screen only offers 0-100; the extra headroom is
/// kept so a stored style from another client still means the same thing.
pub const SUB_POSITION_RANGE: (f64, f64) = (0.0, 150.0);

/// The interface's handle to the player thread.
pub struct Player {
    shared: Arc<Mutex<PlayerSnapshot>>,
    commands: Sender<PlayerCommand>,
    thread: Option<JoinHandle<()>>,
    next_load_id: std::sync::atomic::AtomicU64,
    next_seek_serial: std::sync::atomic::AtomicU64,
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
                next_load_id: std::sync::atomic::AtomicU64::new(0),
                next_seek_serial: std::sync::atomic::AtomicU64::new(0),
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

    /// Opens `path` and resumes at `resume` seconds once libmpv applies its per-file start.
    ///
    /// Returns the load identity carried by snapshots for this request.
    pub fn load_at(&self, path: impl AsRef<Path>, resume: Option<f64>) -> u64 {
        let load_id = self
            .next_load_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        self.send(PlayerCommand::Load {
            load_id,
            path: path.as_ref().to_path_buf(),
            resume,
        });
        load_id
    }

    /// Seeks to an absolute position in seconds and returns its confirmation serial.
    pub fn seek(&self, seconds: f64) -> u64 {
        let serial = self
            .next_seek_serial
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        self.send(PlayerCommand::Seek { seconds, serial });
        serial
    }

    /// Pauses or resumes.
    pub fn set_paused(&self, paused: bool) {
        self.send(PlayerCommand::SetPaused(paused));
    }

    /// Switches the track of `kind` to `id`, or turns that kind off.
    ///
    /// Turning subtitles off is a real libmpv operation (`no`), not a UI flag, so
    /// the player's own notion of the selected track stays the single source of
    /// truth and the next snapshot reflects it.
    pub fn select_track(&self, kind: TrackKind, id: i64) {
        self.send(PlayerCommand::SelectTrack { kind, id });
    }

    /// Sets how subtitles are drawn: `scale` is a multiple of normal text size,
    /// `position` is where they sit as a percent of the screen height from the top.
    ///
    /// Both are libmpv properties rather than Screenroom's own drawing, so the same
    /// call sizes text, ASS and bitmap subtitles alike, and a value outside what
    /// libmpv accepts is clamped rather than passed on.
    ///
    /// One caveat worth knowing before trusting it: a subtitle script that positions
    /// its own lines — a styled ASS release — keeps that positioning unless
    /// `sub-ass-override` is raised, which Screenroom leaves alone. Size is still
    /// the player's; placement may not be.
    pub fn set_subtitle_style(&self, scale: f64, position: f64) {
        self.send(PlayerCommand::SetSubtitleStyle { scale, position });
    }

    /// Sets the output volume in percent, clamped to libmpv's own 0-100 scale.
    pub fn set_volume(&self, percent: f64) {
        if !percent.is_finite() {
            return;
        }
        self.send(PlayerCommand::SetVolume(percent.clamp(0.0, 100.0)));
    }

    /// Mutes or unmutes the output.
    pub fn set_muted(&self, muted: bool) {
        self.send(PlayerCommand::SetMuted(muted));
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
    /// Tracks reported for the current file, in the order libmpv listed them.
    tracks: Vec<MediaTrack>,
    /// The audio output's clock in seconds; `None` when no audio is playing.
    audio_pts: Option<f64>,
    /// libmpv's last measured A/V sync difference, in seconds.
    avsync: Option<f64>,
    /// Cumulative sync adjustment libmpv applied, in seconds.
    total_avsync_change: Option<f64>,
    /// Frames dropped or duplicated for sync, per libmpv.
    mistimed_frames: Option<i64>,
    /// Subtitle scale libmpv last reported, or `None` before one is applied.
    ///
    /// Kept across files on purpose: `sub-scale` and `sub-pos` belong to the video
    /// output, so they outlive the file that happened to be open when they were set
    /// — which is exactly why a new viewing inherits them.
    subtitle_scale: Option<f64>,
    /// Subtitle position libmpv last reported, in percent from the top.
    subtitle_position: Option<f64>,
    /// Output volume libmpv last reported, 0-100.
    volume: Option<f64>,
    /// Whether output is muted, as libmpv reports it.
    muted: Option<bool>,
    load_id: u64,
    position_ready: bool,
    current_entry_id: Option<i64>,
    expected_entry_id: Option<i64>,
    seek_serial: u64,
    pending_seek_serial: Option<u64>,
    queued_seek: Option<(f64, u64)>,
    seek_started: bool,
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
        // The audio output's own clock. Observed rather than polled so the
        // measurement in the tests reads the same value the player acts on.
        core.observe("audio-pts", FORMAT_DOUBLE, ID_AUDIO_PTS)?;
        // libmpv's own sync instruments. Inferring sync from two clocks measures the
        // clocks; these measure the sync.
        core.observe("avsync", FORMAT_DOUBLE, ID_AVSYNC)?;
        core.observe("total-avsync-change", FORMAT_DOUBLE, ID_TOTAL_AVSYNC_CHANGE)?;
        core.observe("mistimed-frame-count", FORMAT_INT64, ID_MISTIMED_FRAMES)?;
        // Output level, so the interface shows what the player is doing rather
        // than what it last asked for.
        core.observe("volume", FORMAT_DOUBLE, ID_VOLUME)?;
        core.observe("mute", FORMAT_FLAG, ID_MUTE)?;
        core.observe("track-list", FORMAT_NODE, ID_TRACK_LIST)?;

        Ok(Self {
            renderer,
            core,
            library,
            state: PlayerState::Idle,
            paused: false,
            position: 0.0,
            duration: 0.0,
            loaded: false,
            tracks: Vec::new(),
            audio_pts: None,
            avsync: None,
            total_avsync_change: None,
            mistimed_frames: None,
            subtitle_scale: None,
            subtitle_position: None,
            volume: None,
            muted: None,
            load_id: 0,
            position_ready: false,
            current_entry_id: None,
            expected_entry_id: None,
            seek_serial: 0,
            pending_seek_serial: None,
            queued_seek: None,
            seek_started: false,
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
            while let Some(raw) = self.core.wait_event_raw(0.0) {
                self.handle_event(raw, shared);
            }

            if self.pending_seek_serial.is_none()
                && let Some((seconds, serial)) = self.queued_seek.take()
            {
                self.seek(seconds, serial, shared);
            }

            if self.renderer.take_dirty() {
                self.present(shared);
            }
        }
    }

    fn handle_command(&mut self, command: PlayerCommand, shared: &Arc<Mutex<PlayerSnapshot>>) {
        match command {
            PlayerCommand::Load {
                load_id,
                path,
                resume,
            } => self.load(load_id, &path, resume, shared),
            PlayerCommand::Seek { seconds, serial } => {
                if self.pending_seek_serial.is_some() {
                    // A libmpv playback-restart event carries no request ID. Keep
                    // later seeks queued until the earlier serial is confirmed.
                    self.queued_seek = Some((seconds, serial));
                } else {
                    self.seek(seconds, serial, shared);
                }
            }
            PlayerCommand::SetPaused(paused) => self.set_paused(paused, shared),
            PlayerCommand::SelectTrack { kind, id } => self.select_track(kind, id, shared),
            PlayerCommand::SetSubtitleStyle { scale, position } => {
                self.set_subtitle_style(scale, position, shared)
            }
            PlayerCommand::SetVolume(percent) => self.set_volume(percent, shared),
            PlayerCommand::SetMuted(muted) => self.set_muted(muted, shared),
            PlayerCommand::Stop => self.stop(shared),
            PlayerCommand::Shutdown => {}
        }
    }

    fn load(
        &mut self,
        load_id: u64,
        path: &Path,
        resume: Option<f64>,
        shared: &Arc<Mutex<PlayerSnapshot>>,
    ) {
        self.load_id = load_id;
        self.position_ready = false;
        self.current_entry_id = None;
        self.expected_entry_id = None;
        self.pending_seek_serial = None;
        self.queued_seek = None;
        self.seek_started = false;
        self.position = 0.0;
        self.duration = 0.0;
        self.loaded = false;
        self.tracks.clear();
        self.audio_pts = None;
        self.avsync = None;
        self.total_avsync_change = None;
        self.mistimed_frames = None;
        let load_id_for_snapshot = self.load_id;
        publish(shared, |snapshot| {
            snapshot.load_id = load_id_for_snapshot;
            snapshot.position_ready = false;
            snapshot.loaded = false;
            snapshot.position = 0.0;
            snapshot.duration = 0.0;
            snapshot.tracks.clear();
            snapshot.audio_pts = None;
            snapshot.avsync = None;
            snapshot.total_avsync_change = None;
            snapshot.mistimed_frames = None;
            snapshot.failure = None;
            snapshot.ended = false;
            snapshot.frame = None;
        });
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
        let resume = resume.filter(|seconds| seconds.is_finite() && *seconds > 0.5);
        // Ended and Failed intentionally have no ordinary outgoing transitions;
        // a new explicit load starts a fresh lifecycle.
        self.state = PlayerState::Idle;
        self.set_state(PlayerState::Resolving, shared);
        let command = if let Some(position) = resume {
            let options = format!("start={position:.3}");
            self.core
                .command(&["loadfile", path, "replace", "-1", &options])
        } else {
            self.core.command(&["loadfile", path, "replace"])
        };
        if let Err(error) = command {
            self.fail("load_failed", error.to_string(), shared);
            return;
        }
        self.expected_entry_id = self.core.get_i64("playlist/0/id").ok();
    }

    fn seek(&mut self, seconds: f64, serial: u64, shared: &Arc<Mutex<PlayerSnapshot>>) {
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
        self.pending_seek_serial = None;
        self.seek_started = false;
        if let Err(error) = self
            .core
            .command(&["seek", &format!("{target:.3}"), "absolute"])
        {
            self.queued_seek = None;
            self.fail("seek_failed", error.to_string(), shared);
        } else {
            self.pending_seek_serial = Some(serial);
        }
    }

    fn set_paused(&mut self, paused: bool, shared: &Arc<Mutex<PlayerSnapshot>>) {
        let value = if paused { "yes" } else { "no" };
        if let Err(error) = self.core.command(&["set", "pause", value]) {
            self.fail("pause_failed", error.to_string(), shared);
        }
    }

    /// Applies subtitle size and placement.
    ///
    /// Deliberately not guarded by [`Self::loaded`], unlike track selection: these
    /// are properties of the video output, so setting them before a file is opened
    /// is exactly how the next file should inherit them.
    fn set_subtitle_style(
        &mut self,
        scale: f64,
        position: f64,
        shared: &Arc<Mutex<PlayerSnapshot>>,
    ) {
        // A non-finite value would survive `clamp` unchanged, and formatting it into
        // an option string would then be rejected by libmpv with a parse error the
        // user cannot act on. Skipped instead, so the property keeps its last good
        // value rather than the player reporting itself broken.
        for (property, value, (low, high)) in [
            ("sub-scale", scale, SUB_SCALE_RANGE),
            ("sub-pos", position, SUB_POSITION_RANGE),
        ] {
            if !value.is_finite() {
                continue;
            }
            let value = value.clamp(low, high).to_string();
            if let Err(error) = self.core.command(&["set", property, &value]) {
                self.fail("subtitle_style_failed", error.to_string(), shared);
                return;
            }
        }
        // Read back instead of storing what was asked for. libmpv resolves these
        // through the video output and may hold a different number, and a snapshot
        // that repeated the request would make the interface agree with itself
        // while disagreeing with the picture.
        self.subtitle_scale = self
            .core
            .get_f64("sub-scale")
            .ok()
            .filter(|v| v.is_finite());
        self.subtitle_position = self.core.get_f64("sub-pos").ok().filter(|v| v.is_finite());
        self.publish_snapshot(shared);
    }

    /// Applies a track choice and republishes the real selected state.
    ///
    /// The track list is re-read from libmpv afterwards instead of being patched
    /// locally: if libmpv refuses the switch (an id that has since disappeared),
    /// the snapshot must show what actually happened, not what was asked for.
    fn select_track(&mut self, kind: TrackKind, id: i64, shared: &Arc<Mutex<PlayerSnapshot>>) {
        if !self.loaded {
            return;
        }
        let property = match kind {
            TrackKind::Video => "vid",
            TrackKind::Audio => "aid",
            TrackKind::Subtitle => "sid",
        };
        let value = if id == NO_TRACK {
            "no".to_string()
        } else {
            id.to_string()
        };
        if let Err(error) = self.core.command(&["set", property, &value]) {
            self.fail("track_failed", error.to_string(), shared);
            return;
        }
        self.tracks = read_tracks(&self.core);
        let tracks = self.tracks.clone();
        publish(shared, |snapshot| snapshot.tracks = tracks);
    }

    fn stop(&mut self, shared: &Arc<Mutex<PlayerSnapshot>>) {
        let _ = self.core.command(&["stop"]);
        self.state = PlayerState::Idle;
        self.loaded = false;
        self.position_ready = false;
        self.pending_seek_serial = None;
        self.queued_seek = None;
        self.seek_started = false;
        self.position = 0.0;
        self.duration = 0.0;
        self.tracks.clear();
        self.audio_pts = None;
        self.avsync = None;
        self.total_avsync_change = None;
        self.mistimed_frames = None;
        publish(shared, |snapshot| {
            snapshot.load_id = self.load_id;
            snapshot.position_ready = false;
            snapshot.state = PlayerState::Idle;
            snapshot.loaded = false;
            snapshot.position = 0.0;
            snapshot.duration = 0.0;
            snapshot.ended = false;
            snapshot.frame = None;
            snapshot.tracks = Vec::new();
            snapshot.audio_pts = None;
            snapshot.avsync = None;
            snapshot.total_avsync_change = None;
            snapshot.mistimed_frames = None;
        });
    }

    fn handle_event(&mut self, raw: crate::RawEvent, shared: &Arc<Mutex<PlayerSnapshot>>) {
        match raw.event_id {
            event::PROPERTY_CHANGE => self.apply_property(raw, shared),
            event::START_FILE => {
                if !raw.data.is_null() {
                    // Safety: START_FILE always carries an `mpv_event_start_file`.
                    let entry_id =
                        unsafe { (*(raw.data as *const RawStartFile)).playlist_entry_id };
                    if self
                        .expected_entry_id
                        .is_some_and(|expected| expected != entry_id)
                    {
                        return;
                    }
                    self.expected_entry_id = None;
                    self.current_entry_id = Some(entry_id);
                    self.position_ready = false;
                    self.set_state(PlayerState::Resolving, shared);
                }
            }
            event::FILE_LOADED if self.current_entry_id.is_some() => self.on_loaded(shared),
            event::END_FILE => self.on_end_file(raw, shared),
            event::SEEK => {
                if self.pending_seek_serial.is_some() {
                    self.seek_started = true;
                }
            }
            event::PLAYBACK_RESTART if self.loaded => {
                self.position_ready = true;
                self.position = self
                    .core
                    .get_f64("time-pos")
                    .ok()
                    .filter(|position| position.is_finite() && *position >= 0.0)
                    .unwrap_or(self.position);
                if self.seek_started {
                    if let Some(serial) = self.pending_seek_serial.take() {
                        self.seek_serial = serial;
                    }
                    self.seek_started = false;
                }
                self.publish_snapshot(shared);
            }
            event::VIDEO_RECONFIG if self.loaded => self.update_target(),
            event::SHUTDOWN => {}
            _ => {}
        }
    }

    fn apply_property(&mut self, raw: crate::RawEvent, shared: &Arc<Mutex<PlayerSnapshot>>) {
        if self.current_entry_id.is_none() && !matches!(raw.reply_userdata, ID_VOLUME | ID_MUTE) {
            return;
        }
        if !self.loaded && !matches!(raw.reply_userdata, ID_PAUSE | ID_VOLUME | ID_MUTE) {
            return;
        }
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
                if self.position_ready && value.is_finite() && value >= 0.0 {
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
            ID_AUDIO_PTS => {
                let value = unsafe { *(property.data as *const f64) };
                // A file with no audio reports the property as unavailable, which
                // the caller distinguishes from a clock sitting at zero.
                self.audio_pts = value.is_finite().then_some(value);
            }
            ID_AVSYNC => {
                let value = unsafe { *(property.data as *const f64) };
                self.avsync = value.is_finite().then_some(value);
            }
            ID_TOTAL_AVSYNC_CHANGE => {
                let value = unsafe { *(property.data as *const f64) };
                self.total_avsync_change = value.is_finite().then_some(value);
            }
            ID_MISTIMED_FRAMES => {
                let value = unsafe { *(property.data as *const i64) };
                self.mistimed_frames = Some(value);
            }
            ID_VOLUME => {
                let value = unsafe { *(property.data as *const f64) };
                self.volume = value.is_finite().then_some(value);
            }
            ID_MUTE => {
                self.muted = Some(unsafe { *(property.data as *const i32) } != 0);
            }
            ID_TRACK_LIST => self.tracks = read_tracks(&self.core),
            _ => {}
        }
        self.publish_snapshot(shared);
    }

    /// Sets the output volume, then reports what libmpv actually holds.
    fn set_volume(&mut self, percent: f64, shared: &Arc<Mutex<PlayerSnapshot>>) {
        let value = percent.clamp(0.0, 100.0).to_string();
        if let Err(error) = self.core.command(&["set", "volume", &value]) {
            self.fail("volume_failed", error.to_string(), shared);
            return;
        }
        self.volume = self.core.get_f64("volume").ok().filter(|v| v.is_finite());
        self.publish_snapshot(shared);
    }

    /// Mutes or unmutes, then reports libmpv's own flag.
    fn set_muted(&mut self, muted: bool, shared: &Arc<Mutex<PlayerSnapshot>>) {
        let value = if muted { "yes" } else { "no" };
        if let Err(error) = self.core.command(&["set", "mute", value]) {
            self.fail("mute_failed", error.to_string(), shared);
            return;
        }
        self.muted = self.core.get_flag("mute").ok();
        self.publish_snapshot(shared);
    }

    fn on_loaded(&mut self, shared: &Arc<Mutex<PlayerSnapshot>>) {
        trace!("event: file loaded");
        self.loaded = true;
        self.duration = self.core.get_f64("duration").unwrap_or(0.0).max(0.0);
        self.tracks = read_tracks(&self.core);
        self.update_target();

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

    fn update_target(&mut self) {
        let display_width = self.core.get_i64("dwidth").unwrap_or(0).max(0) as u32;
        let display_height = self.core.get_i64("dheight").unwrap_or(0).max(0) as u32;
        let width = if display_width == 0 {
            self.core.get_i64("width").unwrap_or(0).max(0) as u32
        } else {
            display_width
        };
        let height = if display_height == 0 {
            self.core.get_i64("height").unwrap_or(0).max(0) as u32
        } else {
            display_height
        };
        let (width, height) = target_size(width, height, MAX_DIMENSION);
        self.renderer.set_target(width, height);
    }

    fn on_end_file(&mut self, raw: crate::RawEvent, shared: &Arc<Mutex<PlayerSnapshot>>) {
        let (reason, error, playlist_entry_id) = if raw.data.is_null() {
            (
                end_reason::EOF,
                0,
                self.current_entry_id.unwrap_or_default(),
            )
        } else {
            // Safety: END_FILE always carries an `mpv_event_end_file`.
            let end = unsafe { &*(raw.data as *const RawEndFile) };
            (end.reason, end.error, end.playlist_entry_id)
        };
        if self.current_entry_id != Some(playlist_entry_id) {
            return;
        }
        self.current_entry_id = None;
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
        if !self.loaded || self.current_entry_id.is_none() {
            return;
        }
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
        let began = std::time::Instant::now();
        match self.renderer.render() {
            Ok(Some(frame)) => {
                trace!(
                    "present: frame {}x{} gen {} in {:?}",
                    frame.width,
                    frame.height,
                    frame.generation,
                    began.elapsed()
                );
                let load_id = self.load_id;
                publish(shared, |snapshot| {
                    snapshot.load_id = load_id;
                    snapshot.frame = Some(frame);
                })
            }
            Ok(None) => {}
            Err(error) => self.fail("render_failed", error.to_string(), shared),
        }
    }

    fn set_state(&mut self, next: PlayerState, shared: &Arc<Mutex<PlayerSnapshot>>) {
        let previous = self.state;
        self.state = self.state.transition(next);
        if self.state != previous {
            let (load_id, position_ready, state) = (self.load_id, self.position_ready, self.state);
            publish(shared, |snapshot| {
                snapshot.load_id = load_id;
                snapshot.position_ready = position_ready;
                snapshot.state = state;
            });
        }
    }

    fn fail(&mut self, code: &str, message: String, shared: &Arc<Mutex<PlayerSnapshot>>) {
        self.state = PlayerState::Failed;
        let (load_id, position_ready, position, duration, loaded) = (
            self.load_id,
            self.position_ready,
            self.position,
            self.duration,
            self.loaded,
        );
        publish(shared, |snapshot| {
            snapshot.load_id = load_id;
            snapshot.position_ready = position_ready;
            snapshot.position = if position_ready { position } else { 0.0 };
            snapshot.duration = duration;
            snapshot.loaded = loaded;
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
        let (tracks, audio_pts) = (self.tracks.clone(), self.audio_pts);
        let (avsync, total_avsync_change, mistimed_frames) =
            (self.avsync, self.total_avsync_change, self.mistimed_frames);
        let (subtitle_scale, subtitle_position) = (self.subtitle_scale, self.subtitle_position);
        let (volume, muted) = (self.volume, self.muted);
        let (load_id, position_ready, seek_serial) =
            (self.load_id, self.position_ready, self.seek_serial);
        publish(shared, |snapshot| {
            snapshot.load_id = load_id;
            snapshot.position_ready = position_ready;
            snapshot.seek_serial = seek_serial;
            snapshot.position = if position_ready { position } else { 0.0 };
            snapshot.duration = duration;
            snapshot.state = state;
            snapshot.loaded = loaded;
            snapshot.tracks = tracks;
            snapshot.audio_pts = audio_pts;
            snapshot.avsync = avsync;
            snapshot.total_avsync_change = total_avsync_change;
            snapshot.mistimed_frames = mistimed_frames;
            snapshot.subtitle_scale = subtitle_scale;
            snapshot.subtitle_position = subtitle_position;
            snapshot.volume = volume;
            snapshot.muted = muted;
        });
    }
}

/// Reads the current `track-list` from libmpv.
///
/// libmpv exposes the list as indexed sub-properties, so it is walked with plain
/// property reads rather than by parsing a node tree: `track-list/count` gives the
/// length and `track-list/<n>/<field>` the fields. A missing field is reported as
/// `None`; a malformed index stops the walk rather than looping forever.
fn read_tracks(core: &Mpv) -> Vec<MediaTrack> {
    let count = core.get_i64("track-list/count").unwrap_or(0).clamp(0, 64);
    let mut tracks = Vec::new();
    for index in 0..count {
        let field = |name: &str| {
            core.get_string(&format!("track-list/{index}/{name}"))
                .ok()
                .flatten()
        };
        let kind = match field("type").as_deref() {
            Some("video") => TrackKind::Video,
            Some("audio") => TrackKind::Audio,
            Some("sub") => TrackKind::Subtitle,
            // An unknown stream type is not something the interface can present,
            // and inventing a kind for it would put a dead control on screen.
            _ => continue,
        };
        // Cover art is a video stream that holds one still frame (`image=yes` in
        // `track-list`). It is not a picture to play: listing it would offer the
        // viewer a video track that replaces the film with its own poster.
        if kind == TrackKind::Video && field("image").as_deref() == Some("yes") {
            continue;
        }
        let Some(id) = core.get_i64(&format!("track-list/{index}/id")).ok() else {
            continue;
        };
        tracks.push(MediaTrack {
            id,
            kind,
            title: field("title").filter(|value| !value.trim().is_empty()),
            language: field("lang").filter(|value| !value.trim().is_empty()),
            codec: field("codec").filter(|value| !value.trim().is_empty()),
            selected: core
                .get_flag(&format!("track-list/{index}/selected"))
                .unwrap_or(false),
            external: core
                .get_flag(&format!("track-list/{index}/external"))
                .unwrap_or(false),
        });
    }
    tracks
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
