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
    CoreError, Library, LibraryItem, LibraryQuery, LibraryScanner, MediaKind, Preferences, Profile,
    ScanToken, Sort, Source, SourceState,
};
use screenroom_player::{
    MediaTrack, NO_TRACK, Player, PlayerCapability, PlayerSnapshot, TrackKind,
};

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
            browse: Browse::new(),
            detail: None,
            player: None,
            snapshot: PlayerSnapshot::default(),
            frame: None,
            active: None,
            scrub: None,
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
        // After the save, so the episode is stored and marked watched before the
        // next one takes over the session.
        if ended {
            self.advance_to_next_episode();
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
            Message::SubtitleLanguageChanged(value) => self.subtitle_input = value,
            Message::AudioLanguageChanged(value) => self.audio_input = value,
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
            Screen::Player => "Space play/pause · ←/→ seek 10s · Esc back",
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
                        }),
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

        let (audio_choices, audio_selected) = self.track_chooser(TrackKind::Audio);
        let (subtitle_choices, subtitle_selected) = self.track_chooser(TrackKind::Subtitle);
        let track_row: Element<'_, Message> = if audio_choices.is_empty()
            && subtitle_choices.is_empty()
        {
            text("The player reported no selectable audio or subtitle tracks for this file.")
                .size(tokens.type_scale.micro)
                .color(tokens.palette.ink_muted)
                .into()
        } else {
            row![
                text("Audio")
                    .size(tokens.type_scale.micro)
                    .color(tokens.palette.ink_muted),
                pick_list(audio_choices, audio_selected, |choice: TrackChoice| {
                    Message::SelectTrack(TrackKind::Audio, choice.id)
                }),
                text("Subtitles")
                    .size(tokens.type_scale.micro)
                    .color(tokens.palette.ink_muted),
                pick_list(
                    subtitle_choices,
                    subtitle_selected,
                    |choice: TrackChoice| { Message::SelectTrack(TrackKind::Subtitle, choice.id) }
                ),
            ]
            .spacing(tokens.spacing.field_x)
            .align_y(Alignment::Center)
            .into()
        };

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

        container(
            column![header, surface, controls, track_row, footer]
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
    fn choosing_an_unknown_profile_changes_nothing() {
        let (mut app, _directory) = app_with(1);
        let before = app.profile_id.clone();
        let _ = app.update(Message::ProfileChosen("no-such-profile".to_string()));
        assert_eq!(app.profile_id, before);
        assert!(app.error.is_some());
    }
}
