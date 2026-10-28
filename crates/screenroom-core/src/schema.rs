//! SQLite physical schema.
//!
//! The DDL and the `user_version` value intentionally match the library layout
//! the Flutter candidate wrote, so a library already on a device can be opened
//! and migrated forward instead of being reset. Index and constraint names are
//! part of that compatibility surface: do not reorder or rename them casually.

/// Statements executed when creating a fresh library.
pub const CREATE: &[&str] = &[
    "CREATE TABLE sources(id TEXT PRIMARY KEY,label TEXT NOT NULL,root_path TEXT NOT NULL DEFAULT '',
      state TEXT NOT NULL DEFAULT 'disconnected',last_scan_ms INTEGER)",
    "CREATE TABLE works(id TEXT PRIMARY KEY,identity_key TEXT NOT NULL,title TEXT NOT NULL,
      kind TEXT NOT NULL CHECK(kind IN ('movie','series','unknown')),year INTEGER,
      overview TEXT NOT NULL DEFAULT '',genres TEXT NOT NULL DEFAULT '[]',poster_path TEXT,
      confidence REAL NOT NULL DEFAULT 0,
      manual_match INTEGER NOT NULL DEFAULT 0 CHECK(manual_match IN (0,1)),
      metadata_revision INTEGER NOT NULL DEFAULT 0)",
    "CREATE INDEX works_identity ON works(identity_key)",
    "CREATE INDEX works_title ON works(title COLLATE NOCASE,kind)",
    "CREATE TABLE seasons(id TEXT PRIMARY KEY,work_id TEXT NOT NULL REFERENCES works(id),
      number INTEGER NOT NULL,verified_order INTEGER NOT NULL DEFAULT 0,episode_count INTEGER,
      metadata_origin TEXT NOT NULL DEFAULT 'legacy',metadata_provider_id TEXT,
      UNIQUE(work_id,number))",
    "CREATE TABLE episodes(id TEXT PRIMARY KEY,season_id TEXT NOT NULL REFERENCES seasons(id),
      number INTEGER NOT NULL,title TEXT NOT NULL,overview TEXT NOT NULL DEFAULT '',
      runtime_ms INTEGER NOT NULL DEFAULT 0,verified_order INTEGER NOT NULL DEFAULT 0,
      metadata_origin TEXT NOT NULL DEFAULT 'legacy',metadata_provider_id TEXT,
      UNIQUE(season_id,number))",
    "CREATE TABLE editions(id TEXT PRIMARY KEY,work_id TEXT NOT NULL REFERENCES works(id),
      label TEXT NOT NULL,duration_ms INTEGER NOT NULL DEFAULT 0 CHECK(duration_ms>=0))",
    "CREATE TABLE files(id TEXT PRIMARY KEY,source_id TEXT NOT NULL REFERENCES sources(id),
      edition_id TEXT NOT NULL REFERENCES editions(id),relative_path TEXT NOT NULL,
      filename TEXT NOT NULL,size INTEGER NOT NULL CHECK(size>=0),
      modified_ms INTEGER NOT NULL,fingerprint TEXT NOT NULL,
      available INTEGER NOT NULL DEFAULT 0 CHECK(available IN(0,1)),
      added_ms INTEGER NOT NULL,last_scan_id TEXT,season_number INTEGER,
      episode_number INTEGER,episode_end INTEGER,UNIQUE(source_id,relative_path))",
    "CREATE INDEX files_source ON files(source_id,available)",
    "CREATE INDEX files_fingerprint ON files(source_id,fingerprint,size)",
    "CREATE INDEX files_sequence ON files(season_number,episode_number)",
    "CREATE TABLE file_episodes(file_id TEXT NOT NULL REFERENCES files(id),
      episode_id TEXT NOT NULL REFERENCES episodes(id),sequence INTEGER NOT NULL,
      in_ms INTEGER,out_ms INTEGER,PRIMARY KEY(file_id,episode_id))",
    "CREATE TABLE profiles(id TEXT PRIMARY KEY,name TEXT NOT NULL)",
    "CREATE TABLE sessions(id TEXT PRIMARY KEY,
      profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
      file_id TEXT NOT NULL REFERENCES files(id),edition_id TEXT NOT NULL REFERENCES editions(id),
      generation INTEGER NOT NULL,status TEXT NOT NULL,sequence INTEGER NOT NULL,
      seek_generation INTEGER NOT NULL)",
    "CREATE TABLE progress(profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
      edition_id TEXT NOT NULL REFERENCES editions(id),file_id TEXT REFERENCES files(id),
      position_ms INTEGER NOT NULL DEFAULT 0 CHECK(position_ms>=0),
      duration_ms INTEGER NOT NULL DEFAULT 0 CHECK(duration_ms>=0),
      watched INTEGER NOT NULL DEFAULT 0 CHECK(watched IN(0,1)),
      hidden INTEGER NOT NULL DEFAULT 0 CHECK(hidden IN(0,1)),
      updated_ms INTEGER NOT NULL,last_session_id TEXT,PRIMARY KEY(profile_id,edition_id))",
    "CREATE INDEX progress_recent ON progress(profile_id,updated_ms DESC)",
    "CREATE TABLE watchlist(profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
      work_id TEXT NOT NULL REFERENCES works(id),
      favorite INTEGER NOT NULL DEFAULT 0 CHECK(favorite IN(0,1)),
      watchlisted INTEGER NOT NULL DEFAULT 0 CHECK(watchlisted IN(0,1)),
      created_ms INTEGER NOT NULL,PRIMARY KEY(profile_id,work_id))",
    "CREATE TABLE overrides(work_id TEXT NOT NULL REFERENCES works(id),field TEXT NOT NULL,
      value TEXT NOT NULL,updated_ms INTEGER NOT NULL,PRIMARY KEY(work_id,field))",
    "CREATE TABLE markers(id TEXT PRIMARY KEY,edition_id TEXT NOT NULL REFERENCES editions(id),
      kind TEXT NOT NULL,start_ms INTEGER NOT NULL CHECK(start_ms>=0),
      end_ms INTEGER NOT NULL CHECK(end_ms>start_ms),
      duration_ms INTEGER NOT NULL CHECK(duration_ms>=end_ms),origin TEXT NOT NULL)",
    "CREATE TABLE external_identities(work_id TEXT NOT NULL REFERENCES works(id),
      provider TEXT NOT NULL,external_id TEXT NOT NULL,confidence REAL NOT NULL,
      PRIMARY KEY(work_id,provider))",
    "CREATE TABLE jobs(id TEXT PRIMARY KEY,kind TEXT NOT NULL,target TEXT NOT NULL,
      status TEXT NOT NULL,priority INTEGER NOT NULL DEFAULT 0,
      attempts INTEGER NOT NULL DEFAULT 0,next_retry_ms INTEGER NOT NULL DEFAULT 0,error TEXT)",
    "CREATE INDEX jobs_due ON jobs(status,next_retry_ms,priority)",
    "CREATE UNIQUE INDEX jobs_active_target ON jobs(kind,target)
      WHERE status IN ('queued','running','rateLimited','waitingNetwork','needsConfirmation')",
    "CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT NOT NULL)",
    "CREATE TABLE artwork(work_id TEXT PRIMARY KEY REFERENCES works(id),file_id TEXT REFERENCES files(id),
      mime TEXT NOT NULL DEFAULT 'image/jpeg',width INTEGER NOT NULL CHECK(width>0),
      height INTEGER NOT NULL CHECK(height>0),sampled_at_ms INTEGER NOT NULL CHECK(sampled_at_ms>=0),
      created_ms INTEGER NOT NULL,bytes BLOB NOT NULL)",
];

/// Statements that take a schema 2 library to schema 3.
///
/// Additive only: the artwork table is new, and no existing table or index is
/// rewritten, so a library written before posters were retained keeps every row
/// and simply gains the new table. Fresh libraries get the same statement from
/// [`CREATE`], which is why it is spelled once here.
pub const MIGRATE_2_TO_3: &[&str] = &[CREATE[CREATE.len() - 1]];
