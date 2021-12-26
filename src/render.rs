use super::font::Font;
use super::glyph_cache::{GlyphCache, GlyphKey, SubpixelOffset};
use super::glyph_rasterizer::{Format, GlyphRasterizer};
use super::text::Text;
use metal::*;
use std::ops::Range;
use swash::scale::{outline::Outline, ScaleContext};
use swash::zeno::{Origin, Placement};

const TARGET_FORMAT: MTLPixelFormat = MTLPixelFormat::BGRA8Unorm;

pub struct Renderer<G> {
    pub layer: MetalLayer,
    device: Device,
    queue: CommandQueue,
    width: u32,
    height: u32,
    glyph_cache: GlyphCache,
    glyph_rasterizer: G,
    scale_ctx: ScaleContext,
    glyphs: Vec<RenderGlyph>,
    runs: Vec<RenderRun>,
    quads: QuadBatch,
    alpha_pso: RenderPipelineState,
    color_pso: RenderPipelineState,
}

impl<G: GlyphRasterizer> Renderer<G> {
    pub fn new() -> Self {
        let device = Device::system_default().expect("no metal device available");
        let layer = MetalLayer::new();
        layer.set_device(&device);
        layer.set_pixel_format(TARGET_FORMAT);
        layer.set_presents_with_transaction(false);
        let queue = device.new_command_queue();
        let glyph_rasterizer = G::new(&device, &queue);
        let glyph_cache = GlyphCache::new(device.clone());
        let quads = QuadBatch::new(&device);
        let options = CompileOptions::new();
        options.set_language_version(MTLLanguageVersion::V2_2);
        let library = device
            .new_library_with_source(SHADER_SOURCE, &options)
            .unwrap();
        let alpha_pso = build_pso(&device, &library, "alpha_frag");
        let color_pso = build_pso(&device, &library, "color_frag");
        Self {
            device,
            layer,
            queue,
            width: 0,
            height: 0,
            glyph_cache,
            glyph_rasterizer,
            scale_ctx: ScaleContext::new(),
            glyphs: vec![],
            runs: vec![],
            quads,
            alpha_pso,
            color_pso,
        }
    }

    pub fn set_target_size(&mut self, width: u32, height: u32) {
        self.layer
            .set_drawable_size(CGSize::new(width as f64, height as f64));
        self.width = width;
        self.height = height;
    }

    pub fn new_frame(&mut self, bg_color: [f32; 4]) -> FrameRenderer<G> {
        self.glyphs.clear();
        self.runs.clear();
        FrameRenderer {
            r: self,
            bg_color,
            flush_cache: false,
        }
    }
}

pub struct FrameRenderer<'a, G> {
    r: &'a mut Renderer<G>,
    bg_color: [f32; 4],
    flush_cache: bool,
}

impl<'a, G: GlyphRasterizer> FrameRenderer<'a, G> {
    pub fn draw_text(&mut self, x: f32, y: f32, text: &Text) {
        for line in &text.lines {
            let baseline = y + line.y + line.ascent;
            let mut pen_x = x;
            for run in &line.runs {
                let is_color = self
                    .r
                    .scale_ctx
                    .builder(run.font.as_ref())
                    .build()
                    .has_color_outlines();
                let start = self.r.glyphs.len();
                for (id, advance) in run.ids.iter().zip(&run.advances) {
                    let subpx = SubpixelOffset::quantize(pen_x);
                    self.r.glyphs.push(RenderGlyph {
                        id: *id,
                        x: (pen_x + 0.125).floor(),
                        y: baseline,
                        subpx,
                    });
                    if !self.flush_cache
                        && self
                            .r
                            .glyph_cache
                            .map
                            .get(&GlyphKey {
                                font_id: run.font.key,
                                font_size: run.font_size.to_bits(),
                                subpx,
                                id: *id,
                            })
                            .is_none()
                    {
                        self.flush_cache = true;
                    }
                    pen_x += *advance;
                }
                let end = self.r.glyphs.len();
                let [r, g, b, a] = run.color;
                let color = [
                    (r * 255.) as u8,
                    (g * 255.) as u8,
                    (b * 255.) as u8,
                    (a * 255.) as u8,
                ];
                self.r.runs.push(RenderRun {
                    font: run.font.clone(),
                    font_size: run.font_size,
                    is_color,
                    color,
                    glyphs: start..end,
                });
            }
        }
    }

    pub fn render(mut self) {
        if self.flush_cache {
            self.build_cache();
        }
        self.r.quads.prepare(self.r.glyphs.len());
        for run in &self.r.runs {
            let glyphs = self.r.glyphs.get(run.glyphs.clone()).unwrap();
            for glyph in glyphs {
                if let Some(entry) = self.r.glyph_cache.map.get(&GlyphKey {
                    font_id: run.font.key,
                    font_size: run.font_size.to_bits(),
                    subpx: glyph.subpx,
                    id: glyph.id,
                }) {
                    let x0 = glyph.x + entry.left as f32;
                    let y0 = glyph.y - entry.top as f32;
                    let x1 = x0 + entry.width as f32;
                    let y1 = y0 + entry.height as f32;
                    if entry.width != 0 && entry.height != 0 {
                        self.r
                            .quads
                            .add_rect(&[x0, y0, x1, y1], &entry.uv, run.color);
                    }
                }
            }
        }
        self.r.quads.update_buffers();
        let drawable = match self.r.layer.next_drawable() {
            Some(drawable) => drawable,
            None => return,
        };
        let pass = RenderPassDescriptor::new();
        let color_attachment = pass.color_attachments().object_at(0).unwrap();
        color_attachment.set_texture(Some(&drawable.texture()));
        color_attachment.set_load_action(MTLLoadAction::Clear);
        let [r, g, b, a] = self.bg_color;
        color_attachment.set_clear_color(MTLClearColor::new(r as _, g as _, b as _, a as _));
        color_attachment.set_store_action(MTLStoreAction::Store);
        let cmdbuf = self.r.queue.new_command_buffer();
        let encoder = cmdbuf.new_render_command_encoder(&pass);
        if let Some(alpha_atlas) = self.r.glyph_cache.alpha.as_ref() {
            let vp_size = [self.r.width, self.r.height];
            encoder.set_render_pipeline_state(&self.r.alpha_pso);
            encoder.set_vertex_buffer(0, Some(&self.r.quads.vertex_buffer), 0);
            encoder.set_vertex_bytes(1, 8, vp_size.as_ptr() as _);
            encoder.set_fragment_texture(0, Some(&alpha_atlas.texture));
            encoder.draw_indexed_primitives(
                MTLPrimitiveType::Triangle,
                self.r.quads.indices.len() as _,
                MTLIndexType::UInt32,
                &self.r.quads.index_buffer,
                0,
            );
        }
        encoder.end_encoding();
        cmdbuf.present_drawable(&drawable);
        cmdbuf.commit();
    }

    fn build_cache(&mut self) {
        println!("rebuilding cache!");
        use super::glyph::Glyph;
        use super::glyph_cache::ATLAS_SIZE;
        self.r.glyph_cache.clear();
        let mut outline = Outline::new();
        let alpha = self.r.runs.iter().filter(|r| !r.is_color);
        let _color = self.r.runs.iter().filter(|r| r.is_color);
        self.r
            .glyph_rasterizer
            .begin(Format::A8, ATLAS_SIZE, ATLAS_SIZE);
        for run in alpha {
            let mut scaler = self
                .r
                .scale_ctx
                .builder(run.font.as_ref())
                .size(run.font_size)
                .build();
            let glyphs = self.r.glyphs.get(run.glyphs.clone()).unwrap();
            for glyph in glyphs {
                let key = GlyphKey {
                    font_id: run.font.key,
                    font_size: run.font_size.to_bits(),
                    subpx: glyph.subpx,
                    id: glyph.id,
                };
                if !scaler.scale_outline_into(glyph.id, &mut outline) {
                    continue;
                }
                let bounds = outline.bounds();
                let (offset, placement) = Placement::compute(Origin::BottomLeft, (0, 0), &bounds);
                if let Some(entry) = self.r.glyph_cache.insert(
                    key,
                    false,
                    placement.width as u16,
                    placement.height as u16,
                ) {
                    entry.left = placement.left as i16;
                    entry.top = placement.top as i16;
                    let transform = [1.0, 0.0, 0.0, 1.0, offset.x, offset.y];
                    let font = run.font.as_ref();
                    let rect = [
                        (entry.uv[0] * ATLAS_SIZE as f32) as u16,
                        (entry.uv[1] * ATLAS_SIZE as f32) as u16,
                        placement.width as u16,
                        placement.height as u16,
                    ];
                    unsafe {
                        self.r.glyph_rasterizer.add_glyph(&Glyph {
                            unique_id: font.key.value(),
                            font_data: font.data.as_ptr(),
                            font_data_len: font.data.len() as _,
                            font_size: run.font_size,
                            glyph_id: glyph.id,
                            transform,
                            rect,
                            subpx: glyph.subpx.to_f32(),
                            variations: std::ptr::null(),
                            num_variations: 0,
                        });
                    }
                } else {
                    continue;
                }
            }
        }
        if let Some(texture) = self.r.glyph_cache.alpha.as_ref().map(|a| &a.texture) {
            let cmdbuf = self.r.queue.new_command_buffer();
            let id = self.r.glyph_rasterizer.record(cmdbuf, texture);
            cmdbuf.commit();
            // Consider a fence here; not strictly necessary in Metal's default
            // configuration
            self.r.glyph_rasterizer.release(id);
        }
    }
}

struct RenderRun {
    font: Font,
    font_size: f32,
    is_color: bool,
    color: [u8; 4],
    glyphs: Range<usize>,
}

struct RenderGlyph {
    id: u16,
    x: f32,
    y: f32,
    subpx: SubpixelOffset,
}

#[derive(Copy, Clone)]
#[repr(C)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [u8; 4],
}

const VERTEX_SIZE: usize = std::mem::size_of::<Vertex>();
fn buffer_options() -> metal::MTLResourceOptions {
    metal::MTLResourceOptions::CPUCacheModeDefaultCache
        | metal::MTLResourceOptions::StorageModeManaged
}

struct QuadBatch {
    device: Device,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    buffer_cap: usize,
    ranges: Vec<(u32, u32, bool)>,
}

impl QuadBatch {
    fn new(device: &Device) -> Self {
        let buffer_cap = 256;
        Self {
            device: device.clone(),
            vertices: vec![],
            indices: vec![],
            vertex_buffer: device.new_buffer((buffer_cap * 4 * VERTEX_SIZE) as _, buffer_options()),
            index_buffer: device.new_buffer((buffer_cap * 6 * 4) as _, buffer_options()),
            buffer_cap,
            ranges: vec![],
        }
    }

    fn prepare(&mut self, num_glyphs: usize) {
        if self.buffer_cap < num_glyphs {
            self.vertex_buffer = self
                .device
                .new_buffer((num_glyphs * 4 * VERTEX_SIZE) as _, buffer_options());
            self.index_buffer = self
                .device
                .new_buffer((num_glyphs * 6 * 4) as _, buffer_options());
            self.buffer_cap = num_glyphs;
        }
        self.vertices.clear();
        self.indices.clear();
        self.ranges.clear();
    }

    fn add_rect(&mut self, rect: &[f32; 4], uv: &[f32; 4], color: [u8; 4]) {
        let verts = [
            Vertex {
                pos: [rect[0], rect[1]],
                uv: [uv[0], uv[1]],
                color,
            },
            Vertex {
                pos: [rect[0], rect[3]],
                uv: [uv[0], uv[3]],
                color,
            },
            Vertex {
                pos: [rect[2], rect[3]],
                uv: [uv[2], uv[3]],
                color,
            },
            Vertex {
                pos: [rect[2], rect[1]],
                uv: [uv[2], uv[1]],
                color,
            },
        ];
        let vertex_base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&verts);
        const QUAD_INDICES: [u32; 6] = [0, 1, 2, 2, 0, 3];
        self.indices
            .extend(QUAD_INDICES.iter().map(|i| i + vertex_base));
    }

    fn update_buffers(&mut self) {
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.vertices.as_ptr(),
                self.vertex_buffer.contents() as *mut _,
                self.vertices.len(),
            );
            self.vertex_buffer.did_modify_range(metal::NSRange::new(
                0,
                (self.vertices.len() * VERTEX_SIZE) as _,
            ));
            std::ptr::copy_nonoverlapping(
                self.indices.as_ptr(),
                self.index_buffer.contents() as *mut _,
                self.indices.len(),
            );
            self.index_buffer
                .did_modify_range(metal::NSRange::new(0, (self.indices.len() * 4) as _));
        }
    }
}

fn build_pso(device: &Device, library: &Library, frag_name: &str) -> RenderPipelineState {
    let desc = RenderPipelineDescriptor::new();
    let vs = library.get_function("vert", None).unwrap();
    let fs = library.get_function(frag_name, None).unwrap();
    desc.set_vertex_function(Some(&vs));
    desc.set_fragment_function(Some(&fs));
    let attachment = desc.color_attachments().object_at(0).unwrap();
    attachment.set_pixel_format(TARGET_FORMAT);
    attachment.set_blending_enabled(true);
    attachment.set_rgb_blend_operation(MTLBlendOperation::Add);
    attachment.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
    attachment.set_source_rgb_blend_factor(MTLBlendFactor::SourceAlpha);
    let vdesc = VertexDescriptor::new();
    vdesc
        .layouts()
        .object_at(0)
        .unwrap()
        .set_stride(VERTEX_SIZE as _);
    let pos = vdesc.attributes().object_at(0).unwrap();
    pos.set_format(MTLVertexFormat::Float2);
    let uv = vdesc.attributes().object_at(1).unwrap();
    uv.set_format(MTLVertexFormat::Float2);
    uv.set_offset(8);
    let color = vdesc.attributes().object_at(2).unwrap();
    color.set_format(MTLVertexFormat::UChar4Normalized);
    color.set_offset(16);
    desc.set_vertex_descriptor(Some(&vdesc));
    device.new_render_pipeline_state(&desc).unwrap()
}

const SHADER_SOURCE: &str = r#"
#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct Vertex {
    float2 pos [[attribute(0)]];
    float2 uv [[attribute(1)]];
    float4 color [[attribute(2)]];
};

struct FragData {
    float4 pos [[position]];
    float2 uv;
    float4 color;
};

vertex FragData vert(
    Vertex v [[stage_in]],
	constant uint2 *viewport_size [[buffer(1)]]
) {
  FragData out;
  out.pos = float4((v.pos / float2(*viewport_size)) * 2.0, 0.0, 1.0);
  out.pos.x -= 1.0;
  out.pos.y = 1.0 - out.pos.y;
  out.uv = v.uv;
  out.color = v.color;
  return out;
}

fragment float4 alpha_frag(
  FragData in [[stage_in]],
  texture2d<float> texture [[texture(0)]]
 ) {
  constexpr sampler samp (mag_filter::nearest, min_filter::nearest);
  return texture.sample(samp, in.uv).a * in.color;
}

fragment float4 color_frag(
    FragData in [[stage_in]],
    texture2d<float> texture [[ texture(0) ]]
   ) {
    constexpr sampler samp (mag_filter::nearest, min_filter::nearest);
    return texture.sample(samp, in.uv) * in.color;
  }
"#;
