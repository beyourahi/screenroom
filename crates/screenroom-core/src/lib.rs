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
pub mod paths;
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
    Artwork, ArtworkCandidate, BackupInfo, BackupReport, Checkpoint, Ingest, Library, LibraryQuery,
    PathSemantics, Preferences, RestoreReport, Sort,
};
pub use paths::{PathStyle, location_key};
pub use scanner::{
    LibraryScanner, MEDIA_EXTENSIONS, ScanIssue, ScanSummary, ScanToken, ScanUpdate,
};

/// Schema version of the SQLite library written by this crate.
///
/// Version 2 is the layout the Flutter candidate wrote; version 3 adds the
/// artwork table; version 4 adds the indexes on the work/edition/file joins the
/// catalogue runs on every screen. Both steps are additive, so a library from
/// any of those versions opens without a destructive migration.
pub const SCHEMA_VERSION: i64 = 4;

/// Version of the versioned JSON backup format accepted by [`Library`].
pub const BACKUP_FORMAT_VERSION: i64 = 2;
