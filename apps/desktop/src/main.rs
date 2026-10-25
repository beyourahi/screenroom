//! Screenroom desktop client (Windows, Linux, macOS).
//!
//! Milestone status: this is the M1 feasibility skeleton, not the finished
//! desktop UI. It deliberately contains only what has been verified:
//!
//! * the shared Rust core drives the library projection,
//! * libmpv availability is reported instead of assumed,
//! * the shell compiles against iced 0.14 with the wgpu renderer.
//!
//! The design system presentation, full library browsing, player screen and the
//! libmpv render bridge land in later milestones; see `docs/IMPLEMENTATION_PLAN.md`.

mod video_surface;

use std::path::PathBuf;
use std::sync::Arc;

use iced::widget::shader;
use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Alignment, Element, Length, Task};
use screenroom_core::{Library, LibraryQuery};
use screenroom_player::PlayerCapability;
use video_surface::{Frame, VideoSurface};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Screenroom")
        .theme(App::theme)
        .window_size((1280.0, 800.0))
        .run()
}

/// Where the library lives, or in memory when the environment says so.
fn library_path() -> Option<PathBuf> {
    std::env::var_os("SCREENROOM_LIBRARY").map(PathBuf::from)
}

struct App {
    library: Arc<Library>,
    profile_id: String,
    items: Vec<screenroom_core::LibraryItem>,
    player: PlayerCapability,
    error: Option<String>,
    /// Current decoded frame for the presentation bridge; `None` means no video.
    frame: Option<Frame>,
}

#[derive(Debug, Clone)]
enum Message {
    Refresh,
    Close,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let (library, error) = match library_path() {
            Some(path) => match Library::open(&path) {
                Ok(library) => (library, None),
                Err(error) => (
                    Library::open_in_memory().expect("in-memory library"),
                    Some(format!("{}: {}", error.code(), error)),
                ),
            },
            None => (Library::open_in_memory().expect("in-memory library"), None),
        };
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
            player: screenroom_player::capability(),
            error,
            frame: None,
        };
        app.reload();
        (app, Task::none())
    }

    fn reload(&mut self) {
        let query = LibraryQuery {
            profile_id: Some(self.profile_id.clone()),
            sort: screenroom_core::Sort::Title,
            ..LibraryQuery::page(100)
        };
        match self.library.items(&query) {
            Ok(items) => {
                self.items = items;
                self.error = None;
            }
            Err(error) => self.error = Some(format!("{}: {}", error.code(), error)),
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Refresh => self.reload(),
            Message::Close => return iced::exit(),
        }
        Task::none()
    }

    fn theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }

    fn view(&self) -> Element<'_, Message> {
        let header = row![
            text("Screenroom").size(28),
            text(format!(
                "{} item(s) · libmpv {}",
                self.items.len(),
                if self.player.available {
                    self.player.api_version.as_deref().unwrap_or("available")
                } else {
                    "unavailable"
                }
            ))
            .size(14),
            button("Refresh").on_press(Message::Refresh),
            button("Close").on_press(Message::Close),
        ]
        .spacing(16)
        .align_y(Alignment::Center);

        let body: Element<'_, Message> = if self.frame.is_some() {
            // The decoded frame is drawn on the GPU through the shared wgpu device.
            shader::Shader::new(VideoSurface::new(self.frame.clone()))
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else if let Some(error) = &self.error {
            text(error.clone()).into()
        } else if self.items.is_empty() {
            column![
                text("No media sources yet").size(20),
                text("Add an authorized folder to scan your library.")
                    .size(14),
                text(if self.player.available {
                    "Local playback is available on this device."
                } else {
                    "Local playback is unavailable on this device; the reason is reported, not hidden."
                })
                .size(12),
            ]
            .spacing(8)
            .into()
        } else {
            let mut list = column![].spacing(4);
            for item in &self.items {
                let label = if item.episode_label().is_empty() {
                    item.title.clone()
                } else {
                    format!("{} · {}", item.title, item.episode_label())
                };
                list = list.push(
                    row![
                        text(label),
                        text(if item.available { "" } else { "unavailable" }).size(12)
                    ]
                    .spacing(12),
                );
            }
            scrollable(list).height(Length::Fill).into()
        };

        container(column![header, body].spacing(24))
            .padding(28)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
