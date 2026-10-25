//! Typed domain model.
//!
//! Every identifier is an application-generated UUID string. Provider IDs and
//! filesystem locators are attributes, never identity.

use serde::{Deserialize, Serialize};

/// Broad shape of a library entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    /// A single film, including alternate cuts.
    Movie,
    /// An episodic series.
    Series,
    /// A playable file whose shape could not be determined honestly.
    Unknown,
}

impl MediaKind {
    /// Storage form of the kind, matching the existing library rows.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Movie => "movie",
            Self::Series => "series",
            Self::Unknown => "unknown",
        }
    }

    /// Parses a stored kind.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "movie" => Some(Self::Movie),
            "series" => Some(Self::Series),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

/// Availability of an authorized media root.
///
/// `PermissionLost` is deliberately distinct from `Disconnected` and `Missing`:
/// the drive is present but the grant was revoked, which has a different
/// recovery action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceState {
    /// The root is reachable and readable.
    Available,
    /// The volume or mount is not attached.
    Disconnected,
    /// The path exists but the OS denied access.
    PermissionLost,
    /// The path is gone.
    Missing,
    /// The user removed the source from the library; originals are untouched.
    Removed,
}

impl SourceState {
    /// Storage form, matching the existing library rows.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Disconnected => "disconnected",
            Self::PermissionLost => "permissionLost",
            Self::Missing => "missing",
            Self::Removed => "removed",
        }
    }

    /// Parses a stored source state.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "available" => Some(Self::Available),
            "disconnected" => Some(Self::Disconnected),
            "permissionLost" => Some(Self::PermissionLost),
            "missing" => Some(Self::Missing),
            "removed" => Some(Self::Removed),
            _ => None,
        }
    }
}

/// An authorized media root the user granted the app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    /// Stable application identifier.
    pub id: String,
    /// User-facing label, independent of the mount path.
    pub label: String,
    /// Canonical filesystem root.
    pub root_path: String,
    /// Current availability.
    pub state: SourceState,
    /// Last successful scan, in Unix milliseconds.
    pub last_scan_ms: Option<i64>,
}

/// A local, non-authenticated viewer profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    /// Stable application identifier.
    pub id: String,
    /// Display name.
    pub name: String,
}

/// One playable file projected for a library or home screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryItem {
    /// Media file identifier.
    pub id: String,
    /// Stable work identifier (movie or series).
    pub work_id: String,
    /// Edition (cut/runtime) identifier.
    pub edition_id: String,
    /// Owning source identifier.
    pub source_id: String,
    /// Work title.
    pub title: String,
    /// File name as found on disk.
    pub filename: String,
    /// Path relative to the source root.
    pub relative_path: String,
    /// Movie, series or unknown.
    pub kind: MediaKind,
    /// Work synopsis, empty when unknown.
    pub overview: String,
    /// Episode title for series entries.
    pub episode_title: String,
    /// Episode synopsis for series entries.
    pub episode_overview: String,
    /// App-owned poster path, if artwork bytes were retained.
    pub poster_path: Option<String>,
    /// Genre labels known for the work.
    pub genres: Vec<String>,
    /// Release year, when confident.
    pub year: Option<i64>,
    /// Season number for series entries.
    pub season: Option<i64>,
    /// First episode number covered by this file.
    pub episode: Option<i64>,
    /// Last episode number covered by this file, for combined episodes.
    pub episode_end: Option<i64>,
    /// Known duration in milliseconds.
    pub duration_ms: i64,
    /// Resume position in milliseconds.
    pub position_ms: i64,
    /// When the file was added, in Unix milliseconds.
    pub added_ms: i64,
    /// Whether the file is readable now.
    pub available: bool,
    /// Whether the profile finished this edition.
    pub watched: bool,
    /// Whether the work is a favourite.
    pub favorite: bool,
    /// Whether the work is on the watchlist.
    pub watchlisted: bool,
    /// Whether a user correction protects this match from automatic enrichment.
    pub manual_match: bool,
}

impl LibraryItem {
    /// `S01E02`, `S01E02–E03` for combined files, or empty for films.
    pub fn episode_label(&self) -> String {
        match (self.kind, self.season, self.episode) {
            (MediaKind::Series, Some(season), Some(episode)) => {
                let mut label = format!("S{season:02}E{episode:02}");
                if let Some(end) = self.episode_end
                    && end != episode
                {
                    label.push_str(&format!("\u{2013}E{end:02}"));
                }
                label
            }
            _ => String::new(),
        }
    }
}

/// A library suggestion restricted to what the device actually holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recommendation {
    /// Candidate entry.
    pub item: LibraryItem,
    /// Plain-language explanation of why it was suggested.
    pub reason: String,
}

/// An authoritative viewing session.
///
/// Native player callbacks carry this identity; the store rejects progress from
/// any other session or generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackSession {
    /// Session identifier.
    pub id: String,
    /// Owning profile.
    pub profile_id: String,
    /// Media file being played.
    pub file_id: String,
    /// Edition being played.
    pub edition_id: String,
    /// Monotonic generation used to reject stale callbacks.
    pub generation: i64,
}

/// A verified interval the player may offer to skip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentMarker {
    /// Marker identifier.
    pub id: String,
    /// Edition the marker belongs to.
    pub edition_id: String,
    /// `intro` or `credits`.
    pub kind: String,
    /// Start in milliseconds.
    pub start_ms: i64,
    /// End in milliseconds.
    pub end_ms: i64,
    /// Edition duration the marker was validated against.
    pub duration_ms: i64,
}

/// Durable background work state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundJob {
    /// Job identifier.
    pub id: String,
    /// Job kind, for example `metadata`.
    pub kind: String,
    /// Target identifier (usually a work).
    pub target: String,
    /// Lifecycle state.
    pub status: String,
    /// Attempts so far.
    pub attempts: i64,
    /// Scheduling priority; higher runs first.
    pub priority: i64,
    /// Earliest retry, in Unix milliseconds.
    pub next_retry_ms: i64,
    /// Last error, if any.
    pub error: Option<String>,
}
