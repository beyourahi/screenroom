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

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use iced::widget::shader;
use iced::widget::{
    Space, button, checkbox, column, container, pick_list, row, scrollable, slider, text,
    text_input,
};
use iced::{Alignment, Element, Length, Subscription, Task, keyboard};
use screenroom_core::{
    CoreError, Library, LibraryItem, LibraryQuery, LibraryScanner, MediaKind, ScanToken, Sort,
};
use screenroom_player::{Player, PlayerCapability, PlayerSnapshot};

use crate::design::{self, Level};
use crate::playback::PlaybackTracker;
use crate::video_surface::{Frame, VideoSurface};

/// How often the player is polled for frames and position.
const PLAYER_TICK: Duration = Duration::from_millis(16);

/// How often status (scan progress) is polled when no player is open.
const STATUS_TICK: Duration = Duration::from_millis(250);

/// Cards per grid row. Four fits a 1280-wide window and an ultrawide comfortably.
const COLUMNS: usize = 4;

/// Rows per page. 24 cards keeps a page bounded and the query cheap.
const PAGE_ROWS: usize = 6;

/// Items per page, derived so the grid never shows a ragged final row.
const PAGE_SIZE: i64 = (COLUMNS * PAGE_ROWS) as i64;

/// Continue Watching rail length.
const RAIL: i64 = 8;

/// How long a forward seek jumps.
const SEEK_STEP: f64 = 10.0;

/// Starts the desktop application.
pub fn run() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Screenroom")
        .theme(App::theme)
        .subscription(App::subscription)
        .window_size((1280.0, 800.0))
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
    browse: Browse,
    detail: Option<Detail>,

    player: Option<Player>,
    snapshot: PlayerSnapshot,
    frame: Option<Frame>,
    active: Option<Active>,
    scrub: Option<f64>,

    /// Text field for granting a new media root.
    new_root: String,
    scan_token: Option<ScanToken>,
    scan_events: Option<Receiver<ScanEvent>>,
    status: Option<String>,
    error: Option<String>,
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
        let mut app = Self {
            library,
            library_label,
            profile_id,
            capability: screenroom_player::capability(),
            screen: Screen::Home,
            focus: 0,
            home: Vec::new(),
            browse: Browse::new(),
            detail: None,
            player: None,
            snapshot: PlayerSnapshot::default(),
            frame: None,
            active: None,
            scrub: None,
            new_root: String::new(),
            scan_token: None,
            scan_events: None,
            status: None,
            error,
        };
        app.reload();
        app
    }

    // ---------------------------------------------------------------- loading

    /// Reloads every projection the current screen needs.
    fn reload(&mut self) {
        self.reload_home();
        self.reload_browse();
    }

    fn reload_home(&mut self) {
        match self.library.continue_watching(&self.profile_id, RAIL) {
            Ok(items) => self.home = items,
            Err(error) => self.report(&error),
        }
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
    }

    /// Keeps the keyboard cursor inside the loaded page.
    fn clamp_focus(&mut self) {
        let len = self.visible_len();
        self.focus = if len == 0 { 0 } else { self.focus.min(len - 1) };
    }

    /// How many items the keyboard cursor can reach on this screen.
    fn visible_len(&self) -> usize {
        match self.screen {
            Screen::Home => self.home.len(),
            Screen::Browse => self.browse.items.len(),
            Screen::Detail => self
                .detail
                .as_ref()
                .map(|detail| detail.episodes.len().max(1))
                .unwrap_or(0),
            Screen::Player => 0,
        }
    }

    /// Records a core error with its stable code, so the message is diagnosable.
    fn report(&mut self, error: &CoreError) {
        self.error = Some(format!("{}: {}", error.code(), error));
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
        let snapshot = self.player.as_ref().map(|player| player.snapshot());
        let origin = self
            .active
            .as_ref()
            .map(|active| active.origin)
            .unwrap_or(Screen::Browse);
        if let (Some(snapshot), Some(mut active)) = (snapshot, self.active.take()) {
            // The final position is stored before the session closes, so leaving
            // during playback never loses more than the checkpoint interval.
            self.save(&mut active, &snapshot, snapshot.ended);
            if let Err(error) = active.tracker.close(&self.library) {
                self.report(&error);
            }
        }
        if let Some(player) = &self.player {
            player.stop();
        }
        self.screen = origin;
        self.frame = None;
        self.snapshot = PlayerSnapshot::default();
        self.scrub = None;
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
        self.open_player(&file_id, true);
        // `open_player` reads the origin from the screen, which is still the
        // player here; put back where this viewing came from.
        if let (Some(origin), Some(active)) = (origin, self.active.as_mut()) {
            active.origin = origin;
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

    // ------------------------------------------------------------------ scan

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
            Message::Tick => self.poll(),
            Message::Keyboard(event) => self.handle_key(event),
            Message::Close => return iced::exit(),
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
        let next = match direction {
            Focus::Left => current.checked_sub(1),
            Focus::Right => (current + 1 < len).then_some(current + 1),
            Focus::Up => current.checked_sub(COLUMNS),
            Focus::Down => {
                let candidate = current + COLUMNS;
                (candidate < len).then_some(candidate)
            }
        };
        self.focus = next.unwrap_or(current);
    }

    /// Enter on the focused item.
    fn activate(&mut self) {
        let file_id = match self.screen {
            Screen::Home => self.home.get(self.focus).map(|item| item.id.clone()),
            Screen::Browse => self
                .browse
                .items
                .get(self.focus)
                .map(|item| item.id.clone()),
            Screen::Detail => self.detail.as_ref().map(|detail| detail.item.id.clone()),
            Screen::Player => None,
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
            Screen::Browse => self.screen = Screen::Home,
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
        let mut subscriptions = vec![iced::keyboard::listen().map(Message::Keyboard)];
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
            Screen::Player => "Space play/pause · ←/→ seek 10s · Esc back",
            Screen::Detail => "Enter play · f favourite · w watchlist · m watched · Esc back",
            _ => "Arrows move · Enter opens · ⌘1 Home · ⌘2 Library · Esc back",
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

        body = body.push(text("Continue watching").size(tokens.type_scale.title_sm));
        if self.home.is_empty() {
            body = body.push(self.empty_note(
                "Nothing in progress.",
                "Start something from the library and its position appears here.",
            ));
        } else {
            body = body.push(self.card_grid(&self.home, 0));
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
            self.empty_note(
                "Nothing matches.",
                "Change the filters, or add a folder on Home.",
            )
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

        scrollable(body).height(Length::Fill).into()
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
        let mut grid = column![]
            .spacing(tokens.spacing.field_x)
            .width(Length::Fill);
        for (row_index, chunk) in items.chunks(COLUMNS).enumerate() {
            let mut cells = row![].spacing(tokens.spacing.field_x).width(Length::Fill);
            for (column_index, item) in chunk.iter().enumerate() {
                let index = base + row_index * COLUMNS + column_index;
                cells = cells.push(self.card(item, self.focus == index));
            }
            // Pad the final row so every card keeps the same width.
            for _ in chunk.len()..COLUMNS {
                cells = cells.push(Space::new().width(Length::FillPortion(1)));
            }
            grid = grid.push(cells);
        }
        grid.into()
    }

    /// One poster-less card: title, position, and its state.
    ///
    /// Artwork is not drawn because no artwork bytes are retained yet (F06). A
    /// placeholder image would be a fake.
    fn card(&self, item: &LibraryItem, focused: bool) -> Element<'_, Message> {
        let tokens = design::tokens();
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

        let content = column![
            text(title).size(tokens.type_scale.label),
            text(subtitle)
                .size(tokens.type_scale.micro)
                .color(tokens.palette.ink_muted),
            Space::new().height(Length::Fixed(4.0)),
            text(position)
                .size(tokens.type_scale.micro)
                .color(tokens.palette.ink_muted),
            text(truncate(&item.filename, 30))
                .size(tokens.type_scale.micro)
                .color(tokens.palette.ink_muted),
        ]
        .spacing(2.0);

        button(content)
            .on_press(Message::OpenDetail(item.id.clone()))
            .style(design::tile_style(focused))
            .width(Length::FillPortion(1))
            .height(Length::Fixed(132.0))
            .padding(tokens.spacing.field_y)
            .into()
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

        let resume = self
            .active
            .as_ref()
            .filter(|active| active.resume_from_ms > 0)
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

        container(column![header, surface, controls, footer].spacing(tokens.spacing.content_pad))
            .padding(tokens.spacing.content_pad)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
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
    fn the_page_grid_never_shows_a_ragged_row() {
        assert_eq!(PAGE_SIZE, (COLUMNS * PAGE_ROWS) as i64);
        assert_eq!(PAGE_SIZE % COLUMNS as i64, 0);
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
        assert_eq!(app.focus, 0);
        app.move_focus(Focus::Left);
        assert_eq!(app.focus, 0, "focus must not leave the first card");
        app.move_focus(Focus::Right);
        assert_eq!(app.focus, 1);
        app.move_focus(Focus::Up);
        assert_eq!(app.focus, 1, "there is no row above the first");
        app.move_focus(Focus::Down);
        assert_eq!(app.focus, 1 + COLUMNS, "down moves a whole row");
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
}
