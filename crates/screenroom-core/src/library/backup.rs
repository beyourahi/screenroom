//! Backup and restore of the whole library.
//!
//! The library is one SQLite database, so a backup is a consistent copy of that
//! database rather than an export that could drift from it. `VACUUM INTO` does the
//! copy: it is the only supported way to get a coherent snapshot of a live
//! connection, and it produces an ordinary readable database with no journal to
//! replay.
//!
//! Restoring has to be safe in both directions, which shapes every decision here:
//!
//! * a backup is always a copy *to a different file* than the live library, so a
//!   mistaken destination cannot truncate the user's history,
//! * the file is validated (integrity, schema range, format version) before
//!   anything is replaced, so an unrelated `.db` is refused rather than half-read,
//! * the current library is copied aside first, so a restore the user regrets is
//!   itself recoverable,
//! * the replacement happens in one transaction through `ATTACH`, so an
//!   interrupted restore leaves the previous library intact instead of a mixture
//!   of the two.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::{BACKUP_FORMAT_VERSION, SCHEMA_VERSION};

use super::{Library, new_id, now_ms};

/// The library's tables, in an order that satisfies the foreign keys on insert.
///
/// Delete order is the reverse, so children go before parents. Both orders exist
/// because the restore cannot rely on `ON DELETE CASCADE` alone: a table whose
/// parent is emptied first would lose its own rows before they were read.
const TABLES: &[&str] = &[
    "sources",
    "works",
    "seasons",
    "episodes",
    "editions",
    "files",
    "artwork",
    "file_episodes",
    "profiles",
    "sessions",
    "progress",
    "watchlist",
    "overrides",
    "markers",
    "external_identities",
    "jobs",
    "settings",
];

/// Setting key holding the backup format version inside a backup file.
const FORMAT_KEY: &str = "backupFormatVersion";

/// Setting key holding the creation time inside a backup file.
const CREATED_KEY: &str = "backupCreatedMs";

/// What a backup file contains, read without changing anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupInfo {
    /// Path the information was read from.
    pub path: String,
    /// Versioned backup format, from [`crate::BACKUP_FORMAT_VERSION`].
    pub format_version: i64,
    /// Library schema version the backup was written with.
    pub schema_version: i64,
    /// When the backup was written, in Unix milliseconds, when recorded.
    pub created_ms: Option<i64>,
    /// Authorized folders in the backup.
    pub sources: i64,
    /// Works in the backup.
    pub works: i64,
    /// Playable files in the backup.
    pub files: i64,
    /// Viewer profiles in the backup.
    pub profiles: i64,
}

impl std::fmt::Display for BackupInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} works, {} files, {} folders, {} profiles (schema {}, format {})",
            self.works,
            self.files,
            self.sources,
            self.profiles,
            self.schema_version,
            self.format_version
        )
    }
}

/// The outcome of writing a backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupReport {
    /// The file that was written.
    pub path: String,
    /// Bytes written.
    pub bytes: u64,
    /// Contents of the file, as read back and validated.
    pub info: BackupInfo,
}

/// The outcome of restoring a backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreReport {
    /// The backup that was applied.
    pub restored_from: String,
    /// The safety copy of the previous library, when one was written.
    ///
    /// `None` for an in-memory library, which has nothing on disk to preserve.
    pub previous_saved_to: Option<String>,
    /// What the library holds now.
    pub info: BackupInfo,
}

impl Library {
    /// Writes a consistent copy of the whole library to `path`.
    ///
    /// The copy is made to a temporary file in the destination directory and
    /// renamed into place only once it is complete and stamped, so an interrupted
    /// backup never leaves a truncated file where a valid one used to be, and a
    /// failure never leaves a half-written file at the destination at all.
    pub fn backup_to(&self, path: impl AsRef<Path>) -> Result<BackupReport> {
        let destination = absolute(path.as_ref(), "backup")?;
        if let Some(live) = &self.path
            && same_file(live, &destination)
        {
            return Err(CoreError::library(
                "backup_is_library",
                "Choose a different file: that path is the library itself.",
            ));
        }

        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let staging = staging_path(&destination)?;

        // A leftover staging file would make SQLite refuse to write; it is ours
        // and is not the user's data, so removing it is safe.
        let _ = std::fs::remove_file(&staging);

        // Scoped so the connection borrow ends before the file is moved.
        {
            let connection = self.connection();
            connection.execute("VACUUM INTO ?1", [staging.to_string_lossy().to_string()])?;
            stamp(&staging)?;
        }

        std::fs::rename(&staging, &destination).map_err(|error| {
            let _ = std::fs::remove_file(&staging);
            CoreError::Io(error)
        })?;

        let bytes = std::fs::metadata(&destination)?.len();
        let info = inspect(&destination)?;
        Ok(BackupReport {
            path: destination.to_string_lossy().to_string(),
            bytes,
            info,
        })
    }

    /// Validates a backup file without changing anything.
    pub fn inspect_backup(path: impl AsRef<Path>) -> Result<BackupInfo> {
        inspect(path.as_ref())
    }

    /// Replaces this library's contents with a validated backup.
    ///
    /// The previous library is copied aside first, so this is reversible. Nothing
    /// is read from the backup until it has passed validation, and nothing is
    /// written until the copy is in place — the whole replacement is one
    /// transaction, so a failure at any point leaves the library as it was.
    pub fn restore_from(&self, path: impl AsRef<Path>) -> Result<RestoreReport> {
        let source = path.as_ref();
        if let Some(live) = &self.path
            && same_file(live, source)
        {
            return Err(CoreError::library(
                "restore_is_library",
                "That file is the library itself, so there is nothing to restore from it.",
            ));
        }
        let info = inspect(source)?;

        let previous_saved_to = match &self.path {
            Some(live) => {
                let safety = safety_path(live);
                let report = self.backup_to(&safety)?;
                Some(report.path)
            }
            None => None,
        };

        let mut connection = self.connection();
        // SQLite refuses `DETACH` inside a transaction, so the attachment is
        // opened and closed around the copy rather than inside it.
        connection.execute(
            "ATTACH DATABASE ?1 AS backup",
            [source.to_string_lossy().to_string()],
        )?;
        let outcome = replace_contents(&mut connection);
        // Best-effort detach: a failure to detach must not hide the copy's own
        // error, and the attachment dies with the connection regardless.
        let _ = connection.execute_batch("DETACH DATABASE backup;");
        outcome?;
        drop(connection);

        Ok(RestoreReport {
            restored_from: source.to_string_lossy().to_string(),
            previous_saved_to,
            info,
        })
    }

    /// Where this library lives on disk, if it is not an in-memory one.
    pub fn path(&self) -> Option<PathBuf> {
        self.path.clone()
    }
}

/// Empties every table and refills it from the `backup` attachment.
///
/// One transaction, with foreign keys deferred to the commit: the constraints are
/// then checked against the library as a whole instead of at each intermediate
/// step, which is the only way a partial refill could pass at all. If anything
/// fails the transaction rolls back and the previous library is untouched.
fn replace_contents(connection: &mut Connection) -> Result<()> {
    let tx = connection.transaction()?;
    tx.execute_batch("PRAGMA defer_foreign_keys = ON;")?;
    for table in TABLES.iter().rev() {
        tx.execute(&format!("DELETE FROM main.{table}"), [])?;
    }
    // A backup written before a table existed simply has no rows for it, and a
    // restore must not fail over artwork it never had. The tables the file does
    // hold are read, and the rest keep the empty state the delete above left.
    let present: std::collections::HashSet<String> = {
        let mut statement =
            tx.prepare("SELECT name FROM backup.sqlite_master WHERE type = 'table'")?;
        let names = statement.query_map([], |row| row.get::<_, String>(0))?;
        names.collect::<rusqlite::Result<_>>()?
    };
    for table in TABLES {
        if !present.contains(*table) {
            continue;
        }
        tx.execute(
            &format!("INSERT INTO main.{table} SELECT * FROM backup.{table}"),
            [],
        )?;
    }
    // The stamp describes the backup file, not this library. Copying it across
    // would leave the live database claiming to be a backup of itself.
    tx.execute("DELETE FROM main.settings WHERE key = ?1", [FORMAT_KEY])?;
    tx.execute("DELETE FROM main.settings WHERE key = ?1", [CREATED_KEY])?;
    tx.commit()?;
    Ok(())
}

/// Stamps a freshly written backup with its format and creation time.
fn stamp(path: &Path) -> Result<()> {
    let connection = Connection::open(path)?;
    connection.execute_batch(&format!(
        "INSERT INTO settings(key, value) VALUES('{FORMAT_KEY}', '{BACKUP_FORMAT_VERSION}')
           ON CONFLICT(key) DO UPDATE SET value = excluded.value;
         INSERT INTO settings(key, value) VALUES('{CREATED_KEY}', '{}')
           ON CONFLICT(key) DO UPDATE SET value = excluded.value;",
        now_ms()
    ))?;
    Ok(())
}

/// Reads and validates a backup file.
fn inspect(path: &Path) -> Result<BackupInfo> {
    if !path.is_absolute() {
        return Err(CoreError::library(
            "invalid_backup_path",
            "Choose an absolute backup path.",
        ));
    }
    if !path.is_file() {
        return Err(CoreError::library(
            "backup_missing",
            "That backup file is not available.",
        ));
    }

    // Read-only, so validation can never modify the file it is checking.
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| unreadable())?;

    // SQLite opens an arbitrary file lazily, so a file that is not a database at
    // all is only discovered at the first read. Both cases are the same problem
    // for the user — this file cannot be restored from — so both report the same
    // recoverable code rather than leaking a raw database error.
    let integrity: String = connection
        .query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0))
        .map_err(|_| unreadable())?;
    if integrity != "ok" {
        return Err(CoreError::library(
            "backup_corrupt",
            "That backup failed its integrity check.",
        ));
    }

    let schema_version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
        .map_err(|_| unreadable())?;
    if !(2..=SCHEMA_VERSION).contains(&schema_version) {
        return Err(CoreError::library(
            "backup_unsupported",
            "That backup was written by an incompatible Screenroom version.",
        ));
    }

    let format_version = setting(&connection, FORMAT_KEY)
        .and_then(|value| value.parse::<i64>().ok())
        .ok_or_else(|| {
            CoreError::library(
                "backup_unrecognised",
                "That file is a SQLite database but not a Screenroom backup.",
            )
        })?;
    if format_version > BACKUP_FORMAT_VERSION {
        return Err(CoreError::library(
            "backup_unsupported",
            "That backup is newer than this Screenroom can restore.",
        ));
    }

    let created_ms = setting(&connection, CREATED_KEY).and_then(|value| value.parse::<i64>().ok());

    Ok(BackupInfo {
        path: path.to_string_lossy().to_string(),
        format_version,
        schema_version,
        created_ms,
        sources: count(&connection, "sources")?,
        works: count(&connection, "works")?,
        files: count(&connection, "files")?,
        profiles: count(&connection, "profiles")?,
    })
}

/// The one refusal for a file that cannot be read as a Screenroom backup.
fn unreadable() -> CoreError {
    CoreError::library("backup_unreadable", "That file is not a Screenroom backup.")
}

/// Reads one setting from an arbitrary connection.
fn setting(connection: &Connection, key: &str) -> Option<String> {
    connection
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()
        .ok()
        .flatten()
}

/// Counts rows in a table.
fn count(connection: &Connection, table: &str) -> Result<i64> {
    // `table` is one of the fixed names in `TABLES`, never user input.
    Ok(
        connection.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })?,
    )
}

/// Requires an absolute path, because a backup's location must be unambiguous.
fn absolute(path: &Path, what: &'static str) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(CoreError::library(
            "invalid_backup_path",
            format!("Choose an absolute path for the {what}."),
        ));
    }
    if path.is_dir() {
        return Err(CoreError::library(
            "backup_is_folder",
            "Choose a file name, not a folder.",
        ));
    }
    Ok(path.to_path_buf())
}

/// A unique staging path beside the destination, on the same filesystem.
///
/// Same directory so the final move is a rename rather than a copy, which is what
/// makes the replacement atomic.
fn staging_path(destination: &Path) -> Result<PathBuf> {
    let parent = destination.parent().ok_or_else(|| {
        CoreError::library("invalid_backup_path", "Choose an absolute backup path.")
    })?;
    let name = destination
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "library".to_string());
    Ok(parent.join(format!(".{name}.{}.partial", new_id())))
}

/// The path a pre-restore safety copy is written to.
fn safety_path(library: &Path) -> PathBuf {
    let name = library
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "library".to_string());
    library.with_file_name(format!("{name}.before-restore"))
}

/// True when both paths name the same file on disk.
///
/// Compared after canonicalisation, so `./library.db` and `/tmp/library.db` are
/// recognised as the same file; the fallback covers a path that does not exist
/// yet, where canonicalisation cannot help.
fn same_file(left: &Path, right: &Path) -> bool {
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}
