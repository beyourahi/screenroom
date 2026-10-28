//! The desktop application: Home, Browse, Detail and the player.
//!
//! What this layer is responsible for, and what it is not:
//!
//! * the rules — paging bounds, literal search, progress ordering, session
//!   generations — live in `screenroom-core`; this module decides *when* to ask,
//! * it shows the frame the player published, and nothing it did not publish,
//! * it reports playback failures with the player's own message instead of
//!   leaving an empty black area,
//! * it never invents media: an unavailable file is described as unavailable and
//!   offered a recovery path.
//!
//! Visual roles come from [`crate::design`], which maps the private Dropout
//! Design System's semantic tokens onto iced. No colour literal lives here.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use iced::widget::shader;
use iced::widget::{
    Space, button, checkbox, column, container, image, pick_list, row, scrollable, slider, text,
    text_input,
};
use iced::{Alignment, ContentFit, Element, Length, Subscription, Task, keyboard};
use screenroom_core::{
    CoreError, Library, LibraryItem, LibraryQuery, LibraryScanner, MediaKind, Preferences, Profile,
    Recommendation, ScanToken, SegmentMarker, Sort, Source, SourceState,
};
use screenroom_player::{
    MediaTrack, NO_TRACK, Player, PlayerCapability, PlayerSnapshot, TrackKind,
};

use crate::artwork::{ArtworkEvent, Extractor};
use crate::design::{self, Level};
use crate::playback::PlaybackTracker;
use crate::video_surface::{Frame, VideoSurface};

/// How often the player is polled for frames and position.
const PLAYER_TICK: Duration = Duration::from_millis(16);

/// How often status (scan progress) is polled when no player is open.
const STATUS_TICK: Duration = Duration::from_millis(250);

/// The window width assumed until a real resize event arrives.
///
/// Matches the initial `window_size` below, so the first frame lays out the same way
/// the second one will.
const INITIAL_WINDOW_WIDTH: f32 = 1280.0;

/// Rows a page is built from, so a page stays bounded and the query cheap.
///
/// This is the *paging* unit and deliberately does not follow the window: a page is
/// a fixed 24 items however wide the window is, which is what keeps the measured
/// query budget (see `IMPLEMENTATION_STATUS.md`) valid. Only the visual column count
/// adapts, and a page that does not fill its last row is padded rather than
/// re-queried.
const PAGE_ROWS: usize = 6;

/// Items per page.
const PAGE_SIZE: i64 = (DEFAULT_COLUMNS * PAGE_ROWS) as i64;

/// Cards per row in an ordinary window.
///
/// The column count is derived from the width at runtime (see [`grid_columns`]);
/// this is the reference point that keeps a 1280-wide window looking as it did when
/// the count was a constant.
const DEFAULT_COLUMNS: usize = 4;

/// Continue Watching rail length.
const RAIL: i64 = 8;

/// How long a forward seek jumps.
const SEEK_STEP: f64 = 10.0;

/// Poster height on a card, in logical pixels.
///
/// Fixed rather than proportional so a title's text starts at the same height on
/// every card in a row, and a poster with an unusual aspect is cropped rather
/// than pushing its neighbours out of line.
const CARD_POSTER_HEIGHT: f32 = 150.0;

/// Poster size on the detail screen, in logical pixels.
const DETAIL_POSTER: (f32, f32) = (210.0, 315.0);

/// How many extra works are queued for artwork while a screen is idle.
///
/// A page's own works are always queued first; this fills the library in during
/// the quiet moments instead of in one long burst after a scan.
const ARTWORK_IDLE_BATCH: i64 = 4;

/// Suggestions shown on Home.
const SUGGESTIONS: i64 = 8;

/// Update cycles in which a launch may try to point the menu bar's Quit at the
/// window's close action.
///
/// AppKit builds the default menu bar after the application object exists, so the
/// attempt made while starting can legitimately find no menu. A budget rather than
/// one try: the attempt is cheap, it stops the moment it succeeds, and it gives up
/// after a few seconds instead of every update forever.
const QUIT_ROUTING_BUDGET: u8 = 40;

/// Whether this launch should put a window on screen.
///
/// A hidden window still gives the process everything a check needs — the event
/// loop, AppKit's menu bar, the database — without taking anything from whoever is
/// using the machine. Only two things ask for one: a check launch, and
/// `SCREENROOM_WINDOW=hidden`.
fn hides_its_window() -> bool {
    crate::quit::check_requested()
        || std::env::var_os("SCREENROOM_WINDOW").is_some_and(|value| value == "hidden")
}

/// Starts the desktop application.
pub fn run() -> iced::Result {
    // Set through `window` rather than a dedicated builder method, and before
    // `window_size`, which only replaces the size and keeps everything else.
    let window = iced::window::Settings {
        visible: !hides_its_window(),
        ..Default::default()
    };

    iced::application(App::new, App::update, App::view)
        .title("Screenroom")
        .theme(App::theme)
        .subscription(App::subscription)
        .window(window)
        .window_size((1280.0, 800.0))
        // Closing the window is not the same as the process ending: the app has to
        // be given the chance to store the position it is at and close the viewing
        // session before it goes away, which `exit_on_close_request` would skip.
        .exit_on_close_request(false)
        .run()
}

/// Where the library lives when `SCREENROOM_LIBRARY` is not set.
///
/// The app must persist progress without being told where to do it: a throwaway
/// in-memory library would silently discard every checkpoint on quit, which is
/// exactly the behaviour this milestone is meant to prove.
pub fn default_library_path() -> Option<PathBuf> {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .or_else(|| std::env::var_os("LOCALAPPDATA"))
            .map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
            })
    }?;
    Some(base.join("Screenroom").join("library.db"))
}

/// Where a backup is written when the user has not chosen somewhere else.
///
/// The name is dated so a second backup cannot silently replace the first, and it
/// sits in the same folder as the library: the library's directory is known to be
/// writable, which a guessed Documents folder is not.
pub fn default_backup_path() -> String {
    let Some(library) = default_library_path() else {
        return String::new();
    };
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default();
    library
        .with_file_name(format!("screenroom-backup-{stamp}.db"))
        .to_string_lossy()
        .to_string()
}

/// Which surface is on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// Continue Watching and the library entry points.
    Home,
    /// The paged, filterable library.
    Browse,
    /// One work or episode, with its actions.
    Detail,
    /// Video playback.
    Player,
    /// Viewer, playback, folder and data settings.
    Settings,
}

/// The library kind filter offered in Browse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindFilter {
    /// Every kind.
    All,
    /// Films only.
    Movies,
    /// Series only.
    Series,
}

impl std::fmt::Display for KindFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

impl KindFilter {
    /// The label shown in the picker.
    fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Movies => "Movies",
            Self::Series => "Series",
        }
    }

    /// The stored kind this filter selects, if any.
    fn kind(self) -> Option<String> {
        match self {
            Self::All => None,
            Self::Movies => Some(MediaKind::Movie.as_str().to_string()),
            Self::Series => Some(MediaKind::Series.as_str().to_string()),
        }
    }
}

/// A sort choice with its menu label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortChoice {
    /// Title A→Z.
    Title,
    /// Newest additions first.
    Added,
    /// Newest release year first.
    Year,
    /// Most recently watched first.
    Progress,
}

impl std::fmt::Display for SortChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

impl SortChoice {
    /// Every choice, in menu order.
    const ALL: [Self; 4] = [Self::Title, Self::Added, Self::Year, Self::Progress];

    /// The label shown in the picker.
    fn label(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::Added => "Recently added",
            Self::Year => "Year",
            Self::Progress => "Recently watched",
        }
    }

    /// The core ordering this choice maps to.
    fn sort(self) -> Sort {
        match self {
            Self::Title => Sort::Title,
            Self::Added => Sort::Added,
            Self::Year => Sort::Year,
            Self::Progress => Sort::Progress,
        }
    }
}

/// Browse screen state, including what is currently loaded.
struct Browse {
    text: String,
    kind: KindFilter,
    genre: Option<String>,
    sort: SortChoice,
    watchlist_only: bool,
    favorites_only: bool,
    available_only: bool,
    page: usize,
    items: Vec<LibraryItem>,
    total: i64,
    genres: Vec<String>,
}

impl Browse {
    fn new() -> Self {
        Self {
            text: String::new(),
            kind: KindFilter::All,
            genre: None,
            sort: SortChoice::Title,
            watchlist_only: false,
            favorites_only: false,
            available_only: false,
            page: 0,
            items: Vec::new(),
            total: 0,
            genres: Vec::new(),
        }
    }

    /// The page count implied by the current total.
    fn page_count(&self) -> usize {
        if self.total <= 0 {
            1
        } else {
            ((self.total as usize).div_ceil(PAGE_SIZE as usize)).max(1)
        }
    }

    /// True when anything is narrowing the result set.
    ///
    /// Sort order is deliberately excluded: it reorders results but can never
    /// remove one, so offering "change the filters" for a sort would be a dead end.
    fn narrowed(&self) -> bool {
        !self.text.trim().is_empty()
            || self.kind != KindFilter::All
            || self.genre.is_some()
            || self.watchlist_only
            || self.favorites_only
            || self.available_only
    }
}

/// Detail screen state for one file.
struct Detail {
    /// The file being shown.
    item: LibraryItem,
    /// Sibling episodes of the same work, when it is a series.
    episodes: Vec<LibraryItem>,
    /// Distinct season numbers of the work.
    seasons: Vec<i64>,
}

/// The viewing session currently open, and the policy around it.
struct Active {
    tracker: PlaybackTracker,
    file_id: String,
    title: String,
    resume_from_ms: i64,
    /// Where Back should return to.
    origin: Screen,
}

/// Progress reported by a background scan.
#[derive(Debug)]
enum ScanEvent {
    /// Cumulative counters, plus whether the walk is finished.
    Progress {
        visited: u64,
        imported: u64,
        issues: usize,
        done: bool,
        complete: bool,
    },
    /// The scan failed to start or aborted.
    Failed(String),
}

/// Application state.
pub struct App {
    library: Arc<Library>,
    /// Human-readable location of the open library, shown so the user can see
    /// which database their progress is being written to.
    library_label: String,
    profile_id: String,
    capability: PlayerCapability,

    screen: Screen,
    /// Keyboard cursor inside the current screen's item list.
    focus: usize,

    home: Vec<LibraryItem>,
    /// Local suggestions with the reason each one was offered.
    suggestions: Vec<Recommendation>,
    browse: Browse,
    detail: Option<Detail>,

    /// Retained posters, keyed by work. The bytes live in the library; this is
    /// the decoded handle the image widget draws, and nothing is fetched.
    artwork: HashMap<String, image::Handle>,
    /// Works already handed to the extractor, so one work is not queued twice.
    artwork_queued: HashSet<String>,
    /// Works whose frame could not be read, so a reload does not retry forever.
    artwork_failed: HashSet<String>,
    /// How many works came back without a drawable frame, shown honestly.
    artwork_failures: usize,
    extractor: Option<Extractor>,
    /// Whether this instance may run background work at all.
    ///
    /// A launch may: extraction is what makes the grid drawable. A test may not:
    /// it would start ffmpeg processes and write into a fixture library, which is
    /// a side effect no unit test should have.
    background_work: bool,

    player: Option<Player>,
    snapshot: PlayerSnapshot,
    frame: Option<Frame>,
    active: Option<Active>,
    scrub: Option<f64>,

    /// Intro and credits markers stored for the file now open.
    markers: Vec<SegmentMarker>,
    /// A segment whose start was pinned and is waiting for its end.
    mark_pending: Option<(String, i64)>,
    /// The intro marker already auto-skipped, so a seek engine that reports the
    /// pre-seek position one more time cannot skip twice.
    auto_skipped: Option<String>,

    /// Window width in logical pixels, updated from resize events.
    ///
    /// Held because the grid's column count is derived from it. Kept as a plain f32
    /// rather than a `Size` so a test can set a width without a window existing.
    window_width: f32,

    /// Stored preferences, held so a per-frame tick never reads the database.
    preferences: Preferences,

    /// Viewer profiles, reloaded whenever they can have changed.
    profiles: Vec<Profile>,
    /// Authorized folders, reloaded whenever they can have changed.
    sources: Vec<Source>,
    /// New-profile text field.
    new_profile: String,
    /// Source chosen for relinking, and the new root typed for it.
    relink_target: Option<String>,
    relink_path: String,
    /// Destination for the next backup, pre-filled with a dated default.
    backup_path: String,
    /// Backup chosen for restoring, and whether the confirm step is armed.
    restore_path: String,
    armed_restore: Option<String>,
    /// Language text fields, applied when submitted rather than per keystroke.
    subtitle_input: String,
    audio_input: String,
    /// Folder whose removal is waiting for confirmation.
    armed_remove: Option<String>,

    /// Text field for granting a new media root.
    new_root: String,
    scan_token: Option<ScanToken>,
    scan_events: Option<Receiver<ScanEvent>>,
    status: Option<String>,
    error: Option<String>,

    /// Update cycles left in which to point the menu bar's Quit at the window's
    /// close action, and what the attempt settled on.
    ///
    /// Zero means this `App` will never do it. A launch starts with the full
    /// budget; an `App` a test built starts at zero, because a test process has
    /// no menu bar to route and AppKit must not be reached off the main thread.
    quit_routing_budget: u8,
    quit_routing: crate::quit::QuitRouting,
    /// File a check launch was asked to report the routing outcome to.
    quit_check: Option<PathBuf>,
}

/// User intent.
#[derive(Debug, Clone)]
pub enum Message {
    /// Navigate to Home.
    GoHome,
    /// Navigate to the library.
    GoBrowse,
    /// Go up one level.
    Back,
    /// Reload every projection from the library.
    Refresh,

    /// The search field changed.
    SearchChanged(String),
    /// The kind filter changed.
    KindChanged(KindFilter),
    /// The genre filter changed.
    GenreChanged(Option<String>),
    /// The sort order changed.
    SortChanged(SortChoice),
    /// Toggle the watchlist-only filter.
    ToggleWatchlistOnly(bool),
    /// Toggle the favourites-only filter.
    ToggleFavoritesOnly(bool),
    /// Toggle the available-only filter.
    ToggleAvailableOnly(bool),
    /// Go to the next page.
    NextPage,
    /// Go to the previous page.
    PreviousPage,

    /// Move the keyboard cursor.
    Move(Focus),
    /// Activate the focused row or card.
    Activate,
    /// Open the detail screen for a file.
    OpenDetail(String),
    /// Play a file from the stored position.
    Play(String),
    /// Play a file from the beginning, ignoring stored progress.
    Restart(String),
    /// Toggle the favourite flag of the shown work.
    ToggleFavorite(String),
    /// Toggle the watchlist flag of the shown work.
    ToggleWatchlist(String),
    /// Mark the shown edition watched or unwatched.
    ToggleWatched(String),
    /// Remove the shown edition from Continue Watching.
    HideFromContinue(String),

    /// The new-root text field changed.
    NewRootChanged(String),
    /// Grant and scan the folder in the text field.
    AddFolder,
    /// Cancel a running scan.
    CancelScan,
    /// Rescan one folder the library already holds.
    RescanSource(String),
    /// Ask to remove a folder, or confirm that request when repeated.
    RemoveSource(String),
    /// Withdraw a removal or restore confirmation.
    CancelArmed,
    /// Choose the folder to relink.
    RelinkTargetChosen(String),
    /// The relink path text field changed.
    RelinkPathChanged(String),
    /// Point the chosen folder at the typed path and rescan it.
    RelinkSource,

    /// Navigate to Settings.
    GoSettings,
    /// The active profile changed.
    ProfileChosen(String),
    /// The new-profile text field changed.
    NewProfileChanged(String),
    /// Add the profile in the text field and switch to it.
    AddProfile,
    /// Remove a profile.
    RemoveProfile(String),
    /// Turn automatic next-episode playback on or off.
    ToggleAutoplay(bool),
    /// Turn automatic intro skipping on or off.
    ToggleAutoSkip(bool),
    /// Jump to the end of the intro or credits segment the playhead is inside.
    SkipSegment,
    /// Pin the start of a segment, or finish the one already pinned.
    MarkSegment(String),
    /// Store the pinned marker with its end at the playhead.
    FinishSegment,
    /// Apply the typed subtitle language, or clear it when empty.
    ApplySubtitleLanguage,
    /// Apply the typed audio language, or clear it when empty.
    ApplyAudioLanguage,
    /// The subtitle language field changed.
    SubtitleLanguageChanged(String),
    /// The audio language field changed.
    AudioLanguageChanged(String),
    /// The backup destination changed.
    BackupPathChanged(String),
    /// Write a backup now.
    BackupNow,
    /// The restore source changed.
    RestorePathChanged(String),
    /// Validate the chosen backup, then restore from it on the second press.
    RestoreNow,
    /// Check a backup file and report what it holds without changing anything.
    InspectRestore,
    /// Dismiss the last status line.
    ClearStatus,

    /// Leave the player and return to the library.
    BackFromPlayer,
    /// Play, pause, or replay a finished file.
    TogglePlay,
    /// The scrub control moved; nothing is committed yet.
    Scrub(f64),
    /// Commit the scrubbed position.
    SeekCommit,
    /// Seek by a fixed step, positive forwards.
    SeekStep(f64),
    /// Choose a track, or turn a kind off with [`NO_TRACK`].
    SelectTrack(TrackKind, i64),
    /// The window was resized to this width in logical pixels.
    Resized(f32),
    /// Subtitle text size, in percent of normal, as a slider reports it.
    SubtitleScale(f64),
    /// Subtitle placement, in percent of the screen height from the top.
    SubtitlePosition(f64),
    /// Poll the player and the scan.
    Tick,
    /// A keyboard event.
    Keyboard(keyboard::Event),
    /// Quit.
    Close,
}

/// A direction the keyboard cursor can move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// One card left.
    Left,
    /// One card right.
    Right,
    /// One row up.
    Up,
    /// One row down.
    Down,
}

/// Drives the player at [`PLAYER_TICK`], and status at [`STATUS_TICK`].
///
/// The timer thread ends as soon as its receiver drops, so an idle Home screen
/// does not wake the runtime sixty times a second.
fn ticks(interval: Duration) -> Subscription<Message> {
    use iced::futures::StreamExt;
    use iced::futures::channel::mpsc;

    Subscription::run_with(interval, |interval| {
        let interval = *interval;
        let (sender, receiver) = mpsc::unbounded::<()>();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(interval);
                if sender.unbounded_send(()).is_err() {
                    break;
                }
            }
        });
        receiver.map(|()| Message::Tick)
    })
}

impl App {
    fn new() -> (Self, Task<Message>) {
        // A launch is what gives the menu bar's Quit its budget to be re-pointed at
        // the window's close action; see `route_quit` for why that happens in the
        // update loop rather than here.
        let (mut app, task) = Self::launch();
        app.quit_routing_budget = QUIT_ROUTING_BUDGET;
        app.quit_check = crate::quit::check_path();
        app.background_work = true;
        // The library was read while background work was still disabled, so the
        // first screen's posters are queued here: without this, extraction would
        // not start until the viewer happened to navigate somewhere.
        app.ensure_artwork();
        (app, task)
    }

    fn launch() -> (Self, Task<Message>) {
        let (path, label) = match std::env::var_os("SCREENROOM_LIBRARY") {
            Some(path) => (PathBuf::from(path), String::from("SCREENROOM_LIBRARY")),
            None => match default_library_path() {
                Some(path) => {
                    let label = path.display().to_string();
                    (path, label)
                }
                None => (PathBuf::from("<in-memory>"), String::from("in-memory")),
            },
        };

        if label == "in-memory" {
            return (
                Self::with_library(
                    Library::open_in_memory().expect("in-memory library"),
                    None,
                    label,
                ),
                Task::none(),
            );
        }

        match Library::open(&path) {
            Ok(library) => (Self::with_library(library, None, label), Task::none()),
            Err(error) => {
                // A library that cannot be opened is reported, not silently
                // replaced by an empty one: the user's history is in there.
                (
                    Self::with_library(
                        Library::open_in_memory().expect("in-memory library"),
                        Some(format!("{}: {}", error.code(), error)),
                        label,
                    ),
                    Task::none(),
                )
            }
        }
    }

    /// Builds the application around an existing library, for tests.
    pub fn with_library(library: Library, error: Option<String>, library_label: String) -> Self {
        let library = Arc::new(library);
        let profile_id = library
            .profiles()
            .ok()
            .and_then(|profiles| profiles.into_iter().next())
            .map(|profile| profile.id)
            .unwrap_or_default();
        // A preference read must never keep the library from opening, so an
        // unreadable preferences row falls back to the defaults.
        let preferences = library.preferences().unwrap_or_default();
        let mut app = Self {
            library,
            library_label,
            profile_id,
            capability: screenroom_player::capability(),
            screen: Screen::Home,
            focus: 0,
            home: Vec::new(),
            suggestions: Vec::new(),
            browse: Browse::new(),
            detail: None,
            artwork: HashMap::new(),
            artwork_queued: HashSet::new(),
            artwork_failed: HashSet::new(),
            artwork_failures: 0,
            extractor: None,
            background_work: false,
            player: None,
            snapshot: PlayerSnapshot::default(),
            frame: None,
            active: None,
            scrub: None,
            markers: Vec::new(),
            mark_pending: None,
            auto_skipped: None,
            window_width: INITIAL_WINDOW_WIDTH,
            preferences,
            profiles: Vec::new(),
            sources: Vec::new(),
            new_profile: String::new(),
            relink_target: None,
            relink_path: String::new(),
            backup_path: default_backup_path(),
            restore_path: String::new(),
            armed_restore: None,
            subtitle_input: String::new(),
            audio_input: String::new(),
            armed_remove: None,
            new_root: String::new(),
            scan_token: None,
            scan_events: None,
            status: None,
            error,
            // Only a launch arms this; see the field's own note.
            quit_routing_budget: 0,
            quit_routing: crate::quit::QuitRouting::Unsupported,
            quit_check: None,
        };
        // The language fields start from what is stored, so the screen never
        // shows an empty box for a preference that is actually set.
        app.subtitle_input = app
            .preferences
            .subtitle_language
            .clone()
            .unwrap_or_default();
        app.audio_input = app.preferences.audio_language.clone().unwrap_or_default();
        app.reload();
        app
    }

    // ---------------------------------------------------------------- loading

    /// Reloads every projection the current screen needs.
    fn reload(&mut self) {
        self.reload_home();
        self.reload_browse();
        self.reload_administration();
        self.ensure_artwork();
    }

    /// Reloads the profile and folder lists the Settings screen administers.
    ///
    /// Failures are reported rather than replaced by an empty list: an empty
    /// Settings screen would suggest there are no profiles or folders, which is a
    /// different and untrue statement.
    fn reload_administration(&mut self) {
        match self.library.profiles() {
            Ok(profiles) => self.profiles = profiles,
            Err(error) => self.report(&error),
        }
        match self.library.sources() {
            Ok(sources) => self.sources = sources,
            Err(error) => self.report(&error),
        }
        if self
            .relink_target
            .as_ref()
            .is_some_and(|id| !self.sources.iter().any(|source| &source.id == id))
        {
            self.relink_target = None;
        }
    }

    fn reload_home(&mut self) {
        match self.library.continue_watching(&self.profile_id, RAIL) {
            Ok(items) => self.home = items,
            Err(error) => self.report(&error),
        }
        match self.library.recommendations(&self.profile_id, SUGGESTIONS) {
            Ok(suggestions) => self.suggestions = suggestions,
            Err(error) => self.report(&error),
        }
        self.ensure_artwork();
    }

    /// Runs the current Browse query: a count, then exactly one page of rows.
    fn reload_browse(&mut self) {
        let base = LibraryQuery {
            profile_id: Some(self.profile_id.clone()),
            text: self.browse.text.clone(),
            kind: self.browse.kind.kind(),
            genre: self.browse.genre.clone(),
            available_only: self.browse.available_only,
            watchlist_only: self.browse.watchlist_only,
            favorites_only: self.browse.favorites_only,
            sort: self.browse.sort.sort(),
            ..LibraryQuery::page(PAGE_SIZE)
        };

        match self.library.count(&base) {
            Ok(total) => self.browse.total = total,
            Err(error) => {
                self.report(&error);
                return;
            }
        }

        // A filter that shrank the result set must not leave the viewer on a page
        // that no longer exists.
        let pages = self.browse.page_count();
        if self.browse.page >= pages {
            self.browse.page = pages - 1;
        }

        let query = LibraryQuery {
            offset: (self.browse.page * PAGE_SIZE as usize) as i64,
            ..base
        };
        match self.library.items(&query) {
            Ok(items) => self.browse.items = items,
            Err(error) => self.report(&error),
        }

        if let Ok(genres) = self.library.known_genres() {
            // A genre that is no longer present must not stay selected, or the
            // library would look empty for no visible reason.
            if let Some(selected) = &self.browse.genre
                && !genres.contains(selected)
            {
                self.browse.genre = None;
            }
            self.browse.genres = genres;
        }
        self.clamp_focus();
        self.ensure_artwork();
    }

    // -------------------------------------------------------------- artwork

    /// Works the current screen shows, deduplicated.
    ///
    /// Only these are asked for first: a grid must draw what is on it before it
    /// spends the machine's time on works nobody is looking at.
    fn visible_work_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = Vec::new();
        let mut add = |work_id: &str| {
            if !ids.iter().any(|known| known == work_id) {
                ids.push(work_id.to_string());
            }
        };
        for item in &self.home {
            add(&item.work_id);
        }
        for item in &self.browse.items {
            add(&item.work_id);
        }
        if let Some(detail) = &self.detail {
            add(&detail.item.work_id);
            for episode in &detail.episodes {
                add(&episode.work_id);
            }
        }
        for suggestion in &self.suggestions {
            add(&suggestion.item.work_id);
        }
        ids
    }

    /// Loads every stored poster the screen shows and queues the ones missing.
    ///
    /// Nothing is extracted while a file is playing: video decoding owns the GPU
    /// and the disk, and a poster can wait — which is also the rule that keeps
    /// watching from being interrupted by background work.
    fn ensure_artwork(&mut self) {
        if self.screen == Screen::Player {
            return;
        }
        let ids = self.visible_work_ids();
        if ids.is_empty() {
            return;
        }

        let missing: Vec<String> = ids
            .iter()
            .filter(|work_id| {
                !self.artwork.contains_key(*work_id) && !self.artwork_failed.contains(*work_id)
            })
            .cloned()
            .collect();
        if !missing.is_empty() {
            match self.library.artworks_for(&missing) {
                Ok(found) => {
                    for art in found {
                        self.artwork
                            .entry(art.work_id)
                            .or_insert_with(|| image::Handle::from_bytes(art.bytes));
                    }
                }
                Err(error) => self.report(&error),
            }
        }

        if !self.background_work {
            return;
        }
        let mut candidates = match self.library.artwork_candidates_in(&ids) {
            Ok(candidates) => candidates,
            Err(error) => return self.report(&error),
        };
        if candidates.is_empty() {
            // Nothing on screen needs a poster, so the queue takes a few others
            // and the library fills in while the viewer reads.
            candidates = match self.library.artwork_candidates(ARTWORK_IDLE_BATCH) {
                Ok(candidates) => candidates,
                Err(error) => return self.report(&error),
            };
        }
        candidates.retain(|candidate| {
            !self.artwork_queued.contains(&candidate.work_id)
                && !self.artwork_failed.contains(&candidate.work_id)
        });
        if candidates.is_empty() {
            return;
        }
        if self.extractor.is_none() {
            self.extractor = Some(Extractor::spawn(Arc::clone(&self.library)));
        }
        let Some(extractor) = &self.extractor else {
            return;
        };
        for candidate in candidates {
            let work_id = candidate.work_id.clone();
            // Recorded only when the work was actually accepted: a full queue must
            // leave the work free to be offered again rather than look queued.
            if extractor.request(candidate) {
                self.artwork_queued.insert(work_id);
            }
        }
    }

    /// Takes whatever the extractor finished since the last tick.
    fn drain_artwork(&mut self) {
        let Some(extractor) = &self.extractor else {
            return;
        };
        for event in extractor.poll() {
            match event {
                ArtworkEvent::Stored { work_id } => {
                    self.artwork_queued.remove(&work_id);
                    if let Ok(Some(art)) = self.library.artwork(&work_id) {
                        self.artwork
                            .insert(work_id, image::Handle::from_bytes(art.bytes));
                    }
                }
                ArtworkEvent::Failed { work_id, reason } => {
                    self.artwork_queued.remove(&work_id);
                    self.artwork_failed.insert(work_id);
                    self.artwork_failures += 1;
                    // Said once, not once per title: a folder of unreadable files
                    // should not turn the footer into a scrolling list.
                    if self.artwork_failures == 1 {
                        self.status = Some(format!("No poster could be drawn — {reason}"));
                    }
                }
            }
        }
    }

    /// Keeps the keyboard cursor inside the loaded page.
    fn clamp_focus(&mut self) {
        let len = self.visible_len();
        self.focus = if len == 0 { 0 } else { self.focus.min(len - 1) };
    }

    /// The Home item at a cursor position: suggestions first, then progress.
    ///
    /// The order is the drawing order, so an index means the same card the cursor
    /// highlights and the one Enter opens.
    fn home_item(&self, index: usize) -> Option<&LibraryItem> {
        if index < self.suggestions.len() {
            self.suggestions
                .get(index)
                .map(|suggestion| &suggestion.item)
        } else {
            self.home.get(index - self.suggestions.len())
        }
    }

    /// How many items the keyboard cursor can reach on this screen.
    fn visible_len(&self) -> usize {
        match self.screen {
            // Suggestions are reachable from the same cursor as Continue Watching,
            // so the arrow keys cover the whole screen rather than half of it.
            Screen::Home => self.home.len() + self.suggestions.len(),
            Screen::Browse => self.browse.items.len(),
            // Settings is a form with no grid, and the player has its own keys.
            Screen::Settings | Screen::Player => 0,
            Screen::Detail => self
                .detail
                .as_ref()
                .map(|detail| detail.episodes.len().max(1))
                .unwrap_or(0),
        }
    }

    /// Records a core error with its stable code, so the message is diagnosable.
    fn report(&mut self, error: &CoreError) {
        self.error = Some(format!("{}: {}", error.code(), error));
    }

    /// Points the menu bar's Quit at the window's close action, once it can.
    ///
    /// On macOS the Quit item AppKit installs ends the process below the event
    /// loop, so a viewing in progress is finalised by nobody. Re-pointing it at
    /// the action the close button performs makes the two the same operation, and
    /// this is where that happens because the menu bar only exists once the
    /// application has finished starting.
    ///
    /// Best-effort and reported, never fatal: if it never succeeds, quitting still
    /// works — it simply cannot store the position it is at, and a user reading the
    /// log should be able to find out that is why.
    ///
    /// Returns whether the process should now stop, which it does when a check
    /// launch has its answer.
    fn route_quit(&mut self) -> bool {
        // The budget is the whole guard. It is not enough to check whether the last
        // outcome was graceful, because the starting value answers that before
        // anything has been tried.
        if self.quit_routing_budget == 0 {
            return false;
        }
        let routing = crate::quit::route_quit_through_close();
        self.quit_routing = routing;
        // Spending the budget is what stops every later update from asking the
        // window server again, whether the answer was good or the budget ran out.
        let settled = if routing.is_graceful() {
            self.quit_routing_budget = 0;
            true
        } else {
            self.quit_routing_budget -= 1;
            if self.quit_routing_budget == 0 {
                eprintln!(
                    "screenroom: the menu bar's Quit could not be routed through the window \
                     close ({routing}); quitting may not store the position it is at."
                );
            }
            self.quit_routing_budget == 0
        };
        if !settled {
            return false;
        }

        // A check launch asked for nothing but this answer.
        let Some(path) = self.quit_check.clone() else {
            return false;
        };
        if let Err(error) = crate::quit::report_check(&path, routing) {
            eprintln!(
                "screenroom: could not write the quit-routing check to {}: {error}",
                path.display()
            );
        }
        true
    }

    // ------------------------------------------------------------ navigation

    fn open_detail(&mut self, file_id: &str) {
        let item = match self.library.item(file_id, Some(&self.profile_id)) {
            Ok(Some(item)) => item,
            Ok(None) => {
                self.error = Some("That file is no longer in the library.".to_string());
                return;
            }
            Err(error) => return self.report(&error),
        };

        let mut episodes = Vec::new();
        let mut seasons = Vec::new();
        if item.kind == MediaKind::Series {
            if let Ok(found) = self.library.seasons_for_work(&item.work_id) {
                seasons = found;
            }
            let query = LibraryQuery {
                profile_id: Some(self.profile_id.clone()),
                work_id: Some(item.work_id.clone()),
                sort: Sort::Title,
                ..LibraryQuery::page(500)
            };
            match self.library.items(&query) {
                Ok(found) => episodes = found,
                Err(error) => return self.report(&error),
            }
        }

        self.detail = Some(Detail {
            item,
            episodes,
            seasons,
        });
        self.focus = 0;
        self.screen = Screen::Detail;
        self.ensure_artwork();
    }

    /// Opens a file: session first, then the player, then the resume seek.
    fn open_player(&mut self, file_id: &str, from_start: bool) {
        let origin = match self.screen {
            Screen::Detail => Screen::Detail,
            _ => Screen::Browse,
        };
        let Some(item) = self
            .library
            .item(file_id, Some(&self.profile_id))
            .ok()
            .flatten()
        else {
            self.error = Some("That file is no longer in the library.".to_string());
            return;
        };

        if !item.available {
            self.error = Some(format!(
                "{} is not readable right now; relink its source and rescan.",
                item.filename
            ));
            return;
        }
        let Some(path) = self.media_path(&item) else {
            // The library still lists the file but it is gone, or the source is
            // no longer mounted. Say so and name the recovery, do not play nothing.
            self.error = Some(format!(
                "{} cannot be read right now; relink its source and rescan.",
                item.filename
            ));
            return;
        };

        if self.player.is_none() {
            match Player::start() {
                Ok(player) => self.player = Some(player),
                Err(error) => {
                    self.error = Some(format!("Playback is unavailable: {error}"));
                    return;
                }
            }
        }

        let opened = match PlaybackTracker::open(&self.library, &self.profile_id, &item.id) {
            Ok(opened) => opened,
            Err(error) => return self.report(&error),
        };

        let resume = if from_start { 0 } else { opened.resume_from_ms };
        let player = self.player.as_ref().expect("player started above");
        // Before the file, not after: these are video-output properties, so setting
        // them first means the very first subtitle drawn is already the right size.
        player.set_subtitle_style(
            f64::from(self.preferences.subtitle_scale_percent) / 100.0,
            f64::from(self.preferences.subtitle_position_percent),
        );
        player.load_at(&path, Some(resume as f64 / 1_000.0));
        self.active = Some(Active {
            tracker: opened.tracker,
            file_id: item.id.clone(),
            title: item.title.clone(),
            resume_from_ms: resume,
            origin,
        });
        self.snapshot = player.snapshot();
        self.frame = None;
        self.scrub = None;
        // Segments are per edition, so a ruled intro does not follow the film into
        // a different cut of it.
        self.markers.clear();
        self.reload_markers(&item.edition_id);
        self.mark_pending = None;
        self.auto_skipped = None;
        self.screen = Screen::Player;
    }

    /// Absolute path of a library item, resolved through its authorized source.
    fn media_path(&self, item: &LibraryItem) -> Option<PathBuf> {
        let source = self.library.source(&item.source_id).ok()?;
        let path = Path::new(&source.root_path).join(&item.relative_path);
        path.exists().then_some(path)
    }

    /// Stores progress and closes the session, then returns to the library.
    fn leave_player(&mut self) {
        let origin = self
            .active
            .as_ref()
            .map(|active| active.origin)
            .unwrap_or(Screen::Browse);
        // The final position is stored before the session closes, so leaving
        // during playback never loses more than the checkpoint interval.
        self.stop_playback();
        self.screen = origin;
        self.reload();
        self.refresh_detail();
    }

    /// Re-reads the open detail item so its progress and flags stay truthful.
    fn refresh_detail(&mut self) {
        let Some(file_id) = self.detail.as_ref().map(|detail| detail.item.id.clone()) else {
            return;
        };
        if let Ok(Some(item)) = self.library.item(&file_id, Some(&self.profile_id))
            && let Some(detail) = self.detail.as_mut()
        {
            detail.item = item;
        }
    }

    // -------------------------------------------------------------- playback

    /// A player snapshot mid-file, for tests that need one without libmpv.
    #[cfg(test)]
    fn playing_snapshot(position: f64, duration: f64) -> PlayerSnapshot {
        PlayerSnapshot {
            state: screenroom_player::PlayerState::Playing,
            position,
            duration,
            loaded: true,
            ..PlayerSnapshot::default()
        }
    }

    /// Writes one checkpoint, reporting failures instead of hiding them.
    fn save(&mut self, active: &mut Active, snapshot: &PlayerSnapshot, ended: bool) {
        if !snapshot.loaded || snapshot.duration_ms() <= 0 {
            return;
        }
        match active.tracker.record(
            &self.library,
            snapshot.position_ms(),
            snapshot.duration_ms(),
            ended,
        ) {
            Ok(_) => {}
            Err(error) => self.report(&error),
        }
    }

    fn poll(&mut self) {
        self.drain_scan();
        self.drain_artwork();
        let Some(player) = &self.player else { return };
        let snapshot = player.snapshot();
        let changed_frame = snapshot.frame.as_ref().is_some_and(|frame| {
            self.frame.as_ref().map(|held| held.generation) != Some(frame.generation)
        });
        if changed_frame {
            self.frame = snapshot.frame.clone();
        }
        let ended = snapshot.ended && !self.snapshot.ended;
        self.snapshot = snapshot;
        // Taken out of `self` so the save can borrow the library and the tracker
        // at the same time; it goes straight back.
        if let Some(mut active) = self.active.take() {
            let snapshot = self.snapshot.clone();
            self.save(&mut active, &snapshot, ended);
            self.active = Some(active);
        }
        // After the save, so the episode is stored and marked watched before the
        // next one takes over the session.
        if ended {
            self.advance_to_next_episode();
        }
        // After the save, so skipping does not jump the checkpoint around, and
        // last, so a finished file is never seeked away from its end.
        if !ended {
            self.maybe_auto_skip_intro();
        }
    }

    fn toggle_play(&mut self) {
        let Some(player) = &self.player else { return };
        match play_action(self.snapshot.state) {
            // Un-pausing a finished file does nothing at all, so a replay reopens
            // it; that also gives the new viewing its own session and generation.
            PlayAction::Replay => self.replay(),
            PlayAction::Pause => player.set_paused(true),
            PlayAction::Resume => player.set_paused(false),
        }
    }

    /// Reopens the current file from the beginning and keeps the return path.
    ///
    /// The finished session is saved and closed first, so a replay cannot leave
    /// an open session behind or lose the fact that the file was watched.
    fn replay(&mut self) {
        let Some(file_id) = self.active.as_ref().map(|active| active.file_id.clone()) else {
            return;
        };
        let origin = self.active.as_ref().map(|active| active.origin);
        self.finish_active();
        self.open_player(&file_id, true);
        // `open_player` reads the origin from the screen, which is still the
        // player here; put back where this viewing came from.
        if let (Some(origin), Some(active)) = (origin, self.active.as_mut()) {
            active.origin = origin;
        }
    }

    /// Ends any viewing in progress: final position saved, session closed.
    ///
    /// Used both when the window closes and when the app quits, so the two paths
    /// cannot disagree about what happens to an open file.
    fn stop_playback(&mut self) {
        // The player's own snapshot is the newest truth; the held one is the
        // fallback, and is what makes this path testable without a player.
        let snapshot = self
            .player
            .as_ref()
            .map(|player| player.snapshot())
            .unwrap_or_else(|| self.snapshot.clone());
        if let Some(mut active) = self.active.take() {
            self.save(&mut active, &snapshot, snapshot.ended);
            if let Err(error) = active.tracker.close(&self.library) {
                self.report(&error);
            }
        }
        if let Some(player) = &self.player {
            player.stop();
        }
        self.frame = None;
        self.scrub = None;
        self.snapshot = PlayerSnapshot::default();
    }

    /// Saves and closes the current viewing, stops the player, clears its state.
    ///
    /// Shared by replay and autoplay, which both end one viewing and start
    /// another: the finished viewing has to be stored first in either case, and
    /// both must leave no open session behind.
    fn finish_active(&mut self) {
        if let Some(mut active) = self.active.take() {
            let snapshot = self.player.as_ref().map(|player| player.snapshot());
            if let Some(snapshot) = snapshot {
                self.save(&mut active, &snapshot, snapshot.ended);
            }
            if let Err(error) = active.tracker.close(&self.library) {
                self.report(&error);
            }
        }
        if let Some(player) = &self.player {
            player.stop();
        }
        self.frame = None;
        self.scrub = None;
        self.snapshot = PlayerSnapshot::default();
    }

    /// Continues into the next verified episode once one finishes.
    ///
    /// The finished viewing is already saved and marked watched before this runs,
    /// so finding nothing to play leaves the library correct. A gap stops the
    /// chain rather than skipping it: `next_episode` reports `None` instead of
    /// guessing, and that `None` is respected.
    ///
    /// Autoplay starts only a file that is readable right now. An unreadable next
    /// episode leaves the finished player on screen with the reason, rather than
    /// swapping one failure for another the viewer never asked for.
    /// The episode that should follow `file_id`, or `None` to stop.
    ///
    /// Split out from the transition so the decision can be tested without a
    /// player: the choice of what plays next is product logic, and only starting
    /// it needs libmpv.
    fn next_episode_after(&self, file_id: &str) -> Result<Option<LibraryItem>, CoreError> {
        if !self.preferences.autoplay_next {
            return Ok(None);
        }
        self.library.next_episode(file_id, Some(&self.profile_id))
    }

    fn advance_to_next_episode(&mut self) {
        let Some(finished) = self.active.as_ref().map(|active| active.file_id.clone()) else {
            return;
        };
        let next = match self.next_episode_after(&finished) {
            Ok(next) => next,
            Err(error) => return self.report(&error),
        };
        let Some(next) = next else {
            return;
        };
        let title = self
            .active
            .as_ref()
            .map(|active| active.title.clone())
            .unwrap_or_default();
        if !next.available {
            self.status = Some(format!(
                "{title} finished. The next episode cannot be read right now; relink its folder."
            ));
            return;
        }
        let origin = self.active.as_ref().map(|active| active.origin);
        self.finish_active();
        self.open_player(&next.id, false);
        if let (Some(origin), Some(active)) = (origin, self.active.as_mut()) {
            active.origin = origin;
        }
        // Say that the switch was automatic, so it reads as continuing the series
        // rather than as the app jumping somewhere on its own.
        if self.screen == Screen::Player {
            self.status = Some(format!("Finished {title}. Playing next: {}", next.title));
        }
    }

    fn commit_seek(&mut self) {
        let Some(seconds) = self.scrub.take() else {
            return;
        };
        self.seek_to(seconds);
    }

    fn seek_to(&mut self, seconds: f64) {
        let Some(player) = &self.player else { return };
        // The seek generation must advance before the player reports the new
        // position, otherwise the core would reject those checkpoints.
        if let Some(active) = &mut self.active
            && let Err(error) = active.tracker.seek(&self.library)
        {
            self.report(&error);
            return;
        }
        player.seek(seconds);
    }

    // -------------------------------------------------------- skip segments

    /// The stored marker the playhead is inside, if any.
    ///
    /// Returned by value so the caller can seek without holding a borrow on the
    /// marker list it is about to update.
    fn active_marker(&self) -> Option<SegmentMarker> {
        let position = self.snapshot.position_ms();
        self.markers
            .iter()
            .find(|marker| position >= marker.start_ms && position < marker.end_ms)
            .cloned()
    }

    /// Jumps past the segment the playhead is in.
    fn skip_segment(&mut self) {
        let Some(marker) = self.active_marker() else {
            self.status = Some("No intro or credits marker covers this position yet.".to_string());
            return;
        };
        self.seek_to(marker.end_ms as f64 / 1_000.0);
        self.status = Some(
            if marker.kind == "intro" {
                "Skipped the intro."
            } else {
                "Skipped the credits."
            }
            .to_string(),
        );
    }

    /// Pins a segment's start, or closes the one already pinned.
    fn toggle_segment_mark(&mut self, kind: &str) {
        if self
            .mark_pending
            .as_ref()
            .is_some_and(|(pending, _)| pending == kind)
        {
            self.finish_segment_mark();
        } else {
            self.start_segment_mark(kind);
        }
    }

    fn start_segment_mark(&mut self, kind: &str) {
        let position = self.snapshot.position_ms();
        if position <= 0 {
            self.status =
                Some("Play into the segment first, then pin where it starts.".to_string());
            return;
        }
        self.mark_pending = Some((kind.to_string(), position));
        self.status = Some(format!(
            "{kind} start pinned at {} — press again where the segment ends.",
            clock(position as f64 / 1_000.0)
        ));
    }

    /// Stores the pinned marker, ending it at the playhead.
    ///
    /// The end is taken from the player rather than asked for in a field: the
    /// position on screen is the one the viewer is describing.
    fn finish_segment_mark(&mut self) {
        let Some((kind, start_ms)) = self.mark_pending.take() else {
            return;
        };
        let Some(file_id) = self.active.as_ref().map(|active| active.file_id.clone()) else {
            return;
        };
        let edition_id = match self.library.item(&file_id, Some(&self.profile_id)) {
            Ok(Some(item)) => item.edition_id,
            Ok(None) => {
                self.status = Some("That file is no longer in the library.".to_string());
                return;
            }
            Err(error) => return self.report(&error),
        };
        let end_ms = self.snapshot.position_ms().max(start_ms);
        match self
            .library
            .mark_segment(&edition_id, &kind, start_ms, end_ms)
        {
            Ok(marker) => {
                self.reload_markers(&marker.edition_id);
                self.status = Some(format!(
                    "{kind} marked {} – {}; it is offered next time this file plays.",
                    clock(marker.start_ms as f64 / 1_000.0),
                    clock(marker.end_ms as f64 / 1_000.0)
                ));
            }
            Err(error) => self.report(&error),
        }
    }

    fn reload_markers(&mut self, edition_id: &str) {
        match self.library.markers(edition_id) {
            Ok(markers) => self.markers = markers,
            Err(error) => self.report(&error),
        }
    }

    /// Skips a marked intro once, when the preference asks for it.
    fn maybe_auto_skip_intro(&mut self) {
        if !self.preferences.auto_skip_intro {
            return;
        }
        let position = self.snapshot.position_ms();
        let found = self
            .markers
            .iter()
            .find(|marker| {
                marker.kind == "intro" && position >= marker.start_ms && position < marker.end_ms
            })
            .map(|marker| (marker.id.clone(), marker.end_ms));
        let Some((id, end_ms)) = found else {
            return;
        };
        if self.auto_skipped.as_deref() == Some(id.as_str()) {
            return;
        }
        self.auto_skipped = Some(id);
        self.seek_to(end_ms as f64 / 1_000.0);
        self.status = Some("Skipped the intro.".to_string());
    }

    // ------------------------------------------------------------------ scan

    /// The track chooser's options and current selection for one kind.
    ///
    /// The player is the single source of truth: the selection is read back from
    /// the snapshot rather than remembered here, so a switch libmpv refused shows
    /// as not having happened. Subtitles add a real "Off" entry, because turning
    /// them off is a libmpv operation and a state the user has to be able to see.
    fn track_chooser(&self, kind: TrackKind) -> (Vec<TrackChoice>, Option<TrackChoice>) {
        let mut choices: Vec<TrackChoice> = self
            .snapshot
            .tracks
            .iter()
            .filter(|track| track.kind == kind)
            .map(TrackChoice::from_track)
            .collect();
        let in_use = self
            .snapshot
            .tracks
            .iter()
            .filter(|track| track.kind == kind && track.selected)
            .map(|track| track.id)
            .next();
        if kind == TrackKind::Subtitle && !choices.is_empty() {
            choices.insert(0, TrackChoice::off());
        }
        let selected = choices
            .iter()
            .find(|choice| in_use == Some(choice.id))
            .cloned()
            // Subtitles default to "Off" rather than to an empty control, so the
            // row always states what is happening.
            .or_else(|| choices.first().cloned());
        (choices, selected)
    }

    // -------------------------------------------------------------- settings

    /// Switches the active viewer profile.
    ///
    /// The switch is not a display change: progress, lists and Continue Watching
    /// are projected for one profile, so every screen is reloaded from the new
    /// profile rather than reusing what the previous one was showing.
    fn choose_profile(&mut self, profile_id: String) {
        if !self.profiles.iter().any(|profile| profile.id == profile_id) {
            self.error = Some("That profile is no longer in the library.".to_string());
            return;
        }
        self.profile_id = profile_id.clone();
        self.focus = 0;
        self.detail = None;
        self.error = None;
        // The library is stored as the profile to open with, so the choice
        // survives a restart instead of resetting to the first profile.
        let preferences = Preferences {
            default_profile: Some(profile_id),
            ..self.preferences.clone()
        };
        self.save_preferences(preferences);
        self.reload();
    }

    /// Adds the profile in the text field and makes it the active one.
    fn add_profile(&mut self) {
        let name = self.new_profile.trim().to_string();
        if name.is_empty() {
            self.error = Some("Type a name for the new profile first.".to_string());
            return;
        }
        match self.library.add_profile(&name) {
            Ok(profile) => {
                self.new_profile.clear();
                self.reload_administration();
                self.status = Some(format!("Added {}", profile.name));
                self.choose_profile(profile.id);
            }
            Err(error) => self.report(&error),
        }
    }

    /// Removes a profile and everything it watched.
    fn remove_profile(&mut self, profile_id: String) {
        let name = self
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .map(|profile| profile.name.clone())
            .unwrap_or_else(|| "that profile".to_string());
        if let Err(error) = self.library.remove_profile(&profile_id) {
            return self.report(&error);
        }
        // The removed profile cannot stay the active one, and a stored default
        // pointing at it would fail the next preference save.
        let fallback = self
            .library
            .profiles()
            .ok()
            .and_then(|profiles| profiles.into_iter().next())
            .map(|profile| profile.id);
        let preferences = Preferences {
            default_profile: fallback.clone().filter(|id| *id != self.profile_id),
            ..self.preferences.clone()
        };
        self.save_preferences(preferences);
        if self.profile_id == profile_id
            && let Some(fallback) = fallback
        {
            self.profile_id = fallback;
        }
        self.status = Some(format!("Removed {name} and its watch history."));
        self.reload();
    }

    /// Stores the whole preference set, keeping the in-memory copy in step.
    fn save_preferences(&mut self, preferences: Preferences) {
        match self.library.set_preferences(&preferences) {
            Ok(()) => {
                self.preferences = preferences;
                self.error = None;
            }
            Err(error) => self.report(&error),
        }
    }

    /// Stores a subtitle style and shows it on any subtitle already on screen.
    ///
    /// Both halves matter: the stored value is what the next viewing inherits, and
    /// pushing it to the running player is what makes the change visible while
    /// something is playing, which is the only moment a subtitle style means
    /// anything.
    fn apply_subtitle_style(&mut self, scale_percent: u16, position_percent: u16) {
        let preferences = Preferences {
            subtitle_scale_percent: scale_percent,
            subtitle_position_percent: position_percent,
            ..self.preferences.clone()
        };
        if preferences == self.preferences {
            return;
        }
        let before_ok = self.error.is_none();
        self.save_preferences(preferences);
        if before_ok && self.error.is_none() {
            self.status = Some("Subtitle style saved.".to_string());
        }
        self.push_subtitle_style();
    }

    /// Sends the stored subtitle style to the player, if one is running.
    ///
    /// A no-op with no player: there is nothing to draw the subtitles, and the
    /// stored preference is applied when one opens.
    fn push_subtitle_style(&self) {
        let Some(player) = &self.player else {
            return;
        };
        player.set_subtitle_style(
            f64::from(self.preferences.subtitle_scale_percent) / 100.0,
            f64::from(self.preferences.subtitle_position_percent),
        );
    }

    /// Applies a typed language tag, or clears the preference when blank.
    fn apply_language(&mut self, subtitle: bool) {
        let (value, current) = if subtitle {
            (
                self.subtitle_input.clone(),
                self.preferences.subtitle_language.clone(),
            )
        } else {
            (
                self.audio_input.clone(),
                self.preferences.audio_language.clone(),
            )
        };
        let trimmed = value.trim();
        let next = (!trimmed.is_empty()).then(|| trimmed.to_string());
        if next == current {
            return;
        }
        let preferences = if subtitle {
            Preferences {
                subtitle_language: next,
                ..self.preferences.clone()
            }
        } else {
            Preferences {
                audio_language: next,
                ..self.preferences.clone()
            }
        };
        let before_ok = self.error.is_none();
        self.save_preferences(preferences);
        if before_ok && self.error.is_none() {
            self.status = Some("Preference saved.".to_string());
        }
    }

    /// Rescans a folder the library already holds.
    fn rescan_source(&mut self, source_id: String) {
        if let Err(error) = self
            .library
            .set_source_state(&source_id, SourceState::Available)
        {
            return self.report(&error);
        }
        self.reload_administration();
        self.start_scan(source_id);
    }

    /// Points a folder at a new location, then rescans it.
    ///
    /// Relinking is what makes a removable drive work again after the mount path
    /// changes: the grant is re-issued, every file under it becomes unavailable
    /// until the rescan re-proves each one, and nothing is deleted in the meantime.
    fn relink_source(&mut self) {
        let Some(source_id) = self.relink_target.clone() else {
            self.error = Some("Choose the folder to relink first.".to_string());
            return;
        };
        let raw = self.relink_path.trim().to_string();
        if raw.is_empty() {
            self.error = Some("Type the folder's new location first.".to_string());
            return;
        }
        if let Err(error) = self.library.relink_source(&source_id, PathBuf::from(&raw)) {
            return self.report(&error);
        }
        self.relink_path.clear();
        self.status = Some("Folder relinked — rescanning.".to_string());
        self.reload_administration();
        self.start_scan(source_id);
    }

    /// Removes a folder from the library, or asks first.
    ///
    /// Removal hides the folder's files and keeps every work, progress row and
    /// list entry, so it is recoverable — but it is still a large visible change,
    /// which is why the first press only asks.
    fn remove_source(&mut self, source_id: String) {
        if self.armed_remove.as_deref() != Some(source_id.as_str()) {
            self.armed_remove = Some(source_id);
            self.status = Some(
                "Press Remove again to hide this folder's files. Nothing on disk is deleted."
                    .to_string(),
            );
            return;
        }
        self.armed_remove = None;
        match self.library.remove_source(&source_id) {
            Ok(()) => {
                self.status = Some(
                    "Folder removed from the library. Its files are untouched on disk.".to_string(),
                );
                self.reload();
            }
            Err(error) => self.report(&error),
        }
    }

    /// Writes a backup of the whole library to the typed path.
    fn backup_now(&mut self) {
        let raw = self.backup_path.trim().to_string();
        if raw.is_empty() {
            self.error = Some("Choose where the backup should be written.".to_string());
            return;
        }
        match self.library.backup_to(PathBuf::from(&raw)) {
            Ok(report) => {
                self.error = None;
                self.status = Some(format!(
                    "Backed up {} KB — {}.",
                    report.bytes / 1024,
                    report.info
                ));
            }
            Err(error) => self.report(&error),
        }
    }

    /// Reports what a backup file holds without changing anything.
    fn inspect_restore(&mut self) {
        let raw = self.restore_path.trim().to_string();
        if raw.is_empty() {
            self.error = Some("Choose the backup to restore from.".to_string());
            return;
        }
        self.armed_restore = None;
        match Library::inspect_backup(PathBuf::from(&raw)) {
            Ok(info) => {
                self.error = None;
                self.status = Some(format!("That backup holds {info}"));
            }
            Err(error) => self.report(&error),
        }
    }

    /// Restores from the chosen backup, after one confirmation.
    ///
    /// Restore replaces this library's profile, progress and list data with the
    /// backup's, so the first press only states what will happen. The core copies
    /// the current library aside before it writes anything, and that safety copy
    /// is named in the result so the user is told where it is.
    fn restore_now(&mut self) {
        let raw = self.restore_path.trim().to_string();
        if raw.is_empty() {
            self.error = Some("Choose the backup to restore from.".to_string());
            return;
        }
        if self.armed_restore.as_deref() != Some(raw.as_str()) {
            self.armed_restore = Some(raw.clone());
            let summary = Library::inspect_backup(PathBuf::from(&raw))
                .map(|info| info.to_string())
                .unwrap_or_else(|error| format!("unreadable ({})", error.code()));
            self.status = Some(format!(
                "Press Restore again to replace this library with {summary}. \
                 Your current library is copied aside first."
            ));
            return;
        }
        self.armed_restore = None;
        match self.library.restore_from(PathBuf::from(&raw)) {
            Ok(report) => {
                // The active profile may not exist in the restored data, so the
                // session is re-derived rather than left pointing at a profile
                // that is gone.
                if !self
                    .library
                    .profiles()
                    .map(|profiles| profiles.iter().any(|profile| profile.id == self.profile_id))
                    .unwrap_or(false)
                {
                    self.profile_id = self
                        .library
                        .profiles()
                        .ok()
                        .and_then(|profiles| profiles.into_iter().next())
                        .map(|profile| profile.id)
                        .unwrap_or_default();
                }
                self.detail = None;
                self.reload();
                let safety = report
                    .previous_saved_to
                    .map(|path| format!(" The previous library is at {path}."))
                    .unwrap_or_default();
                self.status = Some(format!("Restored {} entries.{safety}", report.info.works));
            }
            Err(error) => self.report(&error),
        }
    }

    /// Grants the typed folder and scans it on a worker thread.
    ///
    /// Scanning never blocks the UI, and a second scan of one source is refused
    /// by the core rather than racing the first.
    fn add_folder(&mut self) {
        let raw = self.new_root.trim().to_string();
        if raw.is_empty() {
            self.error = Some("Type or paste an absolute folder path first.".to_string());
            return;
        }
        let root = PathBuf::from(&raw);
        if !root.is_absolute() {
            self.error = Some("Choose an absolute folder path.".to_string());
            return;
        }

        let source = match self
            .library
            .sources()
            .map(|sources| sources.into_iter().find(|source| source.root_path == raw))
        {
            Ok(Some(source)) => Ok(source),
            Ok(None) => self.library.add_source(&root, None),
            Err(error) => Err(error),
        };
        let source = match source {
            Ok(source) => source,
            Err(error) => return self.report(&error),
        };

        self.new_root.clear();
        self.error = None;
        self.start_scan(source.id);
    }

    /// Starts a bounded background scan for one source.
    fn start_scan(&mut self, source_id: String) {
        let token = ScanToken::new();
        self.scan_token = Some(token.clone());
        self.status = Some("Scanning…".to_string());
        let (sender, receiver) = std::sync::mpsc::channel();
        self.scan_events = Some(receiver);
        let library = Arc::clone(&self.library);
        let progress_sender = sender.clone();
        std::thread::spawn(move || {
            let scanner =
                LibraryScanner::new(&library).with_limits(250_000, 64, Duration::from_secs(2));
            let outcome = scanner.scan(&source_id, &token, |update| {
                let _ = progress_sender.send(ScanEvent::Progress {
                    visited: update.visited,
                    imported: update.imported,
                    issues: update.errors.len(),
                    done: update.done,
                    complete: false,
                });
            });
            match outcome {
                Ok(summary) => {
                    let _ = sender.send(ScanEvent::Progress {
                        visited: summary.visited,
                        imported: summary.imported,
                        issues: summary.issues.len(),
                        done: true,
                        complete: summary.complete && !summary.cancelled,
                    });
                }
                Err(error) => {
                    let _ = sender.send(ScanEvent::Failed(format!("{}: {}", error.code(), error)));
                }
            }
        });
    }

    /// Applies whatever the scan worker has reported since the last tick.
    fn drain_scan(&mut self) {
        let Some(receiver) = self.scan_events.as_ref() else {
            return;
        };
        let mut finished = false;
        let mut failure = None;
        let mut status = None;
        loop {
            match receiver.try_recv() {
                Ok(ScanEvent::Progress {
                    visited,
                    imported,
                    issues,
                    done,
                    complete,
                }) => {
                    if done {
                        finished = true;
                        status = Some(if complete {
                            format!("Scan finished — {visited} entries, {imported} imported.")
                        } else {
                            format!(
                                "Scan ended early — {visited} entries, {imported} imported, \
                                 {issues} issue(s). Nothing was deleted."
                            )
                        });
                    } else {
                        status = Some(format!("Scanning… {visited} entries, {imported} imported"));
                    }
                }
                Ok(ScanEvent::Failed(message)) => {
                    finished = true;
                    failure = Some(message);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    finished = true;
                    break;
                }
            }
        }
        if let Some(status) = status {
            self.status = Some(status);
        }
        if let Some(failure) = failure {
            self.error = Some(failure);
        }
        if finished {
            self.scan_events = None;
            self.scan_token = None;
            self.reload();
        }
    }

    // ---------------------------------------------------------------- update

    fn update(&mut self, message: Message) -> Task<Message> {
        // Done here rather than while starting, because the menu bar this changes
        // does not exist until the application has finished starting.
        if self.route_quit() {
            return iced::exit();
        }
        match message {
            Message::GoHome => {
                self.screen = Screen::Home;
                self.focus = 0;
                self.reload_home();
            }
            Message::GoBrowse => {
                self.screen = Screen::Browse;
                self.focus = 0;
                self.reload_browse();
            }
            Message::Back => self.back(),
            Message::Refresh => {
                self.reload();
                self.refresh_detail();
            }

            Message::SearchChanged(text) => {
                self.browse.text = text;
                self.browse.page = 0;
                self.reload_browse();
            }
            Message::KindChanged(kind) => {
                self.browse.kind = kind;
                self.browse.page = 0;
                self.reload_browse();
            }
            Message::GenreChanged(genre) => {
                self.browse.genre = genre;
                self.browse.page = 0;
                self.reload_browse();
            }
            Message::SortChanged(sort) => {
                self.browse.sort = sort;
                self.browse.page = 0;
                self.reload_browse();
            }
            Message::ToggleWatchlistOnly(value) => {
                self.browse.watchlist_only = value;
                self.browse.page = 0;
                self.reload_browse();
            }
            Message::ToggleFavoritesOnly(value) => {
                self.browse.favorites_only = value;
                self.browse.page = 0;
                self.reload_browse();
            }
            Message::ToggleAvailableOnly(value) => {
                self.browse.available_only = value;
                self.browse.page = 0;
                self.reload_browse();
            }
            Message::NextPage => {
                if self.browse.page + 1 < self.browse.page_count() {
                    self.browse.page += 1;
                    self.focus = 0;
                    self.reload_browse();
                }
            }
            Message::PreviousPage => {
                if self.browse.page > 0 {
                    self.browse.page -= 1;
                    self.focus = 0;
                    self.reload_browse();
                }
            }

            Message::Move(direction) => self.move_focus(direction),
            Message::Activate => self.activate(),
            Message::OpenDetail(file_id) => self.open_detail(&file_id),
            Message::Play(file_id) => self.open_player(&file_id, false),
            Message::Restart(file_id) => self.open_player(&file_id, true),
            Message::ToggleFavorite(work_id) => {
                self.set_flag(&work_id, Flag::Favorite);
            }
            Message::ToggleWatchlist(work_id) => {
                self.set_flag(&work_id, Flag::Watchlist);
            }
            Message::ToggleWatched(edition_id) => {
                let watched = self
                    .detail
                    .as_ref()
                    .map(|detail| detail.item.watched)
                    .unwrap_or(false);
                if let Err(error) =
                    self.library
                        .mark_watched(&self.profile_id, &edition_id, !watched)
                {
                    self.report(&error);
                } else {
                    self.reload();
                    self.refresh_detail();
                }
            }
            Message::HideFromContinue(edition_id) => {
                if let Err(error) = self.library.hide_continue(&self.profile_id, &edition_id) {
                    self.report(&error);
                } else {
                    self.status = Some("Removed from Continue Watching.".to_string());
                    self.reload();
                }
            }

            Message::NewRootChanged(value) => self.new_root = value,
            Message::AddFolder => self.add_folder(),
            Message::RescanSource(source_id) => self.rescan_source(source_id),
            Message::RemoveSource(source_id) => self.remove_source(source_id),
            Message::CancelArmed => {
                self.armed_remove = None;
                self.armed_restore = None;
                self.status = None;
            }
            Message::RelinkTargetChosen(source_id) => self.relink_target = Some(source_id),
            Message::RelinkPathChanged(value) => self.relink_path = value,
            Message::RelinkSource => self.relink_source(),

            Message::GoSettings => {
                self.screen = Screen::Settings;
                self.focus = 0;
                self.armed_remove = None;
                self.armed_restore = None;
                self.reload_administration();
            }
            Message::ProfileChosen(profile_id) => self.choose_profile(profile_id),
            Message::NewProfileChanged(value) => self.new_profile = value,
            Message::AddProfile => self.add_profile(),
            Message::RemoveProfile(profile_id) => self.remove_profile(profile_id),
            Message::ToggleAutoplay(enabled) => {
                let preferences = Preferences {
                    autoplay_next: enabled,
                    ..self.preferences.clone()
                };
                self.save_preferences(preferences);
            }
            Message::ToggleAutoSkip(enabled) => {
                let preferences = Preferences {
                    auto_skip_intro: enabled,
                    ..self.preferences.clone()
                };
                self.save_preferences(preferences);
            }
            Message::SkipSegment => self.skip_segment(),
            Message::MarkSegment(kind) => self.toggle_segment_mark(&kind),
            Message::FinishSegment => self.finish_segment_mark(),
            Message::SubtitleLanguageChanged(value) => self.subtitle_input = value,
            Message::AudioLanguageChanged(value) => self.audio_input = value,
            Message::Resized(width) => {
                // A zero or non-finite width arrives while a window is being
                // minimised or created; keeping the previous value avoids a layout
                // that briefly has no columns at all.
                if width.is_finite() && width > 0.0 {
                    self.window_width = width;
                }
            }
            Message::SubtitleScale(value) => self.apply_subtitle_style(
                round_percent(
                    value,
                    Preferences::SUBTITLE_SCALE_MIN_PERCENT,
                    Preferences::SUBTITLE_SCALE_MAX_PERCENT,
                ),
                self.preferences.subtitle_position_percent,
            ),
            Message::SubtitlePosition(value) => self.apply_subtitle_style(
                self.preferences.subtitle_scale_percent,
                round_percent(
                    value,
                    Preferences::SUBTITLE_POSITION_MIN_PERCENT,
                    Preferences::SUBTITLE_POSITION_MAX_PERCENT,
                ),
            ),
            Message::ApplySubtitleLanguage => self.apply_language(true),
            Message::ApplyAudioLanguage => self.apply_language(false),
            Message::BackupPathChanged(value) => self.backup_path = value,
            Message::BackupNow => self.backup_now(),
            Message::RestorePathChanged(value) => {
                self.restore_path = value;
                // A different file is a different decision, so the confirmation
                // does not carry over to it.
                self.armed_restore = None;
            }
            Message::InspectRestore => self.inspect_restore(),
            Message::RestoreNow => self.restore_now(),
            Message::ClearStatus => {
                self.status = None;
                self.error = None;
            }
            Message::CancelScan => {
                if let Some(token) = &self.scan_token {
                    token.cancel();
                    self.status = Some("Stopping the scan…".to_string());
                }
            }

            Message::BackFromPlayer => self.leave_player(),
            Message::TogglePlay => self.toggle_play(),
            Message::Scrub(seconds) => self.scrub = Some(seconds),
            Message::SeekCommit => self.commit_seek(),
            Message::SeekStep(delta) => self.seek_step(delta),
            Message::SelectTrack(kind, id) => {
                if let Some(player) = &self.player {
                    player.select_track(kind, id);
                }
            }
            Message::Tick => self.poll(),
            Message::Keyboard(event) => self.handle_key(event),
            Message::Close => {
                // Quitting while a file is open would otherwise leave the session
                // row `active`: the last periodic checkpoint is within a second of
                // the truth, but a session nobody can still write to is a lie the
                // next launch has to clean up. Stopping the player first also
                // releases the audio device before the process goes away.
                self.stop_playback();
                return iced::exit();
            }
        }
        Task::none()
    }

    /// Moves the keyboard cursor by one card or row, clamped to the page.
    fn move_focus(&mut self, direction: Focus) {
        let len = self.visible_len();
        if len == 0 {
            self.focus = 0;
            return;
        }
        let current = self.focus.min(len - 1);
        // Up and Down move by a whole row, so the step has to be the column count the
        // grid was actually drawn with: a fixed step would land on the wrong card as
        // soon as the window is wide enough to change the layout.
        let step = self.columns();
        let next = match direction {
            Focus::Left => current.checked_sub(1),
            Focus::Right => (current + 1 < len).then_some(current + 1),
            Focus::Up => current.checked_sub(step),
            Focus::Down => {
                let candidate = current + step;
                (candidate < len).then_some(candidate)
            }
        };
        self.focus = next.unwrap_or(current);
    }

    /// Enter on the focused item.
    fn activate(&mut self) {
        let file_id = match self.screen {
            Screen::Home => self.home_item(self.focus).map(|item| item.id.clone()),
            Screen::Browse => self
                .browse
                .items
                .get(self.focus)
                .map(|item| item.id.clone()),
            Screen::Detail => self.detail.as_ref().map(|detail| detail.item.id.clone()),
            Screen::Player | Screen::Settings => None,
        };
        match (self.screen, file_id) {
            // From Detail, Enter means "play"; the arrows move within a series.
            (Screen::Detail, Some(file_id)) => self.open_player(&file_id, false),
            (_, Some(file_id)) => self.open_detail(&file_id),
            _ => {}
        }
    }

    /// One step up the hierarchy: Player → its origin, Detail → Browse, else Home.
    fn back(&mut self) {
        match self.screen {
            Screen::Player => self.leave_player(),
            Screen::Detail => {
                self.screen = Screen::Browse;
                self.detail = None;
                self.focus = 0;
                self.reload_browse();
            }
            Screen::Browse | Screen::Settings => self.screen = Screen::Home,
            Screen::Home => {}
        }
    }

    /// Flips a work-level flag and reloads so the change is visible everywhere.
    fn set_flag(&mut self, work_id: &str, flag: Flag) {
        let current = self
            .detail
            .as_ref()
            .map(|detail| match flag {
                Flag::Favorite => detail.item.favorite,
                Flag::Watchlist => detail.item.watchlisted,
            })
            .unwrap_or(false);
        let result = match flag {
            Flag::Favorite => self
                .library
                .set_favorite(&self.profile_id, work_id, !current),
            Flag::Watchlist => self
                .library
                .set_watchlisted(&self.profile_id, work_id, !current),
        };
        match result {
            Ok(()) => {
                self.reload();
                self.refresh_detail();
            }
            Err(error) => self.report(&error),
        }
    }

    fn handle_key(&mut self, event: keyboard::Event) {
        use iced::keyboard::Key;
        use iced::keyboard::key::Named;
        let keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
            return;
        };

        // Screen switching uses a modifier on purpose: a bare letter would also
        // be typed into the search field and steal the keystroke.
        if modifiers.command() || modifiers.control() {
            match key {
                Key::Character(ref character) if character.as_str() == "1" => {
                    let _ = self.update(Message::GoHome);
                }
                Key::Character(ref character) if character.as_str() == "2" => {
                    let _ = self.update(Message::GoBrowse);
                }
                Key::Character(ref character) if character.as_str() == "3" => {
                    let _ = self.update(Message::GoSettings);
                }
                _ => {}
            }
            return;
        }
        // Every branch must produce `()`, so the mutating calls are statements.
        match self.screen {
            Screen::Player => match key {
                Key::Named(Named::Escape) => self.leave_player(),
                Key::Named(Named::Space) => self.toggle_play(),
                Key::Named(Named::ArrowLeft) => self.seek_step(-SEEK_STEP),
                Key::Named(Named::ArrowRight) => self.seek_step(SEEK_STEP),
                Key::Character(character) => match character.as_str() {
                    "s" => self.skip_segment(),
                    "i" => self.toggle_segment_mark("intro"),
                    "c" => self.toggle_segment_mark("credits"),
                    _ => {}
                },
                _ => {}
            },
            Screen::Detail => match key {
                Key::Named(Named::Escape) => self.back(),
                Key::Named(Named::Enter) => self.activate(),
                Key::Named(Named::ArrowUp) => self.move_focus(Focus::Up),
                Key::Named(Named::ArrowDown) => self.move_focus(Focus::Down),
                Key::Character(character) => match character.as_str() {
                    "f" => self.toggle_focused_flag(Flag::Favorite),
                    "w" => self.toggle_focused_flag(Flag::Watchlist),
                    "m" => self.toggle_focused_watched(),
                    _ => {}
                },
                _ => {}
            },
            // Settings is a form, not a grid: the arrows belong to whichever
            // field has focus, so only Escape is claimed here.
            Screen::Settings => {
                if let Key::Named(Named::Escape) = key {
                    self.back();
                }
            }
            _ => match key {
                Key::Named(Named::Escape) => self.back(),
                Key::Named(Named::Enter) => self.activate(),
                Key::Named(Named::ArrowLeft) => self.move_focus(Focus::Left),
                Key::Named(Named::ArrowRight) => self.move_focus(Focus::Right),
                Key::Named(Named::ArrowUp) => self.move_focus(Focus::Up),
                Key::Named(Named::ArrowDown) => self.move_focus(Focus::Down),
                _ => {}
            },
        }
    }

    /// Seeks by `delta` seconds, clamped to the media.
    fn seek_step(&mut self, delta: f64) {
        let target = (self.snapshot.position + delta).clamp(0.0, self.snapshot.duration.max(0.0));
        self.seek_to(target);
    }

    /// Flips the shown detail item's work-level flag from a key press.
    fn toggle_focused_flag(&mut self, flag: Flag) {
        let Some(work_id) = self
            .detail
            .as_ref()
            .map(|detail| detail.item.work_id.clone())
        else {
            return;
        };
        self.set_flag(&work_id, flag);
    }

    /// Marks the shown edition watched or unwatched from a key press.
    fn toggle_focused_watched(&mut self) {
        let Some(edition_id) = self
            .detail
            .as_ref()
            .map(|detail| detail.item.edition_id.clone())
        else {
            return;
        };
        let _ = self.update(Message::ToggleWatched(edition_id));
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            iced::keyboard::listen().map(Message::Keyboard),
            // Routed through the same Close handling as the header button, so the
            // window's close box and that button cannot disagree about whether the
            // open file was stored.
            iced::window::close_requests().map(|_| Message::Close),
            // The grid's column count follows the window, so a 3440-wide ultrawide
            // shows more cards rather than four very wide ones. Resizes arrive as
            // events rather than being polled, so this costs nothing while idle.
            iced::window::resize_events().map(|(_id, size)| Message::Resized(size.width)),
        ];
        let interval = if self.screen == Screen::Player {
            PLAYER_TICK
        } else {
            STATUS_TICK
        };
        subscriptions.push(ticks(interval));
        Subscription::batch(subscriptions)
    }

    fn theme(&self) -> iced::Theme {
        design::theme()
    }

    // ------------------------------------------------------------------ view

    fn view(&self) -> Element<'_, Message> {
        let body = match self.screen {
            Screen::Home => self.home_view(),
            Screen::Browse => self.browse_view(),
            Screen::Detail => self.detail_view(),
            Screen::Player => self.player_view(),
            Screen::Settings => self.settings_view(),
        };
        container(
            column![self.header(), body, self.footer()]
                .spacing(design::tokens().spacing.content_pad),
        )
        .style(|_theme| design::page_style())
        .padding(design::tokens().spacing.shell_pad)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    /// The persistent top bar: where you are, what is playing locally, and the
    /// two facts a user must be able to check (library file, token provenance).
    fn header(&self) -> Element<'_, Message> {
        let tokens = design::tokens();
        let capability = if self.capability.available {
            format!(
                "libmpv {}",
                self.capability.api_version.as_deref().unwrap_or("?")
            )
        } else {
            format!("no playback — {}", self.capability.reason)
        };

        let mut nav = row![
            button(text("Home").size(tokens.type_scale.label))
                .on_press(Message::GoHome)
                .style(design::quiet_style(self.screen == Screen::Home))
                .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
            button(text("Library").size(tokens.type_scale.label))
                .on_press(Message::GoBrowse)
                .style(design::quiet_style(self.screen == Screen::Browse))
                .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
            button(text("Settings").size(tokens.type_scale.label))
                .on_press(Message::GoSettings)
                .style(design::quiet_style(self.screen == Screen::Settings))
                .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
        ]
        .spacing(tokens.spacing.field_x)
        .align_y(Alignment::Center);

        // The Home button already says where you are; the label only earns its
        // place when it carries counts the buttons cannot.
        if self.screen != Screen::Home {
            nav = nav.push(
                text(self.screen_label())
                    .size(tokens.type_scale.caption)
                    .color(tokens.palette.ink_muted),
            );
        }

        if self.scan_token.is_some() {
            nav = nav.push(
                button(text("Cancel scan").size(tokens.type_scale.caption))
                    .on_press(Message::CancelScan)
                    .style(design::quiet_style(false))
                    .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
            );
        }

        let facts = column![
            text(capability).size(tokens.type_scale.micro),
            text(format!("library {}", self.library_label)).size(tokens.type_scale.micro),
            text(format!("theme {}", design::provenance())).size(tokens.type_scale.micro),
        ]
        .spacing(0.0);

        container(
            row![
                text("Screenroom").size(tokens.type_scale.subtitle),
                nav,
                Space::new().width(Length::Fill),
                button(text("Refresh").size(tokens.type_scale.caption))
                    .on_press(Message::Refresh)
                    .style(design::quiet_style(false))
                    .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
                button(text("Close").size(tokens.type_scale.caption))
                    .on_press(Message::Close)
                    .style(design::quiet_style(false))
                    .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
                facts,
            ]
            .spacing(tokens.spacing.field_x)
            .align_y(Alignment::Center),
        )
        .style(|_theme| design::container_style(Level::Panel, false))
        .padding(tokens.spacing.content_pad)
        .width(Length::Fill)
        .into()
    }

    fn screen_label(&self) -> String {
        match self.screen {
            Screen::Home => "Home".to_string(),
            Screen::Browse => format!(
                "Library · {} item(s) · page {}/{}",
                self.browse.total,
                self.browse.page + 1,
                self.browse.page_count()
            ),
            Screen::Detail => "Details".to_string(),
            Screen::Player => "Player".to_string(),
            Screen::Settings => "Settings".to_string(),
        }
    }

    /// A status/error strip plus the keyboard legend, so navigation is
    /// discoverable without a mouse.
    fn footer(&self) -> Element<'_, Message> {
        let tokens = design::tokens();
        let line = if let Some(error) = &self.error {
            text(error.clone())
                .size(tokens.type_scale.caption)
                .color(tokens.palette.destructive)
        } else if let Some(status) = &self.status {
            text(status.clone()).size(tokens.type_scale.caption)
        } else {
            text("").size(tokens.type_scale.caption)
        };
        let hint = match self.screen {
            Screen::Player => {
                "Space play/pause · ←/→ seek 10s · s skip segment · i intro mark · c credits mark · Esc back"
            }
            Screen::Detail => "Enter play · f favourite · w watchlist · m watched · Esc back",
            Screen::Settings => "Esc back · ⌘1 Home · ⌘2 Library",
            _ => "Arrows move · Enter opens · ⌘1 Home · ⌘2 Library · ⌘3 Settings · Esc back",
        };
        container(
            row![
                line,
                Space::new().width(Length::Fill),
                text(hint)
                    .size(tokens.type_scale.micro)
                    .color(tokens.palette.ink_muted),
            ]
            .spacing(tokens.spacing.field_x)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .into()
    }

    // ------------------------------------------------------------------ Home

    fn home_view(&self) -> Element<'_, Message> {
        let tokens = design::tokens();
        let mut body = column![]
            .spacing(tokens.spacing.content_pad)
            .width(Length::Fill);

        // Suggestions first, because for a new library they are the only thing
        // there is to do; Continue watching takes the cursor positions after them.
        if !self.suggestions.is_empty() {
            body = body.push(text("Suggested for you").size(tokens.type_scale.title_sm));
            body = body.push(self.suggestion_grid(0));
        }

        body = body.push(text("Continue watching").size(tokens.type_scale.title_sm));
        if self.home.is_empty() {
            body = body.push(self.empty_note(
                "Nothing in progress.",
                "Start something from the library and its position appears here.",
            ));
        } else {
            body = body.push(self.card_grid(&self.home, self.suggestions.len()));
        }

        body = body.push(text("Your library").size(tokens.type_scale.subtitle));
        body = body.push(self.add_folder_row());

        scrollable(body).height(Length::Fill).into()
    }

    /// The folder-granting control. Without it an empty library has no way out.
    fn add_folder_row(&self) -> Element<'_, Message> {
        let tokens = design::tokens();
        container(
            column![
                text("Add a folder of your own media").size(tokens.type_scale.label),
                row![
                    text_input("Absolute folder path", &self.new_root)
                        .on_input(Message::NewRootChanged)
                        .on_submit(Message::AddFolder)
                        .style(design::input_style())
                        .padding(tokens.spacing.field_y)
                        .width(Length::Fill),
                    button(text("Add and scan").size(tokens.type_scale.label))
                        .on_press(Message::AddFolder)
                        .style(design::action_style(false))
                        .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
                ]
                .spacing(tokens.spacing.field_x)
                .align_y(Alignment::Center),
                text("Files are read in place and never moved, renamed or deleted.")
                    .size(tokens.type_scale.micro)
                    .color(tokens.palette.ink_muted),
                if self.artwork_failures > 0 {
                    text(format!(
                        "{} title(s) had no readable frame, so they keep a title-only card.",
                        self.artwork_failures
                    ))
                    .size(tokens.type_scale.micro)
                    .color(tokens.palette.destructive)
                } else {
                    text(String::new()).size(tokens.type_scale.micro)
                },
            ]
            .spacing(tokens.spacing.field_y),
        )
        .style(|_theme| design::container_style(Level::Panel, false))
        .padding(tokens.spacing.content_pad)
        .width(Length::Fill)
        .into()
    }

    // -------------------------------------------------------------- Browse

    fn browse_view(&self) -> Element<'_, Message> {
        let tokens = design::tokens();
        let filters = column![
            row![
                text_input("Search titles and filenames", &self.browse.text)
                    .on_input(Message::SearchChanged)
                    .style(design::input_style())
                    .padding(tokens.spacing.field_y)
                    .width(Length::Fill),
                pick_list(
                    [KindFilter::All, KindFilter::Movies, KindFilter::Series],
                    Some(self.browse.kind),
                    Message::KindChanged,
                ),
                pick_list(
                    SortChoice::ALL,
                    Some(self.browse.sort),
                    Message::SortChanged
                ),
                pick_list(
                    self.browse.genres.clone(),
                    self.browse.genre.clone(),
                    |genre: String| Message::GenreChanged(Some(genre)),
                )
                .placeholder("Any genre"),
            ]
            .spacing(tokens.spacing.field_x)
            .align_y(Alignment::Center),
            row![
                checkbox(self.browse.watchlist_only)
                    .label("Watchlist")
                    .on_toggle(Message::ToggleWatchlistOnly),
                checkbox(self.browse.favorites_only)
                    .label("Favourites")
                    .on_toggle(Message::ToggleFavoritesOnly),
                checkbox(self.browse.available_only)
                    .label("Available only")
                    .on_toggle(Message::ToggleAvailableOnly),
                Space::new().width(Length::Fill),
                button(text("Previous").size(tokens.type_scale.caption))
                    .on_press_maybe((self.browse.page > 0).then_some(Message::PreviousPage))
                    .style(design::quiet_style(false))
                    .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
                text(format!(
                    "page {} / {}",
                    self.browse.page + 1,
                    self.browse.page_count()
                ))
                .size(tokens.type_scale.caption),
                button(text("Next").size(tokens.type_scale.caption))
                    .on_press_maybe(
                        (self.browse.page + 1 < self.browse.page_count())
                            .then_some(Message::NextPage),
                    )
                    .style(design::quiet_style(false))
                    .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
            ]
            .spacing(tokens.spacing.field_x)
            .align_y(Alignment::Center),
        ]
        .spacing(tokens.spacing.field_y);

        let body: Element<'_, Message> = if self.browse.total == 0 {
            let (title, detail) =
                browse_empty_copy(!self.sources.is_empty(), self.browse.narrowed());
            self.empty_note(title, detail)
        } else {
            self.card_grid(&self.browse.items, 0)
        };

        scrollable(
            column![filters, body]
                .spacing(tokens.spacing.content_pad)
                .width(Length::Fill),
        )
        .height(Length::Fill)
        .into()
    }

    // -------------------------------------------------------------- Detail

    fn detail_view(&self) -> Element<'_, Message> {
        let tokens = design::tokens();
        let Some(detail) = &self.detail else {
            return self.empty_note("Nothing selected.", "Open something from the library.");
        };
        let item = &detail.item;

        let facts = [
            item.kind.as_str().to_string(),
            item.year.map(|year| year.to_string()).unwrap_or_default(),
            item.episode_label(),
            if item.genres.is_empty() {
                String::new()
            } else {
                item.genres.join(", ")
            },
        ]
        .into_iter()
        .filter(|fact| !fact.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");

        let progress = if item.duration_ms > 0 {
            format!(
                "{} / {}",
                clock(item.position_ms as f64 / 1_000.0),
                clock(item.duration_ms as f64 / 1_000.0)
            )
        } else {
            "duration reported by the player on open".to_string()
        };

        let actions = row![
            if item.position_ms > 0 {
                button(text("Resume").size(tokens.type_scale.label))
                    .on_press_maybe(item.available.then_some(Message::Play(item.id.clone())))
                    .style(design::action_style(true))
                    .padding([tokens.spacing.field_y, tokens.spacing.field_x])
            } else {
                button(text("Play").size(tokens.type_scale.label))
                    .on_press_maybe(item.available.then_some(Message::Play(item.id.clone())))
                    .style(design::action_style(true))
                    .padding([tokens.spacing.field_y, tokens.spacing.field_x])
            },
            button(text("Start over").size(tokens.type_scale.label))
                .on_press_maybe(item.available.then_some(Message::Restart(item.id.clone())))
                .style(design::quiet_style(false))
                .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
            button(
                text(if item.favorite {
                    "Favourited"
                } else {
                    "Favourite"
                })
                .size(tokens.type_scale.label)
            )
            .on_press(Message::ToggleFavorite(item.work_id.clone()))
            .style(design::quiet_style(false))
            .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
            button(
                text(if item.watchlisted {
                    "On watchlist"
                } else {
                    "Watchlist"
                })
                .size(tokens.type_scale.label)
            )
            .on_press(Message::ToggleWatchlist(item.work_id.clone()))
            .style(design::quiet_style(false))
            .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
            button(
                text(if item.watched {
                    "Watched"
                } else {
                    "Mark watched"
                })
                .size(tokens.type_scale.label)
            )
            .on_press(Message::ToggleWatched(item.edition_id.clone()))
            .style(design::quiet_style(false))
            .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
            button(text("Hide from Continue").size(tokens.type_scale.label))
                .on_press(Message::HideFromContinue(item.edition_id.clone()))
                .style(design::quiet_style(false))
                .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
        ]
        .spacing(tokens.spacing.field_x)
        .align_y(Alignment::Center);

        let mut body = column![
            text(item.title.clone()).size(tokens.type_scale.title),
            text(facts)
                .size(tokens.type_scale.caption)
                .color(tokens.palette.ink_muted),
            text(progress).size(tokens.type_scale.label),
            actions,
        ]
        .spacing(tokens.spacing.field_y)
        .width(Length::Fill);

        if item.overview.trim().is_empty() {
            body = body.push(
                text("No synopsis stored for this title yet.")
                    .size(tokens.type_scale.caption)
                    .color(tokens.palette.ink_muted),
            );
        } else {
            body = body.push(text(item.overview.clone()).size(tokens.type_scale.body));
        }
        body = body.push(
            text(item.relative_path.clone())
                .size(tokens.type_scale.micro)
                .color(tokens.palette.ink_muted),
        );
        if !item.available {
            body = body.push(
                text("This file is not readable right now. Relink its source and rescan.")
                    .size(tokens.type_scale.label)
                    .color(tokens.palette.destructive),
            );
        }

        if item.kind == MediaKind::Series {
            body = body.push(
                text(if detail.seasons.is_empty() {
                    "Episodes".to_string()
                } else {
                    format!(
                        "Seasons {} · {} episode(s) held",
                        detail
                            .seasons
                            .iter()
                            .map(|season| season.to_string())
                            .collect::<Vec<_>>()
                            .join(", "),
                        detail.episodes.len()
                    )
                })
                .size(tokens.type_scale.subtitle),
            );
            if detail.episodes.is_empty() {
                body = body.push(self.empty_note(
                    "No other episodes of this series are in the library.",
                    "Add the folder that holds them and rescan.",
                ));
            } else {
                body = body.push(self.episode_list(&detail.episodes));
            }
        }

        // The poster sits beside the facts, not above them: the title, the actions
        // and the synopsis are what a viewer reads, and a large image would push
        // them below the fold on a short window.
        let details: Element<'_, Message> = match self.artwork.get(&item.work_id) {
            Some(handle) => row![
                image(handle.clone())
                    .width(Length::Fixed(DETAIL_POSTER.0))
                    .height(Length::Fixed(DETAIL_POSTER.1))
                    .content_fit(ContentFit::Cover),
                body,
            ]
            .spacing(tokens.spacing.content_pad)
            .align_y(Alignment::Start)
            .into(),
            None => body.into(),
        };

        scrollable(details).height(Length::Fill).into()
    }

    /// A compact one-per-line episode list; the arrow keys walk it.
    fn episode_list(&self, episodes: &[LibraryItem]) -> Element<'_, Message> {
        let tokens = design::tokens();
        let mut list = column![].spacing(2.0).width(Length::Fill);
        for (index, episode) in episodes.iter().enumerate() {
            let label = episode.episode_label();
            let heading = if label.is_empty() {
                episode.title.clone()
            } else {
                format!("{label}  {}", episode.title)
            };
            let progress = if episode.duration_ms > 0 && episode.position_ms > 0 {
                format!(
                    "  {} / {}",
                    clock(episode.position_ms as f64 / 1_000.0),
                    clock(episode.duration_ms as f64 / 1_000.0)
                )
            } else {
                String::new()
            };
            let suffix = if episode.watched {
                "  · watched"
            } else if !episode.available {
                "  · unavailable"
            } else {
                ""
            };
            list = list.push(
                button(text(format!("{heading}{progress}{suffix}")).size(tokens.type_scale.label))
                    .on_press(Message::Play(episode.id.clone()))
                    .style(design::tile_style(self.focus == index))
                    .width(Length::Fill)
                    .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
            );
        }
        scrollable(list).height(Length::Fixed(240.0)).into()
    }

    // ------------------------------------------------------------- shared UI

    /// A bounded grid of cards, where `base` is the focus index of the first card.
    fn card_grid(&self, items: &[LibraryItem], base: usize) -> Element<'_, Message> {
        let tokens = design::tokens();
        // Computed once, so the row chunking, the focus index and the padding all
        // agree even if a resize event arrives mid-view.
        let columns = self.columns();
        let mut grid = column![]
            .spacing(tokens.spacing.field_x)
            .width(Length::Fill);
        for (row_index, chunk) in items.chunks(columns).enumerate() {
            let mut cells = row![].spacing(tokens.spacing.field_x).width(Length::Fill);
            for (column_index, item) in chunk.iter().enumerate() {
                let index = base + row_index * columns + column_index;
                cells = cells.push(self.card(item, self.focus == index));
            }
            // Pad the final row so every card keeps the same width. A page is a fixed
            // 24 items, so with an odd column count the last row is genuinely short:
            // padding is what stops those cards stretching to fill it.
            for _ in chunk.len()..columns {
                cells = cells.push(Space::new().width(Length::FillPortion(1)));
            }
            grid = grid.push(cells);
        }
        grid.into()
    }

    /// Cards per row for the window currently open.
    fn columns(&self) -> usize {
        grid_columns(self.window_width)
    }

    /// One card: retained poster when there is one, then title and state.
    ///
    /// No placeholder image is drawn for a work that has none. A generic poster
    /// would claim to be this title's artwork, and a title-only card is the
    /// truthful version of "no artwork was found".
    fn card(&self, item: &LibraryItem, focused: bool) -> Element<'_, Message> {
        self.card_with(item, focused, None)
    }

    /// A card with an optional line saying why it is being shown.
    fn card_with(
        &self,
        item: &LibraryItem,
        focused: bool,
        note: Option<&str>,
    ) -> Element<'_, Message> {
        let tokens = design::tokens();
        let handle = self.artwork.get(&item.work_id);
        let title = truncate(&item.title, 34);
        let subtitle = {
            let label = item.episode_label();
            match (label.is_empty(), item.year) {
                (false, _) => label,
                (true, Some(year)) => year.to_string(),
                (true, None) => item.kind.as_str().to_string(),
            }
        };
        let position = if item.duration_ms > 0 && item.position_ms > 0 {
            let ratio = (item.position_ms as f32 / item.duration_ms as f32).clamp(0.0, 1.0);
            format!(
                "{:.0}% · {}",
                ratio * 100.0,
                clock(item.position_ms as f64 / 1_000.0)
            )
        } else if item.watched {
            "watched".to_string()
        } else if !item.available {
            "unavailable".to_string()
        } else {
            "not started".to_string()
        };

        let mut content = column![].spacing(2.0);
        if let Some(handle) = handle {
            content = content.push(
                image(handle.clone())
                    .width(Length::Fill)
                    .height(Length::Fixed(CARD_POSTER_HEIGHT))
                    .content_fit(ContentFit::Cover),
            );
            content = content.push(Space::new().height(Length::Fixed(4.0)));
        }
        content = content
            .push(text(title).size(tokens.type_scale.label))
            .push(
                text(subtitle)
                    .size(tokens.type_scale.micro)
                    .color(tokens.palette.ink_muted),
            )
            .push(Space::new().height(Length::Fixed(4.0)))
            .push(
                text(position)
                    .size(tokens.type_scale.micro)
                    .color(tokens.palette.ink_muted),
            )
            .push(
                text(truncate(&item.filename, 30))
                    .size(tokens.type_scale.micro)
                    .color(tokens.palette.ink_muted),
            );
        if let Some(note) = note {
            content = content.push(
                text(truncate(note, 34))
                    .size(tokens.type_scale.micro)
                    .color(tokens.palette.signal),
            );
        }

        let mut height = 132.0;
        if handle.is_some() {
            height += CARD_POSTER_HEIGHT + 4.0;
        }
        if note.is_some() {
            height += 18.0;
        }

        button(content)
            .on_press(Message::OpenDetail(item.id.clone()))
            .style(design::tile_style(focused))
            .width(Length::FillPortion(1))
            .height(Length::Fixed(height))
            .padding(tokens.spacing.field_y)
            .into()
    }

    /// The suggestion grid, which says why each card is offered.
    fn suggestion_grid(&self, base: usize) -> Element<'_, Message> {
        let tokens = design::tokens();
        let columns = self.columns();
        let mut grid = column![]
            .spacing(tokens.spacing.field_x)
            .width(Length::Fill);
        for (row_index, chunk) in self.suggestions.chunks(columns).enumerate() {
            let mut cells = row![].spacing(tokens.spacing.field_x).width(Length::Fill);
            for (column_index, suggestion) in chunk.iter().enumerate() {
                let index = base + row_index * columns + column_index;
                cells = cells.push(self.card_with(
                    &suggestion.item,
                    self.focus == index,
                    Some(&suggestion.reason),
                ));
            }
            for _ in chunk.len()..columns {
                cells = cells.push(Space::new().width(Length::FillPortion(1)));
            }
            grid = grid.push(cells);
        }
        grid.into()
    }

    /// An honest empty/blocked state: a title and the concrete next action.
    fn empty_note(&self, title: &str, detail: &str) -> Element<'_, Message> {
        let tokens = design::tokens();
        container(
            column![
                text(title.to_string()).size(tokens.type_scale.label),
                text(detail.to_string())
                    .size(tokens.type_scale.caption)
                    .color(tokens.palette.ink_muted),
            ]
            .spacing(2.0),
        )
        .style(|_theme| design::container_style(Level::Panel, false))
        .padding(tokens.spacing.content_pad)
        .width(Length::Fill)
        .into()
    }

    // ------------------------------------------------------------- Player

    // -------------------------------------------------------------- Settings

    /// Viewer, playback, folder and data settings.
    ///
    /// Every control here performs the action it names: there is no control that
    /// only looks enabled. Where an action is destructive — removing a folder,
    /// restoring a backup — the first press states what will happen and the second
    /// carries it out, which is the honest form of a confirmation when the app has
    /// no separate dialog chrome.
    fn settings_view(&self) -> Element<'_, Message> {
        let tokens = design::tokens();
        let heading = settings_heading;
        let note = settings_note;
        let panel = settings_panel;

        // --- viewer -------------------------------------------------------
        let mut viewers = column![].spacing(tokens.spacing.field_y);
        for profile in &self.profiles {
            let active = profile.id == self.profile_id;
            let mut row: iced::widget::Row<'_, Message> =
                row![text(profile.name.clone()).size(tokens.type_scale.body),]
                    .spacing(tokens.spacing.field_x)
                    .align_y(Alignment::Center);
            if active {
                row = row.push(
                    text("active".to_string())
                        .size(tokens.type_scale.micro)
                        .color(tokens.palette.signal),
                );
            } else {
                row = row.push(
                    button(text("Use").size(tokens.type_scale.label))
                        .on_press(Message::ProfileChosen(profile.id.clone()))
                        .style(design::action_style(false))
                        .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
                );
            }
            // The last profile cannot be removed, so the control is absent rather
            // than present and failing.
            if self.profiles.len() > 1 {
                row = row.push(
                    button(text("Remove").size(tokens.type_scale.label))
                        .on_press(Message::RemoveProfile(profile.id.clone()))
                        .style(design::danger_style(false))
                        .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
                );
            }
            viewers = viewers.push(row);
        }
        viewers = viewers
            .push(Space::new().height(tokens.spacing.field_y))
            .push(
                row![
                    text_input("New profile name", &self.new_profile)
                        .on_input(Message::NewProfileChanged)
                        .on_submit(Message::AddProfile)
                        .style(design::input_style())
                        .padding(tokens.spacing.field_y)
                        .width(220.0),
                    button(text("Add profile").size(tokens.type_scale.label))
                        .on_press(Message::AddProfile)
                        .style(design::action_style(false))
                        .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
                ]
                .spacing(tokens.spacing.field_x)
                .align_y(Alignment::Center),
            )
            .push(note(
                "Each profile keeps its own progress, favourites and watchlist.",
            ));

        // --- playback -----------------------------------------------------
        let playback = column![
            checkbox(self.preferences.autoplay_next)
                .label("Play the next episode automatically")
                .on_toggle(Message::ToggleAutoplay)
                .size(18.0),
            note(
                "A gap in a season stops the chain: the next episode is never skipped guessing.",
            ),
            checkbox(self.preferences.auto_skip_intro)
                .label("Skip marked intros automatically")
                .on_toggle(Message::ToggleAutoSkip)
                .size(18.0),
            note("Only a segment you marked is skipped; an intro is never guessed."),
            row![
                text("Subtitle language".to_string()).size(tokens.type_scale.label),
                text_input("e.g. en", &self.subtitle_input)
                    .on_input(Message::SubtitleLanguageChanged)
                    .on_submit(Message::ApplySubtitleLanguage)
                    .style(design::input_style())
                    .padding(tokens.spacing.field_y)
                    .width(140.0),
                button(text("Apply").size(tokens.type_scale.label))
                    .on_press(Message::ApplySubtitleLanguage)
                    .style(design::quiet_style(false))
                    .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
            ]
            .spacing(tokens.spacing.field_x)
            .align_y(Alignment::Center),
            row![
                text("Audio language".to_string()).size(tokens.type_scale.label),
                text_input("e.g. en", &self.audio_input)
                    .on_input(Message::AudioLanguageChanged)
                    .on_submit(Message::ApplyAudioLanguage)
                    .style(design::input_style())
                    .padding(tokens.spacing.field_y)
                    .width(140.0),
                button(text("Apply").size(tokens.type_scale.label))
                    .on_press(Message::ApplyAudioLanguage)
                    .style(design::quiet_style(false))
                    .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
            ]
            .spacing(tokens.spacing.field_x)
            .align_y(Alignment::Center),
            row![
                text("Subtitle size".to_string()).size(tokens.type_scale.label),
                slider(
                    f64::from(Preferences::SUBTITLE_SCALE_MIN_PERCENT)
                        ..=f64::from(Preferences::SUBTITLE_SCALE_MAX_PERCENT),
                    f64::from(self.preferences.subtitle_scale_percent),
                    Message::SubtitleScale,
                )
                .step(5.0)
                .width(220.0),
                text(format!("{}%", self.preferences.subtitle_scale_percent))
                    .size(tokens.type_scale.caption)
                    .color(tokens.palette.ink_muted),
            ]
            .spacing(tokens.spacing.field_x)
            .align_y(Alignment::Center),
            row![
                text("Subtitle position".to_string()).size(tokens.type_scale.label),
                slider(
                    f64::from(Preferences::SUBTITLE_POSITION_MIN_PERCENT)
                        ..=f64::from(Preferences::SUBTITLE_POSITION_MAX_PERCENT),
                    f64::from(self.preferences.subtitle_position_percent),
                    Message::SubtitlePosition,
                )
                .step(1.0)
                .width(220.0),
                text(format!("{}%", self.preferences.subtitle_position_percent))
                    .size(tokens.type_scale.caption)
                    .color(tokens.palette.ink_muted),
            ]
            .spacing(tokens.spacing.field_x)
            .align_y(Alignment::Center),
            note(
                "Position is measured from the top of the picture, so 100% is the bottom. \
                 A subtitle file that places its own lines keeps that placement; the \
                 size still applies.",
            ),
            note(
                "Press Enter to apply. Leave a field empty and press Enter to let the player choose.",
            ),
        ]
        .spacing(tokens.spacing.field_y);

        // --- folders ------------------------------------------------------
        let mut folders = column![].spacing(tokens.spacing.field_y);
        if self.sources.is_empty() {
            folders = folders.push(note(
                "No folders yet. Grant one below and Screenroom scans it in place.",
            ));
        }
        for source in &self.sources {
            let armed = self.armed_remove.as_deref() == Some(source.id.as_str());
            folders = folders
                .push(
                    column![
                        text(source.label.clone()).size(tokens.type_scale.body),
                        text(source.root_path.clone())
                            .size(tokens.type_scale.micro)
                            .color(tokens.palette.ink_muted),
                        note(source_state_label(source.state)),
                    ]
                    .spacing(tokens.spacing.field_y / 2.0),
                )
                .push(
                    row![
                        button(text("Rescan").size(tokens.type_scale.label))
                            .on_press(Message::RescanSource(source.id.clone()))
                            .style(design::quiet_style(false))
                            .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
                        button(
                            text(if armed {
                                "Press again to remove"
                            } else {
                                "Remove"
                            })
                            .size(tokens.type_scale.label)
                        )
                        .on_press(Message::RemoveSource(source.id.clone()))
                        .style(design::danger_style(armed))
                        .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
                    ]
                    .spacing(tokens.spacing.field_x)
                    .align_y(Alignment::Center),
                );
        }
        folders = folders.push(text("Grant a folder".to_string()).size(tokens.type_scale.label));
        folders = folders.push(self.add_folder_row());

        if !self.sources.is_empty() {
            let choices: Vec<SourceChoice> = self
                .sources
                .iter()
                .map(|source| SourceChoice {
                    id: source.id.clone(),
                    label: source.label.clone(),
                })
                .collect();
            let chosen = self
                .relink_target
                .as_ref()
                .and_then(|id| choices.iter().find(|choice| &choice.id == id).cloned());
            folders = folders
                .push(Space::new().height(tokens.spacing.field_y))
                .push(
                    text("Relink a folder".to_string()).size(tokens.type_scale.label),
                )
                .push(
                    row![
                        pick_list(choices, chosen, |choice: SourceChoice| {
                            Message::RelinkTargetChosen(choice.id)
                        })
                        // Without a placeholder the control is blank until a folder
                        // is chosen, which reads as a broken control rather than
                        // as one waiting for a choice.
                        .placeholder("Choose a folder"),
                        text_input("New location, for example /Volumes/Media", &self.relink_path)
                            .on_input(Message::RelinkPathChanged)
                            .on_submit(Message::RelinkSource)
                            .style(design::input_style())
                            .padding(tokens.spacing.field_y)
                            .width(Length::Fill),
                        button(text("Relink and rescan").size(tokens.type_scale.label))
                            .on_press(Message::RelinkSource)
                            .style(design::action_style(false))
                            .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
                    ]
                    .spacing(tokens.spacing.field_x)
                    .align_y(Alignment::Center),
                )
                .push(note(
                    "Use this when a removable drive mounts somewhere new. Nothing on disk is changed.",
                ));
        }

        // --- data ---------------------------------------------------------
        let restore_armed = self.armed_restore.is_some();
        let data = column![
            text("Back up".to_string()).size(tokens.type_scale.label),
            row![
                text_input("Backup file path", &self.backup_path)
                    .on_input(Message::BackupPathChanged)
                    .on_submit(Message::BackupNow)
                    .style(design::input_style())
                    .padding(tokens.spacing.field_y)
                    .width(Length::Fill),
                button(text("Back up now").size(tokens.type_scale.label))
                    .on_press(Message::BackupNow)
                    .style(design::action_style(false))
                    .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
            ]
            .spacing(tokens.spacing.field_x)
            .align_y(Alignment::Center),
            Space::new().height(tokens.spacing.field_y),
            text("Restore".to_string()).size(tokens.type_scale.label),
            row![
                text_input("Backup file to restore from", &self.restore_path)
                    .on_input(Message::RestorePathChanged)
                    .on_submit(Message::InspectRestore)
                    .style(design::input_style())
                    .padding(tokens.spacing.field_y)
                    .width(Length::Fill),
                button(text("Check file").size(tokens.type_scale.label))
                    .on_press(Message::InspectRestore)
                    .style(design::quiet_style(false))
                    .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
                button(
                    text(if restore_armed {
                        "Press again to restore"
                    } else {
                        "Restore"
                    })
                    .size(tokens.type_scale.label)
                )
                .on_press(Message::RestoreNow)
                .style(design::danger_style(restore_armed))
                .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
            ]
            .spacing(tokens.spacing.field_x)
            .align_y(Alignment::Center),
            note(
                "Restoring replaces profiles, progress and lists with the backup's. Your current \
                 library is copied to a .before-restore file first.",
            ),
        ]
        .spacing(tokens.spacing.field_y);

        let body = column![
            heading("Viewer"),
            panel(viewers.into()),
            heading("Playback"),
            panel(playback.into()),
            heading("Folders"),
            panel(folders.into()),
            heading("Data"),
            panel(data.into()),
            Space::new().height(tokens.spacing.content_pad),
        ]
        .spacing(tokens.spacing.field_x);

        scrollable(body)
            .height(Length::Fill)
            .width(Length::Fill)
            .into()
    }

    fn player_view(&self) -> Element<'_, Message> {
        let tokens = design::tokens();
        let title = self
            .active
            .as_ref()
            .map(|active| active.title.clone())
            .unwrap_or_else(|| "Player".to_string());
        let state = match (&self.snapshot.failure, self.snapshot.state) {
            (Some(failure), _) => format!("{}: {}", failure.code, failure.message),
            (None, state) => format!("{state:?}"),
        };

        let header = row![
            button(text("Back").size(tokens.type_scale.label))
                .on_press(Message::BackFromPlayer)
                .style(design::quiet_style(false))
                .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
            text(title).size(tokens.type_scale.subtitle),
            text(state)
                .size(tokens.type_scale.micro)
                .color(tokens.palette.ink_muted),
        ]
        .spacing(tokens.spacing.field_x)
        .align_y(Alignment::Center);

        let surface: Element<'_, Message> = match (&self.snapshot.failure, &self.frame) {
            (Some(failure), _) => container(
                text(format!("Playback failed — {}", failure.message))
                    .size(tokens.type_scale.body)
                    .color(tokens.palette.destructive),
            )
            .style(|_theme| design::container_style(Level::Panel, false))
            .padding(tokens.spacing.content_pad)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into(),
            (None, Some(frame)) => shader::Shader::new(VideoSurface::new(Some(frame.clone())))
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            (None, None) => container(
                text("Preparing streams…")
                    .size(tokens.type_scale.body)
                    .color(tokens.palette.ink_muted),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into(),
        };

        let duration = self.snapshot.duration.max(0.0);
        let position = self.scrub.unwrap_or(self.snapshot.position);
        let controls = row![
            button(text(play_action(self.snapshot.state).label()).size(tokens.type_scale.label))
                .on_press(Message::TogglePlay)
                .style(design::action_style(true))
                .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
            text(clock(position)).size(tokens.type_scale.caption),
            slider(0.0..=duration.max(1.0), position, Message::Scrub)
                .on_release(Message::SeekCommit)
                .step(0.1)
                .width(Length::Fill),
            text(clock(duration)).size(tokens.type_scale.caption),
        ]
        .spacing(tokens.spacing.field_x)
        .align_y(Alignment::Center);
        // The skip button appears only when a stored marker actually covers the
        // playhead: an always-visible button would claim a segment the library
        // does not know about.
        let mut segment_row: iced::widget::Row<'_, Message> = row![]
            .spacing(tokens.spacing.field_x)
            .align_y(Alignment::Center);
        if let Some(marker) = self.active_marker() {
            let label = if marker.kind == "intro" {
                "Skip intro"
            } else {
                "Skip credits"
            };
            segment_row = segment_row.push(
                button(text(label).size(tokens.type_scale.label))
                    .on_press(Message::SkipSegment)
                    .style(design::action_style(true))
                    .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
            );
        }
        segment_row = segment_row.push(
            button(
                text(match &self.mark_pending {
                    Some((kind, start)) => {
                        format!("End {kind} at {}", clock(*start as f64 / 1_000.0))
                    }
                    None => "Mark intro start".to_string(),
                })
                .size(tokens.type_scale.label),
            )
            .on_press(Message::MarkSegment("intro".to_string()))
            .style(design::quiet_style(false))
            .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
        );
        segment_row = segment_row.push(
            button(text("Mark credits start").size(tokens.type_scale.label))
                .on_press(Message::MarkSegment("credits".to_string()))
                .style(design::quiet_style(false))
                .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
        );
        if self.mark_pending.is_some() {
            segment_row = segment_row.push(
                button(text("Finish segment").size(tokens.type_scale.label))
                    .on_press(Message::FinishSegment)
                    .style(design::action_style(false))
                    .padding([tokens.spacing.field_y, tokens.spacing.field_x]),
            );
        }
        let marked = self
            .markers
            .iter()
            .map(|marker| {
                format!(
                    "{} {}–{}",
                    marker.kind,
                    clock(marker.start_ms as f64 / 1_000.0),
                    clock(marker.end_ms as f64 / 1_000.0)
                )
            })
            .collect::<Vec<_>>();
        let segment_note = if marked.is_empty() {
            "No segments stored for this file. Pin them while watching; nothing is guessed."
                .to_string()
        } else {
            marked.join(" · ")
        };
        let segments = column![
            segment_row,
            text(segment_note)
                .size(tokens.type_scale.micro)
                .color(tokens.palette.ink_muted),
        ]
        .spacing(tokens.spacing.field_y);

        let (audio_choices, audio_selected) = self.track_chooser(TrackKind::Audio);
        let (subtitle_choices, subtitle_selected) = self.track_chooser(TrackKind::Subtitle);
        // A picker with nothing to pick is a dead control, so each kind renders
        // as a chooser only when the player actually reported tracks of it. The
        // other kind says so in words instead of showing an inert dropdown.
        let reported_any = !audio_choices.is_empty() || !subtitle_choices.is_empty();
        let mut track_row: iced::widget::Row<'_, Message> = row![]
            .spacing(tokens.spacing.field_x)
            .align_y(Alignment::Center);
        for (label, choices, selected, kind) in [
            ("Audio", audio_choices, audio_selected, TrackKind::Audio),
            (
                "Subtitles",
                subtitle_choices,
                subtitle_selected,
                TrackKind::Subtitle,
            ),
        ] {
            track_row = track_row.push(
                text(label)
                    .size(tokens.type_scale.micro)
                    .color(tokens.palette.ink_muted),
            );
            track_row = if choices.is_empty() {
                track_row.push(
                    text("none in this file")
                        .size(tokens.type_scale.micro)
                        .color(tokens.palette.disabled_foreground),
                )
            } else {
                track_row.push(
                    pick_list(choices, selected, move |choice: TrackChoice| {
                        Message::SelectTrack(kind, choice.id)
                    })
                    .placeholder(label),
                )
            };
        }
        let track_row: Element<'_, Message> = if !reported_any {
            text("The player reported no selectable audio or subtitle tracks for this file.")
                .size(tokens.type_scale.micro)
                .color(tokens.palette.ink_muted)
                .into()
        } else {
            track_row.into()
        };

        let resume = self
            .active
            .as_ref()
            // The stored position no longer describes anything once the file has
            // finished, so the line is dropped rather than left claiming a resume
            // beside an Ended player.
            .filter(|active| active.resume_from_ms > 0 && !self.snapshot.ended)
            .map(|active| {
                text(format!(
                    "resumed from {}",
                    clock(active.resume_from_ms as f64 / 1_000.0)
                ))
                .size(tokens.type_scale.micro)
            })
            .unwrap_or_else(|| text("").size(tokens.type_scale.micro));

        let footer = row![
            resume,
            Space::new().width(Length::Fill),
            text(format!(
                "file {}",
                self.active
                    .as_ref()
                    .map(|active| active.file_id.as_str())
                    .unwrap_or("-")
            ))
            .size(tokens.type_scale.micro)
            .color(tokens.palette.ink_muted),
        ]
        .spacing(tokens.spacing.field_x);

        container(
            column![header, surface, controls, segments, track_row, footer]
                .spacing(tokens.spacing.content_pad),
        )
        .padding(tokens.spacing.content_pad)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

/// A section title on the Settings screen.
fn settings_heading(label: &str) -> Element<'_, Message> {
    text(label.to_string())
        .size(design::tokens().type_scale.subtitle)
        .into()
}

/// An explanatory line under a Settings control.
///
/// These carry the consequences a control cannot state by itself — that a gap
/// stops autoplay, that restore copies the current library aside — so the copy is
/// part of the control's meaning rather than decoration.
fn settings_note(label: &str) -> Element<'_, Message> {
    text(label.to_string())
        .size(design::tokens().type_scale.micro)
        .color(design::tokens().palette.ink_muted)
        .into()
}

/// Wraps one Settings section in the raised panel surface.
fn settings_panel(content: Element<'_, Message>) -> Element<'_, Message> {
    container(content)
        .style(|_theme| design::container_style(Level::Panel, false))
        .padding(design::tokens().spacing.content_pad)
        .width(Length::Fill)
        .into()
}

/// One entry in the folder chooser on the Settings screen.
///
/// Carries the label and the identifier together, so the list shows the user's own
/// folder name while the message that comes back names the folder by its stable
/// id rather than by a display string that could change underneath it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceChoice {
    /// Stable source identifier, used in the message.
    pub id: String,
    /// User-facing folder label, shown in the list.
    pub label: String,
}

impl std::fmt::Display for SourceChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// Plain-language description of a folder's availability.
///
/// Each state names a different recovery, which is why they are not collapsed
/// into "unavailable": a disconnected drive is plugged back in, a revoked grant
/// is granted again, and a missing path is relinked.
fn source_state_label(state: SourceState) -> &'static str {
    match state {
        SourceState::Available => "Available",
        SourceState::Disconnected => {
            "Drive not attached — reconnect it, then Rescan. Your history is kept."
        }
        SourceState::PermissionLost => "Access was withdrawn — grant access to the folder again.",
        SourceState::Missing => "Folder is gone — relink it to its new location.",
        SourceState::Removed => "Removed from the library. Files on disk are untouched.",
    }
}

/// One entry in the player's track chooser.
///
/// `id` is both the identity compared against the player's reported selection and
/// the value handed back when the entry is picked, so "Off" is a real [`NO_TRACK`]
/// selection rather than a separate kind of value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackChoice {
    /// libmpv track id, or [`NO_TRACK`] for the "Off" entry.
    pub id: i64,
    /// The label shown in the list.
    pub label: String,
}

impl TrackChoice {
    /// The entry that switches a kind off.
    fn off() -> Self {
        Self {
            id: NO_TRACK,
            label: "Off".to_string(),
        }
    }

    /// Labels a reported track for the chooser.
    fn from_track(track: &MediaTrack) -> Self {
        Self {
            id: track.id,
            label: track.label(),
        }
    }
}

impl std::fmt::Display for TrackChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// What the player's single transport control does in a given state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayAction {
    /// Stop the picture where it is.
    Pause,
    /// Continue from the paused position.
    Resume,
    /// Start the finished file again from the beginning.
    Replay,
}

impl PlayAction {
    /// The control's label for this action.
    pub fn label(self) -> &'static str {
        match self {
            Self::Pause => "Pause",
            Self::Resume => "Play",
            Self::Replay => "Replay",
        }
    }
}

/// Chooses the transport action for a player state.
///
/// The player's state machine treats `Ended` as terminal — it maps both
/// `(Ended, _)` and `(_, Ended)` to `Ended` — so clearing the pause flag on a
/// finished file is a no-op that would leave the control dead. A finished file
/// gets [`PlayAction::Replay`] instead, which reopens it.
pub fn play_action(state: screenroom_player::PlayerState) -> PlayAction {
    use screenroom_player::PlayerState;
    match state {
        PlayerState::Paused => PlayAction::Resume,
        PlayerState::Ended => PlayAction::Replay,
        _ => PlayAction::Pause,
    }
}

/// A work-level flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flag {
    /// Favourite.
    Favorite,
    /// Watchlist.
    Watchlist,
}

/// Copy for an empty Browse screen: a title and the concrete next action.
///
/// Three genuinely different situations arrive at the same "nothing here" moment,
/// and naming the wrong one sends the viewer somewhere useless: a first run has no
/// filters to change, and a folder that was granted but never read has nothing to
/// change either. Both used to read "Nothing matches. Change the filters", which
/// describes neither.
///
/// `has_sources` means at least one folder is granted; `narrowed` means a search or
/// filter is excluding rows.
pub fn browse_empty_copy(has_sources: bool, narrowed: bool) -> (&'static str, &'static str) {
    match (has_sources, narrowed) {
        (false, _) => (
            "No media yet.",
            "Add a folder on Home and Screenroom reads it where it is — \
             nothing is moved, renamed or copied.",
        ),
        (true, true) => (
            "Nothing matches those filters.",
            "Clear the search or turn the filters off; the folder itself is fine.",
        ),
        (true, false) => (
            "Nothing imported from your folders yet.",
            "Rescan a folder in Settings. If it looks empty, check that the drive \
             is connected.",
        ),
    }
}

/// Cards per grid row for a window this wide, in logical pixels.
///
/// The grid was four fixed columns, which is right for the 1280-wide default window
/// and wrong at both ends of the range this owner actually uses. Four cards across a
/// 3440px ultrawide are ~860px each, so a poster grid reads as four billboards; at a
/// narrow window they are cramped. The count follows a comfortable card width instead.
///
/// A pure function of the width, so the layout at 3440×1440 and at 1280×800 are both
/// asserted without a window existing — which matters because the ultrawide is the
/// one machine this cannot be run on.
///
/// The bounds are deliberate: two columns is the fewest that still reads as a grid,
/// and eight is where a 3440-wide window lands before cards start looking thin.
/// Everything outside the range clamps rather than erroring, because a window
/// manager is allowed to ask for any width at all.
pub fn grid_columns(window_width: f32) -> usize {
    /// Card width the layout aims for, including the gap between cards.
    const TARGET_CARD_PX: f32 = 300.0;
    /// Fewest columns that still reads as a grid.
    const MIN_COLUMNS: usize = 2;
    /// Most columns offered, so an ultrawide does not produce slivers.
    const MAX_COLUMNS: usize = 8;

    if !window_width.is_finite() {
        return DEFAULT_COLUMNS;
    }
    // The shell pads both sides, so the usable width is narrower than the window.
    let usable = window_width - 2.0 * design::tokens().spacing.shell_pad;
    if usable <= 0.0 {
        return MIN_COLUMNS;
    }
    let columns = (usable / TARGET_CARD_PX).floor();
    // A NaN cast to usize is 0, and 0 columns would divide by zero below.
    if !columns.is_finite() || columns < 1.0 {
        return MIN_COLUMNS;
    }
    (columns as usize).clamp(MIN_COLUMNS, MAX_COLUMNS)
}

/// Rounds a slider position to a whole percent inside `min..=max`.
///
/// A slider works in floats and a preference is stored as a whole percent, so the
/// conversion happens here rather than in the view, where it would be one more
/// place for the shown value and the stored one to disagree.
pub fn round_percent(value: f64, min: u16, max: u16) -> u16 {
    if !value.is_finite() {
        return min;
    }
    (value.round().max(0.0) as u16).clamp(min, max)
}

/// Truncates a label to `max` characters with an ellipsis.
///
/// Cards are fixed height, so an overlong filename must not reflow the grid.
fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let mut out: String = value.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Formats seconds as `m:ss` or `h:mm:ss`.
pub fn clock(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "0:00".to_string();
    }
    let total = seconds.round() as u64;
    let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use screenroom_core::{Ingest, parse_filename};

    #[test]
    fn clock_formats_short_and_long_media() {
        assert_eq!(clock(0.0), "0:00");
        assert_eq!(clock(9.4), "0:09");
        assert_eq!(clock(92.0), "1:32");
        assert_eq!(clock(3_661.0), "1:01:01");
        assert_eq!(clock(-4.0), "0:00");
        assert_eq!(clock(f64::NAN), "0:00");
    }

    #[test]
    fn the_grid_shows_more_cards_as_the_window_widens() {
        // The two windows this owner actually uses, asserted without either existing.
        // The ultrawide is the machine this cannot be run on, so the layout rule is
        // the only thing that can be checked here.
        let laptop = grid_columns(1280.0);
        let ultrawide = grid_columns(3440.0);
        assert_eq!(
            laptop, DEFAULT_COLUMNS,
            "a 1280-wide window keeps its old look"
        );
        assert!(
            ultrawide > laptop,
            "an ultrawide must show more cards, not wider ones: {ultrawide} vs {laptop}"
        );

        // And the width of a card on the ultrawide stays in the range the rule aims
        // for, which is the actual complaint: four columns of 860px each.
        let usable = 3440.0 - 2.0 * design::tokens().spacing.shell_pad;
        let card = usable / ultrawide as f32;
        assert!(
            card < 600.0,
            "a card on the ultrawide is {card:.0}px wide, which is a billboard"
        );
    }

    #[test]
    fn the_column_count_is_monotonic_and_always_usable() {
        // A window manager may ask for any width at all, including nonsense.
        let mut previous = 0;
        for width in [
            320.0_f32, 800.0, 1024.0, 1280.0, 1920.0, 2560.0, 3440.0, 5120.0,
        ] {
            let columns = grid_columns(width);
            assert!(
                (2..=8).contains(&columns),
                "{width}px produced {columns} columns, which is outside the range"
            );
            assert!(
                columns >= previous,
                "widening the window reduced the column count: {width}px gave {columns}"
            );
            previous = columns;
        }

        // Degenerate widths must still produce a drawable grid rather than zero
        // columns, which would divide by zero in the card layout.
        for width in [0.0_f32, -100.0, 10.0, f32::NAN, f32::INFINITY] {
            let columns = grid_columns(width);
            assert!(columns >= 2, "{width} produced {columns} columns");
        }
    }

    #[test]
    fn keyboard_rows_move_by_the_columns_the_grid_was_drawn_with() {
        // Up/Down step a whole row, so the step and the layout have to agree: a fixed
        // step would land on the wrong card on a wider window. This is the coupling
        // that a constant column count used to hide.
        let (mut app, _directory) = app_with(24);
        app.window_width = 3440.0;
        let columns = app.columns();
        assert!(columns > DEFAULT_COLUMNS, "this test needs a wider grid");

        app.focus = 0;
        app.move_focus(Focus::Down);
        assert_eq!(app.focus, columns, "Down must move exactly one row");
        app.move_focus(Focus::Up);
        assert_eq!(app.focus, 0, "Up must return to the same column");

        // Right stays one item, so moving along a row is unaffected.
        app.move_focus(Focus::Right);
        assert_eq!(app.focus, 1);
    }

    #[test]
    fn a_resize_event_that_has_no_usable_width_is_ignored() {
        // A minimize or a window being created reports a zero width; keeping the last
        // good value avoids a frame laid out with no usable width.
        let (mut app, _directory) = app_with(4);
        app.window_width = 1920.0;
        for width in [0.0_f32, -1.0, f32::NAN, f32::INFINITY] {
            let _ = app.update(Message::Resized(width));
            assert_eq!(
                app.window_width, 1920.0,
                "a width of {width} must not replace a usable one"
            );
        }
        let _ = app.update(Message::Resized(3440.0));
        assert_eq!(app.window_width, 3440.0);
    }

    #[test]
    fn an_empty_browse_screen_names_the_situation_it_is_actually_in() {
        // A first run, an unread folder and an over-narrow search all produce an
        // empty grid. Telling all three to "change the filters" sends two of them
        // to a control that cannot help.
        let first_run = browse_empty_copy(false, false);
        assert!(first_run.0.contains("No media"));
        assert!(first_run.1.contains("Add a folder"));
        assert!(!first_run.1.contains("filter"));

        // The same copy with filters showing, because there is still nothing to
        // filter: the next action does not change.
        assert_eq!(browse_empty_copy(false, true), first_run);

        let unread = browse_empty_copy(true, false);
        assert!(unread.1.contains("Rescan"));
        assert!(!unread.1.contains("Add a folder"));

        let filtered = browse_empty_copy(true, true);
        assert!(filtered.0.contains("filters"));
        assert!(filtered.1.contains("Clear the search"));

        // Every state is a real sentence with a next action, never a blank panel.
        for (title, detail) in [first_run, unread, filtered] {
            assert!(!title.is_empty() && !detail.is_empty());
            assert!(detail.ends_with('.'), "copy should be a sentence: {detail}");
        }
    }

    #[test]
    fn sort_order_alone_does_not_count_as_narrowing_the_library() {
        // Sorting can never remove a row, so an empty grid is not the sort's fault
        // and "clear the filters" would be a dead end.
        let mut browse = Browse::new();
        browse.sort = SortChoice::Progress;
        assert!(!browse.narrowed());

        for narrow in [
            |b: &mut Browse| b.text = "x".into(),
            |b: &mut Browse| b.kind = KindFilter::Movies,
            |b: &mut Browse| b.genre = Some("Drama".into()),
            |b: &mut Browse| b.watchlist_only = true,
            |b: &mut Browse| b.favorites_only = true,
            |b: &mut Browse| b.available_only = true,
        ] {
            let mut browse = Browse::new();
            narrow(&mut browse);
            assert!(browse.narrowed(), "a narrowing filter went unnoticed");
        }

        // Whitespace in the search box is not a search.
        let mut browse = Browse::new();
        browse.text = "   ".into();
        assert!(!browse.narrowed());
    }

    #[test]
    fn a_slider_position_becomes_a_whole_percent_inside_the_offered_range() {
        let (min, max) = (
            Preferences::SUBTITLE_SCALE_MIN_PERCENT,
            Preferences::SUBTITLE_SCALE_MAX_PERCENT,
        );
        assert_eq!(round_percent(125.4, min, max), 125);
        assert_eq!(round_percent(125.6, min, max), 126);
        // A slider can report a hair outside its range while the handle snaps, and
        // a preference outside the range is refused by the core, so it is clamped.
        assert_eq!(round_percent(-8.0, min, max), min);
        assert_eq!(round_percent(9_999.0, min, max), max);
        // A non-finite position has no nearest percent; the minimum is at least a
        // value the core will accept and store.
        assert_eq!(round_percent(f64::NAN, min, max), min);
        assert_eq!(round_percent(f64::INFINITY, min, max), min);
    }

    #[test]
    fn the_offered_subtitle_range_is_one_the_core_accepts() {
        // The two crates define these bounds independently: the player against what
        // libmpv will take, the core against what it will store. If either drifts,
        // a slider at its end would store a refused value and the Settings screen
        // would show an error for a control the user can see.
        let (scale_low, scale_high) = screenroom_player::SUB_SCALE_RANGE;
        assert!(scale_low <= f64::from(Preferences::SUBTITLE_SCALE_MIN_PERCENT) / 100.0);
        assert!(scale_high >= f64::from(Preferences::SUBTITLE_SCALE_MAX_PERCENT) / 100.0);
        let (position_low, position_high) = screenroom_player::SUB_POSITION_RANGE;
        assert!(position_low <= f64::from(Preferences::SUBTITLE_POSITION_MIN_PERCENT));
        assert!(position_high >= f64::from(Preferences::SUBTITLE_POSITION_MAX_PERCENT));
    }

    #[test]
    fn truncation_keeps_cards_a_fixed_height() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("exactly-ten", 11), "exactly-ten");
        let long = truncate("a-very-long-title-that-would-reflow-a-card", 12);
        assert_eq!(long.chars().count(), 12);
        assert!(long.ends_with('…'));
    }

    #[test]
    fn a_finished_file_offers_a_replay_instead_of_a_dead_pause() {
        use screenroom_player::PlayerState;
        assert_eq!(play_action(PlayerState::Ended), PlayAction::Replay);
        assert_eq!(play_action(PlayerState::Paused), PlayAction::Resume);
        assert_eq!(play_action(PlayerState::Playing), PlayAction::Pause);
        // Every state must produce a labelled control, never an empty one.
        for state in [
            PlayerState::Idle,
            PlayerState::Resolving,
            PlayerState::Preparing,
            PlayerState::Playing,
            PlayerState::Paused,
            PlayerState::Ended,
            PlayerState::Failed,
        ] {
            assert!(!play_action(state).label().is_empty());
        }
    }

    #[test]
    fn the_page_size_is_a_whole_number_of_reference_rows() {
        // The column count is now derived from the window width, so a page need not
        // fill its last row. What keeps the grid square is the layout padding a
        // partial row with spacers; this only keeps the page size a whole number of
        // rows for the width the reference grid describes.
        assert_eq!(PAGE_SIZE, (DEFAULT_COLUMNS * PAGE_ROWS) as i64);
        assert_eq!(PAGE_SIZE % DEFAULT_COLUMNS as i64, 0);
    }

    #[test]
    fn kind_filters_map_onto_the_stored_kinds() {
        assert_eq!(KindFilter::All.kind(), None);
        assert_eq!(KindFilter::Movies.kind().as_deref(), Some("movie"));
        assert_eq!(KindFilter::Series.kind().as_deref(), Some("series"));
        for filter in [KindFilter::All, KindFilter::Movies, KindFilter::Series] {
            assert!(!filter.label().is_empty());
        }
    }

    #[test]
    fn every_sort_choice_is_offered_and_labelled() {
        assert_eq!(SortChoice::ALL.len(), 4);
        for choice in SortChoice::ALL {
            assert!(!choice.label().is_empty());
        }
        assert_eq!(SortChoice::Added.sort(), Sort::Added);
    }

    /// Builds a library with `count` ingested files under one real temp folder.
    fn library_with(count: usize) -> (Library, tempfile::TempDir) {
        let directory = tempfile::TempDir::new().expect("temp dir");
        let library = Library::open_in_memory().expect("library");
        let source = library.add_source(directory.path(), None).expect("source");
        for index in 0..count {
            let name = format!("Title.{index:03}.2020.mkv");
            std::fs::write(directory.path().join(&name), b"not really a video")
                .expect("write placeholder");
            let parsed = parse_filename(&name, None);
            library
                .ingest_file(Ingest {
                    source_id: &source.id,
                    relative_path: &name,
                    fingerprint: &format!("fingerprint-{index}"),
                    size: 18,
                    modified_ms: 1,
                    scan_id: "test",
                    parsed: Some(&parsed),
                    expected_root: Some(&source.root_path),
                })
                .expect("ingest");
        }
        (library, directory)
    }

    fn app_with(count: usize) -> (App, tempfile::TempDir) {
        let (library, directory) = library_with(count);
        let mut app = App::with_library(library, None, "test".into());
        app.screen = Screen::Browse;
        app.reload_browse();
        (app, directory)
    }

    #[test]
    fn a_retained_poster_is_cached_for_its_card() {
        // The whole point of retaining bytes is that drawing needs no network and
        // no second decode of the media file; the card gets a handle from the row.
        let (library, _directory) = library_with(2);
        let item = library.items(&LibraryQuery::page(1)).expect("items")[0].clone();
        library
            .store_artwork(
                &item.work_id,
                None,
                &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10],
                400,
                600,
                1_000,
            )
            .expect("artwork");

        let mut app = App::with_library(library, None, "test".into());
        app.screen = Screen::Browse;
        app.reload_browse();
        assert!(
            app.artwork.contains_key(&item.work_id),
            "a stored poster must be handed to the card"
        );
    }

    #[test]
    fn a_test_built_app_never_starts_an_extraction() {
        // Extraction spawns ffmpeg and writes into the library; a unit test must not
        // do either. Only a launch sets this, and the queue is what it gates.
        let (mut app, _directory) = app_with(4);
        app.screen = Screen::Browse;
        app.reload_browse();
        assert!(app.extractor.is_none());
        assert!(app.artwork_queued.is_empty());
    }

    #[test]
    fn a_marker_covers_only_the_interval_it_describes() {
        // The button appears from the start of the segment to its end and not one
        // frame outside it: a skip offered over the opening scene would cut the
        // film, and one offered after the intro would do nothing.
        let (mut app, _directory) = app_with(1);
        app.markers = vec![SegmentMarker {
            id: "marker".to_string(),
            edition_id: "edition".to_string(),
            kind: "intro".to_string(),
            start_ms: 60_000,
            end_ms: 120_000,
            duration_ms: 3_600_000,
        }];

        app.snapshot = App::playing_snapshot(30.0, 3_600.0);
        assert!(app.active_marker().is_none());
        app.snapshot = App::playing_snapshot(60.0, 3_600.0);
        assert_eq!(
            app.active_marker().map(|marker| marker.kind),
            Some("intro".to_string())
        );
        app.snapshot = App::playing_snapshot(120.0, 3_600.0);
        assert!(app.active_marker().is_none());
    }

    #[test]
    fn automatic_skipping_only_happens_once_and_only_when_asked_for() {
        let (mut app, _directory) = app_with(1);
        app.markers = vec![SegmentMarker {
            id: "marker".to_string(),
            edition_id: "edition".to_string(),
            kind: "intro".to_string(),
            start_ms: 60_000,
            end_ms: 120_000,
            duration_ms: 3_600_000,
        }];
        app.snapshot = App::playing_snapshot(90.0, 3_600.0);

        // Off by default: nothing moves on its own.
        app.maybe_auto_skip_intro();
        assert!(app.auto_skipped.is_none());
        assert_ne!(app.status.as_deref(), Some("Skipped the intro."));

        app.preferences.auto_skip_intro = true;
        app.maybe_auto_skip_intro();
        assert_eq!(app.auto_skipped.as_deref(), Some("marker"));
        assert_eq!(app.status.as_deref(), Some("Skipped the intro."));

        // A player that reports the pre-seek position again must not skip twice,
        // and the status is not overwritten on every frame.
        app.status = None;
        app.maybe_auto_skip_intro();
        assert!(app.status.is_none());
    }

    #[test]
    fn the_home_cursor_walks_suggestions_before_continue_watching() {
        let (mut app, _directory) = app_with(2);
        let all = app.library.items(&LibraryQuery::page(10)).expect("items");
        app.suggestions = vec![Recommendation {
            item: all[0].clone(),
            reason: "Recently added".to_string(),
        }];
        app.home = vec![all[1].clone()];
        app.screen = Screen::Home;
        app.focus = 0;

        assert_eq!(app.visible_len(), 2, "both cards are reachable");
        assert_eq!(
            app.home_item(0).map(|item| item.id.clone()),
            Some(all[0].id.clone())
        );
        assert_eq!(
            app.home_item(1).map(|item| item.id.clone()),
            Some(all[1].id.clone())
        );
        assert!(app.home_item(2).is_none());
    }

    #[test]
    fn a_test_built_app_never_reaches_for_the_menu_bar() {
        // Routing ⌘Q talks to AppKit, which is only legal on the main thread. A
        // library is built here, not launched, so the budget must stay at zero: if
        // it did not, the first `update` in any of these tests would call into the
        // window server from a test thread.
        let (mut app, _directory) = app_with(1);
        assert_eq!(app.quit_routing_budget, 0, "not armed by construction");
        let _ = app.update(Message::Refresh);
        assert_eq!(app.quit_routing_budget, 0, "and never armed by an update");
        assert_eq!(app.quit_routing, crate::quit::QuitRouting::Unsupported);
    }

    #[test]
    fn opening_an_unknown_file_changes_nothing() {
        let library = Library::open_in_memory().expect("library");
        let mut app = App::with_library(library, None, "test".into());
        app.open_detail("no-such-file");
        assert_eq!(app.screen, Screen::Home);
        assert!(app.detail.is_none());
        assert!(app.error.is_some(), "an unknown id must be reported");
    }

    #[test]
    fn an_unreadable_item_is_refused_with_a_recovery_hint() {
        let directory = tempfile::TempDir::new().expect("temp dir");
        let library = Library::open_in_memory().expect("library");
        let source = library.add_source(directory.path(), None).expect("source");
        let parsed = parse_filename("Gone.2020.mkv", None);
        let file_id = library
            .ingest_file(Ingest {
                source_id: &source.id,
                relative_path: "Gone.2020.mkv",
                fingerprint: "f",
                size: 10,
                modified_ms: 1,
                scan_id: "s",
                parsed: Some(&parsed),
                expected_root: Some(&source.root_path),
            })
            .expect("ingest");
        let mut app = App::with_library(library, None, "test".into());
        app.open_player(&file_id, false);
        assert_eq!(app.screen, Screen::Home);
        assert!(
            app.error
                .as_deref()
                .is_some_and(|error| error.contains("relink")),
            "an unavailable file must name the recovery"
        );
        assert!(app.active.is_none());
    }

    #[test]
    fn focus_walks_the_grid_and_stops_at_the_edges() {
        let (mut app, _directory) = app_with(10);
        // A test-built app has never been resized, so it is laid out for the
        // reference width; the wider case is covered separately.
        assert_eq!(app.columns(), DEFAULT_COLUMNS);
        assert_eq!(app.focus, 0);
        app.move_focus(Focus::Left);
        assert_eq!(app.focus, 0, "focus must not leave the first card");
        app.move_focus(Focus::Right);
        assert_eq!(app.focus, 1);
        app.move_focus(Focus::Up);
        assert_eq!(app.focus, 1, "there is no row above the first");
        app.move_focus(Focus::Down);
        assert_eq!(app.focus, 1 + DEFAULT_COLUMNS, "down moves a whole row");
        // Walk to the last card and confirm it does not run past the page.
        for _ in 0..20 {
            app.move_focus(Focus::Right);
            app.move_focus(Focus::Down);
        }
        assert!(app.focus < app.visible_len());
        assert_eq!(app.focus, app.visible_len() - 1);
    }

    #[test]
    fn focus_is_clamped_when_a_page_shrinks() {
        let (mut app, _directory) = app_with(10);
        app.focus = 9;
        app.browse.text = "Title.000".to_string();
        app.reload_browse();
        assert_eq!(app.visible_len(), 1);
        assert_eq!(app.focus, 0, "a shrunken page must pull focus back in");
    }

    #[test]
    fn paging_is_bounded_and_never_runs_off_the_end() {
        let (mut app, _directory) = app_with(30);
        assert_eq!(app.browse.page_count(), 2);
        let _ = app.update(Message::PreviousPage);
        assert_eq!(app.browse.page, 0, "there is no page before the first");
        let _ = app.update(Message::NextPage);
        assert_eq!(app.browse.page, 1);
        assert_eq!(
            app.browse.items.len(),
            6,
            "the last page holds the remainder"
        );
        let _ = app.update(Message::NextPage);
        assert_eq!(app.browse.page, 1, "there is no page past the last");
    }

    #[test]
    fn search_is_literal_and_survives_punctuation() {
        let (mut app, _directory) = app_with(12);
        app.browse.text = "Title.00".to_string();
        app.reload_browse();
        assert_eq!(
            app.browse.total, 10,
            "the prefix matches Title.000 through Title.009"
        );
        // A quote must be treated as text, never as SQL.
        app.browse.text = "O'Brien%'".to_string();
        app.reload_browse();
        assert_eq!(app.browse.total, 0);
        assert!(app.error.is_none(), "a literal search must not error");
    }

    #[test]
    fn a_finished_edition_is_marked_watched_and_leaves_continue_watching() {
        let (mut app, _directory) = app_with(2);
        let item = app.browse.items[0].clone();
        app.open_detail(&item.id);
        assert_eq!(app.screen, Screen::Detail);
        let _ = app.update(Message::ToggleWatched(item.edition_id.clone()));
        let detail = app.detail.as_ref().expect("detail");
        assert!(detail.item.watched);
        let _ = app.update(Message::ToggleWatched(item.edition_id.clone()));
        let detail = app.detail.as_ref().expect("detail");
        assert!(!detail.item.watched);
    }

    #[test]
    fn favourite_and_watchlist_survive_a_reload() {
        let (mut app, _directory) = app_with(2);
        let item = app.browse.items[0].clone();
        app.open_detail(&item.id);
        let _ = app.update(Message::ToggleFavorite(item.work_id.clone()));
        let _ = app.update(Message::ToggleWatchlist(item.work_id.clone()));
        let detail = app.detail.as_ref().expect("detail");
        assert!(detail.item.favorite);
        assert!(detail.item.watchlisted);
        app.reload();
        assert!(app.browse.items.iter().any(|row| row.favorite));
    }

    #[test]
    fn the_watchlist_filter_restricts_the_library() {
        let (mut app, _directory) = app_with(4);
        let item = app.browse.items[0].clone();
        app.library
            .set_watchlisted(&app.profile_id, &item.work_id, true)
            .expect("watchlist");
        let _ = app.update(Message::ToggleWatchlistOnly(true));
        assert_eq!(app.browse.total, 1);
        assert_eq!(app.browse.items[0].id, item.id);
        let _ = app.update(Message::ToggleWatchlistOnly(false));
        assert_eq!(app.browse.total, 4);
    }

    #[test]
    fn back_walks_home_browse_detail_and_stops_at_home() {
        let (mut app, _directory) = app_with(2);
        let _ = app.update(Message::GoBrowse);
        assert_eq!(app.screen, Screen::Browse);
        let item = app.browse.items[0].clone();
        app.open_detail(&item.id);
        assert_eq!(app.screen, Screen::Detail);
        app.back();
        assert_eq!(app.screen, Screen::Browse);
        app.back();
        assert_eq!(app.screen, Screen::Home);
        app.back();
        assert_eq!(app.screen, Screen::Home, "Home is the top of the hierarchy");
    }

    #[test]
    fn adding_a_folder_requires_an_absolute_path() {
        let library = Library::open_in_memory().expect("library");
        let mut app = App::with_library(library, None, "test".into());
        app.new_root = "relative/media".to_string();
        let _ = app.update(Message::AddFolder);
        assert!(
            app.error
                .as_deref()
                .is_some_and(|error| error.contains("absolute")),
            "a relative path must be refused with the reason"
        );
        assert!(app.scan_token.is_none());
    }

    #[test]
    fn the_default_library_path_is_absolute_and_named() {
        let path = default_library_path().expect("a data directory on this host");
        assert!(path.is_absolute());
        assert!(path.ends_with("Screenroom/library.db"));
    }

    /// A series of two episodes, so autoplay has something real to follow.
    fn series_app() -> (App, Vec<String>, tempfile::TempDir) {
        let directory = tempfile::TempDir::new().expect("temp dir");
        let library = Library::open_in_memory().expect("library");
        let source = library.add_source(directory.path(), None).expect("source");
        let mut ids = Vec::new();
        for name in ["Show.S01E01.Pilot.mkv", "Show.S01E02.Second.mkv"] {
            std::fs::write(directory.path().join(name), b"not really a video").expect("write");
            let parsed = parse_filename(name, None);
            ids.push(
                library
                    .ingest_file(Ingest {
                        source_id: &source.id,
                        relative_path: name,
                        fingerprint: &format!("fingerprint-{name}"),
                        size: 18,
                        modified_ms: 1,
                        scan_id: "test",
                        parsed: Some(&parsed),
                        expected_root: Some(&source.root_path),
                    })
                    .expect("ingest"),
            );
        }
        let app = App::with_library(library, None, "test".into());
        (app, ids, directory)
    }

    #[test]
    fn autoplay_follows_the_next_verified_episode() {
        let (app, ids, _directory) = series_app();
        let next = app
            .next_episode_after(&ids[0])
            .expect("lookup")
            .expect("the second episode follows the first");
        assert_eq!(next.id, ids[1]);
        assert!(
            app.next_episode_after(&ids[1]).unwrap().is_none(),
            "the last episode has no successor to guess at"
        );
    }

    #[test]
    fn autoplay_stands_down_when_the_preference_is_off() {
        let (mut app, ids, _directory) = series_app();
        let _ = app.update(Message::ToggleAutoplay(false));
        assert!(!app.preferences.autoplay_next);
        assert!(
            app.next_episode_after(&ids[0]).unwrap().is_none(),
            "a viewer who turned autoplay off must not have it happen anyway"
        );
        // The preference is stored, not just held in memory.
        assert!(!app.library.preferences().unwrap().autoplay_next);
    }

    #[test]
    fn a_movie_has_no_next_episode() {
        let (app, _directory) = app_with(1);
        let item = app.browse.items[0].clone();
        assert!(
            app.next_episode_after(&item.id).unwrap().is_none(),
            "autoplay must not invent an episode after a film"
        );
    }

    #[test]
    fn the_track_chooser_offers_off_and_reads_the_selection_back() {
        let (mut app, _directory) = app_with(1);
        assert!(
            app.track_chooser(TrackKind::Subtitle).0.is_empty(),
            "no reported tracks means no chooser to offer"
        );
        app.snapshot.tracks = vec![
            MediaTrack {
                id: 1,
                kind: TrackKind::Audio,
                title: Some("English 5.1".to_string()),
                language: Some("en".to_string()),
                codec: Some("aac".to_string()),
                selected: true,
                external: false,
            },
            MediaTrack {
                id: 2,
                kind: TrackKind::Subtitle,
                title: None,
                language: Some("fr".to_string()),
                codec: Some("subrip".to_string()),
                selected: false,
                external: false,
            },
        ];

        let (audio, selected) = app.track_chooser(TrackKind::Audio);
        assert_eq!(audio.len(), 1);
        assert_eq!(selected.map(|choice| choice.id), Some(1));
        assert!(audio[0].label.contains("English 5.1"));

        // A kind the player reported nothing for yields no options, so the view
        // renders a statement rather than an inert dropdown.
        app.snapshot
            .tracks
            .retain(|track| track.kind != TrackKind::Subtitle);
        assert!(app.track_chooser(TrackKind::Subtitle).0.is_empty());
        app.snapshot.tracks = vec![
            MediaTrack {
                id: 1,
                kind: TrackKind::Audio,
                title: Some("English 5.1".to_string()),
                language: Some("en".to_string()),
                codec: Some("aac".to_string()),
                selected: true,
                external: false,
            },
            MediaTrack {
                id: 2,
                kind: TrackKind::Subtitle,
                title: None,
                language: Some("fr".to_string()),
                codec: Some("subrip".to_string()),
                selected: false,
                external: false,
            },
        ];

        let (subtitles, selected) = app.track_chooser(TrackKind::Subtitle);
        assert_eq!(
            subtitles.len(),
            2,
            "the chooser offers Off as well as the track"
        );
        assert_eq!(subtitles[0].id, NO_TRACK);
        assert_eq!(subtitles[0].label, "Off");
        assert_eq!(
            selected.map(|choice| choice.id),
            Some(NO_TRACK),
            "subtitles that are off must show as Off, not as an empty control"
        );
    }

    #[test]
    fn switching_a_profile_keeps_each_viewers_data_separate() {
        let (mut app, _directory) = app_with(3);
        let work = app.browse.items[0].work_id.clone();
        let first = app.profile_id.clone();
        app.library.set_favorite(&first, &work, true).unwrap();

        let _ = app.update(Message::NewProfileChanged("Guest".to_string()));
        let _ = app.update(Message::AddProfile);
        let guest = app.profile_id.clone();
        assert_ne!(guest, first, "adding a profile switches to it");
        assert_eq!(app.profiles.len(), 2);
        assert!(
            !app.browse.items.iter().any(|item| item.favorite),
            "the new profile must not inherit the first profile's favourites"
        );

        // The choice is stored, so a restart reopens as the same viewer.
        assert_eq!(
            app.library.preferences().unwrap().default_profile,
            Some(guest.clone())
        );

        // And the original viewer still has their own favourite.
        let _ = app.update(Message::ProfileChosen(first));
        assert_eq!(
            app.library
                .count(&LibraryQuery {
                    profile_id: Some(app.profile_id.clone()),
                    favorites_only: true,
                    ..LibraryQuery::page(PAGE_SIZE)
                })
                .unwrap(),
            1,
            "the first viewer still has their own favourite"
        );
        assert_eq!(
            app.library
                .count(&LibraryQuery {
                    profile_id: Some(guest),
                    favorites_only: true,
                    ..LibraryQuery::page(PAGE_SIZE)
                })
                .unwrap(),
            0,
            "and the other viewer has none of it"
        );
    }

    #[test]
    fn the_last_profile_is_kept() {
        let (mut app, _directory) = app_with(1);
        assert_eq!(app.profiles.len(), 1);
        let only = app.profile_id.clone();
        let _ = app.update(Message::RemoveProfile(only.clone()));
        assert_eq!(app.profiles.len(), 1, "a device always keeps one profile");
        assert_eq!(app.profile_id, only);
        assert!(
            app.error
                .as_deref()
                .is_some_and(|e| e.contains("one profile")),
            "the refusal is explained"
        );
    }

    #[test]
    fn an_invalid_language_tag_is_reported_and_not_stored() {
        let (mut app, _directory) = app_with(1);
        let _ = app.update(Message::SubtitleLanguageChanged(
            "en; drop table".to_string(),
        ));
        let _ = app.update(Message::ApplySubtitleLanguage);
        assert_eq!(app.library.preferences().unwrap().subtitle_language, None);
        assert!(app.error.is_some(), "the refusal is shown");

        let _ = app.update(Message::SubtitleLanguageChanged("fr".to_string()));
        let _ = app.update(Message::ApplySubtitleLanguage);
        assert_eq!(
            app.library
                .preferences()
                .unwrap()
                .subtitle_language
                .as_deref(),
            Some("fr")
        );
    }

    #[test]
    fn removing_a_folder_asks_first_and_keeps_the_watch_history() {
        let (mut app, _directory) = app_with(2);
        let source = app.sources[0].clone();
        let item = app.browse.items[0].clone();
        app.library
            .set_favorite(&app.profile_id, &item.work_id, true)
            .unwrap();

        let _ = app.update(Message::RemoveSource(source.id.clone()));
        assert_eq!(
            app.armed_remove.as_deref(),
            Some(source.id.as_str()),
            "the first press asks rather than acts"
        );
        assert_eq!(app.sources.len(), 1, "nothing has happened yet");

        let _ = app.update(Message::RemoveSource(source.id.clone()));
        assert!(app.armed_remove.is_none());
        assert_eq!(app.browse.total, 0, "the folder's files leave the library");
        assert!(
            app.library.sources().unwrap().is_empty(),
            "the folder leaves the visible list"
        );
        assert!(
            app.library.source(&source.id).is_ok(),
            "but its row survives, so recovery copy can still name it"
        );
        // The favourite is hidden while the folder is out of the library, and it
        // is not deleted: re-admitting the folder brings it straight back. That,
        // not the queried count, is the claim that nothing was destroyed.
        assert_eq!(
            app.library
                .count(&LibraryQuery {
                    profile_id: Some(app.profile_id.clone()),
                    favorites_only: true,
                    ..LibraryQuery::page(PAGE_SIZE)
                })
                .unwrap(),
            0,
            "a removed folder's entries are hidden, not offered"
        );
        app.library
            .set_source_state(&source.id, SourceState::Available)
            .expect("re-admit");
        app.reload();
        assert_eq!(
            app.library
                .count(&LibraryQuery {
                    profile_id: Some(app.profile_id.clone()),
                    favorites_only: true,
                    ..LibraryQuery::page(PAGE_SIZE)
                })
                .unwrap(),
            1,
            "the favourite was kept through the removal"
        );
    }

    #[test]
    fn a_backup_and_restore_round_trip_through_the_settings_screen() {
        let directory = tempfile::TempDir::new().expect("temp dir");
        let library = Library::open(directory.path().join("library.db")).expect("library");
        let media = directory.path().join("media");
        std::fs::create_dir_all(&media).unwrap();
        let source = library.add_source(&media, None).expect("source");
        let name = "Heat.1995.mkv";
        std::fs::write(media.join(name), b"not really a video").unwrap();
        let parsed = parse_filename(name, None);
        let file_id = library
            .ingest_file(Ingest {
                source_id: &source.id,
                relative_path: name,
                fingerprint: "f",
                size: 18,
                modified_ms: 1,
                scan_id: "test",
                parsed: Some(&parsed),
                expected_root: Some(&source.root_path),
            })
            .expect("ingest");
        let mut app = App::with_library(library, None, "test".into());
        app.profile_id = app.library.profiles().unwrap().remove(0).id;
        let backup = directory.path().join("backup.db");
        app.backup_path = backup.to_string_lossy().to_string();
        let _ = app.update(Message::BackupNow);
        assert!(backup.is_file(), "the backup is written: {:?}", app.error);
        assert!(app.error.is_none(), "unexpected error: {:?}", app.error);

        // A change made after the backup, so the restore is observable.
        app.library
            .set_favorite(&app.profile_id, "anything", true)
            .ok();
        let session = app
            .library
            .start_session(&app.profile_id, &file_id)
            .unwrap();
        app.library
            .checkpoint(screenroom_core::Checkpoint {
                session: &session,
                position_ms: 5_000,
                duration_ms: 60_000,
                sequence: 1,
                seek_generation: 0,
                ended: false,
            })
            .unwrap();
        app.library.end_session(&session).unwrap();
        assert_eq!(
            app.library
                .continue_watching(&app.profile_id, 5)
                .unwrap()
                .len(),
            1
        );

        app.restore_path = backup.to_string_lossy().to_string();
        let _ = app.update(Message::RestoreNow);
        assert!(app.armed_restore.is_some(), "restore asks before replacing");
        assert_eq!(
            app.library
                .continue_watching(&app.profile_id, 5)
                .unwrap()
                .len(),
            1,
            "asking changes nothing"
        );

        let _ = app.update(Message::RestoreNow);
        assert!(app.armed_restore.is_none());
        assert!(app.error.is_none(), "unexpected error: {:?}", app.error);
        assert_eq!(
            app.library
                .continue_watching(&app.profile_id, 5)
                .unwrap()
                .len(),
            0,
            "the restored state is the backup's, not the later change"
        );
        let safety = app
            .status
            .clone()
            .expect("the result names the safety copy");
        assert!(safety.contains("before-restore"), "status was: {safety}");
    }

    #[test]
    fn inspecting_a_backup_reports_what_it_holds_without_restoring() {
        let (mut app, _directory) = app_with(2);
        let directory = tempfile::TempDir::new().expect("temp dir");
        let backup = directory.path().join("copy.db");
        app.library.backup_to(&backup).expect("backup");
        app.restore_path = backup.to_string_lossy().to_string();
        let _ = app.update(Message::InspectRestore);
        let status = app.status.clone().expect("a summary");
        assert!(status.contains("2 works"), "status was: {status}");
        assert!(
            app.armed_restore.is_none(),
            "checking a file is not a restore"
        );
        assert_eq!(app.browse.total, 2, "and it changed nothing");
    }

    #[test]
    fn the_settings_screen_is_reachable_and_leaves_again() {
        let (mut app, _directory) = app_with(2);
        let _ = app.update(Message::GoSettings);
        assert_eq!(app.screen, Screen::Settings);
        assert!(!app.profiles.is_empty());
        assert!(!app.sources.is_empty());
        app.back();
        assert_eq!(app.screen, Screen::Home);
    }

    #[test]
    fn quitting_with_a_file_open_saves_the_final_position() {
        let (mut app, _directory) = app_with(1);
        let item = app.browse.items[0].clone();
        // A session opened exactly as playback opens one, without starting libmpv:
        // the quit path's job is to close whatever is open and store where it got
        // to, which is testable on its own.
        let opened =
            PlaybackTracker::open(&app.library, &app.profile_id, &item.id).expect("a session");
        app.active = Some(Active {
            tracker: opened.tracker,
            file_id: item.id.clone(),
            title: item.title.clone(),
            resume_from_ms: opened.resume_from_ms,
            origin: Screen::Browse,
        });
        app.snapshot = App::playing_snapshot(42.5, 100.0);

        app.stop_playback();

        assert!(app.active.is_none(), "quitting ends the viewing");
        assert_eq!(
            app.snapshot.position, 0.0,
            "and clears the player state it was showing"
        );
        let watching = app
            .library
            .continue_watching(&app.profile_id, 5)
            .expect("continue watching");
        assert_eq!(watching.len(), 1, "the viewing is offered to resume");
        assert_eq!(
            watching[0].position_ms, 42_500,
            "the position at quit is what was stored, to the tenth of a second"
        );
        // And a later viewing resumes from there rather than from the start.
        let reopened =
            PlaybackTracker::open(&app.library, &app.profile_id, &item.id).expect("a new session");
        assert_eq!(reopened.resume_from_ms, 42_500);
    }

    #[test]
    fn choosing_an_unknown_profile_changes_nothing() {
        let (mut app, _directory) = app_with(1);
        let before = app.profile_id.clone();
        let _ = app.update(Message::ProfileChosen("no-such-profile".to_string()));
        assert_eq!(app.profile_id, before);
        assert!(app.error.is_some());
    }
}
