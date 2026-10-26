//! The libmpv → wgpu presentation bridge.
//!
//! libmpv can only target OpenGL, Direct3D 11 or a CPU buffer, while iced draws
//! through wgpu. This module implements the **portable software bridge**: a
//! decoded RGBA frame is uploaded into a wgpu texture inside iced's shader
//! `Primitive::prepare` and drawn letterboxed by a small WGSL shader.
//!
//! Cost and consequences:
//!
//! * one full-frame upload per displayed frame (unavoidable without a
//!   platform-specific zero-copy import),
//! * the texture is recreated only when the frame size changes,
//! * an identical frame generation is not re-uploaded,
//! * letterbox bars are drawn by the shader, so the source aspect ratio is
//!   preserved without stretching (PRD Q03).
//!
//! The OpenGL (macOS) and D3D11 (Windows) zero-copy paths slot in behind
//! [`FrameSource`] without touching the widget or the shader.

use iced::widget::shader::{self, Primitive, Program};
use iced::{Rectangle, mouse};

/// Aspect-correct fit factors for one frame inside a target rectangle.
///
/// Returns the multipliers applied to UV coordinates centered on 0.5: `1.0`
/// keeps the full axis, a smaller value letterboxes that axis.
pub fn fit_factors(source_width: u32, source_height: u32, target: Rectangle) -> (f32, f32) {
    if source_width == 0 || source_height == 0 || target.width <= 0.0 || target.height <= 0.0 {
        return (1.0, 1.0);
    }
    let source_aspect = source_width as f32 / source_height as f32;
    let target_aspect = target.width / target.height;
    if source_aspect > target_aspect {
        (1.0, target_aspect / source_aspect)
    } else {
        (source_aspect / target_aspect, 1.0)
    }
}

/// Frames come from the player crate so both sides share one definition.
pub use screenroom_player::Frame;

/// A widget that draws the current video frame, or nothing when there is none.
#[derive(Debug, Clone, Default)]
pub struct VideoSurface {
    frame: Option<Frame>,
}

impl VideoSurface {
    /// Builds a surface showing `frame`.
    pub fn new(frame: Option<Frame>) -> Self {
        Self { frame }
    }
}

#[derive(Debug)]
pub struct VideoPrimitive {
    frame: Option<Frame>,
    bounds: Rectangle,
}

impl Program<crate::app::Message> for VideoSurface {
    type State = ();
    type Primitive = VideoPrimitive;

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: mouse::Cursor,
        bounds: Rectangle,
    ) -> Self::Primitive {
        VideoPrimitive {
            frame: self.frame.clone(),
            bounds,
        }
    }
}

impl Primitive for VideoPrimitive {
    type Pipeline = VideoPipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _bounds: &Rectangle,
        _viewport: &shader::Viewport,
    ) {
        let Some(frame) = self.frame.as_ref().filter(|frame| frame.is_well_formed()) else {
            return;
        };
        if pipeline.uploaded_generation == Some(frame.generation)
            && pipeline.texture_size == (frame.width, frame.height)
        {
            return;
        }

        if pipeline.texture_size != (frame.width, frame.height) {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("screenroom-video-frame"),
                size: wgpu::Extent3d {
                    width: frame.width,
                    height: frame.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            pipeline.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("screenroom-video-frame"),
                layout: &pipeline.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&pipeline.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: pipeline.fit_buffer.as_entire_binding(),
                    },
                ],
            }));
            pipeline.texture = Some(texture);
            pipeline.texture_size = (frame.width, frame.height);
        }

        if let Some(texture) = &pipeline.texture {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &frame.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(frame.width * 4),
                    rows_per_image: Some(frame.height),
                },
                wgpu::Extent3d {
                    width: frame.width,
                    height: frame.height,
                    depth_or_array_layers: 1,
                },
            );
        }

        let (fit_x, fit_y) = fit_factors(frame.width, frame.height, self.bounds);
        queue.write_buffer(
            &pipeline.fit_buffer,
            0,
            bytemuck_f32(&[fit_x, fit_y, 0.0, 0.0]),
        );
        pipeline.uploaded_generation = Some(frame.generation);
    }

    fn draw(&self, pipeline: &Self::Pipeline, render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        let Some(bind_group) = &pipeline.bind_group else {
            return false;
        };
        render_pass.set_pipeline(&pipeline.render_pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.draw(0..3, 0..1);
        true
    }
}

/// GPU state shared by every video frame primitive.
pub struct VideoPipeline {
    render_pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    fit_buffer: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
    texture: Option<wgpu::Texture>,
    texture_size: (u32, u32),
    uploaded_generation: Option<u64>,
}

impl std::fmt::Debug for VideoPipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoPipeline")
            .field("texture_size", &self.texture_size)
            .field("uploaded_generation", &self.uploaded_generation)
            .finish_non_exhaustive()
    }
}

impl shader::Pipeline for VideoPipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("screenroom-video-surface"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("screenroom-video-surface"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("screenroom-video-surface"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("screenroom-video-surface"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("screenroom-video-surface"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let fit_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screenroom-video-fit"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            render_pipeline,
            layout,
            sampler,
            fit_buffer,
            bind_group: None,
            texture: None,
            texture_size: (0, 0),
            uploaded_generation: None,
        }
    }

    fn trim(&mut self) {
        // Keeping the last texture avoids churn while paused or stepping frames.
    }
}

/// Writes four `f32` values as bytes without pulling in `bytemuck`.
fn bytemuck_f32(values: &[f32; 4]) -> &[u8] {
    // Safety: `f32` has no invalid bit patterns and the slice is plain data.
    unsafe {
        std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values))
    }
}

const SHADER: &str = r#"
struct Fit {
    fit: vec2<f32>,
    padding: vec2<f32>,
};

@group(0) @binding(0) var frame_texture: texture_2d<f32>;
@group(0) @binding(1) var frame_sampler: sampler;
@group(0) @binding(2) var<uniform> fit: Fit;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    // One oversized triangle covering the widget bounds.
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(3.0, 1.0),
    );
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 2.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(2.0, 0.0),
    );
    var out: VertexOutput;
    out.position = vec4<f32>(positions[index], 0.0, 1.0);
    out.uv = uvs[index];
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Preserve the source aspect ratio: bars sample outside and render black.
    var uv = (input.uv - vec2<f32>(0.5, 0.5)) * fit.fit + vec2<f32>(0.5, 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    return textureSample(frame_texture, frame_sampler, uv);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(width: f32, height: f32) -> Rectangle {
        Rectangle {
            x: 0.0,
            y: 0.0,
            width,
            height,
        }
    }

    #[test]
    fn letterboxes_wider_source_into_square() {
        let (x, y) = fit_factors(1920, 800, bounds(500.0, 500.0));
        assert_eq!(x, 1.0);
        assert!((y - 500.0 / 1200.0).abs() < 1e-6);
    }

    #[test]
    fn pillarboxes_taller_source_into_wide() {
        let (x, y) = fit_factors(800, 1200, bounds(1000.0, 500.0));
        assert!(x < 1.0);
        assert_eq!(y, 1.0);
    }

    #[test]
    fn matching_aspect_has_no_bars() {
        let (x, y) = fit_factors(1920, 1080, bounds(1280.0, 720.0));
        assert!((x - 1.0).abs() < 1e-6);
        assert!((y - 1.0).abs() < 1e-6);
    }

    #[test]
    fn degenerate_inputs_do_not_panic_or_stretch() {
        assert_eq!(fit_factors(0, 0, bounds(100.0, 100.0)), (1.0, 1.0));
        assert_eq!(fit_factors(100, 100, bounds(0.0, 100.0)), (1.0, 1.0));
    }

    #[test]
    fn a_frame_from_the_player_is_accepted_by_the_surface() {
        let frame = Frame {
            width: 2,
            height: 2,
            rgba: std::sync::Arc::new(vec![7; 16]),
            generation: 3,
        };
        assert!(frame.is_well_formed());
        let surface = VideoSurface::new(Some(frame));
        assert!(surface.frame.is_some());
    }
}
