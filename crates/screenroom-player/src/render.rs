//! The libmpv software renderer, feeding [`Frame`]s to the presentation layer.
//!
//! libmpv's render API offers OpenGL, Direct3D 11 and a CPU (`"sw"`) target (see
//! `mpv/render.h`). This module implements the CPU target: it needs no GPU
//! context of its own and is the only backend that can be proven on a machine
//! without a platform-specific texture-import path.
//!
//! Cost, stated honestly: one full-frame scale-and-convert per displayed frame,
//! on the CPU. That is why the target size is bounded ([`MAX_DIMENSION`]) and why
//! the OpenGL / D3D11 zero-copy paths remain the performance goal recorded in
//! `docs/FEASIBILITY_GATES.md`.
//!
//! Threading: every call here must happen on the thread that created the render
//! context; [`crate::Player`] owns one such thread and never shares it.

use std::ffi::{c_int, c_void};
use std::ptr::{self, NonNull};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{Frame, LibMpv, PlayerError};

/// `MPV_RENDER_UPDATE_FRAME`: a new frame is ready to be rendered.
pub const UPDATE_FRAME: u64 = 1;

/// Upper bound on either rendered dimension.
///
/// The software bridge copies every pixel twice per frame (render, then upload),
/// so an unbounded 4K target would cost far more than the display can show on a
/// typical desktop window. 1920 keeps 1080p exact and downscales larger sources.
pub const MAX_DIMENSION: u32 = 1920;

// `mpv_render_param_type` values used here (`mpv/render.h`).
const PARAM_API_TYPE: c_int = 1;
const PARAM_SW_SIZE: c_int = 17;
const PARAM_SW_FORMAT: c_int = 18;
const PARAM_SW_STRIDE: c_int = 19;
const PARAM_SW_POINTER: c_int = 20;

#[repr(C)]
struct RenderParam {
    kind: c_int,
    data: *mut c_void,
}

/// Scales a video size down to [`MAX_DIMENSION`] while preserving its aspect.
///
/// Whole-pixel rounding is used, because a fractional surface would blur the
/// picture for no benefit. A source already inside the bound is returned as is.
pub fn target_size(video_width: u32, video_height: u32, max_dimension: u32) -> (u32, u32) {
    let longest = video_width.max(video_height);
    if video_width == 0 || video_height == 0 {
        return (1, 1);
    }
    if longest <= max_dimension {
        return (video_width, video_height);
    }
    let scale = max_dimension as f64 / longest as f64;
    let scale_axis = |value: u32| ((value as f64 * scale).round() as u32).max(1);
    (scale_axis(video_width), scale_axis(video_height))
}

/// Rounds a byte count up to a 64-byte boundary.
///
/// libmpv asks for 64-byte aligned strides and pointers so its SIMD paths can
/// write directly instead of falling back to an internal copy.
fn align64(value: usize) -> usize {
    value.div_ceil(64) * 64
}

/// A libmpv render context configured for the software (`"sw"`) target.
pub struct SoftwareRenderer {
    library: Arc<LibMpv>,
    context: NonNull<c_void>,
    /// Set from libmpv's update callback, cleared by the render loop.
    dirty: Box<AtomicBool>,
    width: u32,
    height: u32,
    stride: usize,
    buffer: Vec<u8>,
    offset: usize,
    generation: u64,
}

impl std::fmt::Debug for SoftwareRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SoftwareRenderer")
            .field("size", &(self.width, self.height))
            .field("stride", &self.stride)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

/// The callback libmpv invokes when a frame is available.
///
/// It must not call back into libmpv, so it only raises a flag; the render
/// thread does the work.
unsafe extern "C" fn mark_dirty(context: *mut c_void) {
    if context.is_null() {
        return;
    }
    // Safety: the pointer is the `Box<AtomicBool>` owned by the renderer, which
    // outlives the render context (see `Drop`).
    let dirty = unsafe { &*(context as *const AtomicBool) };
    dirty.store(true, Ordering::Release);
}

impl SoftwareRenderer {
    /// Creates the render context for `core`.
    ///
    /// The caller must already have set `vo=libmpv` on the core before
    /// initialisation, which is what tells libmpv to render on demand.
    pub fn new(core: &crate::Mpv) -> Result<Self, PlayerError> {
        let library = Arc::clone(core.library());
        let symbols = library.render_symbols().ok_or_else(|| {
            PlayerError::Unavailable(format!(
                "{} does not export the libmpv render API",
                library.path()
            ))
        })?;

        let mut dirty = Box::new(AtomicBool::new(false));
        let callback_data = (&mut *dirty as *mut AtomicBool).cast::<c_void>();

        let api_type = c"sw";
        let mut params = [
            RenderParam {
                kind: PARAM_API_TYPE,
                data: api_type.as_ptr() as *mut c_void,
            },
            RenderParam {
                kind: 0,
                data: ptr::null_mut(),
            },
        ];
        let mut context: *mut c_void = ptr::null_mut();
        let code = unsafe {
            (symbols.create)(&mut context, core.raw_handle(), params.as_mut_ptr().cast())
        };
        if code < 0 {
            return Err(PlayerError::Mpv {
                context: "mpv_render_context_create(sw)".to_string(),
                message: library.message(code),
                code,
            });
        }
        let context = NonNull::new(context).ok_or_else(|| {
            PlayerError::Unavailable("mpv_render_context_create returned null".into())
        })?;

        unsafe { (symbols.set_update_callback)(context.as_ptr(), Some(mark_dirty), callback_data) };

        Ok(Self {
            library,
            context,
            dirty,
            width: 0,
            height: 0,
            stride: 0,
            buffer: Vec::new(),
            offset: 0,
            generation: 0,
        })
    }

    /// True when libmpv signalled that new output is available.
    pub fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::AcqRel)
    }

    /// `mpv_render_context_update` flags.
    pub fn update_flags(&self) -> Result<u64, PlayerError> {
        let symbols = self.symbols()?;
        Ok(unsafe { (symbols.update)(self.context.as_ptr()) })
    }

    /// Sets the render target size, reallocating only when it changes.
    pub fn set_target(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if (self.width, self.height) == (width, height) {
            return;
        }
        self.width = width;
        self.height = height;
        self.stride = align64(width as usize * 4);
        let needed = self.stride * height as usize + 64;
        if self.buffer.len() != needed {
            self.buffer = vec![0u8; needed];
        }
        let base = self.buffer.as_ptr() as usize;
        self.offset = (64 - (base % 64)) % 64;
    }

    /// Renders the current frame into the target and returns it packed as RGBA8.
    ///
    /// Returns `Ok(None)` when libmpv had no new frame. The packed copy is needed
    /// because libmpv's `"rgb0"` target writes into a stride-padded buffer while
    /// the presentation layer wants tightly packed rows. The fourth byte of that
    /// target is *undefined*, so presenters must force opaque output rather than
    /// trust it — see [`crate::Frame`].
    pub fn render(&mut self) -> Result<Option<Frame>, PlayerError> {
        if self.width == 0 || self.height == 0 {
            return Ok(None);
        }
        let symbols = self.symbols()?;

        let mut size = [self.width as c_int, self.height as c_int];
        let format = c"rgb0";
        let stride = self.stride;
        // `MPV_RENDER_PARAM_SW_POINTER` is documented as `void*` — the address of
        // the first pixel, not a pointer to a pointer. Passing `&pointer` here
        // makes libmpv render over the caller's stack.
        let pointer = self.aligned_ptr();
        let mut params = [
            RenderParam {
                kind: PARAM_SW_SIZE,
                data: size.as_mut_ptr().cast(),
            },
            RenderParam {
                kind: PARAM_SW_FORMAT,
                data: format.as_ptr() as *mut c_void,
            },
            RenderParam {
                kind: PARAM_SW_STRIDE,
                data: (&stride as *const usize as *mut usize).cast(),
            },
            RenderParam {
                kind: PARAM_SW_POINTER,
                data: pointer.cast(),
            },
            RenderParam {
                kind: 0,
                data: ptr::null_mut(),
            },
        ];

        let began = std::time::Instant::now();
        let code = unsafe { (symbols.render)(self.context.as_ptr(), params.as_mut_ptr().cast()) };
        let drawn = began.elapsed();
        if code < 0 {
            return Err(PlayerError::Mpv {
                context: "mpv_render_context_render".to_string(),
                message: self.library.message(code),
                code,
            });
        }

        self.generation += 1;
        let began = std::time::Instant::now();
        let frame = self.pack(pointer);
        if std::env::var_os("SCREENROOM_PLAYER_TRACE").is_some() {
            eprintln!(
                "[render] {}x{} stride {} align {} — libmpv {:?}, pack {:?}",
                self.width,
                self.height,
                self.stride,
                (pointer as usize) % 64,
                drawn,
                began.elapsed()
            );
        }
        Ok(Some(frame))
    }

    /// Returns the raw render context.
    ///
    /// Unused by the software path; it is the entry point the OpenGL and D3D11
    /// zero-copy adapters need, and it is kept (with its lifetime rules) rather
    /// than re-derived when that work lands.
    #[allow(dead_code)]
    pub fn raw_context(&self) -> *mut c_void {
        self.context.as_ptr()
    }

    fn symbols(&self) -> Result<crate::RenderSymbols, PlayerError> {
        self.library.render_symbols().ok_or_else(|| {
            PlayerError::Unavailable(format!(
                "{} does not export the libmpv render API",
                self.library.path()
            ))
        })
    }

    fn aligned_ptr(&mut self) -> *mut u8 {
        // Safety: `offset` keeps the write inside the allocation
        // (`stride * height + 64` bytes was reserved for exactly this).
        unsafe { self.buffer.as_mut_ptr().add(self.offset) }
    }

    /// Tightens the stride-padded render target into a packed RGBA buffer.
    ///
    /// This is one `copy_from_slice` per row, so it runs at memcpy speed. The
    /// previous per-pixel loop rewrote all four bytes of every pixel and cost
    /// ~40 ms for a single 1920x1080 frame in an unoptimized build, which capped
    /// presentation at ~11 fps; nothing here inspects pixel values any more.
    fn pack(&self, _pointer: *mut u8) -> Frame {
        let width = self.width as usize;
        let height = self.height as usize;
        let row_bytes = width * 4;
        let start = self.offset;
        let source = &self.buffer[start..start + self.stride * height];

        let mut rgba = vec![0u8; row_bytes * height];
        for (row, out_row) in source
            .chunks_exact(self.stride)
            .zip(rgba.chunks_exact_mut(row_bytes))
        {
            out_row.copy_from_slice(&row[..row_bytes]);
        }

        Frame {
            width: self.width,
            height: self.height,
            rgba: Arc::new(rgba),
            generation: self.generation,
        }
    }
}

impl Drop for SoftwareRenderer {
    fn drop(&mut self) {
        if let Some(symbols) = self.library.render_symbols() {
            // Stop callbacks before the flag is dropped, then release the context.
            unsafe { (symbols.set_update_callback)(self.context.as_ptr(), None, ptr::null_mut()) };
            unsafe { (symbols.free)(self.context.as_ptr()) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_sources_keep_their_size() {
        assert_eq!(target_size(1920, 1080, MAX_DIMENSION), (1920, 1080));
        assert_eq!(target_size(640, 480, MAX_DIMENSION), (640, 480));
    }

    #[test]
    fn large_sources_are_downscaled_without_distortion() {
        assert_eq!(target_size(3840, 2160, MAX_DIMENSION), (1920, 1080));
        assert_eq!(target_size(2160, 3840, MAX_DIMENSION), (1080, 1920));
        let (width, height) = target_size(4096, 1716, MAX_DIMENSION);
        assert_eq!(width, 1920);
        assert!(height <= MAX_DIMENSION);
        let source_aspect = 4096.0 / 1716.0;
        let target_aspect = width as f64 / height as f64;
        assert!((source_aspect - target_aspect).abs() < 0.01);
    }

    #[test]
    fn degenerate_sizes_never_produce_zero() {
        assert_eq!(target_size(0, 0, MAX_DIMENSION), (1, 1));
        assert_eq!(target_size(0, 1080, MAX_DIMENSION), (1, 1));
        assert_eq!(target_size(100, 100, 0), (1, 1));
    }

    #[test]
    fn stride_is_aligned_to_64_bytes() {
        for width in [1u32, 17, 1280, 1919] {
            let stride = align64(width as usize * 4);
            assert_eq!(stride % 64, 0);
            assert!(stride >= width as usize * 4);
        }
    }
}
