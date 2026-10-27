//! The SQLite-backed library: sources, catalog, playback state and lists.
//!
//! One [`Library`] owns one connection. Every public method takes `&self`, so a
//! UI thread, a scanner worker and a native player callback can share it; the
//! connection lock serialises writes and transactions.
//!
//! Availability never deletes: marking a source missing or a file unavailable
//! keeps works, editions, progress and lists intact.

mod backup;
mod catalog;
mod lists;
mod playback;
mod settings;
mod sources;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::domain::Profile;
use crate::error::{CoreError, Result};
use crate::{SCHEMA_VERSION, schema};

pub use backup::{BackupInfo, BackupReport, RestoreReport};
pub use catalog::Ingest;
pub use playback::Checkpoint;
pub use settings::Preferences;

/// Ordering choices for a library page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    /// Title, then season and episode.
    #[default]
    Title,
    /// Newest additions first.
    Added,
    /// Newest release year first.
    Year,
    /// Most recently watched first.
    Progress,
}

impl Sort {
    /// SQL `ORDER BY` fragment for this ordering.
    fn order_by(self) -> &'static str {
        match self {
            Self::Title => {
                "title COLLATE NOCASE, COALESCE(season_number, 0), COALESCE(episode_number, 0), id"
            }
            Self::Added => "added_ms DESC, id",
            Self::Year => "year DESC, title COLLATE NOCASE, id",
            Self::Progress => "progress_updated_ms DESC, id",
        }
    }
}

/// A bounded, paged library query.
///
/// `limit` is clamped to `1..=500` so a client can never request an unbounded
/// page; the UI must page rather than load a 10k-item library at once.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryQuery {
    /// Profile whose progress and lists are projected. `None` means no profile.
    pub profile_id: Option<String>,
    /// Case-insensitive substring match over work title and filename.
    pub text: String,
    /// Restrict to one kind.
    pub kind: Option<String>,
    /// Restrict to one work.
    pub work_id: Option<String>,
    /// Restrict to one genre label.
    pub genre: Option<String>,
    /// Restrict to one season number.
    pub season: Option<i64>,
    /// Only files that are readable right now.
    pub available_only: bool,
    /// `Some(true)` watched only, `Some(false)` unwatched only.
    pub watched: Option<bool>,
    /// Only works on the watchlist.
    pub watchlist_only: bool,
    /// Only favourite works.
    pub favorites_only: bool,
    /// Collapse to one representative file per work.
    pub group_by_work: bool,
    /// Ordering.
    pub sort: Sort,
    /// Zero-based row offset.
    pub offset: i64,
    /// Page size, `1..=500`.
    pub limit: i64,
}

impl LibraryQuery {
    /// A first page of `limit` items for the default ordering.
    pub fn page(limit: i64) -> Self {
        Self {
            limit,
            ..Self::default()
        }
    }
}

/// How the folder holding the library compares the case of a path.
///
/// This decides one thing only: whether two roots that differ just in case are the
/// same folder. It is a property of the file system, not of the operating system,
/// which is why it is a value rather than a `cfg!` — a case-sensitive APFS or NTFS
/// volume exists, and so does a case-insensitive one on Linux.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathSemantics {
    /// `Shows` and `shows` are the same folder. Windows and the default macOS and
    /// iOS volumes behave this way.
    CaseInsensitive,
    /// `Shows` and `shows` are different folders, as on a case-sensitive volume and
    /// on Linux generally.
    CaseSensitive,
}

impl PathSemantics {
    /// What this machine's volumes do by default.
    ///
    /// A guess, and named as one: it follows the platform's usual file system. A
    /// library on an unusual volume can override it with
    /// [`Library::with_path_semantics`].
    pub fn host() -> Self {
        if cfg!(any(windows, target_vendor = "apple")) {
            Self::CaseInsensitive
        } else {
            Self::CaseSensitive
        }
    }
}

/// A single SQLite library.
pub struct Library {
    connection: Mutex<Connection>,
    active_scans: Mutex<HashSet<String>>,
    path_semantics: PathSemantics,
    /// Where the database lives, or `None` for an in-memory library.
    ///
    /// Kept so backup and restore can tell the live file from a copy of it, and
    /// so a safety copy has somewhere to go.
    path: Option<PathBuf>,
}

impl std::fmt::Debug for Library {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Library").finish_non_exhaustive()
    }
}

impl Library {
    /// Opens or creates a library at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        Self::from_connection(
            Connection::open(path)?,
            Some(path.to_path_buf()),
            PathSemantics::host(),
        )
    }

    /// Opens a throwaway in-memory library, used by tests and previews.
    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?, None, PathSemantics::host())
    }

    /// Chooses how this library compares paths.
    ///
    /// The default is
    /// [`PathSemantics::host`], which is a guess about the volumes this machine
    /// mounts: most macOS and Windows volumes fold case, but a case-sensitive APFS
    /// or NTFS volume does not, and on that volume the default would treat two
    /// spellings of one folder as two folders. Naming the semantics explicitly is
    /// the way to be certain, and it is also what makes the other platform's
    /// behaviour testable from this one.
    pub fn with_path_semantics(mut self, semantics: PathSemantics) -> Self {
        self.path_semantics = semantics;
        self
    }

    fn from_connection(
        mut connection: Connection,
        path: Option<PathBuf>,
        path_semantics: PathSemantics,
    ) -> Result<Self> {
        connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")?;
        let version: i64 =
            connection.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?;

        match version {
            0 => {
                let tx = connection.transaction()?;
                tx.execute_batch("PRAGMA foreign_keys = ON;")?;
                for statement in schema::CREATE {
                    tx.execute(statement, [])?;
                }
                tx.execute(
                    "INSERT INTO profiles(id, name) VALUES(?1, ?2)",
                    [new_id(), "Me".to_string()],
                )?;
                tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
                tx.commit()?;
            }
            v if v == SCHEMA_VERSION => {}
            1 => {
                return Err(CoreError::library(
                    "migration_unavailable",
                    "This database needs an unsupported migration.",
                ));
            }
            future => {
                return Err(CoreError::library(
                    "future_database",
                    format!(
                        "This library was written by a newer Screenroom (schema {future}). \
                         Update the app instead of resetting your data."
                    ),
                ));
            }
        }

        let integrity: String =
            connection.query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0))?;
        if integrity != "ok" {
            return Err(CoreError::library(
                "corrupt_database",
                "The library failed its integrity check. Restore a validated backup.",
            ));
        }

        // A crashed process cannot stay authoritative for progress or jobs.
        connection.execute("UPDATE sessions SET status = 'closed'", [])?;
        connection.execute(
            "UPDATE jobs SET status = 'queued' WHERE status = 'running'",
            [],
        )?;

        Ok(Self {
            connection: Mutex::new(connection),
            active_scans: Mutex::new(HashSet::new()),
            path_semantics,
            path,
        })
    }

    /// Borrows the connection for one statement or transaction.
    ///
    /// A panic elsewhere must not brick the library, so a poisoned lock is
    /// recovered rather than unwrapped.
    fn connection(&self) -> MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Registers an in-flight scan for `source_id`.
    ///
    /// Two concurrent walks over one root would double-import and fight over
    /// availability, so the second caller gets `scan_running`.
    pub(crate) fn claim_scan(&self, source_id: &str) -> Result<()> {
        let mut active = self
            .active_scans
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !active.insert(source_id.to_string()) {
            return Err(CoreError::library(
                "scan_running",
                "This source is already being scanned.",
            ));
        }
        Ok(())
    }

    /// Clears the in-flight scan registration.
    pub(crate) fn release_scan(&self, source_id: &str) {
        self.active_scans
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(source_id);
    }

    /// Canonicalises a user-granted root.
    ///
    /// Fails closed with a specific reason: a missing folder and a revoked
    /// permission need different recovery copy in the UI.
    pub(crate) fn canonical_root(&self, root: impl AsRef<Path>) -> Result<PathBuf> {
        let root = root.as_ref();
        if !root.is_absolute() {
            return Err(CoreError::library(
                "invalid_root",
                "Choose an absolute folder path.",
            ));
        }
        if !root.exists() {
            return Err(CoreError::library(
                "source_missing",
                "The selected folder is unavailable.",
            ));
        }
        std::fs::canonicalize(root).map_err(|error| match error.kind() {
            std::io::ErrorKind::PermissionDenied => CoreError::library(
                "permission_lost",
                "The selected folder cannot be read. Grant access again.",
            ),
            std::io::ErrorKind::NotFound => {
                CoreError::library("source_missing", "The selected folder is unavailable.")
            }
            _ => CoreError::Io(error),
        })
    }

    /// Normalises a path for overlap comparison.
    ///
    /// Folded when the volume folds case, untouched when it does not. Note that
    /// folding is **not** a substitute for resolving a path: this compares two
    /// already-canonical roots, so it is only deciding whether the same folder was
    /// named twice.
    ///
    /// Public because the decision above is one a caller may need to see: a bug
    /// report about a folder being refused as a duplicate is answered by this
    /// value, and it is the one part of the library's behaviour that depends on the
    /// volume rather than on the data.
    pub fn path_key(&self, path: &Path) -> String {
        let raw = path.to_string_lossy().to_string();
        match self.path_semantics {
            PathSemantics::CaseInsensitive => raw.to_lowercase(),
            PathSemantics::CaseSensitive => raw,
        }
    }

    /// Lists viewer profiles, oldest first.
    pub fn profiles(&self) -> Result<Vec<Profile>> {
        let connection = self.connection();
        let mut statement = connection.prepare("SELECT id, name FROM profiles ORDER BY rowid")?;
        let rows = statement.query_map([], |row| {
            Ok(Profile {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Adds a profile, keeping the existing viewer data untouched.
    pub fn add_profile(&self, name: &str) -> Result<Profile> {
        let name = validated(name, 120)?.to_string();
        let profile = Profile { id: new_id(), name };
        self.connection().execute(
            "INSERT INTO profiles(id, name) VALUES(?1, ?2)",
            [&profile.id, &profile.name],
        )?;
        Ok(profile)
    }

    /// Renames a profile.
    pub fn rename_profile(&self, id: &str, name: &str) -> Result<()> {
        let name = validated(name, 120)?.to_string();
        let changed = self.connection().execute(
            "UPDATE profiles SET name = ?1 WHERE id = ?2",
            [name, id.to_string()],
        )?;
        if changed == 0 {
            return Err(CoreError::library(
                "profile_missing",
                "That profile is no longer in the library.",
            ));
        }
        Ok(())
    }

    /// Removes a profile and its progress, refusing to leave the device without one.
    pub fn remove_profile(&self, id: &str) -> Result<()> {
        let mut connection = self.connection();
        let tx = connection.transaction()?;
        let count: i64 = tx.query_row("SELECT COUNT(*) FROM profiles", [], |row| row.get(0))?;
        if count <= 1 {
            return Err(CoreError::library(
                "last_profile",
                "Keep at least one profile.",
            ));
        }
        tx.execute("DELETE FROM profiles WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }
}

/// Trims and validates user-supplied text.
pub(crate) fn validated(value: &str, max: usize) -> Result<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > max || trimmed.contains('\0') {
        return Err(CoreError::library("invalid_input", "Enter a valid value."));
    }
    Ok(trimmed)
}

/// Current Unix time in milliseconds.
pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

/// A fresh application-generated identifier.
pub(crate) fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
