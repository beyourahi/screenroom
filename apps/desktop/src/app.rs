//! The desktop application: the library list and the M2 player screen.
//!
//! What this layer is responsible for, and what it is not:
//!
//! * it decides *when* to open a session, checkpoint and close it, using
//!   [`PlaybackTracker`]; the rules themselves live in `screenroom-core`,
//! * it shows the frame the player published, and nothing it did not publish,
//! * it reports playback failures with the player's own message instead of
//!   leaving an empty black area.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use iced::widget::shader;
use iced::widget::{Space, button, column, container, row, scrollable, slider, text};
use iced::{Alignment, Element, Length, Subscription, Task, keyboard};
use screenroom_core::{Library, LibraryItem, LibraryQuery, Sort};
use screenroom_player::{Player, PlayerCapability, PlayerSnapshot};

use crate::playback::PlaybackTracker;
use crate::video_surface::{Frame, VideoSurface};

/// How often the player is polled for frames and position.
///
/// 16 ms keeps a 30 fps source smooth without a subscription that keeps the
/// runtime awake when no player is open.
const TICK: Duration = Duration::from_millis(16);

/// Starts the desktop application.
pub fn run() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Screenroom")
        .theme(App::theme)
        .subscription(App::subscription)
        .window_size((1280.0, 800.0))
        .run()
}

/// Which surface is on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Library,
    Player,
}

/// The viewing session currently open, and the policy around it.
struct Active {
    tracker: PlaybackTracker,
    file_id: String,
    title: String,
    resume_from_ms: i64,
}

/// Application state.
pub struct App {
    library: Arc<Library>,
    profile_id: String,
    items: Vec<LibraryItem>,
    player: Option<Player>,
    capability: PlayerCapability,
    screen: Screen,
    snapshot: PlayerSnapshot,
    frame: Option<Frame>,
    active: Option<Active>,
    scrub: Option<f64>,
    error: Option<String>,
}

/// User intent.
#[derive(Debug, Clone)]
pub enum Message {
    /// Reload the library projection.
    Refresh,
    /// Open one file.
    Open(String),
    /// Leave the player and return to the library.
    Back,
    /// Play or pause.
    TogglePlay,
    /// The scrub control moved; nothing is committed yet.
    Scrub(f64),
    /// Commit the scrubbed position.
    SeekCommit,
    /// Poll the player.
    Tick,
    /// A keyboard event.
    Keyboard(keyboard::Event),
    /// Quit.
    Close,
}

/// Drives the player: one thread at [`TICK`], alive only while the player screen
/// is visible because the subscription is dropped with it.
fn ticks() -> Subscription<Message> {
    use iced::futures::StreamExt;
    use iced::futures::channel::mpsc;

    Subscription::run(|| {
        let (sender, receiver) = mpsc::unbounded::<()>();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(TICK);
                if sender.unbounded_send(()).is_err() {
                    // The subscription was dropped: the player is gone.
                    break;
                }
            }
        });
        receiver.map(|()| Message::Tick)
    })
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let (library, error) = match std::env::var_os("SCREENROOM_LIBRARY") {
            Some(path) => match Library::open(PathBuf::from(path)) {
                Ok(library) => (library, None),
                Err(error) => (
                    Library::open_in_memory().expect("in-memory library"),
                    Some(format!("{}: {}", error.code(), error)),
                ),
            },
            None => (Library::open_in_memory().expect("in-memory library"), None),
        };
        (Self::with_library(library, error), Task::none())
    }

    /// Builds the application around an existing library, for tests.
    pub fn with_library(library: Library, error: Option<String>) -> Self {
        let library = Arc::new(library);
        let profile_id = library
            .profiles()
            .ok()
            .and_then(|profiles| profiles.into_iter().next())
            .map(|profile| profile.id)
            .unwrap_or_default();
        let mut app = Self {
            library,
            profile_id,
            items: Vec::new(),
            player: None,
            capability: screenroom_player::capability(),
            screen: Screen::Library,
            snapshot: PlayerSnapshot::default(),
            frame: None,
            active: None,
            scrub: None,
            error,
        };
        app.reload();
        app
    }

    fn reload(&mut self) {
        let query = LibraryQuery {
            profile_id: Some(self.profile_id.clone()),
            sort: Sort::Title,
            ..LibraryQuery::page(200)
        };
        match self.library.items(&query) {
            Ok(items) => {
                self.items = items;
                self.error = None;
            }
            Err(error) => self.error = Some(format!("{}: {}", error.code(), error)),
        }
    }

    /// Opens a file: session first, then the player, then the resume seek.
    fn open(&mut self, file_id: &str) {
        let Some(item) = self.items.iter().find(|item| item.id == file_id).cloned() else {
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
            Err(error) => {
                self.error = Some(format!("{}: {}", error.code(), error));
                return;
            }
        };

        let player = self.player.as_ref().expect("player started above");
        player.load_at(&path, Some(opened.resume_from_ms as f64 / 1_000.0));
        self.active = Some(Active {
            tracker: opened.tracker,
            file_id: item.id.clone(),
            title: item.title.clone(),
            resume_from_ms: opened.resume_from_ms,
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
        if let (Some(snapshot), Some(mut active)) = (snapshot, self.active.take()) {
            // The final position is stored before the session closes, so leaving
            // during playback never loses more than the checkpoint interval.
            self.save(&mut active, &snapshot, snapshot.ended);
            if let Err(error) = active.tracker.close(&self.library) {
                self.error = Some(format!("{}: {}", error.code(), error));
            }
        }
        if let Some(player) = &self.player {
            player.stop();
        }
        self.screen = Screen::Library;
        self.frame = None;
        self.snapshot = PlayerSnapshot::default();
        self.scrub = None;
        self.reload();
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
            Err(error) => self.error = Some(format!("{}: {}", error.code(), error)),
        }
    }

    fn poll(&mut self) {
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
        let paused = matches!(self.snapshot.state, screenroom_player::PlayerState::Paused);
        player.set_paused(!paused);
    }

    fn commit_seek(&mut self) {
        let Some(seconds) = self.scrub.take() else {
            return;
        };
        let Some(player) = &self.player else { return };
        // The seek generation must advance before the player reports the new
        // position, otherwise the core would reject those checkpoints.
        if let Some(active) = &mut self.active
            && let Err(error) = active.tracker.seek(&self.library)
        {
            self.error = Some(format!("{}: {}", error.code(), error));
            return;
        }
        player.seek(seconds);
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Refresh => self.reload(),
            Message::Open(file_id) => self.open(&file_id),
            Message::Back => self.leave_player(),
            Message::TogglePlay => self.toggle_play(),
            Message::Scrub(seconds) => self.scrub = Some(seconds),
            Message::SeekCommit => self.commit_seek(),
            Message::Tick => self.poll(),
            Message::Keyboard(event) => self.handle_key(event),
            Message::Close => return iced::exit(),
        }
        Task::none()
    }

    fn handle_key(&mut self, event: keyboard::Event) {
        use iced::keyboard::Key;
        use iced::keyboard::key::Named;
        let keyboard::Event::KeyPressed { key, .. } = event else {
            return;
        };
        match key {
            Key::Named(Named::Escape) => {
                if self.screen == Screen::Player {
                    self.leave_player();
                }
            }
            Key::Named(Named::Space) if self.screen == Screen::Player => self.toggle_play(),
            _ => {}
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![iced::keyboard::listen().map(Message::Keyboard)];
        if self.screen == Screen::Player {
            subscriptions.push(ticks());
        }
        Subscription::batch(subscriptions)
    }

    fn theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }

    fn view(&self) -> Element<'_, Message> {
        match self.screen {
            Screen::Library => self.library_view(),
            Screen::Player => self.player_view(),
        }
    }

    fn library_view(&self) -> Element<'_, Message> {
        let capability = if self.capability.available {
            format!(
                "local playback · libmpv {}",
                self.capability.api_version.as_deref().unwrap_or("?")
            )
        } else {
            format!("playback unavailable · {}", self.capability.reason)
        };

        let header = row![
            text("Screenroom").size(28),
            text(format!("{} item(s)", self.items.len())).size(14),
            text(capability).size(12),
            button("Refresh").on_press(Message::Refresh),
            button("Close").on_press(Message::Close),
        ]
        .spacing(16)
        .align_y(Alignment::Center);

        let body: Element<'_, Message> = if let Some(error) = &self.error {
            text(error.clone()).into()
        } else if self.items.is_empty() {
            column![
                text("No media sources yet").size(20),
                text("Add an authorized folder to scan your library.").size(14),
            ]
            .spacing(8)
            .into()
        } else {
            let mut list = column![].spacing(4);
            for item in &self.items {
                list = list.push(self.item_row(item));
            }
            scrollable(list).height(Length::Fill).into()
        };

        container(column![header, body].spacing(24))
            .padding(28)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn item_row(&self, item: &LibraryItem) -> Element<'_, Message> {
        let subtitle = if item.episode_label().is_empty() {
            item.title.clone()
        } else {
            format!("{} · {}", item.title, item.episode_label())
        };
        let progress = if item.duration_ms > 0 && item.position_ms > 0 {
            format!(
                "{} / {}",
                clock(item.position_ms as f64 / 1_000.0),
                clock(item.duration_ms as f64 / 1_000.0)
            )
        } else if item.duration_ms > 0 {
            clock(item.duration_ms as f64 / 1_000.0)
        } else {
            "duration unknown".to_string()
        };
        let mut open = button(text(format!("{subtitle}  ·  {progress}"))).width(Length::Fill);
        if item.available {
            open = open.on_press(Message::Open(item.id.clone()));
        } else {
            open = open.on_press(Message::Refresh);
        }
        let state = if item.watched {
            "watched"
        } else if !item.available {
            "unavailable"
        } else {
            ""
        };
        row![open, text(state).size(12)]
            .spacing(12)
            .align_y(Alignment::Center)
            .into()
    }

    fn player_view(&self) -> Element<'_, Message> {
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
            button("Back").on_press(Message::Back),
            text(title).size(22),
            text(state).size(12),
        ]
        .spacing(16)
        .align_y(Alignment::Center);

        let surface: Element<'_, Message> = match (&self.snapshot.failure, &self.frame) {
            (Some(failure), _) => text(format!("Playback failed — {}", failure.message))
                .size(16)
                .into(),
            (None, Some(frame)) => shader::Shader::new(VideoSurface::new(Some(frame.clone())))
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            (None, None) => text("Preparing streams…").size(16).into(),
        };

        let duration = self.snapshot.duration.max(0.0);
        let position = self.scrub.unwrap_or(self.snapshot.position);
        let controls = row![
            button(
                if matches!(self.snapshot.state, screenroom_player::PlayerState::Paused) {
                    "Play"
                } else {
                    "Pause"
                }
            )
            .on_press(Message::TogglePlay),
            text(clock(position)).size(13),
            slider(0.0..=duration.max(1.0), position, Message::Scrub)
                .on_release(Message::SeekCommit)
                .step(0.1)
                .width(Length::Fill),
            text(clock(duration)).size(13),
        ]
        .spacing(12)
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
                .size(12)
            })
            .unwrap_or_else(|| text("").size(12));

        let footer = row![
            resume,
            Space::new().width(Length::Fill),
            text(format!(
                "file {}",
                self.active
                    .as_ref()
                    .map(|a| a.file_id.as_str())
                    .unwrap_or("-")
            ))
            .size(11),
        ]
        .spacing(12);

        container(column![header, surface, controls, footer].spacing(16))
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
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
    fn opening_an_unknown_file_changes_nothing() {
        let library = Library::open_in_memory().expect("library");
        let mut app = App::with_library(library, None);
        app.open("no-such-file");
        assert_eq!(app.screen, Screen::Library);
        assert!(app.active.is_none());
        assert!(app.error.is_none());
    }

    #[test]
    fn an_unreadable_item_is_refused_with_a_recovery_hint() {
        let directory = tempfile::TempDir::new().expect("temp dir");
        let library = Library::open_in_memory().expect("library");
        let source = library.add_source(directory.path(), None).expect("source");
        let parsed = screenroom_core::parse_filename("Gone.2020.mkv", None);
        let file_id = library
            .ingest_file(screenroom_core::Ingest {
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
        let mut app = App::with_library(library, None);
        app.reload();
        app.open(&file_id);
        assert_eq!(app.screen, Screen::Library);
        assert!(
            app.error
                .as_deref()
                .is_some_and(|error| error.contains("relink"))
        );
    }
}
