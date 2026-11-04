//! Drawing frames with Metal, into a layer of their own: 16-bit float, so
//! nothing is lost between a 12-bit source and the screen, with the
//! colour space the mod names. The system matches that to each screen.

use {
    super::geometry,
    metal::{
        CommandBuffer, CommandBufferRef, CommandQueue, Device, MTLClearColor, MTLLoadAction,
        MTLOrigin, MTLPixelFormat, MTLPrimitiveType, MTLRegion, MTLResourceOptions, MTLSize,
        MTLStorageMode, MTLStoreAction, MTLTextureUsage, MetalLayer, RenderPassDescriptor,
        RenderPipelineDescriptor, RenderPipelineState, Texture, TextureDescriptor, TextureRef,
    },
    moonframe_core::VideoView,
    moonframe_frames::{Format, FrameInfo, Pattern},
    std::{cell::Cell, ffi::c_void},
};

const SHADER: &str = include_str!("shader.metal");
/// Textures taken in turn: one being filled while others are still read.
const TEXTURES: usize = 3;
/// The layer's format.
pub(super) const LAYER_FORMAT: MTLPixelFormat = MTLPixelFormat::RGBA16Float;

/// The shader's `Uniforms`, field for field.
#[repr(C)]
struct Uniforms {
    scale: [f32; 2],
    offset: [f32; 2],
    picture: [f32; 2],
    black: f32,
    span: f32,
    gain: f32,
    inverse_gamma: f32,
    kind: i32,
    red_x: i32,
    red_y: i32,
    nearest: i32,
}

/// The shader's `GuideVertex`: `float2` then `float4`, aligned to 16.
#[repr(C, align(16))]
struct GuideVertex {
    position: [f32; 2],
    pad: [f32; 2],
    color: [f32; 4],
}

pub(super) struct Renderer {
    layer: MetalLayer,
    queue: CommandQueue,
    device: Device,
    frame: RenderPipelineState,
    guides: RenderPipelineState,
    textures: Vec<FrameTexture>,
    /// What the textures were made for.
    made: Option<(u32, u32, Format)>,
    /// The one filled last, and the one to fill next.
    current: usize,
    next: usize,
    /// The drawable's size, as last drawn.
    size: Cell<[f32; 2]>,
}

/// CPU uploads must wait for the last GPU read of this texture. Keeping
/// three textures lets other frames proceed while earlier ones finish.
struct FrameTexture {
    texture: Texture,
    reading: Option<CommandBuffer>,
}

fn texture_format(format: Format) -> MTLPixelFormat {
    match format {
        Format::Rgba8 => MTLPixelFormat::RGBA8Unorm,
        Format::Rgba16 => MTLPixelFormat::RGBA16Unorm,
        Format::Rgba16Float => MTLPixelFormat::RGBA16Float,
        Format::Mono16 | Format::Bayer16 => MTLPixelFormat::R16Unorm,
    }
}

impl Renderer {
    pub(super) fn new(device: Device, layer: MetalLayer) -> Result<Self, String> {
        let library = device
            .new_library_with_source(SHADER, &metal::CompileOptions::new())
            .map_err(|error| format!("the video shader: {error}"))?;
        let pipeline = |vertex: &str, fragment: &str, blend: bool| -> Result<_, String> {
            let descriptor = RenderPipelineDescriptor::new();
            let function = |name: &str| library.get_function(name, None);
            let (vertex, fragment) = (function(vertex)?, function(fragment)?);
            descriptor.set_vertex_function(Some(&vertex));
            descriptor.set_fragment_function(Some(&fragment));
            let attachment = descriptor
                .color_attachments()
                .object_at(0)
                .ok_or_else(|| "no colour attachment".to_owned())?;
            attachment.set_pixel_format(LAYER_FORMAT);
            if blend {
                attachment.set_blending_enabled(true);
                attachment.set_source_rgb_blend_factor(metal::MTLBlendFactor::SourceAlpha);
                attachment
                    .set_destination_rgb_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);
            }
            device.new_render_pipeline_state(&descriptor)
        };
        let frame = pipeline("video_vertex", "video_fragment", false)?;
        let guides = pipeline("guide_vertex", "guide_fragment", true)?;
        Ok(Self {
            layer,
            queue: device.new_command_queue(),
            device,
            frame,
            guides,
            textures: Vec::new(),
            made: None,
            current: 0,
            next: 0,
            size: Cell::new([1., 1.]),
        })
    }

    /// Copies a frame into the next texture; [`Renderer::commit`] makes it
    /// the one shown, once the read proved good.
    pub(super) fn upload(&mut self, info: &FrameInfo, pixels: *const u8) {
        let wanted = (info.width, info.height, info.format);
        if self.made != Some(wanted) {
            let descriptor = TextureDescriptor::new();
            descriptor.set_pixel_format(texture_format(info.format));
            descriptor.set_width(info.width.into());
            descriptor.set_height(info.height.into());
            descriptor.set_storage_mode(MTLStorageMode::Shared);
            descriptor.set_usage(MTLTextureUsage::ShaderRead);
            self.textures = (0..TEXTURES)
                .map(|_| FrameTexture {
                    texture: self.device.new_texture(&descriptor),
                    reading: None,
                })
                .collect();
            self.made = Some(wanted);
            self.next = 0;
        }
        let region = MTLRegion {
            origin: MTLOrigin { x: 0, y: 0, z: 0 },
            size: MTLSize {
                width: info.width.into(),
                height: info.height.into(),
                depth: 1,
            },
        };
        let texture = &mut self.textures[self.next];
        if let Some(reading) = texture.reading.take() {
            // This is the video's worker, never the UI thread. The ring
            // reader checks afterwards whether the frame was lapped.
            reading.wait_until_completed();
        }
        texture
            .texture
            .replace_region(region, 0, pixels.cast::<c_void>(), info.stride.into());
    }

    pub(super) fn commit(&mut self) {
        self.current = self.next;
        self.next = (self.next + 1) % TEXTURES;
    }

    /// The drawable's size in pixels, as last drawn.
    pub(super) fn size(&self) -> [f32; 2] {
        self.size.get()
    }

    /// Draws the current texture as `view` says.
    pub(super) fn draw(&mut self, info: &FrameInfo, view: &VideoView) {
        objc2::rc::autoreleasepool(|_| {
            let layer = self.layer.clone();
            let Some(drawable) = layer.next_drawable() else {
                return;
            };
            let commands = self.queue.new_command_buffer().to_owned();
            self.encode(&commands, drawable.texture(), info, view);
            commands.present_drawable(drawable);
            commands.commit();
        });
    }

    /// Records drawing the current texture into `target`.
    fn encode(
        &mut self,
        commands: &CommandBufferRef,
        target: &TextureRef,
        info: &FrameInfo,
        view: &VideoView,
    ) {
        let Some(texture) = self.textures.get(self.current) else {
            return;
        };
        let size = [target.width() as f32, target.height() as f32];
        self.size.set(size);
        let picture = [info.width as f32, info.height as f32];
        let placement = geometry::place(size, picture, view);
        let uniforms = uniforms(info, view, picture, placement);
        let pass = RenderPassDescriptor::new();
        if let Some(attachment) = pass.color_attachments().object_at(0) {
            attachment.set_texture(Some(target));
            attachment.set_load_action(MTLLoadAction::Clear);
            attachment.set_clear_color(MTLClearColor::new(0., 0., 0., 1.));
            attachment.set_store_action(MTLStoreAction::Store);
        }
        let encoder = commands.new_render_command_encoder(pass);
        encoder.set_render_pipeline_state(&self.frame);
        encoder.set_fragment_bytes(
            0,
            size_of::<Uniforms>() as u64,
            (&raw const uniforms).cast(),
        );
        encoder.set_fragment_texture(0, Some(&texture.texture));
        encoder.draw_primitives(MTLPrimitiveType::TriangleStrip, 0, 4);
        let vertices: Vec<GuideVertex> = view
            .guides
            .iter()
            .flat_map(|guide| geometry::guide_triangles(guide, size, picture, placement))
            .map(|[x, y, red, green, blue, alpha]| GuideVertex {
                position: [x, y],
                pad: [0.; 2],
                color: [red, green, blue, alpha],
            })
            .collect();
        if !vertices.is_empty() {
            let buffer = self.device.new_buffer_with_data(
                vertices.as_ptr().cast(),
                size_of_val(vertices.as_slice()) as u64,
                MTLResourceOptions::StorageModeShared,
            );
            encoder.set_render_pipeline_state(&self.guides);
            encoder.set_vertex_buffer(0, Some(&buffer), 0);
            encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, vertices.len() as u64);
        }
        encoder.end_encoding();
        // All draws use one queue, so completion of the last read also
        // covers earlier draws of the same frame (zoom, pan or resize).
        self.textures[self.current].reading = Some(commands.to_owned());
    }
}

/// What the shader needs for `info` under `view`. Integer textures read
/// as 0 to 1, so code values scale down by the format's full range.
fn uniforms(
    info: &FrameInfo,
    view: &VideoView,
    picture: [f32; 2],
    placement: geometry::Placement,
) -> Uniforms {
    let tone = geometry::tone(view, info.format);
    let range = geometry::full_range(info.format);
    let (red_x, red_y) = match info.pattern {
        Pattern::Rggb => (0, 0),
        Pattern::Bggr => (1, 1),
        Pattern::Grbg => (1, 0),
        Pattern::Gbrg => (0, 1),
    };
    Uniforms {
        scale: [placement.scale; 2],
        offset: placement.offset,
        picture,
        black: tone.black / range,
        span: ((tone.white - tone.black) / range).max(f32::EPSILON),
        gain: tone.gain,
        inverse_gamma: 1. / tone.gamma,
        kind: match info.format {
            Format::Rgba8 | Format::Rgba16 | Format::Rgba16Float => 0,
            Format::Mono16 => 1,
            Format::Bayer16 => 2,
        },
        red_x,
        red_y,
        nearest: i32::from(geometry::nearest(view.filter, placement.scale)),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    //! These run the real shader and read back what it drew, so that the
    //! sums, the demosaicing and the guides are seen to work on a GPU.

    use {
        super::*,
        moonframe_core::{VideoColor, VideoFilter, VideoGuide},
    };

    fn view() -> VideoView {
        VideoView {
            zoom: None,
            center: [0.5, 0.5],
            filter: VideoFilter::Nearest,
            color: VideoColor::Srgb,
            extended: false,
            black: 0.,
            white: None,
            gain: 1.,
            gamma: 1.,
            guides: Vec::new(),
            probe: None,
        }
    }

    /// Draws `pixels` into a float target of `size` and returns its RGBA;
    /// `None` where there is no GPU to test on.
    fn drawn(
        format: Format,
        pattern: Pattern,
        (width, height): (u32, u32),
        pixels: &[u8],
        size: u64,
        view: &VideoView,
    ) -> Option<Vec<[f32; 4]>> {
        let device = Device::system_default()?;
        let mut renderer = Renderer::new(device.clone(), MetalLayer::new()).unwrap();
        let info = FrameInfo {
            number: 1,
            width,
            height,
            format,
            stride: width * format.bytes_per_pixel() as u32,
            time: 0,
            pattern,
        };
        renderer.upload(&info, pixels.as_ptr());
        renderer.commit();
        let descriptor = TextureDescriptor::new();
        descriptor.set_pixel_format(LAYER_FORMAT);
        descriptor.set_width(size);
        descriptor.set_height(size);
        descriptor.set_storage_mode(MTLStorageMode::Managed);
        descriptor.set_usage(MTLTextureUsage::RenderTarget);
        let target = device.new_texture(&descriptor);
        let commands = renderer.queue.new_command_buffer().to_owned();
        renderer.encode(&commands, &target, &info, view);
        let blit = commands.new_blit_command_encoder();
        blit.synchronize_resource(&target);
        blit.end_encoding();
        commands.commit();
        commands.wait_until_completed();
        let mut half = vec![0u16; (size * size * 4) as usize];
        target.get_bytes(
            half.as_mut_ptr().cast(),
            size * 8,
            MTLRegion {
                origin: MTLOrigin { x: 0, y: 0, z: 0 },
                size: MTLSize {
                    width: size,
                    height: size,
                    depth: 1,
                },
            },
            0,
        );
        Some(
            half.chunks(4)
                .map(|pixel| {
                    [
                        geometry::half(pixel[0]),
                        geometry::half(pixel[1]),
                        geometry::half(pixel[2]),
                        geometry::half(pixel[3]),
                    ]
                })
                .collect(),
        )
    }

    fn near(got: f32, wanted: f32) {
        assert!((got - wanted).abs() < 0.002, "{got} is not {wanted}");
    }

    fn words(values: &[u16]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }

    #[test]
    fn reusing_a_texture_waits_for_its_gpu_read() {
        let Some(device) = Device::system_default() else {
            return;
        };
        let mut renderer = Renderer::new(device.clone(), MetalLayer::new()).unwrap();
        let info = FrameInfo {
            number: 1,
            width: 1,
            height: 1,
            format: Format::Rgba8,
            stride: 4,
            time: 0,
            pattern: Pattern::Rggb,
        };
        renderer.upload(&info, [255u8; 4].as_ptr());
        renderer.commit();
        let descriptor = TextureDescriptor::new();
        descriptor.set_pixel_format(LAYER_FORMAT);
        descriptor.set_width(1);
        descriptor.set_height(1);
        descriptor.set_storage_mode(MTLStorageMode::Managed);
        descriptor.set_usage(MTLTextureUsage::RenderTarget);
        let target = device.new_texture(&descriptor);
        let gate = device.new_shared_event();
        let commands = renderer.queue.new_command_buffer().to_owned();
        commands.encode_wait_for_event(&gate, 1);
        renderer.encode(&commands, &target, &info, &view());
        let blit = commands.new_blit_command_encoder();
        blit.synchronize_resource(&target);
        blit.end_encoding();
        commands.commit();

        let (uploaded, finished) = std::sync::mpsc::channel();
        // Always release the GPU, even if an assertion fails later.
        let signal = std::thread::spawn(move || {
            let early = finished
                .recv_timeout(std::time::Duration::from_millis(100))
                .is_ok();
            gate.set_signaled_value(1);
            early
        });
        for _ in 0..TEXTURES {
            renderer.upload(&info, [0u8; 4].as_ptr());
            renderer.commit();
        }
        let _ = uploaded.send(());
        assert!(
            !signal.join().unwrap(),
            "overwrote a texture still being read"
        );
        commands.wait_until_completed();
        let mut pixel = [0u16; 4];
        target.get_bytes(
            pixel.as_mut_ptr().cast(),
            8,
            MTLRegion {
                origin: MTLOrigin { x: 0, y: 0, z: 0 },
                size: MTLSize {
                    width: 1,
                    height: 1,
                    depth: 1,
                },
            },
            0,
        );
        near(geometry::half(pixel[0]), 1.);
    }

    #[test]
    fn sixteen_bits_reach_the_target_finer_than_eight_could() {
        // Two neighbours an 8-bit path would make one value.
        let (low, high) = (32768u16, 32896u16);
        let pixels = words(&[low, low, low, 65535, high, high, high, 65535]);
        let Some(drawn) = drawn(Format::Rgba16, Pattern::Rggb, (2, 1), &pixels, 2, &view()) else {
            return;
        };
        // The picture is 2 by 1 in a 2 by 2 target, so its row is in the
        // middle; the second row of the target holds it.
        let (left, right) = (drawn[0][0].max(drawn[2][0]), drawn[1][0].max(drawn[3][0]));
        assert!(right > left, "{left} and {right} came out the same");
    }

    #[test]
    fn white_scales_twelve_bits_to_full_range() {
        let pixels = words(&[4095, 4095, 4095, 4095]);
        let view = VideoView {
            white: Some(4095.),
            ..view()
        };
        let Some(drawn) = drawn(Format::Mono16, Pattern::Rggb, (2, 2), &pixels, 2, &view) else {
            return;
        };
        near(drawn[0][0], 1.);
        near(drawn[0][1], 1.);
    }

    #[test]
    fn the_shader_and_a_probe_agree_on_the_tone() {
        let view = VideoView {
            black: 100.,
            white: Some(2100.),
            gain: 1.5,
            gamma: 2.2,
            ..view()
        };
        let value = 1200u16;
        let pixels = words(&[value; 4]);
        let Some(drawn) = drawn(Format::Mono16, Pattern::Rggb, (2, 2), &pixels, 2, &view) else {
            return;
        };
        let wanted = geometry::tone(&view, Format::Mono16).apply(f32::from(value));
        near(drawn[0][0], wanted);
    }

    #[test]
    fn a_bayer_frame_of_one_colour_comes_out_that_colour() {
        // RGGB: reds at 4000, greens at 2000, blues at 1000, in 4 by 4.
        let mut sites = Vec::new();
        for y in 0..4 {
            for x in 0..4 {
                sites.push(match (x % 2, y % 2) {
                    (0, 0) => 4000,
                    (1, 1) => 1000,
                    _ => 2000,
                });
            }
        }
        let view = VideoView {
            white: Some(4000.),
            ..view()
        };
        let Some(drawn) = drawn(
            Format::Bayer16,
            Pattern::Rggb,
            (4, 4),
            &words(&sites),
            4,
            &view,
        ) else {
            return;
        };
        // Away from the edges, every site gives the same colour.
        for index in [5, 6, 9, 10] {
            near(drawn[index][0], 1.);
            near(drawn[index][1], 0.5);
            near(drawn[index][2], 0.25);
        }
    }

    #[test]
    fn a_guide_is_drawn_over_the_picture() {
        let pixels = words(&[0; 16 * 16]);
        let view = VideoView {
            guides: vec![VideoGuide {
                x: 0.,
                y: 0.,
                width: 1.,
                height: 1.,
                color: [1., 0., 0., 1.],
            }],
            ..view()
        };
        let Some(drawn) = drawn(Format::Mono16, Pattern::Rggb, (16, 16), &pixels, 16, &view) else {
            return;
        };
        near(drawn[0][0], 1.);
        near(drawn[8 * 16 + 8][0], 0.);
    }
}
