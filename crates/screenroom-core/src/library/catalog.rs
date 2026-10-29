//! Catalog projection and scan ingestion.
//!
//! A scan is incremental: an unchanged file keeps its identity, a replaced file
//! at the same path gets a *new edition* so the old cut keeps its own progress,
//! and an ambiguous filename becomes an unlinked work instead of a wrong match.

use std::path::{Component, Path, PathBuf};

use rusqlite::types::Value;
use rusqlite::{OptionalExtension, Row, params, params_from_iter};

use super::{Library, LibraryQuery, new_id, now_ms};
use crate::domain::{LibraryItem, MediaKind};
use crate::error::{CoreError, Result};
use crate::filename::{ParsedName, parse_filename};

/// Every column a [`LibraryItem`] needs, with the viewer's progress and lists.
const SELECT: &str = "SELECT f.id, f.source_id, f.edition_id, f.relative_path, f.filename,
    f.available, f.added_ms, w.id work_id, w.title, w.kind, w.year, w.overview, w.genres,
    w.manual_match, e.duration_ms, f.season_number, f.episode_number,
    (SELECT CASE WHEN ep.metadata_origin='stale' THEN '' ELSE ep.title END
       FROM episodes ep JOIN file_episodes fe ON fe.episode_id=ep.id
       WHERE fe.file_id=f.id ORDER BY fe.sequence LIMIT 1) episode_title,
    (SELECT CASE WHEN ep.metadata_origin='stale' THEN '' ELSE ep.overview END
       FROM episodes ep JOIN file_episodes fe ON fe.episode_id=ep.id
       WHERE fe.file_id=f.id ORDER BY fe.sequence LIMIT 1) episode_overview,
    f.episode_end, COALESCE(pr.position_ms, 0) position_ms,
    COALESCE(pr.watched, 0) watched, COALESCE(pr.hidden, 0) hidden,
    COALESCE(pr.updated_ms, 0) progress_updated_ms,
    COALESCE(wl.favorite, 0) favorite, COALESCE(wl.watchlisted, 0) watchlisted
    FROM files f JOIN editions e ON e.id = f.edition_id JOIN works w ON w.id = e.work_id
    JOIN sources s ON s.id = f.source_id
    LEFT JOIN progress pr ON pr.edition_id = e.id AND pr.profile_id = ?
    LEFT JOIN watchlist wl ON wl.work_id = w.id AND wl.profile_id = ?";

/// Facts the scanner established about one file.
#[derive(Debug, Clone)]
pub struct Ingest<'a> {
    /// Owning source.
    pub source_id: &'a str,
    /// Path relative to the source root.
    pub relative_path: &'a str,
    /// Bounded content fingerprint.
    pub fingerprint: &'a str,
    /// Size in bytes.
    pub size: i64,
    /// Modification time in Unix milliseconds.
    pub modified_ms: i64,
    /// Identifier of the scan run that found this file.
    pub scan_id: &'a str,
    /// Filename interpretation; re-derived from the name when absent.
    pub parsed: Option<&'a ParsedName>,
    /// Root the scan started from, used to reject a source that moved mid-scan.
    pub expected_root: Option<&'a str>,
}

impl Library {
    /// Returns one page of the library.
    pub fn items(&self, query: &LibraryQuery) -> Result<Vec<LibraryItem>> {
        let (where_sql, filters) = self.where_clause(query)?;
        // The projection's two profile placeholders are bound before the filters.
        let mut values = profile_params(query);
        values.extend(filters);
        let selection = if query.group_by_work {
            let ranked = SELECT.replacen(
                "SELECT ",
                "SELECT ROW_NUMBER() OVER (PARTITION BY w.id ORDER BY \
                 CASE WHEN f.available=1 AND s.state='available' THEN 0 ELSE 1 END, \
                 COALESCE(f.season_number,0), COALESCE(f.episode_number,0), f.id) work_rank, ",
                1,
            );
            format!("SELECT * FROM ({ranked} WHERE {where_sql}) WHERE work_rank = 1")
        } else {
            format!("SELECT * FROM ({SELECT} WHERE {where_sql})")
        };
        let sql = format!(
            "{selection} ORDER BY {} LIMIT ? OFFSET ?",
            query.sort.order_by()
        );
        values.push(Value::Integer(query_page_size(query)?));
        values.push(Value::Integer(query_offset(query)?));

        let connection = self.connection();
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values), item_from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Counts rows matching a query, for pager totals.
    pub fn count(&self, query: &LibraryQuery) -> Result<i64> {
        let (where_sql, filters) = self.where_clause(query)?;
        let aggregate = if query.group_by_work {
            "COUNT(DISTINCT work_id)"
        } else {
            "COUNT(*)"
        };
        let sql = format!("SELECT {aggregate} FROM ({SELECT} WHERE {where_sql})");
        let mut values = profile_params(query);
        values.extend(filters);
        let connection = self.connection();
        let mut statement = connection.prepare(&sql)?;
        Ok(statement.query_row(params_from_iter(values), |row| row.get(0))?)
    }

    /// Reads one entry by media file identifier.
    pub fn item(&self, file_id: &str, profile_id: Option<&str>) -> Result<Option<LibraryItem>> {
        let connection = self.connection();
        let mut statement = connection.prepare(&format!("{SELECT} WHERE f.id = ?"))?;
        let mut rows = statement.query(params![profile_id, profile_id, file_id])?;
        match rows.next()? {
            Some(row) => Ok(Some(item_from_row(row)?)),
            None => Ok(None),
        }
    }

    /// Home-screen resume rows for one profile, excluding hidden and finished work.
    pub fn continue_watching(&self, profile_id: &str, limit: i64) -> Result<Vec<LibraryItem>> {
        let limit = limit.clamp(1, 500);
        let connection = self.connection();
        let mut statement = connection.prepare(&format!(
            "{SELECT} WHERE pr.position_ms > 0 AND pr.watched = 0 AND pr.hidden = 0
             AND s.state != 'removed' ORDER BY pr.updated_ms DESC, f.id LIMIT ?"
        ))?;
        let rows = statement.query_map(params![profile_id, profile_id, limit], item_from_row)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Season numbers that actually contain files for a work.
    pub fn seasons_for_work(&self, work_id: &str) -> Result<Vec<i64>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT DISTINCT f.season_number FROM files f
             JOIN editions e ON e.id = f.edition_id JOIN sources s ON s.id = f.source_id
             WHERE e.work_id = ?1 AND f.season_number BETWEEN 0 AND 999
             AND s.state != 'removed' ORDER BY f.season_number",
        )?;
        let rows = statement.query_map([work_id], |row| row.get::<_, i64>(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Distinct genre labels in the library, capped at 1,000 for a bounded picker.
    pub fn known_genres(&self) -> Result<Vec<String>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT DISTINCT w.genres FROM works w WHERE EXISTS(
               SELECT 1 FROM editions e JOIN files f ON f.edition_id = e.id
               JOIN sources s ON s.id = f.source_id
               WHERE e.work_id = w.id AND s.state != 'removed')
             ORDER BY w.genres LIMIT 200 OFFSET ?1",
        )?;
        let mut labels: Vec<String> = Vec::new();
        let mut offset = 0_i64;
        loop {
            let mut found = 0_usize;
            let mut rows = statement.query([offset])?;
            while let Some(row) = rows.next()? {
                found += 1;
                let genres = row.get::<_, String>(0)?;
                for label in serde_json::from_str::<Vec<String>>(&genres).unwrap_or_default() {
                    let label = label.trim().to_string();
                    if !label.is_empty() && !labels.contains(&label) {
                        labels.push(label);
                    }
                    if labels.len() == 1000 {
                        return Ok(sorted_labels(labels));
                    }
                }
            }
            if found < 200 {
                break;
            }
            offset += 200;
        }
        Ok(sorted_labels(labels))
    }

    /// Next verified episode after `file_id`, or `None` rather than a guess.
    ///
    /// A gap stops autoplay. A season rollover needs a verified episode count
    /// that matches the finished episode, so a missing episode is never skipped.
    pub fn next_episode(
        &self,
        file_id: &str,
        profile_id: Option<&str>,
    ) -> Result<Option<LibraryItem>> {
        let Some(current) = self.item(file_id, profile_id)? else {
            return Ok(None);
        };
        if current.kind != MediaKind::Series {
            return Ok(None);
        }
        let (Some(season), Some(episode_end)) = (current.season, current.episode_end) else {
            return Ok(None);
        };

        // Same season, next episode, and exactly one candidate.
        let following = {
            let connection = self.connection();
            let mut statement = connection.prepare(&format!(
                "{SELECT} WHERE w.id = ? AND f.season_number = ? AND f.episode_number = ?
                 AND f.available = 1 AND s.state = 'available' ORDER BY f.id LIMIT 2"
            ))?;
            let mut rows = statement.query(params![
                profile_id,
                profile_id,
                current.work_id,
                season,
                episode_end + 1
            ])?;
            let mut following = Vec::new();
            while let Some(row) = rows.next()? {
                following.push(item_from_row(row)?);
            }
            following
        };
        match following.len() {
            1 => return Ok(following.into_iter().next()),
            0 => {}
            _ => return Ok(None),
        }
        if season == 0 {
            return Ok(None);
        }

        // A season rollover needs verified ordering that matches the episode we
        // just finished, so a missing episode is never silently skipped.
        let (closed_season, has_next_season) = {
            let connection = self.connection();
            let closed: bool = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM seasons WHERE work_id = ?1 AND number = ?2
                 AND verified_order = 1 AND episode_count = ?3)",
                params![current.work_id, season, episode_end],
                |row| row.get(0),
            )?;
            let next: bool = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM seasons WHERE work_id = ?1 AND number = ?2
                 AND verified_order = 1 AND episode_count > 0)",
                params![current.work_id, season + 1],
                |row| row.get(0),
            )?;
            (closed, next)
        };
        if !closed_season || !has_next_season {
            return Ok(None);
        }

        let next_episode = {
            let connection = self.connection();
            let mut statement = connection.prepare(&format!(
                "{SELECT} WHERE w.id = ? AND f.season_number = ? AND f.episode_number = 1
                 AND f.available = 1 AND s.state = 'available' ORDER BY f.id LIMIT 2"
            ))?;
            let mut rows =
                statement.query(params![profile_id, profile_id, current.work_id, season + 1])?;
            let mut next = Vec::new();
            while let Some(row) = rows.next()? {
                next.push(item_from_row(row)?);
            }
            next
        };
        Ok(if next_episode.len() == 1 {
            next_episode.into_iter().next()
        } else {
            None
        })
    }

    /// Records one scanned file and returns its media file identifier.
    pub fn ingest_file(&self, ingest: Ingest<'_>) -> Result<String> {
        safe_relative(ingest.relative_path)?;
        let filename = Path::new(ingest.relative_path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .ok_or_else(|| CoreError::library("invalid_input", "Enter a valid value."))?;
        let folder = Path::new(ingest.relative_path)
            .parent()
            .map(|parent| parent.to_string_lossy().to_string())
            .unwrap_or_default();
        let parsed_storage;
        let parsed = match ingest.parsed {
            Some(parsed) => parsed,
            None => {
                parsed_storage = parse_filename(&filename, Some(&folder));
                &parsed_storage
            }
        };

        let mut connection = self.connection();
        let tx = connection.transaction()?;
        let (root_path, state): (String, String) = tx
            .query_row(
                "SELECT root_path, state FROM sources WHERE id = ?1",
                [ingest.source_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => CoreError::library(
                    "source_changed",
                    "The source changed while scanning. Restart the scan.",
                ),
                other => CoreError::Sqlite(other),
            })?;
        if state == "removed"
            || ingest
                .expected_root
                .is_some_and(|expected| expected != root_path.as_str())
        {
            return Err(CoreError::library(
                "source_changed",
                "The source changed while scanning. Restart the scan.",
            ));
        }

        if let Some((id, size, fingerprint, modified_ms, edition_id)) = tx
            .query_row(
                "SELECT id, size, fingerprint, modified_ms, edition_id FROM files
                 WHERE source_id = ?1 AND relative_path = ?2",
                params![ingest.source_id, ingest.relative_path],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .map(Some)
            .or_else(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })?
        {
            let small_file = size <= 131_072;
            if size == ingest.size
                && fingerprint == ingest.fingerprint
                && (!small_file || modified_ms == ingest.modified_ms)
            {
                tx.execute(
                    "UPDATE files SET available = 1, last_scan_id = ?1, modified_ms = ?2
                     WHERE id = ?3",
                    params![ingest.scan_id, ingest.modified_ms, id],
                )?;
                tx.commit()?;
                return Ok(id);
            }
            // The bytes at this path changed: a different cut keeps its own progress.
            let work_id: String = tx.query_row(
                "SELECT work_id FROM editions WHERE id = ?1",
                [&edition_id],
                |row| row.get(0),
            )?;
            let new_edition = new_id();
            tx.execute(
                "INSERT INTO editions(id, work_id, label, duration_ms)
                 VALUES(?1, ?2, 'Changed file', 0)",
                params![new_edition, work_id],
            )?;
            tx.execute(
                "UPDATE files SET edition_id = ?1, fingerprint = ?2, size = ?3,
                 modified_ms = ?4, available = 1, last_scan_id = ?5 WHERE id = ?6",
                params![
                    new_edition,
                    ingest.fingerprint,
                    ingest.size,
                    ingest.modified_ms,
                    ingest.scan_id,
                    id
                ],
            )?;
            tx.commit()?;
            return Ok(id);
        }

        // A renamed file with identical sampled bytes keeps its identity, but only
        // for small files and only when the previous locator is really gone.
        let mut candidates = tx.prepare(
            "SELECT id, modified_ms, relative_path FROM files
             WHERE source_id = ?1 AND fingerprint = ?2 AND size = ?3 LIMIT 2",
        )?;
        let candidate = candidates
            .query_map(
                params![ingest.source_id, ingest.fingerprint, ingest.size],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(candidates);
        if let [(candidate_id, candidate_modified, candidate_path)] = candidate.as_slice()
            && ingest.size <= 131_072
            && *candidate_modified == ingest.modified_ms
        {
            let previous_locator = PathBuf::from(&root_path).join(candidate_path);
            if !previous_locator.exists() {
                tx.execute(
                    "UPDATE files SET relative_path = ?1, filename = ?2, available = 1,
                     modified_ms = ?3, last_scan_id = ?4 WHERE id = ?5",
                    params![
                        ingest.relative_path,
                        filename,
                        ingest.modified_ms,
                        ingest.scan_id,
                        candidate_id
                    ],
                )?;
                tx.commit()?;
                return Ok(candidate_id.clone());
            }
        }

        let confident_series = parsed.kind == MediaKind::Series
            && parsed.confidence >= 0.8
            && parsed.season.is_some()
            && parsed.episode.is_some();
        let confident_movie =
            parsed.kind == MediaKind::Movie && parsed.year.is_some() && parsed.confidence >= 0.75;
        let identity_key = if confident_series {
            format!(
                "series:{}:{}:{}",
                ingest.source_id,
                parsed.title.to_lowercase(),
                parsed.year.map(|year| year.to_string()).unwrap_or_default()
            )
        } else if confident_movie {
            format!(
                "movie:{}:{}",
                parsed.title.to_lowercase(),
                parsed.year.unwrap_or_default()
            )
        } else {
            format!("unmatched:{}", new_id())
        };

        let existing_work: Option<(String, i64)> = tx
            .query_row(
                "SELECT id, manual_match FROM works WHERE identity_key = ?1 AND kind = ?2 LIMIT 1",
                params![identity_key, parsed.kind.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (work_id, work_is_new) = match &existing_work {
            Some((work_id, _)) => (work_id.clone(), false),
            None => {
                let work_id = new_id();
                tx.execute(
                    "INSERT INTO works(id, identity_key, title, kind, year, confidence,
                     overview, genres, manual_match)
                     VALUES(?1, ?2, ?3, ?4, ?5, ?6, '', '[]', 0)",
                    params![
                        work_id,
                        identity_key,
                        parsed.title,
                        parsed.kind.as_str(),
                        parsed.year,
                        parsed.confidence
                    ],
                )?;
                (work_id, true)
            }
        };

        let edition_id = new_id();
        tx.execute(
            "INSERT INTO editions(id, work_id, label, duration_ms) VALUES(?1, ?2, ?3, 0)",
            params![edition_id, work_id, filename],
        )?;
        let file_id = new_id();
        tx.execute(
            "INSERT INTO files(id, source_id, edition_id, relative_path, filename, size,
             modified_ms, fingerprint, available, added_ms, last_scan_id, season_number,
             episode_number, episode_end)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?10, ?11, ?12, ?13)",
            params![
                file_id,
                ingest.source_id,
                edition_id,
                ingest.relative_path,
                filename,
                ingest.size,
                ingest.modified_ms,
                ingest.fingerprint,
                now_ms(),
                ingest.scan_id,
                parsed.season,
                parsed.episode,
                parsed.episode_end
            ],
        )?;

        if let (Some(season), Some(episode)) = (parsed.season, parsed.episode) {
            let season_id: String = match tx
                .query_row(
                    "SELECT id FROM seasons WHERE work_id = ?1 AND number = ?2",
                    params![work_id, season],
                    |row| row.get(0),
                )
                .optional()?
            {
                Some(id) => id,
                None => {
                    let id = new_id();
                    tx.execute(
                        "INSERT INTO seasons(id, work_id, number, verified_order)
                         VALUES(?1, ?2, ?3, 0)",
                        params![id, work_id, season],
                    )?;
                    id
                }
            };
            for number in episode..=parsed.episode_end.unwrap_or(episode) {
                let episode_id: String = match tx
                    .query_row(
                        "SELECT id FROM episodes WHERE season_id = ?1 AND number = ?2",
                        params![season_id, number],
                        |row| row.get(0),
                    )
                    .optional()?
                {
                    Some(id) => id,
                    None => {
                        let id = new_id();
                        tx.execute(
                            "INSERT INTO episodes(id, season_id, number, title, verified_order)
                             VALUES(?1, ?2, ?3, ?4, 0)",
                            params![id, season_id, number, format!("Episode {number}")],
                        )?;
                        id
                    }
                };
                tx.execute(
                    "INSERT OR IGNORE INTO file_episodes(file_id, episode_id, sequence, in_ms, out_ms)
                     VALUES(?1, ?2, ?3, NULL, NULL)",
                    params![file_id, episode_id, number - episode],
                )?;
            }
        }

        if metadata_enabled(&tx)? && (work_is_new || !work_is_protected(&tx, &work_id)?) {
            tx.execute(
                "INSERT OR IGNORE INTO jobs(id, kind, target, status, priority, attempts, next_retry_ms)
                 VALUES(?1, 'metadata', ?2, 'queued', 0, 0, 0)",
                params![new_id(), work_id],
            )?;
        }

        tx.commit()?;
        Ok(file_id)
    }

    /// Completes a scan, clearing availability for unseen or confirmed-missing files.
    ///
    /// Partial scans only stat unseen locators while the source root resolves;
    /// bounded and cancelled scans never hide files they could not reach.
    pub fn finish_scan(
        &self,
        source_id: &str,
        scan_id: &str,
        complete: bool,
        expected_root: Option<&str>,
    ) -> Result<()> {
        let mut connection = self.connection();
        let tx = connection.transaction()?;
        let (root_path, state): (String, String) = tx
            .query_row(
                "SELECT root_path, state FROM sources WHERE id = ?1",
                [source_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => CoreError::library(
                    "source_changed",
                    "The source changed while scanning. Restart the scan.",
                ),
                other => CoreError::Sqlite(other),
            })?;
        if state == "removed"
            || expected_root.is_some_and(|expected| expected != root_path.as_str())
        {
            return Err(CoreError::library(
                "source_changed",
                "The source changed while scanning. Restart the scan.",
            ));
        }
        let root = Path::new(&root_path);
        let Ok(resolved_root) = std::fs::canonicalize(root) else {
            tx.commit()?;
            return Ok(());
        };
        if self.path_key(&resolved_root) != self.path_key(root) {
            tx.commit()?;
            return Ok(());
        }
        if complete {
            tx.execute(
                "UPDATE files SET available = 0
                 WHERE source_id = ?1 AND (last_scan_id IS NULL OR last_scan_id != ?2)",
                params![source_id, scan_id],
            )?;
            tx.execute(
                "UPDATE sources SET state = 'available', last_scan_ms = ?1 WHERE id = ?2",
                params![now_ms(), source_id],
            )?;
        } else {
            let mut statement = tx.prepare(
                "SELECT id, relative_path FROM files WHERE source_id = ?1 AND available = 1
                 AND (last_scan_id IS NULL OR last_scan_id != ?2)",
            )?;
            let locators = statement
                .query_map(params![source_id, scan_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            drop(statement);
            let missing: Vec<String> = locators
                .into_iter()
                .filter(|(_, relative_path)| {
                    matches!(std::fs::metadata(root.join(relative_path)),
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound)
                })
                .map(|(id, _)| id)
                .collect();
            // A drive pulled during the loop makes every later file "not found".
            // Only a root that still resolves afterwards makes those answers true.
            if std::fs::canonicalize(root).is_ok() {
                for id in missing {
                    tx.execute("UPDATE files SET available = 0 WHERE id = ?1", [id])?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Builds the shared `WHERE` fragment and its leading arguments.
    fn where_clause(&self, query: &LibraryQuery) -> Result<(String, Vec<Value>)> {
        let mut clauses = vec!["s.state != 'removed'".to_string()];
        let mut values = Vec::new();
        if !query.text.trim().is_empty() {
            clauses
                .push("(w.title LIKE ? ESCAPE '\\' OR f.filename LIKE ? ESCAPE '\\')".to_string());
            let escaped = query
                .text
                .trim()
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_");
            let pattern = format!("%{escaped}%");
            values.push(Value::Text(pattern.clone()));
            values.push(Value::Text(pattern));
        }
        if let Some(kind) = &query.kind {
            let kind = MediaKind::parse(kind).ok_or_else(|| {
                CoreError::library("invalid_filter", "That media kind does not exist.")
            })?;
            clauses.push("w.kind = ?".to_string());
            values.push(Value::Text(kind.as_str().to_string()));
        }
        if let Some(work_id) = &query.work_id {
            clauses.push("w.id = ?".to_string());
            values.push(Value::Text(work_id.clone()));
        }
        if let Some(season) = query.season {
            if !(0..=999).contains(&season) {
                return Err(CoreError::library(
                    "invalid_filter",
                    "Season numbers run from 0 to 999.",
                ));
            }
            clauses.push("f.season_number = ?".to_string());
            values.push(Value::Integer(season));
        }
        if let Some(genre) = &query.genre {
            clauses
                .push("EXISTS(SELECT 1 FROM json_each(w.genres) g WHERE g.value = ?)".to_string());
            values.push(Value::Text(genre.clone()));
        }
        if query.available_only {
            clauses.push("f.available = 1 AND s.state = 'available'".to_string());
        }
        if let Some(watched) = query.watched {
            clauses.push("COALESCE(pr.watched, 0) = ?".to_string());
            values.push(Value::Integer(i64::from(watched)));
        }
        if query.watchlist_only {
            clauses.push("COALESCE(wl.watchlisted, 0) = 1".to_string());
        }
        if query.favorites_only {
            clauses.push("COALESCE(wl.favorite, 0) = 1".to_string());
        }
        Ok((clauses.join(" AND "), values))
    }
}

fn profile_params(query: &LibraryQuery) -> Vec<Value> {
    let value = query
        .profile_id
        .clone()
        .map(Value::Text)
        .unwrap_or(Value::Null);
    vec![value.clone(), value]
}

fn sorted_labels(mut labels: Vec<String>) -> Vec<String> {
    labels.sort_by(|left, right| {
        let folded = left.to_lowercase().cmp(&right.to_lowercase());
        if folded.is_eq() {
            left.cmp(right)
        } else {
            folded
        }
    });
    labels
}

fn query_page_size(query: &LibraryQuery) -> Result<i64> {
    if !(1..=500).contains(&query.limit) {
        return Err(CoreError::library(
            "invalid_page",
            "Ask for between 1 and 500 rows at a time.",
        ));
    }
    Ok(query.limit)
}

fn query_offset(query: &LibraryQuery) -> Result<i64> {
    if query.offset < 0 {
        return Err(CoreError::library("invalid_page", "Invalid page."));
    }
    Ok(query.offset)
}

fn metadata_enabled(tx: &rusqlite::Transaction<'_>) -> rusqlite::Result<bool> {
    let value: Option<String> = tx
        .query_row(
            "SELECT value FROM settings WHERE key = 'metadataEnabled'",
            [],
            |row| row.get(0),
        )
        .ok();
    Ok(value.as_deref() == Some("true"))
}

/// A work is protected when the user corrected its match by hand.
fn work_is_protected(tx: &rusqlite::Transaction<'_>, work_id: &str) -> rusqlite::Result<bool> {
    let protected: i64 = tx.query_row(
        "SELECT manual_match FROM works WHERE id = ?1",
        [work_id],
        |row| row.get(0),
    )?;
    Ok(protected == 1)
}

/// Rejects locators that could escape the authorized root.
fn safe_relative(relative: &str) -> Result<()> {
    let path = Path::new(relative);
    if relative.is_empty() || relative.contains('\0') || path.is_absolute() {
        return Err(CoreError::library(
            "unsafe_path",
            "That file path is outside the authorized folder.",
        ));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
    {
        return Err(CoreError::library(
            "unsafe_path",
            "That file path is outside the authorized folder.",
        ));
    }
    Ok(())
}

fn item_from_row(row: &Row<'_>) -> rusqlite::Result<LibraryItem> {
    let kind: String = row.get("kind")?;
    let genres: String = row.get("genres")?;
    Ok(LibraryItem {
        id: row.get("id")?,
        work_id: row.get("work_id")?,
        edition_id: row.get("edition_id")?,
        source_id: row.get("source_id")?,
        title: row.get("title")?,
        filename: row.get("filename")?,
        relative_path: row.get("relative_path")?,
        kind: MediaKind::parse(&kind).unwrap_or(MediaKind::Unknown),
        overview: row.get("overview")?,
        episode_title: row
            .get::<_, Option<String>>("episode_title")?
            .unwrap_or_default(),
        episode_overview: row
            .get::<_, Option<String>>("episode_overview")?
            .unwrap_or_default(),
        genres: serde_json::from_str(&genres).unwrap_or_default(),
        year: row.get("year")?,
        season: row.get("season_number")?,
        episode: row.get("episode_number")?,
        episode_end: row.get("episode_end")?,
        duration_ms: row.get("duration_ms")?,
        position_ms: row.get("position_ms")?,
        added_ms: row.get("added_ms")?,
        available: row.get::<_, i64>("available")? == 1,
        watched: row.get::<_, i64>("watched")? == 1,
        favorite: row.get::<_, i64>("favorite")? == 1,
        watchlisted: row.get::<_, i64>("watchlisted")? == 1,
        manual_match: row.get::<_, i64>("manual_match")? == 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Ingest;

    #[test]
    fn malformed_genres_do_not_hide_other_known_genres() {
        let directory = tempfile::tempdir().unwrap();
        let library = Library::open_in_memory().unwrap();
        let source = library.add_source(directory.path(), None).unwrap();
        let parsed = parse_filename("Film.2020.mkv", None);
        library
            .ingest_file(Ingest {
                source_id: &source.id,
                relative_path: "Film.2020.mkv",
                fingerprint: "film",
                size: 1,
                modified_ms: 1,
                scan_id: "test",
                parsed: Some(&parsed),
                expected_root: Some(&source.root_path),
            })
            .unwrap();
        let work = library
            .items(&LibraryQuery::page(1))
            .unwrap()
            .remove(0)
            .work_id;
        library
            .connection()
            .execute("UPDATE works SET genres = '{' WHERE id = ?1", [&work])
            .unwrap();

        assert!(library.known_genres().unwrap().is_empty());
    }
}
