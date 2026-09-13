//! Core Engine PDF abstraction (Section 5/7 of the build prompt): the
//! domain layer talks to this trait, never to `pdfium-render` directly, so
//! the underlying PDF engine can be swapped later without touching Phase 3+
//! (Markup/Measurement/Takeoff) code.
//!
//! Scope is deliberately limited to what Phase 2 (PDF Core) needs — open,
//! page metadata, render, extract text, save/reopen — and to what the
//! Section 6 validation spike (`crates/pdf_engine_spike`) actually proved
//! works. Annotation support belongs to Phase 3 (Markup) and is not
//! exposed here yet.

use std::path::Path;

#[derive(thiserror::Error, Debug)]
pub enum PdfCoreError {
    #[error("pdfium error: {0}")]
    Pdfium(#[from] pdfium_render::prelude::PdfiumError),
    #[error("image encode error: {0}")]
    Image(#[from] image::ImageError),
    #[error("page index {0} out of range")]
    PageIndexOutOfRange(usize),
    #[error("tile ({tile_x}, {tile_y}) out of range for a {zoomed_width}x{zoomed_height} page")]
    TileIndexOutOfRange {
        tile_x: u32,
        tile_y: u32,
        zoomed_width: u32,
        zoomed_height: u32,
    },
}

/// A PDF engine capable of opening documents. `Document<'e>` borrows from
/// the engine (`'e`), so an engine must outlive every document it opens —
/// this directly encodes the singleton-lifetime constraint discovered
/// during the Section 6 spike (a PDFium binding must not be constructed
/// more than once per process).
pub trait PdfEngine {
    type Document<'e>: PdfDocument
    where
        Self: 'e;

    fn open<'e>(&'e self, path: &Path) -> Result<Self::Document<'e>, PdfCoreError>;
}

pub trait PdfDocument {
    fn page_count(&self) -> usize;

    /// Page size in PDF points (1/72 inch).
    fn page_size(&self, page_index: usize) -> Result<(f32, f32), PdfCoreError>;

    /// Renders `page_index` to a PNG-encoded byte buffer, scaled so its
    /// width matches `target_width` pixels.
    fn render_page_to_png(
        &self,
        page_index: usize,
        target_width: u32,
    ) -> Result<Vec<u8>, PdfCoreError>;

    fn extract_text(&self, page_index: usize) -> Result<String, PdfCoreError>;

    fn save(&self, path: &Path) -> Result<(), PdfCoreError>;

    /// Renders one tile of `page_index` at zoom level `zoomed_width` (the
    /// full page's pixel width at the current zoom — height follows from
    /// the page's aspect ratio, same as `render_page_to_png`). `tile_x`/
    /// `tile_y` address a `tile_size`x`tile_size` grid over that zoomed
    /// page (see `tile_grid`); edge tiles are cropped shorter/narrower
    /// rather than padded.
    ///
    /// Default implementation: render the full zoomed page via
    /// `render_page_to_png`, then crop in-process with the `image` crate.
    /// This is correct but not yet the performance story the architecture
    /// doc wants (Section 8: "tile-based rendering... not retrofitted") —
    /// it re-rasterizes the whole page per tile rather than rasterizing
    /// only the requested region, so it doesn't save PDFium-side work, only
    /// output bandwidth/memory downstream (e.g. what crosses IPC, what
    /// `TileCache` holds). Revisit if profiling against a large/vector-heavy
    /// drawing (the still-outstanding Section 6 validation) shows this
    /// matters; PDFium's own `clip()` render option was investigated and
    /// rejected for this because it still allocates a full-page-sized
    /// bitmap and just masks it, so it wouldn't actually fix the problem.
    fn render_tile_to_png(
        &self,
        page_index: usize,
        zoomed_width: u32,
        tile_x: u32,
        tile_y: u32,
        tile_size: u32,
    ) -> Result<Vec<u8>, PdfCoreError> {
        use std::io::Cursor;

        let full_png = self.render_page_to_png(page_index, zoomed_width)?;
        let image = image::load_from_memory(&full_png)?;
        let (width, height) = (image.width(), image.height());

        let left = tile_x * tile_size;
        let top = tile_y * tile_size;
        if left >= width || top >= height {
            return Err(PdfCoreError::TileIndexOutOfRange {
                tile_x,
                tile_y,
                zoomed_width: width,
                zoomed_height: height,
            });
        }
        let crop_width = tile_size.min(width - left);
        let crop_height = tile_size.min(height - top);
        let tile = image.crop_imm(left, top, crop_width, crop_height);

        let mut bytes = Cursor::new(Vec::new());
        tile.write_to(&mut bytes, image::ImageFormat::Png)?;
        Ok(bytes.into_inner())
    }

    /// VIEW-03: a small full-page render at a conventional thumbnail width.
    /// Just names the existing `render_page_to_png` capability for this
    /// use rather than adding new rendering logic.
    fn render_thumbnail_png(&self, page_index: usize, thumbnail_width: u32) -> Result<Vec<u8>, PdfCoreError> {
        self.render_page_to_png(page_index, thumbnail_width)
    }
}

/// Computes the tile grid dimensions (columns, rows) covering a page
/// rendered at `zoomed_width`x`zoomed_height` pixels, tiled in
/// `tile_size`x`tile_size` squares. Pure math — no PDFium/rendering
/// involved — so it's tested independently of a real document.
pub fn tile_grid(zoomed_width: u32, zoomed_height: u32, tile_size: u32) -> (u32, u32) {
    let cols = zoomed_width.div_ceil(tile_size);
    let rows = zoomed_height.div_ceil(tile_size);
    (cols, rows)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TileKey {
    pub page_index: usize,
    pub zoomed_width: u32,
    pub tile_x: u32,
    pub tile_y: u32,
}

/// Infrastructure-layer LRU cache for rendered tile PNGs (architecture doc,
/// Section 8: "Tile-based rendering with an LRU cache from the start, not
/// retrofitted"). Bundled into `pdf_core` for now rather than a separate
/// infra crate, same pragmatic reasoning as this crate's other bundling.
pub struct TileCache {
    cache: lru::LruCache<TileKey, Vec<u8>>,
}

impl TileCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: lru::LruCache::new(
                std::num::NonZeroUsize::new(capacity).expect("tile cache capacity must be non-zero"),
            ),
        }
    }

    /// Returns the tile's PNG bytes, rendering (and inserting into the
    /// cache) on a miss.
    pub fn get_or_render<D: PdfDocument>(
        &mut self,
        document: &D,
        key: TileKey,
        tile_size: u32,
    ) -> Result<&Vec<u8>, PdfCoreError> {
        if !self.cache.contains(&key) {
            let png = document.render_tile_to_png(
                key.page_index,
                key.zoomed_width,
                key.tile_x,
                key.tile_y,
                tile_size,
            )?;
            self.cache.put(key.clone(), png);
        }
        Ok(self
            .cache
            .get(&key)
            .expect("just inserted or already present"))
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

pub struct PdfiumEngine {
    pdfium: pdfium_render::prelude::Pdfium,
}

impl PdfiumEngine {
    /// `lib_path` points at a `libpdfium` shared library (see
    /// `crates/pdf_engine_spike/README.md` for how to fetch one). Construct
    /// exactly one `PdfiumEngine` per process and keep it alive for the
    /// process lifetime — see the module docs above.
    pub fn new(lib_path: &Path) -> Result<Self, PdfCoreError> {
        let bindings = pdfium_render::prelude::Pdfium::bind_to_library(lib_path)?;
        Ok(Self {
            pdfium: pdfium_render::prelude::Pdfium::new(bindings),
        })
    }
}

impl PdfEngine for PdfiumEngine {
    type Document<'e> = PdfiumDocument<'e>;

    fn open<'e>(&'e self, path: &Path) -> Result<Self::Document<'e>, PdfCoreError> {
        let document = self.pdfium.load_pdf_from_file(path, None)?;
        Ok(PdfiumDocument { document })
    }
}

pub struct PdfiumDocument<'e> {
    document: pdfium_render::prelude::PdfDocument<'e>,
}

impl<'e> PdfiumDocument<'e> {
    fn get_page(
        &self,
        page_index: usize,
    ) -> Result<pdfium_render::prelude::PdfPage<'_>, PdfCoreError> {
        self.document
            .pages()
            .get(page_index as u16)
            .map_err(|_| PdfCoreError::PageIndexOutOfRange(page_index))
    }
}

impl<'e> PdfDocument for PdfiumDocument<'e> {
    fn page_count(&self) -> usize {
        self.document.pages().len() as usize
    }

    fn page_size(&self, page_index: usize) -> Result<(f32, f32), PdfCoreError> {
        let page = self.get_page(page_index)?;
        Ok((page.width().value, page.height().value))
    }

    fn render_page_to_png(
        &self,
        page_index: usize,
        target_width: u32,
    ) -> Result<Vec<u8>, PdfCoreError> {
        use pdfium_render::prelude::PdfRenderConfig;
        use std::io::Cursor;

        let page = self.get_page(page_index)?;
        let render_config = PdfRenderConfig::new().set_target_width(target_width as i32);
        let bitmap = page.render_with_config(&render_config)?;
        let image = bitmap.as_image();

        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png)?;
        Ok(bytes.into_inner())
    }

    fn extract_text(&self, page_index: usize) -> Result<String, PdfCoreError> {
        let page = self.get_page(page_index)?;
        let text = page.text()?.all();
        Ok(text)
    }

    fn save(&self, path: &Path) -> Result<(), PdfCoreError> {
        self.document.save_to_file(path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn spike_lib_path() -> PathBuf {
        // Reuses the same downloaded libpdfium.so as crates/pdf_engine_spike
        // (see that crate's README for how to fetch it) rather than
        // duplicating the binary.
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("pdf_engine_spike/lib/libpdfium.so")
    }

    fn sample_pdf_path() -> Option<PathBuf> {
        let candidate =
            PathBuf::from("/home/naveen/learn road map/UIUX-Exact-10-Day-Roadmap.md.pdf");
        candidate.exists().then_some(candidate)
    }

    #[test]
    fn open_and_read_document_metadata() {
        let lib_path = spike_lib_path();
        if !lib_path.exists() {
            eprintln!("skipping: libpdfium.so not present at {lib_path:?} (see crates/pdf_engine_spike/README.md)");
            return;
        }
        let Some(pdf_path) = sample_pdf_path() else {
            eprintln!("skipping: sample PDF not present on this machine");
            return;
        };

        let engine = PdfiumEngine::new(&lib_path).expect("engine should initialize");
        let document = engine.open(&pdf_path).expect("document should open");

        assert_eq!(document.page_count(), 7);
        let (width, height) = document.page_size(0).unwrap();
        assert!((width - 612.0).abs() < 0.5);
        assert!((height - 792.0).abs() < 0.5);

        let text = document.extract_text(0).unwrap();
        assert!(!text.is_empty());

        let png_bytes = document.render_page_to_png(0, 800).unwrap();
        assert!(png_bytes.starts_with(&[0x89, b'P', b'N', b'G']));
    }

    #[test]
    fn tile_grid_covers_full_zoomed_page_with_partial_edge_tiles() {
        // 1000x700 at 256px tiles: 4 cols (3*256=768 < 1000 <= 4*256=1024),
        // 3 rows (2*256=512 < 700 <= 3*256=768) — last row/col are partial.
        assert_eq!(tile_grid(1000, 700, 256), (4, 3));
        // Exact multiples shouldn't add a spurious extra tile.
        assert_eq!(tile_grid(1024, 768, 256), (4, 3));
    }

    #[test]
    fn render_tile_to_png_produces_correctly_sized_edge_and_interior_tiles() {
        let lib_path = spike_lib_path();
        if !lib_path.exists() {
            eprintln!("skipping: libpdfium.so not present at {lib_path:?} (see crates/pdf_engine_spike/README.md)");
            return;
        }
        let Some(pdf_path) = sample_pdf_path() else {
            eprintln!("skipping: sample PDF not present on this machine");
            return;
        };

        let engine = PdfiumEngine::new(&lib_path).expect("engine should initialize");
        let document = engine.open(&pdf_path).expect("document should open");

        // Page is 612x792 pt (US Letter); zoom to 900px wide so height > 900
        // too, guaranteeing a multi-tile grid at tile_size 256.
        let full_png = document.render_page_to_png(0, 900).unwrap();
        let full_image = image::load_from_memory(&full_png).unwrap();
        let (zoomed_width, zoomed_height) = (full_image.width(), full_image.height());
        let (cols, rows) = tile_grid(zoomed_width, zoomed_height, 256);
        assert!(cols > 1 && rows > 1, "expected a multi-tile grid to exercise edge cropping");

        // Interior tile: full 256x256.
        let interior = document.render_tile_to_png(0, zoomed_width, 0, 0, 256).unwrap();
        let interior_image = image::load_from_memory(&interior).unwrap();
        assert_eq!((interior_image.width(), interior_image.height()), (256, 256));

        // Last-column tile: narrower than 256 since it's a partial edge tile.
        let last_col = cols - 1;
        let edge = document
            .render_tile_to_png(0, zoomed_width, last_col, 0, 256)
            .unwrap();
        let edge_image = image::load_from_memory(&edge).unwrap();
        let expected_width = zoomed_width - last_col * 256;
        assert_eq!(edge_image.width(), expected_width);
        assert!(expected_width < 256);

        // Out-of-range tile coordinates are a typed error, not a panic.
        assert!(matches!(
            document.render_tile_to_png(0, zoomed_width, cols, 0, 256),
            Err(PdfCoreError::TileIndexOutOfRange { .. })
        ));
    }

    #[test]
    fn render_thumbnail_preserves_aspect_ratio_of_full_render() {
        let lib_path = spike_lib_path();
        if !lib_path.exists() {
            eprintln!("skipping: libpdfium.so not present at {lib_path:?} (see crates/pdf_engine_spike/README.md)");
            return;
        }
        let Some(pdf_path) = sample_pdf_path() else {
            eprintln!("skipping: sample PDF not present on this machine");
            return;
        };

        let engine = PdfiumEngine::new(&lib_path).expect("engine should initialize");
        let document = engine.open(&pdf_path).expect("document should open");

        let full = document.render_page_to_png(0, 800).unwrap();
        let full_image = image::load_from_memory(&full).unwrap();
        let full_aspect = full_image.width() as f64 / full_image.height() as f64;

        let thumb = document.render_thumbnail_png(0, 100).unwrap();
        let thumb_image = image::load_from_memory(&thumb).unwrap();
        assert_eq!(thumb_image.width(), 100);
        let thumb_aspect = thumb_image.width() as f64 / thumb_image.height() as f64;
        assert!((full_aspect - thumb_aspect).abs() < 0.01);
    }

    /// A `PdfDocument` test double that fabricates a solid-color PNG instead
    /// of touching PDFium, so `TileCache` behavior (hit/miss/eviction) can
    /// be tested without a real `libpdfium.so`.
    struct CountingFakeDocument {
        render_calls: std::cell::Cell<u32>,
    }

    impl PdfDocument for CountingFakeDocument {
        fn page_count(&self) -> usize {
            1
        }

        fn page_size(&self, _page_index: usize) -> Result<(f32, f32), PdfCoreError> {
            Ok((100.0, 100.0))
        }

        fn render_page_to_png(
            &self,
            _page_index: usize,
            target_width: u32,
        ) -> Result<Vec<u8>, PdfCoreError> {
            self.render_calls.set(self.render_calls.get() + 1);
            use std::io::Cursor;
            let img = image::RgbImage::from_pixel(target_width, target_width, image::Rgb([255, 0, 0]));
            let mut bytes = Cursor::new(Vec::new());
            image::DynamicImage::ImageRgb8(img).write_to(&mut bytes, image::ImageFormat::Png)?;
            Ok(bytes.into_inner())
        }

        fn extract_text(&self, _page_index: usize) -> Result<String, PdfCoreError> {
            Ok(String::new())
        }

        fn save(&self, _path: &Path) -> Result<(), PdfCoreError> {
            Ok(())
        }
    }

    #[test]
    fn tile_cache_reuses_cached_tile_without_re_rendering() {
        let doc = CountingFakeDocument {
            render_calls: std::cell::Cell::new(0),
        };
        let mut cache = TileCache::new(2);
        let key = TileKey {
            page_index: 0,
            zoomed_width: 400,
            tile_x: 0,
            tile_y: 0,
        };

        cache.get_or_render(&doc, key.clone(), 256).unwrap();
        assert_eq!(doc.render_calls.get(), 1);

        cache.get_or_render(&doc, key, 256).unwrap();
        assert_eq!(doc.render_calls.get(), 1, "second lookup should hit the cache");
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn tile_cache_evicts_least_recently_used_entry() {
        let doc = CountingFakeDocument {
            render_calls: std::cell::Cell::new(0),
        };
        let mut cache = TileCache::new(1);
        let key_a = TileKey {
            page_index: 0,
            zoomed_width: 400,
            tile_x: 0,
            tile_y: 0,
        };
        let key_b = TileKey {
            page_index: 0,
            zoomed_width: 400,
            tile_x: 1,
            tile_y: 0,
        };

        cache.get_or_render(&doc, key_a.clone(), 256).unwrap();
        cache.get_or_render(&doc, key_b, 256).unwrap(); // capacity 1: evicts key_a
        assert_eq!(doc.render_calls.get(), 2);

        cache.get_or_render(&doc, key_a, 256).unwrap(); // must re-render, not cached anymore
        assert_eq!(doc.render_calls.get(), 3);
    }

    #[test]
    fn page_index_out_of_range_is_a_typed_error() {
        let lib_path = spike_lib_path();
        if !lib_path.exists() {
            eprintln!("skipping: libpdfium.so not present");
            return;
        }
        let Some(pdf_path) = sample_pdf_path() else {
            eprintln!("skipping: sample PDF not present on this machine");
            return;
        };

        let engine = PdfiumEngine::new(&lib_path).expect("engine should initialize");
        let document = engine.open(&pdf_path).expect("document should open");

        assert!(matches!(
            document.page_size(9999),
            Err(PdfCoreError::PageIndexOutOfRange(9999))
        ));
    }
}
