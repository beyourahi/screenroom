//! Native folder selection.
//!
//! A media root has to be granted the way the platform grants one. On macOS that
//! is `NSOpenPanel`, which `rfd` wraps; on every other target this build has no
//! picker yet, and it says so instead of pretending the dialog was cancelled —
//! a cancelled dialog and an unavailable one need different words in the UI.
//!
//! The dialog is modal, so it blocks the update loop while it is open. That is the
//! correct behaviour for a folder chooser: nothing else in the window should be
//! usable until the choice is made or dismissed.

use std::path::{Path, PathBuf};

/// What a selection attempt produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Picked {
    /// A folder, or a file, was chosen.
    Path(PathBuf),
    /// The viewer dismissed the dialog.
    Cancelled,
    /// This build has no native picker; the caller should offer the typed path.
    Unavailable(&'static str),
}

/// Opens the platform's folder chooser, starting at `start` when it exists.
pub fn choose_folder(start: Option<&Path>) -> Picked {
    #[cfg(target_os = "macos")]
    {
        let mut dialog = rfd::FileDialog::new().set_title("Add a folder of your media");
        if let Some(start) = start.filter(|path| path.is_dir()) {
            dialog = dialog.set_directory(start);
        }
        match dialog.pick_folder() {
            Some(folder) => Picked::Path(folder),
            None => Picked::Cancelled,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = start;
        Picked::Unavailable(
            "This build has no native folder picker on this platform; type the folder's path.",
        )
    }
}

/// Opens the platform's save panel, suggesting `file_name`, and filters to `.db`.
pub fn choose_backup_path(start: Option<&Path>, file_name: &str) -> Picked {
    #[cfg(target_os = "macos")]
    {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Save a Screenroom backup")
            .set_file_name(file_name)
            .add_filter("Screenroom backup", &["db"]);
        if let Some(start) = start.filter(|path| path.is_dir()) {
            dialog = dialog.set_directory(start);
        }
        match dialog.save_file() {
            Some(path) => Picked::Path(path),
            None => Picked::Cancelled,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (start, file_name);
        Picked::Unavailable("This build has no native save panel; type the backup path.")
    }
}

/// Opens the platform's file panel for a backup to restore from.
pub fn choose_restore_file(start: Option<&Path>) -> Picked {
    #[cfg(target_os = "macos")]
    {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Choose a Screenroom backup")
            .add_filter("Screenroom backup", &["db"]);
        if let Some(start) = start.filter(|path| path.is_dir()) {
            dialog = dialog.set_directory(start);
        }
        match dialog.pick_file() {
            Some(path) => Picked::Path(path),
            None => Picked::Cancelled,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = start;
        Picked::Unavailable("This build has no native file panel; type the backup path.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unavailable_picker_says_so_rather_than_looking_cancelled() {
        // The distinction matters: "cancelled" means the viewer changed their mind
        // and needs no message, while "unavailable" has to point at the typed path.
        let picked = Picked::Unavailable("no picker");
        assert_ne!(picked, Picked::Cancelled);
    }
}
