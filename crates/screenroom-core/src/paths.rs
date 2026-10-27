//! Path identity: deciding when two spellings name the same folder.
//!
//! This exists because two functions that *look* like they compare paths do not
//! compare the same thing:
//!
//! * `std::path::Path` comparisons are lexical. `Path::new(r"D:\Media")` and
//!   `Path::new(r"D:/Media")` are different values on every platform, even though
//!   Windows treats `/` and `\` as the same separator.
//! * `std::fs::canonicalize` does not return the spelling it was given. On Windows
//!   it returns a **verbatim** path, so canonicalizing `D:\Media` yields
//!   `\\?\D:\Media` (and a UNC share comes back as `\\?\UNC\server\share`). A
//!   lexical comparison against the spelling that was stored therefore fails on
//!   every Windows machine, without anything being wrong with the folder.
//!
//! So identity is decided here, on one comparable key, instead of by comparing
//! `Path` values or their `lossy` strings at each call site.
//!
//! # What this does not do
//!
//! It normalises **spelling**, not **resolution**. `..` is deliberately left
//! alone: resolving it textually is wrong whenever a link is involved, so the
//! resolution stays with `std::fs::canonicalize` and this runs on its output.
//!
//! A Windows *volume GUID* path (`\\?\Volume{…}\Media`) is also left distinct from
//! `D:\Media` after the prefix is stripped, because they name the same folder and
//! no string function can know that. That is the removable-storage case: a drive
//! plugged in again under a different letter has a different spelling with the same
//! identity, and the recovery is an explicit relink, not a guessed match.

use crate::library::PathSemantics;

/// The separator and prefix rules of the platform that wrote a path.
///
/// Separate from [`PathSemantics`] because the two are independent: a
/// case-sensitive NTFS volume exists, and so does a case-insensitive mount of a
/// case-sensitive file system. One is about the volume, this is about the syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathStyle {
    /// Drive letters, UNC shares, and both `/` and `\` as separators. Windows.
    Windows,
    /// One separator, and a backslash is an ordinary character in a filename.
    Posix,
}

impl PathStyle {
    /// What this machine uses.
    ///
    /// A value rather than a `cfg!` at each call site so the other platform's rules
    /// can be exercised from here — the same reason
    /// [`PathSemantics`] is a value.
    pub fn host() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else {
            Self::Posix
        }
    }
}

/// The Windows verbatim prefix, after separators have been folded to `/`.
const VERBATIM: &str = "//?/";
/// The verbatim prefix that wraps a UNC share.
const VERBATIM_UNC: &str = "//?/UNC/";

/// One comparable spelling of a location.
///
/// Two keys are equal only when the two paths name the same folder *as far as
/// spelling can tell*. Case is folded when `semantics` says the volume folds it, so
/// the answer depends on the volume rather than on the platform.
///
/// Public because a bug report of the form "it says my folder was replaced by a
/// link" or "it says this folder is already a source" is answered by this value,
/// and because a caller holding a path from somewhere else needs the same answer.
pub fn location_key(raw: &str, semantics: PathSemantics, style: PathStyle) -> String {
    let mut key = raw.to_string();

    if style == PathStyle::Windows {
        // Windows accepts either separator, so fold both to one before anything
        // looks at the shape of the string. On POSIX a backslash is a legal
        // character in a filename and must survive untouched.
        key = key.replace('\\', "/");
        // A UNC share or a drive-letter path is usually written with a doubled
        // leading separator; collapse runs only at the start so the root keeps its
        // recognised shape.
        if key.starts_with("//") {
            let rest = key.trim_start_matches('/');
            key = format!("//{rest}");
        }
    }

    key = strip_verbatim(&key);
    key = collapse_separators(&key);
    key = trim_trailing_separator(&key);

    match semantics {
        PathSemantics::CaseInsensitive => key.to_lowercase(),
        PathSemantics::CaseSensitive => key,
    }
}

/// Removes a Windows verbatim marker, keeping what it wrapped.
///
/// `\\?\D:\Media` becomes `D:\Media`, and `\\?\UNC\server\share` becomes
/// `\\server\share`. The device namespace (`\\.\`) is left alone: `\\.\C:` is a
/// handle on a physical drive, not the folder `C:\`, so it is not the same thing
/// and must not be made to look like it.
fn strip_verbatim(key: &str) -> String {
    if let Some(rest) = key.strip_prefix(VERBATIM_UNC) {
        return format!("//{rest}");
    }
    // Windows treats the `UNC` marker case-insensitively.
    if key.len() >= VERBATIM_UNC.len()
        && key[..VERBATIM_UNC.len()].eq_ignore_ascii_case(VERBATIM_UNC)
    {
        return format!("//{}", &key[VERBATIM_UNC.len()..]);
    }
    match key.strip_prefix(VERBATIM) {
        Some(rest) => rest.to_string(),
        None => key.to_string(),
    }
}

/// Collapses a run of separators into one.
///
/// A UNC prefix keeps its two leading separators, which is the one place a doubled
/// separator means something.
fn collapse_separators(key: &str) -> String {
    let unc = key.starts_with("//");
    let body = if unc {
        key.trim_start_matches('/')
    } else {
        key
    };
    let mut collapsed = String::with_capacity(key.len());
    let mut last_was_separator = false;
    for character in body.chars() {
        let separator = character == '/';
        if !(separator && last_was_separator) {
            collapsed.push(character);
        }
        last_was_separator = separator;
    }
    // The two leading separators of a UNC prefix are the only doubled separator
    // that carries meaning, so they are put back after the collapse.
    if unc && !body.is_empty() {
        format!("//{collapsed}")
    } else {
        collapsed
    }
}

/// Drops a trailing separator, keeping the path absolute.
///
/// `C:/` and `C:` are different locations on Windows (one is the drive root, the
/// other is drive-relative), so a drive that lost its separator gets it back rather
/// than being made equal to the other.
fn trim_trailing_separator(key: &str) -> String {
    let trimmed = key.trim_end_matches('/');
    if trimmed.is_empty() {
        // Everything was separators: a root, which stays a root.
        return "/".to_string();
    }
    if key.len() > trimmed.len() && ends_with_drive_prefix(trimmed) {
        return format!("{trimmed}/");
    }
    trimmed.to_string()
}

/// True when the string ends with a bare Windows drive prefix, as in `C:`.
fn ends_with_drive_prefix(key: &str) -> bool {
    let bytes = key.as_bytes();
    bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(raw: &str) -> String {
        location_key(raw, PathSemantics::CaseInsensitive, PathStyle::Windows)
    }

    fn posix(raw: &str) -> String {
        location_key(raw, PathSemantics::CaseSensitive, PathStyle::Posix)
    }

    #[test]
    fn a_verbatim_path_matches_the_spelling_it_was_canonicalized_from() {
        // This is the Windows failure this module exists for: `canonicalize`
        // answers `\\?\D:\Media` for a folder stored as `D:\Media`, and comparing
        // those two literally rejects a healthy folder as a replaced link.
        assert_eq!(win(r"\\?\D:\Media"), win(r"D:\Media"));
        assert_eq!(win(r"\\?\D:\Media\Shows"), win(r"D:\Media\Shows"));
        // The same, in the forward-slash and lower-case spellings Windows also
        // accepts for the verbatim form.
        assert_eq!(win("//?/D:/Media"), win(r"D:\Media"));
        assert_eq!(win(r"\\?\d:\media"), win(r"D:\Media"));
    }

    #[test]
    fn a_verbatim_unc_path_matches_its_share() {
        assert_eq!(win(r"\\?\UNC\server\share"), win(r"\\server\share"));
        assert_eq!(
            win(r"\\?\UNC\server\share\Shows"),
            win(r"\\server\share\Shows")
        );
        // The marker is case-insensitive in Windows path syntax.
        assert_eq!(win(r"\\?\unc\server\share"), win(r"\\SERVER\share"));
    }

    #[test]
    fn the_device_namespace_is_not_treated_as_a_folder() {
        // `\\.\C:` is a physical drive handle. Stripping the marker would make it
        // equal to the folder `C:\`, which is a different thing.
        assert_ne!(win(r"\\.\C:"), win(r"C:\"));
        // The marker is kept, in the folded form the key uses.
        assert_eq!(win(r"\\.\C:"), "//./c:");
    }

    #[test]
    fn both_windows_separators_name_one_folder() {
        assert_eq!(win(r"D:\Media\Shows"), win("D:/Media/Shows"));
        assert_eq!(win(r"D:\Media\Shows"), win(r"D:\Media/Shows"));
        // And a doubled separator is not a different folder.
        assert_eq!(win(r"D:\Media\\Shows"), win(r"D:\Media\Shows"));
    }

    #[test]
    fn a_trailing_separator_does_not_make_a_different_folder() {
        assert_eq!(win(r"D:\Media\"), win(r"D:\Media"));
        assert_eq!(win("D:/Media//"), win(r"D:\Media"));
        assert_eq!(posix("/Media/Shows/"), posix("/Media/Shows"));
    }

    #[test]
    fn a_drive_root_is_not_the_drive_prefix() {
        // Drive-relative `C:` resolves against the current directory of that drive,
        // so it is genuinely not `C:\` and must keep a distinct key.
        assert_ne!(win(r"C:\"), win("C:"));
        assert_eq!(win(r"C:\"), "c:/");
    }

    #[test]
    fn a_posix_backslash_is_part_of_the_name() {
        // A backslash is a legal character in a POSIX filename. Folding it would
        // report a folder as overlapping its own parent.
        assert_ne!(posix(r"/Media/Shows\Archive"), posix("/Media/Shows"));
        assert_eq!(posix(r"/Media/Shows\Archive"), r"/Media/Shows\Archive");
    }

    #[test]
    fn case_folding_follows_the_volume_not_the_style() {
        // A case-sensitive volume on a Windows-style path must not fold, and a
        // case-insensitive mount of a POSIX path must.
        let windows_sensitive = location_key(
            r"D:\Media",
            PathSemantics::CaseSensitive,
            PathStyle::Windows,
        );
        assert_ne!(
            windows_sensitive,
            location_key(
                r"d:\media",
                PathSemantics::CaseSensitive,
                PathStyle::Windows
            )
        );
        assert_eq!(
            location_key("/Media", PathSemantics::CaseInsensitive, PathStyle::Posix),
            location_key("/media", PathSemantics::CaseInsensitive, PathStyle::Posix)
        );
    }

    #[test]
    fn a_root_stays_a_root_rather_than_becoming_an_empty_key() {
        // An empty key would compare equal to every other empty key and to a
        // relative path, so a root must not reduce to one.
        assert_eq!(posix("/"), "/");
        assert_eq!(posix("///"), "/");
        assert_eq!(win("//server/share/"), "//server/share");
        assert_eq!(win(r"C:\"), "c:/");
    }

    #[test]
    fn a_folder_that_differs_is_never_folded_together() {
        // The other half of the contract: nothing here makes distinct folders equal.
        assert_ne!(win(r"D:\Media"), win(r"D:\Media2"));
        assert_ne!(win(r"D:\Media\Shows"), win(r"D:\Media\Movies"));
        assert_ne!(win(r"D:\Media"), win(r"E:\Media"));
        assert_ne!(posix("/Media"), posix("/Media2"));
        // A prefix is not the folder itself.
        assert_ne!(win(r"D:\Media"), win(r"D:\Media\Shows"));
    }
}
