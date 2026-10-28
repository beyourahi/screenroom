//! Conservative filename interpretation.
//!
//! This is a *hint*, not an identity: it produces a title, a shape and a
//! confidence so the UI can show unknowns honestly instead of inventing a
//! match. Ambiguous results stay `Unknown` and remain playable.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::domain::MediaKind;

static EPISODIC_SE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bS(\d{1,3})[ ._-]*E(\d{1,4})(?:[ ._-]*(?:E|-E?)(\d{1,4}))?")
        .expect("static episodic regex is valid")
});
static EPISODIC_AX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(\d{1,3})x(\d{1,4})(?:-(\d{1,4}))?").expect("static regex is valid")
});
static YEAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(19\d{2}|20\d{2})\b").expect("static year regex is valid"));
static RESOLUTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\d{3,4}x\d{3,4}\b").expect("static resolution regex is valid"));
static QUALITY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:2160p|1080p|720p|480p|bluray|brrip|webrip|web-dl|hdtv|dvdrip|x26[45]|h26[45]|hevc|av1)\b",
    )
    .expect("static quality regex is valid")
});
static BRACKETS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\[\]()]+").expect("static bracket regex is valid"));
static SEPARATORS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[._]").expect("static separator regex is valid"));
static SEASON_FOLDER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^season\s*\d+$").expect("static season folder regex is valid")
});
static NUMERIC_TITLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d+$").expect("static numeric regex is valid"));

struct Episodic {
    start: usize,
    season: i64,
    episode: i64,
    end: Option<i64>,
}

/// What a filename suggests about the media it holds.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedName {
    /// Best-effort title, or the file stem when nothing could be read.
    pub title: String,
    /// Interpreted shape.
    pub kind: MediaKind,
    /// Release year, when it appears before any quality tag.
    pub year: Option<i64>,
    /// Season number, for episodic names.
    pub season: Option<i64>,
    /// First episode number, for episodic names.
    pub episode: Option<i64>,
    /// Last episode number for combined files (`S01E01E02`).
    pub episode_end: Option<i64>,
    /// Confidence in the interpretation, `0.0..=1.0`.
    pub confidence: f64,
}

/// Interprets a filename, optionally using its containing folder for context.
///
/// `folder` is a directory path or relative directory label; only its last
/// component is consulted, and only when the filename yields no title.
pub fn parse_filename(filename: &str, folder: Option<&str>) -> ParsedName {
    let stem = stem_of(filename);
    let mut name = stem.replace('_', ".");
    name = RESOLUTION.replace_all(&name, " ").into_owned();

    let episodic = EPISODIC_SE
        .captures(&name)
        .or_else(|| EPISODIC_AX.captures(&name))
        .and_then(|captures| {
            Some(Episodic {
                start: captures.get(0)?.start(),
                season: captures.get(1)?.as_str().parse().ok()?,
                episode: captures.get(2)?.as_str().parse().ok()?,
                end: captures
                    .get(3)
                    .and_then(|end| end.as_str().parse::<i64>().ok()),
            })
        });
    let years: Vec<_> = YEAR
        .find_iter(&name)
        .filter(|matched| match &episodic {
            Some(episode) => matched.start() < episode.start,
            None => true,
        })
        .filter(|matched| !clean(&name[..matched.start()]).is_empty())
        .collect();
    let year_match = years
        .iter()
        .rev()
        .find(|matched| {
            let before = matched
                .start()
                .checked_sub(1)
                .and_then(|i| name.as_bytes().get(i));
            let after = name.as_bytes().get(matched.end());
            matches!(before, Some(b'(' | b'[')) && matches!(after, Some(b')' | b']'))
        })
        .or_else(|| years.last())
        .copied();
    let year = year_match
        .and_then(|m| m.as_str().parse::<i64>().ok())
        .filter(|value| (1800..=2200).contains(value));
    // A show's year lives on its folder (`From (2022)`), never in its episode
    // names, so an episodic file with no year of its own takes the folder's.
    // Without it every series has a null year and two shows that share a title
    // in one source merge into a single work. Movies are left alone: a folder
    // year there is a collection label (`Duology (2009-2011)`), not a release
    // year, and guessing would be worse than leaving it empty.
    let year = year.or_else(|| {
        episodic
            .is_some()
            .then(|| folder.and_then(folder_year))
            .flatten()
    });

    let cut = year_match
        .map(|m| m.start())
        .or_else(|| episodic.as_ref().map(|episode| episode.start))
        .or_else(|| QUALITY.find(&name).map(|m| m.start()));
    if let Some(cut) = cut {
        name.truncate(cut);
    }

    let cleaned = clean(&name);
    let mut name = cleaned;
    let mut folder_fallback = false;
    if name.is_empty()
        && let Some(folder) = folder
    {
        let label = clean(&Path::new(folder).file_name_stem());
        folder_fallback = !SEASON_FOLDER.is_match(&label) && !label.is_empty();
        name = if SEASON_FOLDER.is_match(&label) {
            String::new()
        } else {
            label
        };
    }

    let unknown = name.is_empty() || NUMERIC_TITLE.is_match(&name);
    let season = episodic.as_ref().map(|episode| episode.season);
    let episode = episodic.as_ref().map(|episode| episode.episode);
    let episode_end = match (episodic.as_ref().and_then(|episode| episode.end), episode) {
        (Some(end), Some(first)) if end >= first && end - first < 100 => Some(end),
        (_, first) => first,
    };

    let kind = if episodic.is_some() {
        MediaKind::Series
    } else if unknown {
        MediaKind::Unknown
    } else {
        MediaKind::Movie
    };
    let confidence = if unknown {
        0.1
    } else if folder_fallback {
        0.4
    } else if episodic.is_some() {
        0.85
    } else if year.is_some() {
        0.75
    } else {
        0.4
    };

    ParsedName {
        title: if unknown { stem } else { name },
        kind,
        year,
        season,
        episode,
        episode_end,
        confidence,
    }
}

fn stem_of(filename: &str) -> String {
    Path::new(filename)
        .file_stem()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| filename.to_string())
}

/// The year in a folder label, if it has one.
fn folder_year(folder: &str) -> Option<i64> {
    let label = Path::new(folder)
        .file_name()?
        .to_string_lossy()
        .into_owned();
    YEAR.find(&label)
        .and_then(|matched| matched.as_str().parse::<i64>().ok())
        .filter(|value| (1800..=2200).contains(value))
}

fn clean(value: &str) -> String {
    let spaced = SEPARATORS.replace_all(value, " ");
    let bracketed = BRACKETS.replace_all(&spaced, " ");
    bracketed
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_end_matches([' ', '-'])
        .to_string()
}

trait FileNameStem {
    fn file_name_stem(&self) -> String;
}

impl FileNameStem for Path {
    fn file_name_stem(&self) -> String {
        self.file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_common_episode_names() {
        let parsed = parse_filename("Show.Name.S01E02.1080p.WEB-DL.mkv", None);
        assert_eq!(parsed.kind, MediaKind::Series);
        assert_eq!(parsed.title, "Show Name");
        assert_eq!(parsed.season, Some(1));
        assert_eq!(parsed.episode, Some(2));
        assert_eq!(parsed.episode_end, Some(2));
        assert!(parsed.confidence >= 0.85);
    }

    #[test]
    fn reads_alternate_episode_notation() {
        let parsed = parse_filename("Another.Show 2x07.mkv", None);
        assert_eq!(parsed.kind, MediaKind::Series);
        assert_eq!(parsed.season, Some(2));
        assert_eq!(parsed.episode, Some(7));
        assert_eq!(parsed.title, "Another Show");
    }

    #[test]
    fn keeps_combined_episode_range() {
        let parsed = parse_filename("Show_S01E01E02.mkv", None);
        assert_eq!(parsed.season, Some(1));
        assert_eq!(parsed.episode, Some(1));
        assert_eq!(parsed.episode_end, Some(2));
    }

    #[test]
    fn rejects_absurd_episode_range() {
        // A range of 100+ episodes is a misread, so the file keeps episode 1 only.
        let parsed = parse_filename("Show.S01E01E150.mkv", None);
        assert_eq!(parsed.episode, Some(1));
        assert_eq!(parsed.episode_end, Some(1));
    }

    #[test]
    fn reads_year_for_movies() {
        let parsed = parse_filename("A.Movie.2021.2160p.BluRay.x265.mkv", None);
        assert_eq!(parsed.kind, MediaKind::Movie);
        assert_eq!(parsed.title, "A Movie");
        assert_eq!(parsed.year, Some(2021));
        assert_eq!(parsed.confidence, 0.75);
    }

    #[test]
    fn prefers_release_year_after_numeric_title_years() {
        for (filename, title, year) in [
            (
                "2001 A Space Odyssey (1968).mkv",
                "2001 A Space Odyssey",
                1968,
            ),
            ("1917 (2019).mkv", "1917", 2019),
            ("Blade Runner 2049 (2017).mkv", "Blade Runner 2049", 2017),
            ("2001 Maniacs (2005).mkv", "2001 Maniacs", 2005),
        ] {
            let parsed = parse_filename(filename, None);
            assert_eq!((parsed.title.as_str(), parsed.year), (title, Some(year)));
        }
    }

    #[test]
    fn resolution_is_not_an_episode_or_confident_folder_match() {
        let parsed = parse_filename("Film.720x480.mkv", Some("Films"));
        assert_eq!(parsed.kind, MediaKind::Movie);
        assert_eq!(parsed.title, "Film");
        assert!(parsed.confidence < 0.75);
        assert_eq!(
            parse_filename("Film.640x360.mkv", None).kind,
            MediaKind::Movie
        );
    }

    #[test]
    fn year_before_the_season_marker_truncates_the_title() {
        let parsed = parse_filename("Show.2019.S01E04.720p.mkv", None);
        assert_eq!(parsed.year, Some(2019));
        assert_eq!(parsed.title, "Show");
        assert_eq!(parsed.season, Some(1));
        assert_eq!(parsed.episode, Some(4));
    }

    #[test]
    fn an_episode_takes_the_show_year_from_its_folder() {
        // What a real library looks like: the year is on the show folder, and
        // the episode files carry none at all.
        let parsed = parse_filename(
            "From (2022)/Season 02/From - s02e04.mkv",
            Some("From (2022)"),
        );
        assert_eq!(parsed.kind, MediaKind::Series);
        assert_eq!(parsed.title, "From");
        assert_eq!(parsed.year, Some(2022));
        assert_eq!(parsed.season, Some(2));
        assert_eq!(parsed.episode, Some(4));
    }

    #[test]
    fn a_folder_year_does_not_override_the_filename() {
        let parsed = parse_filename("Show.2021.S01E01.mkv", Some("Show (2019)"));
        assert_eq!(parsed.year, Some(2021));
    }

    #[test]
    fn a_collection_folder_year_is_not_applied_to_a_movie() {
        // `Duology (2009-2011)` names a range, not a release year. A film whose
        // name has no year stays without one rather than borrowing 2009.
        let parsed = parse_filename(
            "Sherlock Holmes A Game Of Shadows.mkv",
            Some("Sherlock Holmes Duology (2009-2011)"),
        );
        assert_eq!(parsed.kind, MediaKind::Movie);
        assert_eq!(parsed.year, None);
    }

    #[test]
    fn numeric_names_stay_unknown_and_playable() {
        let parsed = parse_filename("12345.mkv", None);
        assert_eq!(parsed.kind, MediaKind::Unknown);
        assert_eq!(parsed.title, "12345");
        assert_eq!(parsed.confidence, 0.1);
    }

    #[test]
    fn empty_name_falls_back_to_the_folder() {
        let parsed = parse_filename("2160p.mkv", Some("Features/The Film"));
        assert_eq!(parsed.kind, MediaKind::Movie);
        assert_eq!(parsed.title, "The Film");
    }

    #[test]
    fn season_folder_does_not_become_a_title() {
        let parsed = parse_filename("1080p.mkv", Some("Show/Season 2"));
        assert_eq!(parsed.kind, MediaKind::Unknown);
        assert_eq!(parsed.title, "1080p");
    }
}
