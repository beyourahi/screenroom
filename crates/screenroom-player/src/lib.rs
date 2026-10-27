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
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::Arc;

use libloading::Library;

mod frame;
mod render;
mod worker;

pub use frame::Frame;
pub use worker::{
    MediaTrack, NO_TRACK, Player, PlayerCommand, PlayerFailure, PlayerSnapshot, SUB_POSITION_RANGE,
    SUB_SCALE_RANGE, TrackKind,
};

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

/// How a packaged installation lays its files out around the executable.
///
/// A value rather than a `cfg!` so both layouts can be asserted from either host.
/// The Windows layout in particular cannot be exercised on this machine, and the
/// rules it encodes — which folders a shipped DLL may live in — are exactly the
/// ones worth pinning down before they are discovered on Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleLayout {
    /// A macOS `.app`: the executable is `Contents/MacOS/<exe>` and a bundled
    /// library lives in `Contents/Frameworks`.
    MacOsAppBundle,
    /// A Windows folder holding the `.exe` — what a portable install and an
    /// installer's program folder both look like.
    WindowsPortable,
    /// Linux, where libmpv comes from the system and nothing is bundled.
    SystemOnly,
}

impl BundleLayout {
    /// The layout of the platform this binary was built for.
    pub fn host() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOsAppBundle
        } else if cfg!(windows) {
            Self::WindowsPortable
        } else {
            Self::SystemOnly
        }
    }
}

/// Whether this platform's own library search consults the working directory.
///
/// Windows does. `libloading` issues `LoadLibraryExW(name, 0, 0)`, and that search
/// order puts the current directory ahead of `PATH`, so a bare name can be satisfied
/// by a file in whatever folder the application was launched from. macOS and Linux
/// do not search the working directory for a bare library name.
///
/// This is a separate question from [`BundleLayout`]: it is about the *order* the two
/// candidate lists are tried in, and it exists so that order can be asserted on the
/// host that cannot observe it.
pub fn system_search_consults_working_directory() -> bool {
    cfg!(windows)
}

/// The bare names the platform's own search resolves.
fn system_candidates() -> Vec<PathBuf> {
    candidates().iter().map(PathBuf::from).collect()
}

/// Builds the search order from the platform search and the packaged paths.
///
/// Pure, and takes the platform fact as an argument, so both orders are assertable
/// where only one can be observed.
///
/// The two orders are deliberate and opposite:
///
/// * Where the platform search consults the working directory (Windows), the
///   explicit paths come **first**. Every one of them names a single file, so
///   nothing about the user's current folder can influence which library loads.
///   A planted `mpv-2.dll` in the folder the user launched from cannot be reached.
/// * Elsewhere the platform search comes first, so a development machine keeps
///   preferring a newer libmpv from its package manager over an older one bundled
///   with an installed copy of the application. There, the system search is by
///   name and location rather than by working directory, so preferring it is not a
///   security trade.
fn library_search_order(
    system: &[PathBuf],
    bundled: &[PathBuf],
    system_search_consults_working_directory: bool,
) -> Vec<PathBuf> {
    let mut order = Vec::with_capacity(system.len() + bundled.len());
    if system_search_consults_working_directory {
        order.extend_from_slice(bundled);
        order.extend_from_slice(system);
    } else {
        order.extend_from_slice(system);
        order.extend_from_slice(bundled);
    }
    order
}

/// The file names a bundled libmpv may have, per platform.
fn bundled_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["mpv-2.dll", "libmpv-2.dll", "libmpv.dll"]
    } else if cfg!(target_os = "macos") {
        &["libmpv.2.dylib", "libmpv.dylib"]
    } else {
        &[]
    }
}

/// Where a packaged installation should look for its own libmpv, in order.
///
/// Pure, and takes the executable's directory rather than reading it, so the layout
/// of a platform this machine is not running can still be asserted.
///
/// **Why the paths are explicit rather than bare names.** A bare name is resolved
/// by the platform's own search, and on Windows that search includes the *current
/// working directory*: `LoadLibraryExW(name, 0, 0)` — which is what `libloading`
/// issues — would happily load an `mpv-2.dll` sitting in whatever folder the user
/// happened to launch from. A media library in Downloads is a plausible place for
/// a planted DLL next to the executable, so the application directory is named
/// explicitly instead and the working directory never enters the search.
///
/// Passing `binary_dir` as the directory that the installed folder's *executable*
/// sits in.
///
/// A folder that does not exist simply fails to load, which is why the list is
/// yielded in preference order rather than filtered: checking existence here would
/// make this depend on the file system and stop being pure.
fn bundled_libmpv_paths(binary_dir: &Path, names: &[&str], layout: BundleLayout) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    match layout {
        BundleLayout::MacOsAppBundle => {
            // `Contents/MacOS/Screenroom` -> `Contents`, then `Contents/Frameworks`.
            let contents = binary_dir.parent().map(Path::to_path_buf);
            for name in names {
                if let Some(contents) = &contents {
                    paths.push(contents.join("Frameworks").join(name));
                }
                // Also beside the executable, which is what a hand-assembled or
                // unpacked copy looks like.
                paths.push(binary_dir.join(name));
            }
        }
        BundleLayout::WindowsPortable => {
            for name in names {
                // Beside the `.exe` first: that is where a portable install puts it
                // and where Windows itself would have looked.
                paths.push(binary_dir.join(name));
                // Then the subfolders a shipped archive or an installer's
                // redistributable step typically uses.
                paths.push(binary_dir.join("bin").join(name));
                paths.push(binary_dir.join("lib").join(name));
            }
        }
        BundleLayout::SystemOnly => {}
    }
    paths
}

/// The exact order `load` would try, for diagnosing a machine that cannot be
/// reached from here.
///
/// A Windows install that reports "no libmpv shared library found" is otherwise a
/// blank refusal. This names every path that was tried, in order, so the answer is
/// visible from the machine itself rather than guessed at from another one. It
/// reads only the executable's own location, so it is safe to run anywhere.
pub fn libmpv_search_plan() -> Vec<PathBuf> {
    library_search_order(
        &system_candidates(),
        &packaged_candidates(),
        system_search_consults_working_directory(),
    )
}

/// libmpv inside a packaged installation, tried after the system paths.
///
/// Built from the running executable's own location rather than from the working
/// directory or an environment variable, so an installed copy works wherever the
/// user put it — and, on Windows, so that the working directory is never searched.
/// Setting `DYLD_LIBRARY_PATH` from inside the process is not an option on a signed
/// macOS application, which is the other reason the path is computed here.
fn packaged_candidates() -> Vec<PathBuf> {
    let Ok(executable) = std::env::current_exe() else {
        return Vec::new();
    };
    let Some(binary_dir) = executable.parent() else {
        return Vec::new();
    };
    bundled_libmpv_paths(binary_dir, bundled_names(), BundleLayout::host())
}

pub(crate) type MpvHandle = *mut c_void;

/// An `mpv_event` as laid out by `client.h`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct RawEvent {
    pub(crate) event_id: c_int,
    pub(crate) error: c_int,
    pub(crate) reply_userdata: u64,
    pub(crate) data: *mut c_void,
}

/// The libmpv render API (`mpv/render.h`).
///
/// Kept optional: a build without the render API can still be detected and
/// reported instead of crashing at start-up. The parameter arrays are passed
/// type-erased (`*mut c_void`) because their exact C layout lives in
/// [`render`], the only module allowed to construct them.
#[derive(Clone, Copy)]
pub(crate) struct RenderSymbols {
    create: unsafe extern "C" fn(*mut *mut c_void, MpvHandle, *mut c_void) -> c_int,
    free: unsafe extern "C" fn(*mut c_void),
    render: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int,
    update: unsafe extern "C" fn(*mut c_void) -> u64,
    set_update_callback:
        unsafe extern "C" fn(*mut c_void, Option<unsafe extern "C" fn(*mut c_void)>, *mut c_void),
}

type FnCreate = unsafe extern "C" fn() -> MpvHandle;
type FnInitialize = unsafe extern "C" fn(MpvHandle) -> c_int;
type FnTerminateDestroy = unsafe extern "C" fn(MpvHandle);
type FnSetOptionString = unsafe extern "C" fn(MpvHandle, *const c_char, *const c_char) -> c_int;
type FnCommand = unsafe extern "C" fn(MpvHandle, *const *const c_char) -> c_int;
type FnGetProperty = unsafe extern "C" fn(MpvHandle, *const c_char, c_int, *mut c_void) -> c_int;
/// `char *mpv_get_property_string(ctx, name)`; the result is freed with [`FnFree`].
type FnGetPropertyString = unsafe extern "C" fn(MpvHandle, *const c_char) -> *mut c_char;
type FnFree = unsafe extern "C" fn(*mut c_void);
type FnErrorString = unsafe extern "C" fn(c_int) -> *const c_char;
type FnWaitEvent = unsafe extern "C" fn(MpvHandle, f64) -> *mut RawEvent;
type FnClientApiVersion = unsafe extern "C" fn() -> u64;
type FnObserveProperty = unsafe extern "C" fn(MpvHandle, u64, *const c_char, c_int) -> c_int;

struct Symbols {
    create: FnCreate,
    initialize: FnInitialize,
    terminate_destroy: FnTerminateDestroy,
    set_option_string: FnSetOptionString,
    command: FnCommand,
    get_property: FnGetProperty,
    get_property_string: FnGetPropertyString,
    free: FnFree,
    error_string: FnErrorString,
    wait_event: FnWaitEvent,
    client_api_version: FnClientApiVersion,
    observe_property: FnObserveProperty,
    render: Option<RenderSymbols>,
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
        let order = library_search_order(
            &system_candidates(),
            &packaged_candidates(),
            system_search_consults_working_directory(),
        );
        let mut attempted = Vec::new();
        for candidate in &order {
            match Self::load_path(candidate) {
                Ok(library) => return Ok(Arc::new(library)),
                Err(error) => attempted.push(format!("{} ({error})", candidate.display())),
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
                get_property_string: *library
                    .get(b"mpv_get_property_string\0")
                    .map_err(|error| missing(path, "mpv_get_property_string", error))?,
                free: *library
                    .get(b"mpv_free\0")
                    .map_err(|error| missing(path, "mpv_free", error))?,
                error_string: *library
                    .get(b"mpv_error_string\0")
                    .map_err(|error| missing(path, "mpv_error_string", error))?,
                wait_event: *library
                    .get(b"mpv_wait_event\0")
                    .map_err(|error| missing(path, "mpv_wait_event", error))?,
                client_api_version: *library
                    .get(b"mpv_client_api_version\0")
                    .map_err(|error| missing(path, "mpv_client_api_version", error))?,
                observe_property: *library
                    .get(b"mpv_observe_property\0")
                    .map_err(|error| missing(path, "mpv_observe_property", error))?,
                render: render_symbols(&library),
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

    /// The render API, when this libmpv exports it.
    pub(crate) fn render_symbols(&self) -> Option<RenderSymbols> {
        self.symbols.render
    }

    pub(crate) fn message(&self, code: c_int) -> String {
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

/// Resolves the render API, which a minimal libmpv build may not export.
fn render_symbols(library: &Library) -> Option<RenderSymbols> {
    unsafe {
        Some(RenderSymbols {
            create: *library.get(b"mpv_render_context_create\0").ok()?,
            free: *library.get(b"mpv_render_context_free\0").ok()?,
            render: *library.get(b"mpv_render_context_render\0").ok()?,
            update: *library.get(b"mpv_render_context_update\0").ok()?,
            set_update_callback: *library
                .get(b"mpv_render_context_set_update_callback\0")
                .ok()?,
        })
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
        Ok(library) => describe(&library),
        Err(error) => PlayerCapability {
            available: false,
            reason: error.to_string(),
            library_path: None,
            api_version: None,
        },
    }
}

/// Reports whether one specific libmpv build can be used.
///
/// This exists so a packaged application's own copy can be checked on the machine
/// that built it. `capability()` prefers the system paths, so on a development
/// machine it will always find Homebrew's libmpv and never exercise the copy inside
/// the bundle — which is exactly the copy that has to work on a machine without
/// Homebrew. Naming the path removes the ambiguity.
pub fn capability_at(path: impl AsRef<Path>) -> PlayerCapability {
    match LibMpv::load_path(path) {
        Ok(library) => describe(&Arc::new(library)),
        Err(error) => PlayerCapability {
            available: false,
            reason: error.to_string(),
            library_path: None,
            api_version: None,
        },
    }
}

/// Turns a loaded library into a capability report.
///
/// A library that loads but cannot create a core is not usable, so that case is
/// reported as unavailable with the library's own path, rather than as available.
fn describe(library: &Arc<LibMpv>) -> PlayerCapability {
    let version = library.api_version();
    let path = library.path().to_string();
    match Mpv::new(library) {
        Ok(_core) => PlayerCapability {
            available: true,
            reason: format!("libmpv {version} loaded"),
            library_path: Some(path),
            api_version: Some(version),
        },
        Err(error) => PlayerCapability {
            available: false,
            reason: format!("libmpv loaded but could not initialise: {error}"),
            library_path: Some(path),
            api_version: Some(version),
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
    /// Creates and initialises one libmpv core with no options set.
    pub fn new(library: &Arc<LibMpv>) -> Result<Self, PlayerError> {
        Self::new_with_options(library, &[])
    }

    /// Creates a core and applies `options` before initialisation.
    ///
    /// libmpv only accepts options before `mpv_initialize`, which is why the
    /// whole set is passed here rather than applied afterwards.
    pub fn new_with_options(
        library: &Arc<LibMpv>,
        options: &[(&str, &str)],
    ) -> Result<Self, PlayerError> {
        let handle = unsafe { (library.symbols.create)() };
        let handle = NonNull::new(handle)
            .ok_or_else(|| PlayerError::Unavailable("mpv_create returned null".into()))?;
        let core = Self {
            handle,
            library: Arc::clone(library),
        };
        for (name, value) in options {
            core.set_option(name, value)?;
        }
        let code = unsafe { (library.symbols.initialize)(handle.as_ptr()) };
        core.check(code, "mpv_initialize")?;
        Ok(core)
    }

    /// The loaded library, for the render context and error strings.
    pub(crate) fn library(&self) -> &Arc<LibMpv> {
        &self.library
    }

    /// The raw `mpv_handle`, for the render API only.
    pub(crate) fn raw_handle(&self) -> MpvHandle {
        self.handle.as_ptr()
    }

    /// Watches a property, delivering changes with `userdata` as `reply_userdata`.
    pub fn observe(&self, name: &str, format: i32, userdata: u64) -> Result<(), PlayerError> {
        let name = to_cstring(name)?;
        let code = unsafe {
            (self.library.symbols.observe_property)(
                self.handle.as_ptr(),
                userdata,
                name.as_ptr(),
                format,
            )
        };
        self.check(
            code,
            &format!("mpv_observe_property({})", name.to_string_lossy()),
        )
    }

    /// Reads an integer property such as `width`.
    pub fn get_i64(&self, name: &str) -> Result<i64, PlayerError> {
        let name = to_cstring(name)?;
        let mut value: i64 = 0;
        let code = unsafe {
            (self.library.symbols.get_property)(
                self.handle.as_ptr(),
                name.as_ptr(),
                4, // MPV_FORMAT_INT64
                (&mut value as *mut i64).cast(),
            )
        };
        self.check(
            code,
            &format!("mpv_get_property({})", name.to_string_lossy()),
        )?;
        Ok(value)
    }

    /// Blocks for the next event and returns it, or `None` on timeout.
    pub(crate) fn wait_event_raw(&self, timeout_seconds: f64) -> Option<RawEvent> {
        let event =
            unsafe { (self.library.symbols.wait_event)(self.handle.as_ptr(), timeout_seconds) };
        if event.is_null() {
            return None;
        }
        // Safety: libmpv owns the event for the duration of the next call, so it
        // is copied out immediately.
        let event = unsafe { *event };
        if event.event_id == event::NONE {
            return None;
        }
        Some(event)
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

    /// Reads a string property such as `audio-codec-name` or `track-list/0/lang`.
    ///
    /// `Ok(None)` means the property is simply *not available* — no audio track, no
    /// language tag — which is ordinary and must not be reported as an error.
    /// `mpv_get_property_string` itself avoids guessing at the C layout of a string
    /// property, which is the whole reason it is used instead of a raw
    /// `mpv_get_property` call.
    pub fn get_string(&self, name: &str) -> Result<Option<String>, PlayerError> {
        let name = to_cstring(name)?;
        let pointer = unsafe {
            (self.library.symbols.get_property_string)(self.handle.as_ptr(), name.as_ptr())
        };
        if pointer.is_null() {
            return Ok(None);
        }
        // Safety: libmpv owns `pointer` until it is freed below, and truncated
        // UTF-16 is impossible here because the string is NUL-terminated bytes.
        let text = unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned();
        unsafe { (self.library.symbols.free)(pointer.cast()) };
        Ok(Some(text))
    }

    /// Reads a boolean property such as `track-list/0/selected`.
    ///
    /// libmpv treats a flag as its own format, not as an integer, so asking for
    /// `MPV_FORMAT_INT64` on one fails and a caller that defaulted to `0` would
    /// silently report every track as unselected. That mistake is why this exists.
    pub fn get_flag(&self, name: &str) -> Result<bool, PlayerError> {
        let name = to_cstring(name)?;
        let mut value: c_int = 0;
        let code = unsafe {
            (self.library.symbols.get_property)(
                self.handle.as_ptr(),
                name.as_ptr(),
                3, // MPV_FORMAT_FLAG
                (&mut value as *mut c_int).cast(),
            )
        };
        self.check(
            code,
            &format!("mpv_get_property({})", name.to_string_lossy()),
        )?;
        Ok(value != 0)
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
    /// The set of tracks changed (a new file, or an external track was added).
    pub const TRACKS_CHANGED: i32 = 12;
    /// The selected tracks changed.
    pub const TRACK_SWITCHED: i32 = 20;
    /// An observed property changed.
    pub const PROPERTY_CHANGE: i32 = 22;
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
    fn a_windows_install_finds_its_own_libmpv_beside_the_executable() {
        // The Windows layout is asserted from macOS because it cannot be run here,
        // and it is the layout with the least room for improvisation: a shipped
        // `mpv-2.dll` may sit beside the `.exe` or in the `bin`/`lib` folder a
        // release archive uses, and nothing else is searched.
        let install = Path::new(r"C:\Program Files\Screenroom");
        let names = ["mpv-2.dll", "libmpv.dll"];
        let paths = bundled_libmpv_paths(install, &names, BundleLayout::WindowsPortable);

        // Compared as paths rather than as strings. `Path::join` uses this host's
        // separator, so building a Windows-shaped path on macOS renders
        // `...\Screenroom/mpv-2.dll`. That is an artefact of testing the layout from
        // the wrong platform and not a defect on Windows, where `join` uses `\`; the
        // folder *structure* is what is being asserted either way.
        assert_eq!(
            paths,
            vec![
                install.join("mpv-2.dll"),
                install.join("bin").join("mpv-2.dll"),
                install.join("lib").join("mpv-2.dll"),
                install.join("libmpv.dll"),
                install.join("bin").join("libmpv.dll"),
                install.join("lib").join("libmpv.dll"),
            ]
        );

        // Order is the point: beside the executable first, and the real name before
        // the alternates.
        assert!(paths[0].ends_with("mpv-2.dll"));
        assert_eq!(paths[0].parent(), Some(install));

        // Every path is anchored to the install folder, never to the process's
        // working directory. A bare name here is what would let a planted DLL in
        // whatever folder the user launched from be loaded instead.
        for path in &paths {
            assert!(
                path.starts_with(install),
                "a bundled candidate must be anchored to the install folder: {}",
                path.display()
            );
            assert!(
                path.file_name().is_some(),
                "a candidate must name a file: {}",
                path.display()
            );
        }
    }

    #[test]
    fn the_windows_name_list_starts_with_the_real_dll_name() {
        // `bundled_names` is platform-gated, so this asserts the list for whichever
        // platform it is running on and says which one that was.
        let names = bundled_names();
        if cfg!(windows) {
            assert_eq!(names, &["mpv-2.dll", "libmpv-2.dll", "libmpv.dll"]);
        } else if cfg!(target_os = "macos") {
            assert_eq!(names, &["libmpv.2.dylib", "libmpv.dylib"]);
        } else {
            assert!(
                names.is_empty(),
                "Linux bundles nothing and resolves nothing"
            );
        }
    }

    #[test]
    fn a_macos_bundle_is_unchanged_by_the_windows_layout() {
        let names = ["libmpv.2.dylib"];
        let paths = bundled_libmpv_paths(
            Path::new("/Applications/Screenroom.app/Contents/MacOS"),
            &names,
            BundleLayout::MacOsAppBundle,
        );
        let rendered: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        assert_eq!(
            rendered,
            vec![
                "/Applications/Screenroom.app/Contents/Frameworks/libmpv.2.dylib",
                "/Applications/Screenroom.app/Contents/MacOS/libmpv.2.dylib",
            ]
        );
    }

    #[test]
    fn a_system_only_layout_bundles_nothing() {
        // The `SystemOnly` branch is the Linux one, and its whole content is
        // "produce no candidates". It is assertable on any host because the layout
        // is passed in rather than read from the environment.
        let names = ["libmpv.so.2", "libmpv.so.1"];
        assert!(
            bundled_libmpv_paths(Path::new("/usr/bin"), &names, BundleLayout::SystemOnly)
                .is_empty()
        );
    }

    #[test]
    fn the_working_directory_is_searched_last_on_windows_and_never_first() {
        // The order is the security property: on Windows every explicit path is
        // tried before any bare name, so the working directory cannot decide which
        // library loads. The other order is asserted too, because it is the one
        // that keeps a development machine on its package manager's newer libmpv.
        let system = vec![PathBuf::from("mpv-2.dll")];
        let bundled = vec![PathBuf::from(r"C:\App\mpv-2.dll")];

        let windows = library_search_order(&system, &bundled, true);
        assert_eq!(windows[0], bundled[0]);
        assert_eq!(windows[1], system[0]);

        let macos = library_search_order(&system, &bundled, false);
        assert_eq!(macos[0], system[0]);
        assert_eq!(macos[1], bundled[0]);

        // And the fact that selects between them matches this platform, so the
        // behaviour under test is the one that will actually run.
        assert_eq!(system_search_consults_working_directory(), cfg!(windows));
        assert_eq!(BundleLayout::host(), {
            if cfg!(target_os = "macos") {
                BundleLayout::MacOsAppBundle
            } else if cfg!(windows) {
                BundleLayout::WindowsPortable
            } else {
                BundleLayout::SystemOnly
            }
        });
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
