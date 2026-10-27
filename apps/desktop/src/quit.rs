//! How the menu bar's Quit reaches the same code path as closing the window.
//!
//! On macOS the menu bar belongs to AppKit, not to the event loop. The Quit item
//! AppKit installs for an application sends `terminate:`, which ends the process
//! *below* iced: no close request is ever delivered, so a viewing in progress is
//! finalised by nobody and the position that survives is whatever the last
//! periodic checkpoint happened to catch. The window's own close button does not
//! work that way — AppKit asks the window, winit queues `CloseRequested`, iced
//! delivers it to [`crate::app`], and the viewing is stored and closed before the
//! process exits.
//!
//! So this module is not a second shutdown path. It points the Quit command at
//! the path that is already correct: `performClose:` is exactly what the close
//! button performs, so after routing, the menu item and the close button are the
//! same operation and cannot disagree about whether the open file was stored.
//!
//! Everything here is additive and best-effort. A menu whose Quit item cannot be
//! identified is left exactly as it was, because a half-rewritten menu bar is
//! worse than the behaviour it replaces.

use std::ffi::CStr;
use std::path::{Path, PathBuf};

/// The selector the application menu's Quit item sends.
///
/// Identification is by action, never by title: the title is localised and
/// carries the application's name, so matching it would work in one language
/// only.
pub const QUIT_SELECTOR: &CStr = c"terminate:";

/// The selector a routed Quit item sends instead — the one the window's own
/// close button performs.
pub const CLOSE_SELECTOR: &CStr = c"performClose:";

/// What examining the menu bar did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitRouting {
    /// The Quit item was found and re-pointed at the window's close action, so
    /// quitting stores the viewing exactly as closing the window does.
    Routed,
    /// This platform has no menu bar to route; the host's own quit behaviour
    /// stands and nothing is wrong.
    Unsupported,
    /// There was no application menu to examine.
    NoMenu,
    /// There was a menu, but no item in it quits the application.
    NoQuitItem,
}

impl QuitRouting {
    /// A short, stable word for diagnostics and logs.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Routed => "routed",
            Self::Unsupported => "unsupported",
            Self::NoMenu => "no-menu",
            Self::NoQuitItem => "no-quit-item",
        }
    }

    /// Whether quitting the way the user expects will store the open viewing.
    ///
    /// False only where the menu exists and the Quit item was not found, which is
    /// the one case a user could be misled by.
    pub fn is_graceful(self) -> bool {
        matches!(self, Self::Routed | Self::Unsupported)
    }
}

impl std::fmt::Display for QuitRouting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Index of the item that quits the application, given each item's action.
///
/// An item with no action — a separator, or a row that only owns a submenu — can
/// never be the one. Taken as a slice so the decision can be checked without a
/// menu bar, which is the only part of this module a test can reach.
fn quit_item_index(actions: &[Option<&CStr>]) -> Option<usize> {
    actions
        .iter()
        .position(|action| *action == Some(QUIT_SELECTOR))
}

/// Environment variable naming a file to write this examination's outcome to.
///
/// A launch that sets it examines the menu bar, writes the outcome there, and
/// stops rather than running a user interface nobody is watching. The menu bar
/// only exists inside a running application, so that is the only way this can be
/// checked at all — and it is checked without a window ever appearing, because a
/// window on the desktop belongs to whoever is using the machine.
///
/// See `scripts/check-quit-routing.sh`.
pub const CHECK_ENV: &str = "SCREENROOM_QUIT_CHECK";

/// The file this launch was asked to report to, if any.
pub fn check_path() -> Option<PathBuf> {
    std::env::var_os(CHECK_ENV).map(PathBuf::from)
}

/// Whether this launch exists only to be checked, and should show nothing.
pub fn check_requested() -> bool {
    check_path().is_some()
}

/// Writes the outcome where the check asked for it.
///
/// Written once, when the outcome is final, so a reader waits for the file rather
/// than guessing how long the application needs to start.
pub fn report_check(path: &Path, routing: QuitRouting) -> std::io::Result<()> {
    std::fs::write(path, format!("{routing}\n"))
}

/// Points the menu bar's Quit at the window's close action, once.
///
/// Called while the application starts, on the thread that owns the menu bar.
pub fn route_quit_through_close() -> QuitRouting {
    #[cfg(target_os = "macos")]
    {
        imp::route()
    }
    #[cfg(not(target_os = "macos"))]
    {
        QuitRouting::Unsupported
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject, Sel};

    use super::{CLOSE_SELECTOR, QuitRouting, quit_item_index};

    /// The application menu's Quit item, wherever AppKit put it.
    ///
    /// The top level is checked first so a menu that keeps Quit beside its
    /// siblings is still found, then one level of submenus — which is where the
    /// application menu's own Quit item lives.
    unsafe fn find_quit_item(menu: *mut AnyObject) -> Option<*mut AnyObject> {
        if let Some(item) = unsafe { item_that_quits(menu) } {
            return Some(item);
        }
        let count: isize = unsafe { msg_send![menu, numberOfItems] };
        for index in 0..count {
            let item: *mut AnyObject = unsafe { msg_send![menu, itemAtIndex: index] };
            if item.is_null() {
                continue;
            }
            let submenu: *mut AnyObject = unsafe { msg_send![item, submenu] };
            if submenu.is_null() {
                continue;
            }
            if let Some(item) = unsafe { item_that_quits(submenu) } {
                return Some(item);
            }
        }
        None
    }

    /// The item in `menu` whose action quits the application, if there is one.
    unsafe fn item_that_quits(menu: *mut AnyObject) -> Option<*mut AnyObject> {
        let index = quit_item_index(&unsafe { item_actions(menu) })?;
        let item: *mut AnyObject = unsafe { msg_send![menu, itemAtIndex: index as isize] };
        (!item.is_null()).then_some(item)
    }

    /// The action of every item in `menu`, `None` where an item has none.
    unsafe fn item_actions(menu: *mut AnyObject) -> Vec<Option<&'static std::ffi::CStr>> {
        let count: isize = unsafe { msg_send![menu, numberOfItems] };
        let mut actions = Vec::with_capacity(count.max(0) as usize);
        for index in 0..count {
            let item: *mut AnyObject = unsafe { msg_send![menu, itemAtIndex: index] };
            // `action` is nullable on a separator or a submenu holder, which is
            // why the return type is the optional form.
            let action: Option<Sel> = if item.is_null() {
                None
            } else {
                unsafe { msg_send![item, action] }
            };
            actions.push(action.map(Sel::name));
        }
        actions
    }

    pub fn route() -> QuitRouting {
        // `AnyClass::get` rather than the `class!` macro so a missing framework is
        // a reported outcome instead of a panic during startup.
        let Some(application_class) = AnyClass::get(c"NSApplication") else {
            return QuitRouting::NoMenu;
        };
        // SAFETY: `sharedApplication` is AppKit's own singleton, and this runs on
        // the main thread, where AppKit requires all of this to happen.
        let application: *mut AnyObject =
            unsafe { msg_send![application_class, sharedApplication] };
        if application.is_null() {
            return QuitRouting::NoMenu;
        }
        let menu: *mut AnyObject = unsafe { msg_send![application, mainMenu] };
        if menu.is_null() {
            return QuitRouting::NoMenu;
        }
        let Some(quit) = (unsafe { find_quit_item(menu) }) else {
            return QuitRouting::NoQuitItem;
        };

        // The replacement is an action with no target, so it travels the responder
        // chain to the key window exactly as the close button's action does. That
        // is what keeps this a redirection rather than a hard-wired window.
        let close = Sel::register(CLOSE_SELECTOR);
        let no_target: *mut AnyObject = std::ptr::null_mut();
        unsafe {
            let _: () = msg_send![quit, setAction: close];
            let _: () = msg_send![quit, setTarget: no_target];
        }
        QuitRouting::Routed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_quit_item_is_found_by_its_action_not_by_its_place() {
        let about = c"orderFrontStandardAboutPanel:";
        let hide = c"hide:";
        // The layout AppKit actually installs: About, a separator, Services, Hide,
        // then Quit last. Nothing may be rewritten before Quit is identified.
        let application_menu = [Some(about), None, None, Some(hide), Some(QUIT_SELECTOR)];
        assert_eq!(quit_item_index(&application_menu), Some(4));

        // A menu that does not quit the application must be left alone, so a
        // rename or a different language cannot turn an unrelated item into Quit.
        let other_menu = [Some(about), None, Some(hide)];
        assert_eq!(quit_item_index(&other_menu), None);
        assert_eq!(quit_item_index(&[]), None);
        assert_eq!(quit_item_index(&[None, None]), None);
    }

    #[test]
    fn the_replacement_is_the_close_button_s_action() {
        // `performClose:` is what NSWindow implements for the close button; if this
        // ever changed, routing would send Quit somewhere the window does not
        // answer, so the constant is pinned here rather than left implicit.
        assert_eq!(CLOSE_SELECTOR.to_bytes(), b"performClose:");
        assert_eq!(QUIT_SELECTOR.to_bytes(), b"terminate:");
        assert_ne!(quit_item_index(&[Some(CLOSE_SELECTOR)]), Some(0));
    }

    #[test]
    fn only_a_menu_that_was_not_routed_is_reported_as_lossy() {
        // A platform with no menu bar is not a problem to report, and a routed
        // menu is the fix working. A menu that was found but had no Quit item is
        // the one case where quitting can still lose the last second.
        assert!(QuitRouting::Routed.is_graceful());
        assert!(QuitRouting::Unsupported.is_graceful());
        assert!(!QuitRouting::NoMenu.is_graceful());
        assert!(!QuitRouting::NoQuitItem.is_graceful());
    }
}
