//! Durable resume: the only place that turns player observations into checkpoints.
//!
//! `screenroom-core` owns the rules (session generations, ordered sequences, seek
//! generations); this module owns the *policy* — when a save is due, what a
//! finished file should resume to, and when to stop writing. Keeping it here,
//! outside both the player thread and the widgets, is what makes resume
//! testable without a window.

use screenroom_core::{Checkpoint, CoreError, Library, PlaybackSession};

/// Minimum change in playhead position before another checkpoint is written.
///
/// One second of stored drift is invisible to a viewer and keeps a 30 fps tick
/// from turning into 30 database writes per second.
const CHECKPOINT_INTERVAL_MS: i64 = 1_000;

/// Finished playback that ended within this distance of the end resumes from the
/// beginning rather than dropping the viewer back into the closing seconds.
const END_TOLERANCE_MS: i64 = 1_000;

/// An opened session plus the position it should start from.
#[derive(Debug)]
pub struct OpenedPlayback {
    /// Tracks this viewing session.
    pub tracker: PlaybackTracker,
    /// Position to seek to once the file is loaded, in milliseconds.
    pub resume_from_ms: i64,
}

/// Converts observations into checkpoints for one viewing session.
#[derive(Debug)]
pub struct PlaybackTracker {
    session: PlaybackSession,
    sequence: i64,
    seek_generation: i64,
    last_saved_ms: Option<i64>,
    closed: bool,
}

impl PlaybackTracker {
    /// Opens a session for `file_id` and decides where playback should resume.
    ///
    /// A file near its end resumes from the start; a file whose duration is still
    /// unknown resumes from its stored position without inventing a duration.
    pub fn open(
        library: &Library,
        profile_id: &str,
        file_id: &str,
    ) -> Result<OpenedPlayback, CoreError> {
        let item = library.item(file_id, Some(profile_id))?.ok_or_else(|| {
            CoreError::library("file_missing", "This file is not in the library.")
        })?;
        let resume_from_ms =
            if item.duration_ms > 0 && item.position_ms >= item.duration_ms - END_TOLERANCE_MS {
                0
            } else {
                item.position_ms.max(0)
            };
        let session = library.start_session(profile_id, file_id)?;
        Ok(OpenedPlayback {
            tracker: Self {
                session,
                sequence: 0,
                seek_generation: 0,
                last_saved_ms: None,
                closed: false,
            },
            resume_from_ms,
        })
    }

    /// Registers a seek and returns the generation later checkpoints must quote.
    pub fn seek(&mut self, library: &Library) -> Result<i64, CoreError> {
        self.seek_generation = library.begin_seek(&self.session)?;
        // The next report belongs to a new generation and must not be throttled
        // against the pre-seek position.
        self.last_saved_ms = None;
        Ok(self.seek_generation)
    }

    /// Stores progress when it is meaningful and due.
    ///
    /// Returns `true` only when the core actually wrote the row, so the caller
    /// never reports a save that was rejected as stale.
    pub fn record(
        &mut self,
        library: &Library,
        position_ms: i64,
        duration_ms: i64,
        ended: bool,
    ) -> Result<bool, CoreError> {
        if self.closed || duration_ms <= 0 || position_ms < 0 {
            return Ok(false);
        }
        // A player may report a position a fraction past the end during the last
        // frame; store the end rather than an impossible value.
        let position_ms = position_ms.min(duration_ms);
        let due = ended
            || match self.last_saved_ms {
                None => true,
                Some(last) => (position_ms - last).abs() >= CHECKPOINT_INTERVAL_MS,
            };
        if !due {
            return Ok(false);
        }
        self.sequence += 1;
        let written = library.checkpoint(Checkpoint {
            session: &self.session,
            position_ms,
            duration_ms,
            sequence: self.sequence,
            seek_generation: self.seek_generation,
            ended,
        })?;
        if written {
            self.last_saved_ms = Some(position_ms);
            // The core closes the session when `ended` is stored.
            self.closed = ended;
        }
        Ok(written)
    }

    /// Closes the session without writing progress.
    pub fn close(self, library: &Library) -> Result<(), CoreError> {
        if self.closed {
            return Ok(());
        }
        library.end_session(&self.session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_policy_constants_stay_consistent() {
        // The "finished, start over" window must not be so wide that ordinary
        // mid-film progress is reset, and the throttle must stay a whole second
        // of media so a frame tick cannot flood the database.
        const { assert!(END_TOLERANCE_MS <= CHECKPOINT_INTERVAL_MS) };
        const { assert!(CHECKPOINT_INTERVAL_MS >= 1_000) };
    }
}
