use std::collections::HashMap;

use cosmic_text::{
    Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache,
};
use smithay::backend::renderer::gles::GlesRenderer;
use tracing::debug;

/// Cached glyph info for rendering.
#[derive(Debug, Clone, Copy)]
pub struct GlyphInfo {
    /// UV coordinates in the atlas texture (normalized 0..1)
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    /// Glyph dimensions in pixels
    pub width: u32,
    pub height: u32,
    /// Offset from cell origin to glyph origin
    pub bearing_x: f32,
    pub bearing_y: f32,
}

/// GPU texture atlas of rasterized glyphs.
pub struct GlyphAtlas {
    font_system: FontSystem,
    swash_cache: SwashCache,
    font_size: f32,

    // Cell metrics derived from the font
    cell_w: u16,
    cell_h: u16,
    baseline: f32,

    // Atlas texture data (CPU side — uploaded to GPU on demand)
    atlas_width: u32,
    atlas_height: u32,
    atlas_data: Vec<u8>, // RGBA
    atlas_cursor_x: u32,
    atlas_cursor_y: u32,
    atlas_row_height: u32,

    // Glyph cache: char → atlas location
    glyphs: HashMap<char, GlyphInfo>,
}

impl GlyphAtlas {
    pub fn new(font_size: f32) -> Result<Self, Box<dyn std::error::Error>> {
        let mut font_system = FontSystem::new();

        // Derive cell metrics from a reference buffer
        let metrics = Metrics::new(font_size, font_size * 1.2);
        let mut buffer = Buffer::new(&mut font_system, metrics);
        buffer.set_text(&mut font_system, "M", Attrs::new().family(Family::Monospace), Shaping::Advanced);

        // Get cell dimensions from font metrics
        let line_height = (font_size * 1.2).ceil() as u16;
        let cell_w = (font_size * 0.6).ceil() as u16; // Approximate monospace width
        let cell_h = line_height;
        let baseline = font_size;

        // Measure more precisely from the buffer layout
        buffer.shape_until_scroll(&mut font_system, false);
        for run in buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let w = glyph.w.ceil() as u16;
                if w > 0 {
                    // Use actual glyph advance for cell width
                    return Ok(Self {
                        font_system,
                        swash_cache: SwashCache::new(),
                        font_size,
                        cell_w: w,
                        cell_h,
                        baseline,
                        atlas_width: 1024,
                        atlas_height: 1024,
                        atlas_data: vec![0u8; 1024 * 1024 * 4],
                        atlas_cursor_x: 1, // leave 1px border
                        atlas_cursor_y: 1,
                        atlas_row_height: 0,
                        glyphs: HashMap::new(),
                    });
                }
            }
        }

        // Fallback if we couldn't measure
        Ok(Self {
            font_system,
            swash_cache: SwashCache::new(),
            font_size,
            cell_w,
            cell_h,
            baseline,
            atlas_width: 1024,
            atlas_height: 1024,
            atlas_data: vec![0u8; 1024 * 1024 * 4],
            atlas_cursor_x: 1,
            atlas_cursor_y: 1,
            atlas_row_height: 0,
            glyphs: HashMap::new(),
        })
    }

    pub fn cell_width(&self) -> u16 {
        self.cell_w
    }

    pub fn cell_height(&self) -> u16 {
        self.cell_h
    }

    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }

    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    pub fn baseline(&self) -> f32 {
        self.baseline
    }

    /// Rasterize ASCII printable range into the atlas.
    pub fn load_ascii(
        &mut self,
        _renderer: &mut GlesRenderer,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for ch in (0x20u8..=0x7Eu8).map(|b| b as char) {
            self.rasterize_glyph(ch);
        }
        debug!("loaded {} ASCII glyphs into atlas", self.glyphs.len());
        Ok(())
    }

    /// Get glyph info, rasterizing on demand if not cached.
    pub fn get_glyph(&mut self, ch: char) -> Option<&GlyphInfo> {
        if !self.glyphs.contains_key(&ch) {
            self.rasterize_glyph(ch);
        }
        self.glyphs.get(&ch)
    }

    /// Atlas pixel data (RGBA, row-major).
    pub fn atlas_data(&self) -> &[u8] {
        &self.atlas_data
    }

    pub fn atlas_width(&self) -> u32 {
        self.atlas_width
    }

    pub fn atlas_height(&self) -> u32 {
        self.atlas_height
    }

    fn rasterize_glyph(&mut self, ch: char) {
        if self.glyphs.contains_key(&ch) {
            return;
        }

        let metrics = Metrics::new(self.font_size, self.font_size * 1.2);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        let s = String::from(ch);
        buffer.set_text(
            &mut self.font_system,
            &s,
            Attrs::new().family(Family::Monospace),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);

        // Find the glyph in the layout
        for run in buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical = glyph.physical((0.0, 0.0), 1.0);

                // Try to get the rasterized image
                if let Some(image) = self
                    .swash_cache
                    .get_image(&mut self.font_system, physical.cache_key)
                {
                    let img_w = image.placement.width as u32;
                    let img_h = image.placement.height as u32;

                    if img_w == 0 || img_h == 0 {
                        // Space or zero-width — store empty glyph
                        self.glyphs.insert(
                            ch,
                            GlyphInfo {
                                u0: 0.0,
                                v0: 0.0,
                                u1: 0.0,
                                v1: 0.0,
                                width: 0,
                                height: 0,
                                bearing_x: 0.0,
                                bearing_y: 0.0,
                            },
                        );
                        return;
                    }

                    // Check if we need to wrap to next row
                    if self.atlas_cursor_x + img_w + 1 > self.atlas_width {
                        self.atlas_cursor_x = 1;
                        self.atlas_cursor_y += self.atlas_row_height + 1;
                        self.atlas_row_height = 0;
                    }

                    // Check if atlas is full
                    if self.atlas_cursor_y + img_h + 1 > self.atlas_height {
                        debug!("glyph atlas full, can't add '{}'", ch);
                        return;
                    }

                    // Copy glyph pixels into atlas
                    let ax = self.atlas_cursor_x;
                    let ay = self.atlas_cursor_y;

                    for py in 0..img_h {
                        for px in 0..img_w {
                            let src_idx = (py * img_w + px) as usize;
                            let dst_x = (ax + px) as usize;
                            let dst_y = (ay + py) as usize;
                            let dst_idx = (dst_y * self.atlas_width as usize + dst_x) * 4;

                            if dst_idx + 3 < self.atlas_data.len() && src_idx < image.data.len() {
                                let alpha = image.data[src_idx];
                                self.atlas_data[dst_idx] = 255; // R
                                self.atlas_data[dst_idx + 1] = 255; // G
                                self.atlas_data[dst_idx + 2] = 255; // B
                                self.atlas_data[dst_idx + 3] = alpha; // A
                            }
                        }
                    }

                    let aw = self.atlas_width as f32;
                    let ah = self.atlas_height as f32;

                    self.glyphs.insert(
                        ch,
                        GlyphInfo {
                            u0: ax as f32 / aw,
                            v0: ay as f32 / ah,
                            u1: (ax + img_w) as f32 / aw,
                            v1: (ay + img_h) as f32 / ah,
                            width: img_w,
                            height: img_h,
                            bearing_x: image.placement.left as f32,
                            bearing_y: image.placement.top as f32,
                        },
                    );

                    self.atlas_cursor_x += img_w + 1;
                    if img_h > self.atlas_row_height {
                        self.atlas_row_height = img_h;
                    }

                    return;
                }
            }
        }

        // Glyph not found in font — store empty
        self.glyphs.insert(
            ch,
            GlyphInfo {
                u0: 0.0,
                v0: 0.0,
                u1: 0.0,
                v1: 0.0,
                width: 0,
                height: 0,
                bearing_x: 0.0,
                bearing_y: 0.0,
            },
        );
    }
}
