//! Screenroom shared core.
//!
//! This crate is the single source of product truth for every Screenroom client
//! (Windows/Linux/macOS desktop, and the TV clients through its contract). It holds the
//! domain model, the SQLite library, filesystem scanning, filename
//! interpretation and content fingerprints, and deliberately contains no UI,
//! no platform player and no network code.
//!
//! Contracts that callers may rely on:
//!
//! * Identity is an application-generated UUID, never a path, filename or
//!   provider ID.
//! * Public time values are integer milliseconds.
//! * Scanning is bounded, cancellable, non-destructive and reports partial
//!   results as [`ScanIssue`] values instead of failing the whole walk.
//! * Playback progress is accepted only from the authoritative
//!   `(session, generation, sequence)` triple, so a late native callback can
//!   never overwrite newer progress.
//! * A missing source, permission loss or a corrupt database never deletes
//!   works, editions, history or lists.

pub mod domain;
pub mod error;
pub mod filename;
pub mod fingerprint;
pub mod library;
pub mod scanner;
mod schema;

pub use domain::{
    BackgroundJob, LibraryItem, MediaKind, PlaybackSession, Profile, Recommendation, SegmentMarker,
    Source, SourceState,
};
pub use error::{CoreError, Result};
pub use filename::{ParsedName, parse_filename};
pub use fingerprint::media_fingerprint;
pub use library::{
    BackupInfo, BackupReport, Checkpoint, Ingest, Library, LibraryQuery, PathSemantics,
    Preferences, RestoreReport, Sort,
};
pub use scanner::{
    LibraryScanner, MEDIA_EXTENSIONS, ScanIssue, ScanSummary, ScanToken, ScanUpdate,
};

/// Schema version of the SQLite library written by this crate.
///
/// This is the same version and physical layout the Flutter candidate wrote, so
/// an existing Screenroom library can be opened without a destructive migration.
pub const SCHEMA_VERSION: i64 = 2;

/// Version of the versioned JSON backup format accepted by [`Library`].
pub const BACKUP_FORMAT_VERSION: i64 = 2;
