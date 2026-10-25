//! Playback sessions and durable resume progress.
//!
//! Rules enforced here, not in the UI:
//!
//! * One active session per library at a time; starting a new one closes the old.
//! * Progress is accepted only from the current session generation and a
//!   strictly increasing event sequence, so a late native callback (a seek
//!   completion, a delayed position event) cannot rewind newer progress.
//! * A seek bumps the seek generation; progress from before the seek is
//!   rejected until the player reports the new generation.
//! * `ended` marks the edition watched and closes the session.

use rusqlite::{OptionalExtension, params};

use super::{Library, new_id, now_ms};
use crate::domain::PlaybackSession;
use crate::error::{CoreError, Result};

/// Longest duration a checkpoint will accept: seven days, in milliseconds.
const MAX_DURATION_MS: i64 = 7 * 24 * 60 * 60 * 1000;

/// One progress report from the player.
#[derive(Debug, Clone, Copy)]
pub struct Checkpoint<'a> {
    /// Authoritative session.
    pub session: &'a PlaybackSession,
    /// Playhead position in milliseconds.
    pub position_ms: i64,
    /// Media duration in milliseconds.
    pub duration_ms: i64,
    /// Monotonic event sequence for this session.
    pub sequence: i64,
    /// Seek generation the player is reporting against.
    pub seek_generation: i64,
    /// True when the player reached the end of the media.
    pub ended: bool,
}

impl Library {
    /// Opens an authoritative session for a media file.
    ///
    /// Any previously active session is closed first, so two native players can
    /// never both own the current progress row.
    pub fn start_session(&self, profile_id: &str, file_id: &str) -> Result<PlaybackSession> {
        let mut connection = self.connection();
        let tx = connection.transaction()?;
        let edition_id: String = tx
            .query_row(
                "SELECT edition_id FROM files WHERE id = ?1",
                [file_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| {
                CoreError::library("file_missing", "This file is no longer in the library.")
            })?;
        let generation: i64 = tx.query_row(
            "SELECT COALESCE(MAX(generation), 0) FROM sessions",
            [],
            |row| row.get(0),
        )?;
        tx.execute(
            "UPDATE sessions SET status = 'closed' WHERE status = 'active'",
            [],
        )?;
        let session = PlaybackSession {
            id: new_id(),
            profile_id: profile_id.to_string(),
            file_id: file_id.to_string(),
            edition_id,
            generation: generation + 1,
        };
        tx.execute(
            "INSERT INTO sessions(id, profile_id, file_id, edition_id, generation, status,
             sequence, seek_generation) VALUES(?1, ?2, ?3, ?4, ?5, 'active', -1, 0)",
            params![
                session.id,
                session.profile_id,
                session.file_id,
                session.edition_id,
                session.generation
            ],
        )?;
        tx.commit()?;
        Ok(session)
    }

    /// Marks that the player started a seek and returns the new seek generation.
    pub fn begin_seek(&self, session: &PlaybackSession) -> Result<i64> {
        let mut connection = self.connection();
        let tx = connection.transaction()?;
        let current: Option<i64> = tx
            .query_row(
                "SELECT seek_generation FROM sessions
                 WHERE id = ?1 AND status = 'active' AND generation = ?2",
                params![session.id, session.generation],
                |row| row.get(0),
            )
            .optional()?;
        let current = current.ok_or_else(|| {
            CoreError::library("stale_session", "This viewing session has ended.")
        })?;
        let next = current + 1;
        tx.execute(
            "UPDATE sessions SET seek_generation = ?1 WHERE id = ?2",
            params![next, session.id],
        )?;
        tx.commit()?;
        Ok(next)
    }

    /// Stores progress if, and only if, this report is still authoritative.
    ///
    /// Returns `false` (without writing) for a stale session, an out-of-order
    /// event, a seek generation mismatch, an impossible duration, or a session
    /// whose profile/file/edition no longer match the row.
    pub fn checkpoint(&self, report: Checkpoint<'_>) -> Result<bool> {
        if report.position_ms < 0
            || report.duration_ms <= 0
            || report.position_ms > report.duration_ms
            || report.sequence < 0
            || report.duration_ms > MAX_DURATION_MS
        {
            return Ok(false);
        }
        let session = report.session;
        let mut connection = self.connection();
        let tx = connection.transaction()?;
        let current: Option<(i64, i64, String, String, String)> = tx
            .query_row(
                "SELECT sequence, seek_generation, profile_id, edition_id, file_id
                 FROM sessions WHERE id = ?1 AND generation = ?2 AND status = 'active'",
                params![session.id, session.generation],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()?;
        let Some((sequence, seek_generation, profile_id, edition_id, file_id)) = current else {
            return Ok(false);
        };
        if report.sequence <= sequence
            || report.seek_generation != seek_generation
            || profile_id != session.profile_id
            || edition_id != session.edition_id
            || file_id != session.file_id
        {
            return Ok(false);
        }

        tx.execute(
            "UPDATE sessions SET sequence = ?1 WHERE id = ?2",
            params![report.sequence, session.id],
        )?;
        tx.execute(
            "INSERT INTO progress(profile_id, edition_id, file_id, position_ms, duration_ms,
             watched, hidden, updated_ms, last_session_id)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8)
             ON CONFLICT(profile_id, edition_id) DO UPDATE SET
             file_id = excluded.file_id, position_ms = excluded.position_ms,
             duration_ms = excluded.duration_ms, hidden = 0,
             updated_ms = excluded.updated_ms, last_session_id = excluded.last_session_id",
            params![
                session.profile_id,
                session.edition_id,
                session.file_id,
                report.position_ms,
                report.duration_ms,
                i64::from(report.ended),
                now_ms(),
                session.id
            ],
        )?;
        tx.execute(
            "UPDATE editions SET duration_ms = ?1 WHERE id = ?2",
            params![report.duration_ms, session.edition_id],
        )?;
        if report.ended {
            tx.execute(
                "UPDATE progress SET watched = 1 WHERE profile_id = ?1 AND edition_id = ?2",
                params![session.profile_id, session.edition_id],
            )?;
            tx.execute(
                "UPDATE sessions SET status = 'closed' WHERE id = ?1",
                [&session.id],
            )?;
        }
        tx.commit()?;
        Ok(true)
    }

    /// Closes a session without writing progress.
    pub fn end_session(&self, session: &PlaybackSession) -> Result<()> {
        self.connection().execute(
            "UPDATE sessions SET status = 'closed' WHERE id = ?1 AND generation = ?2",
            params![session.id, session.generation],
        )?;
        Ok(())
    }
}
