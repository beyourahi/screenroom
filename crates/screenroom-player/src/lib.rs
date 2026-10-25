//! libmpv integration for Screenroom desktop clients.
//!
//! ## Why this crate loads libmpv at runtime
//!
//! libmpv is resolved with `dlopen` instead of being linked. That keeps:
//!
//! * the build reproducible without shipping a native binary in Git,
//! * a missing or version-mismatched libmpv a *reported capability*, never a
//!   start-up crash, and
//! * redistribution honest: the license of the shipped libmpv/FFmpeg build is a
//!   release decision recorded in `docs/LICENSING.md`, not implied by this crate.
//!
//! ## Rendering boundary
//!
//! libmpv's render API offers only OpenGL, Direct3D 11 and software backends
//! (see `mpv/render.h`). iced draws through wgpu, which has no libmpv backend, so
//! the video frame has to be bridged. [`RenderBridge`] records the supported
//! options per platform; see `docs/FEASIBILITY_GATES.md` for which one is proven.
//!
//! ## Threading
//!
//! One [`Mpv`] owns one libmpv core. Commands may be issued from any thread, but
//! render-API calls must all happen on the thread whose GL/D3D context created
//! the render context. This crate exposes only the core client API and
//! deliberately does not wrap the render context yet.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::Path;
use std::ptr::NonNull;
use std::sync::Arc;

use libloading::Library;

/// Default timeout for waiting on a libmpv event, in seconds.
pub const DEFAULT_EVENT_TIMEOUT: f64 = 5.0;

/// Failures from the native boundary.
#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    /// The dynamic library could not be found or opened.
    #[error("libmpv is not available on this device: {0}")]
    Unavailable(String),
    /// A libmpv call returned an error code.
    #[error("{context} failed: {message} (code {code})")]
    Mpv {
        /// What was being attempted.
        context: String,
        /// libmpv's own message for the code.
        message: String,
        /// Negative `mpv_error` code.
        code: i32,
    },
    /// An argument could not be represented as a C string.
    #[error("invalid argument: {0}")]
    Argument(String),
}

/// Which bridge connects libmpv frames to the desktop renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderBridge {
    /// libmpv's software renderer into a CPU buffer, uploaded to a wgpu texture
    /// each frame. Portable, always available, one extra full-frame copy.
    Software,
    /// libmpv renders into a GL framebuffer object on a GL context; the pixels
    /// are then imported or copied into wgpu. Required for hardware decoding on
    /// macOS, where wgpu uses Metal.
    OpenGl,
    /// libmpv renders into a D3D11 texture that wgpu imports as a shared
    /// resource. Windows only, lowest copy cost.
    Direct3D11,
}

/// The bridge this platform should use first.
///
/// This is the *starting* choice for the feasibility work, not a measured
/// victory: the gates document records which bridge has actually been shown to
/// present a frame on real hardware.
pub fn preferred_bridge() -> RenderBridge {
    if cfg!(windows) {
        RenderBridge::Direct3D11
    } else {
        RenderBridge::OpenGl
    }
}

/// Whether libmpv playback is available on this device, and why not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerCapability {
    /// True only when libmpv was loaded and initialised.
    pub available: bool,
    /// Human-readable reason, always set.
    pub reason: String,
    /// Path of the loaded library when available.
    pub library_path: Option<String>,
    /// `major.minor` client API version reported by libmpv.
    pub api_version: Option<String>,
}

/// Candidate library names per platform, in preference order.
fn candidates() -> &'static [&'static str] {
    if cfg!(windows) {
        &["mpv-2.dll", "libmpv-2.dll", "libmpv.dll"]
    } else if cfg!(target_os = "macos") {
        &[
            "libmpv.2.dylib",
            "libmpv.dylib",
            "/opt/homebrew/lib/libmpv.2.dylib",
            "/usr/local/lib/libmpv.2.dylib",
            "/opt/homebrew/lib/libmpv.dylib",
            "/usr/local/lib/libmpv.dylib",
        ]
    } else {
        &["libmpv.so.2", "libmpv.so.1", "libmpv.so"]
    }
}

type MpvHandle = *mut c_void;

/// An `mpv_event` as laid out by `client.h`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct RawEvent {
    event_id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *mut c_void,
}

type FnCreate = unsafe extern "C" fn() -> MpvHandle;
type FnInitialize = unsafe extern "C" fn(MpvHandle) -> c_int;
type FnTerminateDestroy = unsafe extern "C" fn(MpvHandle);
type FnSetOptionString = unsafe extern "C" fn(MpvHandle, *const c_char, *const c_char) -> c_int;
type FnCommand = unsafe extern "C" fn(MpvHandle, *const *const c_char) -> c_int;
type FnGetProperty = unsafe extern "C" fn(MpvHandle, *const c_char, c_int, *mut c_void) -> c_int;
type FnErrorString = unsafe extern "C" fn(c_int) -> *const c_char;
type FnWaitEvent = unsafe extern "C" fn(MpvHandle, f64) -> *mut RawEvent;
type FnClientApiVersion = unsafe extern "C" fn() -> u64;

struct Symbols {
    create: FnCreate,
    initialize: FnInitialize,
    terminate_destroy: FnTerminateDestroy,
    set_option_string: FnSetOptionString,
    command: FnCommand,
    get_property: FnGetProperty,
    error_string: FnErrorString,
    wait_event: FnWaitEvent,
    client_api_version: FnClientApiVersion,
}

/// A loaded libmpv shared library.
pub struct LibMpv {
    path: String,
    symbols: Symbols,
    // Kept alive for as long as any symbol pointer is in use.
    _library: Library,
}

impl std::fmt::Debug for LibMpv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibMpv")
            .field("path", &self.path)
            .field("api_version", &self.api_version())
            .finish()
    }
}

impl LibMpv {
    /// Loads the first available libmpv on this device.
    pub fn load() -> Result<Arc<Self>, PlayerError> {
        let mut attempted = Vec::new();
        for candidate in candidates() {
            match Self::load_path(candidate) {
                Ok(library) => return Ok(Arc::new(library)),
                Err(error) => attempted.push(format!("{candidate} ({error})")),
            }
        }
        Err(PlayerError::Unavailable(format!(
            "no libmpv shared library found; tried {}",
            attempted.join(", ")
        )))
    }

    /// Loads one explicit libmpv build, for a packaged application.
    pub fn load_path(path: impl AsRef<Path>) -> Result<Self, PlayerError> {
        let path = path.as_ref();
        let library = unsafe { Library::new(path) }
            .map_err(|error| PlayerError::Unavailable(format!("{}: {error}", path.display())))?;
        unsafe {
            let symbols = Symbols {
                create: *library
                    .get(b"mpv_create\0")
                    .map_err(|error| missing(path, "mpv_create", error))?,
                initialize: *library
                    .get(b"mpv_initialize\0")
                    .map_err(|error| missing(path, "mpv_initialize", error))?,
                terminate_destroy: *library
                    .get(b"mpv_terminate_destroy\0")
                    .map_err(|error| missing(path, "mpv_terminate_destroy", error))?,
                set_option_string: *library
                    .get(b"mpv_set_option_string\0")
                    .map_err(|error| missing(path, "mpv_set_option_string", error))?,
                command: *library
                    .get(b"mpv_command\0")
                    .map_err(|error| missing(path, "mpv_command", error))?,
                get_property: *library
                    .get(b"mpv_get_property\0")
                    .map_err(|error| missing(path, "mpv_get_property", error))?,
                error_string: *library
                    .get(b"mpv_error_string\0")
                    .map_err(|error| missing(path, "mpv_error_string", error))?,
                wait_event: *library
                    .get(b"mpv_wait_event\0")
                    .map_err(|error| missing(path, "mpv_wait_event", error))?,
                client_api_version: *library
                    .get(b"mpv_client_api_version\0")
                    .map_err(|error| missing(path, "mpv_client_api_version", error))?,
            };
            Ok(Self {
                path: path.display().to_string(),
                symbols,
                _library: library,
            })
        }
    }

    /// libmpv client API as `major.minor`.
    pub fn api_version(&self) -> String {
        let raw = unsafe { (self.symbols.client_api_version)() };
        format!("{}.{}", raw >> 16, raw & 0xffff)
    }

    /// Path of the loaded library.
    pub fn path(&self) -> &str {
        &self.path
    }

    fn message(&self, code: c_int) -> String {
        unsafe {
            let pointer = (self.symbols.error_string)(code);
            if pointer.is_null() {
                format!("mpv error {code}")
            } else {
                CStr::from_ptr(pointer).to_string_lossy().to_string()
            }
        }
    }
}

fn missing(path: &Path, symbol: &str, error: libloading::Error) -> PlayerError {
    PlayerError::Unavailable(format!(
        "{} does not export {symbol}: {error}",
        path.display()
    ))
}

/// Reports whether libmpv can be used, without panicking or throwing.
pub fn capability() -> PlayerCapability {
    match LibMpv::load() {
        Ok(library) => {
            // A library that loads but cannot create a core is not usable.
            match Mpv::new(&library) {
                Ok(_core) => PlayerCapability {
                    available: true,
                    reason: format!("libmpv {} loaded", library.api_version()),
                    library_path: Some(library.path().to_string()),
                    api_version: Some(library.api_version()),
                },
                Err(error) => PlayerCapability {
                    available: false,
                    reason: format!("libmpv loaded but could not initialise: {error}"),
                    library_path: Some(library.path().to_string()),
                    api_version: Some(library.api_version()),
                },
            }
        }
        Err(error) => PlayerCapability {
            available: false,
            reason: error.to_string(),
            library_path: None,
            api_version: None,
        },
    }
}

/// A libmpv core handle.
pub struct Mpv {
    handle: NonNull<c_void>,
    library: Arc<LibMpv>,
}

// The core handle is safe to send between threads for the client API calls wrapped
// here; render-context calls are not wrapped and must stay on their own thread.
unsafe impl Send for Mpv {}
unsafe impl Sync for Mpv {}

impl std::fmt::Debug for Mpv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mpv").finish_non_exhaustive()
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        unsafe { (self.library.symbols.terminate_destroy)(self.handle.as_ptr()) }
    }
}

impl Mpv {
    /// Creates and initialises one libmpv core.
    pub fn new(library: &Arc<LibMpv>) -> Result<Self, PlayerError> {
        let handle = unsafe { (library.symbols.create)() };
        let handle = NonNull::new(handle)
            .ok_or_else(|| PlayerError::Unavailable("mpv_create returned null".into()))?;
        let core = Self {
            handle,
            library: Arc::clone(library),
        };
        let code = unsafe { (library.symbols.initialize)(handle.as_ptr()) };
        core.check(code, "mpv_initialize")?;
        Ok(core)
    }

    fn check(&self, code: c_int, context: &str) -> Result<(), PlayerError> {
        if code < 0 {
            return Err(PlayerError::Mpv {
                context: context.to_string(),
                message: self.library.message(code),
                code,
            });
        }
        Ok(())
    }

    /// Sets an mpv option before playback starts.
    pub fn set_option(&self, name: &str, value: &str) -> Result<(), PlayerError> {
        let name = to_cstring(name)?;
        let value = to_cstring(value)?;
        let code = unsafe {
            (self.library.symbols.set_option_string)(
                self.handle.as_ptr(),
                name.as_ptr(),
                value.as_ptr(),
            )
        };
        self.check(
            code,
            &format!("mpv_set_option_string({})", name.to_string_lossy()),
        )
    }

    /// Runs an mpv command with an exact argument array.
    ///
    /// Arguments are passed as a terminated array, never through a shell, so a
    /// filename containing spaces or quotes cannot become another command.
    pub fn command(&self, arguments: &[&str]) -> Result<(), PlayerError> {
        if arguments.is_empty() {
            return Err(PlayerError::Argument("empty command".into()));
        }
        let owned = arguments
            .iter()
            .map(|argument| to_cstring(argument))
            .collect::<Result<Vec<_>, _>>()?;
        let mut pointers: Vec<*const c_char> = owned.iter().map(|value| value.as_ptr()).collect();
        pointers.push(std::ptr::null());
        let code =
            unsafe { (self.library.symbols.command)(self.handle.as_ptr(), pointers.as_ptr()) };
        self.check(code, &format!("mpv_command({})", arguments[0]))
    }

    /// Reads a numeric property such as `duration`.
    pub fn get_f64(&self, name: &str) -> Result<f64, PlayerError> {
        let name = to_cstring(name)?;
        let mut value: f64 = 0.0;
        let code = unsafe {
            (self.library.symbols.get_property)(
                self.handle.as_ptr(),
                name.as_ptr(),
                5, // MPV_FORMAT_DOUBLE
                (&mut value as *mut f64).cast(),
            )
        };
        self.check(
            code,
            &format!("mpv_get_property({})", name.to_string_lossy()),
        )?;
        Ok(value)
    }

    /// Blocks for the next event, returning its raw identifier.
    ///
    /// `MPV_EVENT_NONE` (0) means the timeout elapsed.
    pub fn wait_event(&self, timeout_seconds: f64) -> i32 {
        let event =
            unsafe { (self.library.symbols.wait_event)(self.handle.as_ptr(), timeout_seconds) };
        if event.is_null() {
            return 0;
        }
        unsafe { (*event).event_id }
    }
}

fn to_cstring(value: &str) -> Result<CString, PlayerError> {
    CString::new(value).map_err(|_| PlayerError::Argument(value.to_string()))
}

/// libmpv event identifiers used by this crate.
pub mod event {
    /// No event within the timeout.
    pub const NONE: i32 = 0;
    /// The core is shutting down.
    pub const SHUTDOWN: i32 = 1;
    /// Playback of a new file started.
    pub const START_FILE: i32 = 6;
    /// Playback of the current file ended.
    pub const END_FILE: i32 = 7;
    /// The file was opened and its streams are known.
    pub const FILE_LOADED: i32 = 8;
}

/// Playback lifecycle shared by every platform adapter.
///
/// Native adapters report transitions; the domain only stores progress from a
/// state where a position is meaningful, which is why `Preparing` and `Resolving`
/// never checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    /// No media attached.
    Idle,
    /// Locating and validating the file.
    Resolving,
    /// The player accepted the media and is preparing streams.
    Preparing,
    /// Playing.
    Playing,
    /// Paused by the user.
    Paused,
    /// Reached the end.
    Ended,
    /// Failed; the reason belongs to the adapter.
    Failed,
}

impl PlayerState {
    /// Whether a position from this state is meaningful to store.
    pub fn accepts_progress(self) -> bool {
        matches!(self, Self::Playing | Self::Paused | Self::Ended)
    }

    /// Applies an event, rejecting transitions that would corrupt progress.
    pub fn transition(self, next: PlayerState) -> PlayerState {
        use PlayerState::*;
        match (self, next) {
            // Failures and end-of-media are terminal for the session.
            (_, Failed) => Failed,
            (Failed, _) => Failed,
            (Ended, _) => Ended,
            (_, Ended) => Ended,
            (Idle, Resolving) => Resolving,
            (Idle, _) => self,
            (Resolving, Preparing) => Preparing,
            (Resolving, Playing) => Playing,
            (Resolving, _) => self,
            (Preparing, Playing) => Playing,
            (Preparing, _) => self,
            (Playing, Paused) => Paused,
            (Paused, Playing) => Playing,
            (Playing, Playing) | (Paused, Paused) => self,
            // Anything else (a pause before playback) is ignored rather than guessed.
            (_, _) => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_is_only_meaningful_after_playing() {
        assert!(!PlayerState::Idle.accepts_progress());
        assert!(!PlayerState::Resolving.accepts_progress());
        assert!(!PlayerState::Preparing.accepts_progress());
        assert!(PlayerState::Playing.accepts_progress());
        assert!(PlayerState::Paused.accepts_progress());
        assert!(PlayerState::Ended.accepts_progress());
    }

    #[test]
    fn prepare_zero_position_cannot_become_playing() {
        assert_eq!(
            PlayerState::Preparing.transition(PlayerState::Paused),
            PlayerState::Preparing
        );
        assert_eq!(
            PlayerState::Preparing.transition(PlayerState::Playing),
            PlayerState::Playing
        );
    }

    #[test]
    fn ended_and_failed_are_terminal() {
        assert_eq!(
            PlayerState::Ended.transition(PlayerState::Playing),
            PlayerState::Ended
        );
        assert_eq!(
            PlayerState::Failed.transition(PlayerState::Playing),
            PlayerState::Failed
        );
        assert_eq!(
            PlayerState::Playing.transition(PlayerState::Failed),
            PlayerState::Failed
        );
    }

    #[test]
    fn capability_is_reported_without_panicking() {
        // Whatever this host has, the answer must be usable and explained.
        let capability = capability();
        assert!(!capability.reason.is_empty());
        assert_eq!(capability.available, capability.api_version.is_some());
        assert_eq!(capability.available, capability.library_path.is_some());
    }

    #[test]
    fn preferred_bridge_matches_platform() {
        let bridge = preferred_bridge();
        if cfg!(windows) {
            assert_eq!(bridge, RenderBridge::Direct3D11);
        } else {
            assert_eq!(bridge, RenderBridge::OpenGl);
        }
    }
}
