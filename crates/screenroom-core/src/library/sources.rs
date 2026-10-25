//! Authorized media roots.
//!
//! A source carries a stable logical identity that survives a moved drive or a
//! changed mount path. Removing a source only marks it removed; originals are
//! never touched and history is retained.

use std::path::{Path, PathBuf};

use rusqlite::{OptionalExtension, Row, params};

use super::{Library, validated};
use crate::domain::{Source, SourceState};
use crate::error::{CoreError, Result};

impl Library {
    /// Lists sources still in the library, ordered by label.
    pub fn sources(&self) -> Result<Vec<Source>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, label, root_path, state, last_scan_ms FROM sources
             WHERE state != 'removed' ORDER BY label COLLATE NOCASE",
        )?;
        let rows = statement.query_map([], source_from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Reads one source, including a removed one so recovery copy can name it.
    pub fn source(&self, id: &str) -> Result<Source> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, label, root_path, state, last_scan_ms FROM sources WHERE id = ?1",
        )?;
        statement
            .query_row([id], source_from_row)
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => {
                    CoreError::library("source_missing", "The source is no longer in the library.")
                }
                other => CoreError::Sqlite(other),
            })
    }

    /// Grants a new media root.
    ///
    /// The path is canonicalised and rejected when it overlaps an existing
    /// source, so one tree cannot be imported twice under two labels.
    pub fn add_source(&self, root: impl AsRef<Path>, label: Option<&str>) -> Result<Source> {
        let root = self.canonical_root(root)?;
        self.check_overlapping_root(&root, None)?;
        let label = match label {
            Some(label) => validated(label, 120)?.to_string(),
            None => root
                .file_name()
                .map(|name| name.to_string_lossy().trim().to_string())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| root.to_string_lossy().to_string()),
        };
        let root_path = root.to_string_lossy().to_string();
        let connection = self.connection();
        // Re-granting a folder the user previously removed restores the same
        // source identity, so its works, history and lists are visible again
        // instead of being stranded on an orphaned row.
        let removed: Option<String> = connection
            .query_row(
                "SELECT id FROM sources WHERE root_path = ?1 AND state = 'removed'",
                [&root_path],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(id) = removed {
            connection.execute(
                "UPDATE sources SET label = ?1, state = 'available' WHERE id = ?2",
                params![label, id],
            )?;
            drop(connection);
            return self.source(&id);
        }
        let source = Source {
            id: super::new_id(),
            label,
            root_path,
            state: SourceState::Available,
            last_scan_ms: None,
        };
        connection.execute(
            "INSERT INTO sources(id, label, root_path, state) VALUES(?1, ?2, ?3, ?4)",
            params![
                source.id,
                source.label,
                source.root_path,
                source.state.as_str()
            ],
        )?;
        Ok(source)
    }

    /// Points a source at a new location after a drive letter or mount changed.
    ///
    /// Every file under the source becomes unavailable: a new grant does not
    /// prove that the file at a locator is the same file. A rescan re-establishes
    /// availability, and fingerprints decide rename candidates.
    pub fn relink_source(&self, id: &str, new_root: impl AsRef<Path>) -> Result<()> {
        let root = self.canonical_root(new_root)?;
        self.check_overlapping_root(&root, Some(id))?;
        let mut connection = self.connection();
        let tx = connection.transaction()?;
        let changed = tx.execute(
            "UPDATE sources SET root_path = ?1, state = 'available' WHERE id = ?2",
            params![root.to_string_lossy().to_string(), id],
        )?;
        if changed == 0 {
            return Err(CoreError::library(
                "source_missing",
                "The source is no longer in the library.",
            ));
        }
        tx.execute("UPDATE files SET available = 0 WHERE source_id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    /// Renames a source label.
    pub fn rename_source(&self, id: &str, label: &str) -> Result<()> {
        let label = validated(label, 120)?.to_string();
        let changed = self.connection().execute(
            "UPDATE sources SET label = ?1 WHERE id = ?2",
            params![label, id],
        )?;
        if changed == 0 {
            return Err(CoreError::library(
                "source_missing",
                "The source is no longer in the library.",
            ));
        }
        Ok(())
    }

    /// Removes a source from the library without touching files on disk.
    pub fn remove_source(&self, id: &str) -> Result<()> {
        self.set_source_state(id, SourceState::Removed)
    }

    /// Records an availability change for a source.
    ///
    /// Any state other than available hides its files from the library while
    /// keeping every work, edition, progress row and list entry.
    pub fn set_source_state(&self, id: &str, state: SourceState) -> Result<()> {
        let mut connection = self.connection();
        let tx = connection.transaction()?;
        let changed = tx.execute(
            "UPDATE sources SET state = ?1 WHERE id = ?2",
            params![state.as_str(), id],
        )?;
        if changed == 0 {
            return Err(CoreError::library(
                "source_missing",
                "The source is no longer in the library.",
            ));
        }
        if state != SourceState::Available {
            tx.execute("UPDATE files SET available = 0 WHERE source_id = ?1", [id])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// True when `candidate` is inside, equals, or contains a listed source.
    fn check_overlapping_root(&self, candidate: &Path, except: Option<&str>) -> Result<()> {
        for source in self.sources()? {
            if Some(source.id.as_str()) == except || source.root_path.is_empty() {
                continue;
            }
            if self.overlaps(candidate, Path::new(&source.root_path)) {
                return Err(CoreError::library(
                    "duplicate_source",
                    "This folder overlaps an existing source.",
                ));
            }
        }
        Ok(())
    }

    fn overlaps(&self, left: &Path, right: &Path) -> bool {
        let left: PathBuf = self.path_key(left).into();
        let right: PathBuf = self.path_key(right).into();
        left == right || left.starts_with(&right) || right.starts_with(&left)
    }
}

fn source_from_row(row: &Row<'_>) -> rusqlite::Result<Source> {
    let state: String = row.get(3)?;
    Ok(Source {
        id: row.get(0)?,
        label: row.get(1)?,
        root_path: row.get(2)?,
        state: SourceState::parse(&state).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                3,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unknown source state {state:?}"),
                )),
            )
        })?,
        last_scan_ms: row.get(4)?,
    })
}
