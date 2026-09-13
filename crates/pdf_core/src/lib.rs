//! Core Engine PDF abstraction (Section 5/7 of the build prompt): the
//! domain layer talks to this trait, never to `pdfium-render` directly, so
//! the underlying PDF engine can be swapped later without touching Phase 3+
//! (Markup/Measurement/Takeoff) code.
//!
//! Scope is deliberately limited to what Phase 2 (PDF Core) needs — open,
//! page metadata, render, extract text, save/reopen — and to what the
//! Section 6 validation spike (`crates/pdf_engine_spike`) actually proved
//! works, plus `flatten_page_with_annotations` for EXPORT-01 (flattened PDF
//! export), added once there was a real `markup` domain to burn into a
//! page. Annotation *editing* (an interactive PDF annotation layer,
//! distinct from write-once export) still belongs to Phase 3 and isn't
//! exposed here.

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
    /// SEC-01: this PDF is encrypted and either no password was supplied
    /// or the one supplied was wrong. Deliberately its own variant (mapped
    /// from Pdfium's own `FPDF_ERR_PASSWORD`, not left folded into the
    /// generic `Pdfium(PdfiumError)` case) so a caller can distinguish "ask
    /// the user for a password and retry" from every other failure mode,
    /// which needs a different UI response entirely.
    #[error("this PDF is password-protected and no password (or the wrong one) was supplied")]
    PasswordRequired,
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

    /// SEC-01: `password` is `None` for the overwhelming majority of PDFs
    /// (unencrypted); an encrypted one without a correct password errors
    /// with `PdfCoreError::PasswordRequired` rather than any other error
    /// variant, so callers can prompt and retry specifically for that case.
    fn open<'e>(&'e self, path: &Path, password: Option<&'e str>) -> Result<Self::Document<'e>, PdfCoreError>;
}

/// One drawable annotation to burn into a page for EXPORT-01. Deliberately
/// minimal — a stroked polyline/polygon covers Rectangle/Line/Arrow/Cloud
/// markups (a rectangle is its 4 corners as a closed path; Arrow's
/// direction indicator isn't drawn — the shaft is enough to show what was
/// marked up, and a real arrowhead is cosmetic, not required by EXPORT-01)
/// and one text run covers Text markups. Coordinates are PDF points,
/// bottom-up (PDF's native origin) — callers translate from whatever
/// top-down coordinate space their own geometry uses before constructing
/// one of these.
pub enum FlattenAnnotation {
    Path {
        points: Vec<(f32, f32)>,
        closed: bool,
        stroke_rgb: (u8, u8, u8),
        stroke_width_pt: f32,
    },
    Text {
        x: f32,
        y: f32,
        text: String,
        font_size_pt: f32,
        rgb: (u8, u8, u8),
    },
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

    /// EXPORT-01: burns `annotations` into `page_index`'s actual page
    /// content, so they survive as real PDF page content rather than
    /// separate application-level `Markup` rows — the point of a
    /// "flattened" export. Default implementation is a no-op, so the
    /// `PdfDocument` test doubles in `document`/`e2e_tests` (which exercise
    /// import, not export) don't need updating for this; `PdfiumDocument`
    /// below overrides it with the real implementation. Takes `&mut self`
    /// (unlike every other method here) because adding page content and
    /// registering a font are the one operation in this trait pdfium-render
    /// itself requires mutable access for.
    fn flatten_page_with_annotations(
        &mut self,
        _page_index: usize,
        _annotations: &[FlattenAnnotation],
    ) -> Result<(), PdfCoreError> {
        Ok(())
    }

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

    fn open<'e>(&'e self, path: &Path, password: Option<&'e str>) -> Result<Self::Document<'e>, PdfCoreError> {
        use pdfium_render::prelude::{PdfiumError, PdfiumInternalError};

        match self.pdfium.load_pdf_from_file(path, password) {
            Ok(document) => Ok(PdfiumDocument { document }),
            Err(PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError)) => {
                Err(PdfCoreError::PasswordRequired)
            }
            Err(e) => Err(e.into()),
        }
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

    fn flatten_page_with_annotations(
        &mut self,
        page_index: usize,
        annotations: &[FlattenAnnotation],
    ) -> Result<(), PdfCoreError> {
        use pdfium_render::prelude::{
            PdfColor, PdfPageObjectCommon, PdfPageObjectsCommon, PdfPagePathObject, PdfPoints,
        };

        // Fetched before `page` below rather than per-`Text` annotation:
        // `fonts_mut()` needs `&mut self.document`, and doing this first
        // (extracting just an owned, Copy `PdfFontToken`) means the loop
        // afterwards only ever needs `&self.document`, which coexists fine
        // with holding `page` at the same time.
        let font = self.document.fonts_mut().helvetica();

        let mut page = self
            .document
            .pages()
            .get(page_index as u16)
            .map_err(|_| PdfCoreError::PageIndexOutOfRange(page_index))?;

        for annotation in annotations {
            match annotation {
                FlattenAnnotation::Path { points, closed, stroke_rgb, stroke_width_pt } => {
                    let Some((&(x0, y0), rest)) = points.split_first() else {
                        continue;
                    };
                    let color = PdfColor::new(stroke_rgb.0, stroke_rgb.1, stroke_rgb.2, 255);
                    let mut path = PdfPagePathObject::new(
                        &self.document,
                        PdfPoints::new(x0),
                        PdfPoints::new(y0),
                        Some(color),
                        Some(PdfPoints::new(*stroke_width_pt)),
                        None,
                    )?;
                    for &(x, y) in rest {
                        path.line_to(PdfPoints::new(x), PdfPoints::new(y))?;
                    }
                    if *closed {
                        path.close_path()?;
                    }
                    page.objects_mut().add_path_object(path)?;
                }
                FlattenAnnotation::Text { x, y, text, font_size_pt, rgb } => {
                    let mut object = page.objects_mut().create_text_object(
                        PdfPoints::new(*x),
                        PdfPoints::new(*y),
                        text.as_str(),
                        font,
                        PdfPoints::new(*font_size_pt),
                    )?;
                    object.set_fill_color(PdfColor::new(rgb.0, rgb.1, rgb.2, 255))?;
                }
            }
        }

        page.flatten()?;
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

    /// Serializes every test below that constructs a real `PdfiumEngine`
    /// (there were already 4 before this pass added 3 more). Discovered
    /// while adding this module's new flatten/export tests: `cargo test -p
    /// pdf_core` runs tests in parallel threads by default, and two
    /// `PdfiumEngine::new()` calls alive at the same time in one process is
    /// exactly the deadlock `crates/pdf_engine_spike/README.md` already
    /// documents (a second PDFium binding in the same process deadlocks) —
    /// so this was already a latent flake, just unlikely enough with only 4
    /// real-engine tests to not have been hit yet; 7 made it reliably
    /// reproduce. A `Mutex` guard is cheaper and more durable than telling
    /// everyone who runs this suite to remember `--test-threads=1`; the
    /// synthetic-fake tests (`tile_cache`, `tile_grid`) don't touch this
    /// lock and keep running in parallel.
    fn real_engine_test_lock() -> &'static std::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
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

        let _guard = real_engine_test_lock().lock().unwrap();
        let engine = PdfiumEngine::new(&lib_path).expect("engine should initialize");
        let document = engine.open(&pdf_path, None).expect("document should open");

        assert_eq!(document.page_count(), 7);
        let (width, height) = document.page_size(0).unwrap();
        assert!((width - 612.0).abs() < 0.5);
        assert!((height - 792.0).abs() < 0.5);

        let text = document.extract_text(0).unwrap();
        assert!(!text.is_empty());

        let png_bytes = document.render_page_to_png(0, 800).unwrap();
        assert!(png_bytes.starts_with(&[0x89, b'P', b'N', b'G']));
    }

    /// SEC-01. Fixture committed at `tests/fixtures/encrypted.pdf` (a
    /// single blank page, user+owner password `secret123`, generated with
    /// `pypdf`) rather than gated on a machine-specific
    /// `sample_pdf_path()` — unlike the rest of this module's real-engine
    /// tests, this one doesn't depend on anything outside the repo besides
    /// `libpdfium.so` itself.
    #[test]
    fn opening_an_encrypted_pdf_requires_the_correct_password() {
        let lib_path = spike_lib_path();
        if !lib_path.exists() {
            eprintln!("skipping: libpdfium.so not present at {lib_path:?} (see crates/pdf_engine_spike/README.md)");
            return;
        }
        let encrypted_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/encrypted.pdf");

        let _guard = real_engine_test_lock().lock().unwrap();
        let engine = PdfiumEngine::new(&lib_path).expect("engine should initialize");

        assert!(matches!(engine.open(&encrypted_path, None), Err(PdfCoreError::PasswordRequired)));
        assert!(matches!(
            engine.open(&encrypted_path, Some("wrong-password")),
            Err(PdfCoreError::PasswordRequired)
        ));

        let document = engine
            .open(&encrypted_path, Some("secret123"))
            .expect("the correct password should open the document");
        assert_eq!(document.page_count(), 1);
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

        let _guard = real_engine_test_lock().lock().unwrap();
        let engine = PdfiumEngine::new(&lib_path).expect("engine should initialize");
        let document = engine.open(&pdf_path, None).expect("document should open");

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

        let _guard = real_engine_test_lock().lock().unwrap();
        let engine = PdfiumEngine::new(&lib_path).expect("engine should initialize");
        let document = engine.open(&pdf_path, None).expect("document should open");

        let full = document.render_page_to_png(0, 800).unwrap();
        let full_image = image::load_from_memory(&full).unwrap();
        let full_aspect = full_image.width() as f64 / full_image.height() as f64;

        let thumb = document.render_thumbnail_png(0, 100).unwrap();
        let thumb_image = image::load_from_memory(&thumb).unwrap();
        assert_eq!(thumb_image.width(), 100);
        let thumb_aspect = thumb_image.width() as f64 / thumb_image.height() as f64;
        assert!((full_aspect - thumb_aspect).abs() < 0.01);
    }

    #[test]
    fn flatten_page_with_annotations_changes_the_rendered_page() {
        let lib_path = spike_lib_path();
        if !lib_path.exists() {
            eprintln!("skipping: libpdfium.so not present at {lib_path:?} (see crates/pdf_engine_spike/README.md)");
            return;
        }
        let Some(pdf_path) = sample_pdf_path() else {
            eprintln!("skipping: sample PDF not present on this machine");
            return;
        };

        let _guard = real_engine_test_lock().lock().unwrap();
        let engine = PdfiumEngine::new(&lib_path).expect("engine should initialize");
        let mut document = engine.open(&pdf_path, None).expect("document should open");

        let before = document.render_page_to_png(0, 400).unwrap();

        document
            .flatten_page_with_annotations(
                0,
                &[
                    FlattenAnnotation::Path {
                        points: vec![(50.0, 50.0), (200.0, 50.0), (200.0, 150.0), (50.0, 150.0)],
                        closed: true,
                        stroke_rgb: (255, 0, 0),
                        stroke_width_pt: 3.0,
                    },
                    FlattenAnnotation::Text {
                        x: 60.0,
                        y: 160.0,
                        text: "exported".to_string(),
                        font_size_pt: 18.0,
                        rgb: (0, 0, 255),
                    },
                ],
            )
            .expect("flattening with annotations should succeed");

        let after = document.render_page_to_png(0, 400).unwrap();
        assert_ne!(before, after, "flattened annotations should change the rendered page");
    }

    #[test]
    fn flatten_page_with_annotations_out_of_range_page_is_a_typed_error() {
        let lib_path = spike_lib_path();
        if !lib_path.exists() {
            eprintln!("skipping: libpdfium.so not present at {lib_path:?} (see crates/pdf_engine_spike/README.md)");
            return;
        }
        let Some(pdf_path) = sample_pdf_path() else {
            eprintln!("skipping: sample PDF not present on this machine");
            return;
        };

        let _guard = real_engine_test_lock().lock().unwrap();
        let engine = PdfiumEngine::new(&lib_path).expect("engine should initialize");
        let mut document = engine.open(&pdf_path, None).expect("document should open");

        let err = document
            .flatten_page_with_annotations(9999, &[])
            .expect_err("out-of-range page index should error");
        assert!(matches!(err, PdfCoreError::PageIndexOutOfRange(9999)));
    }

    #[test]
    fn flattened_export_saves_and_reopens_with_the_same_page_count() {
        let lib_path = spike_lib_path();
        if !lib_path.exists() {
            eprintln!("skipping: libpdfium.so not present at {lib_path:?} (see crates/pdf_engine_spike/README.md)");
            return;
        }
        let Some(pdf_path) = sample_pdf_path() else {
            eprintln!("skipping: sample PDF not present on this machine");
            return;
        };

        let _guard = real_engine_test_lock().lock().unwrap();
        let engine = PdfiumEngine::new(&lib_path).expect("engine should initialize");
        let mut document = engine.open(&pdf_path, None).expect("document should open");
        let original_page_count = document.page_count();

        document
            .flatten_page_with_annotations(
                0,
                &[FlattenAnnotation::Path {
                    points: vec![(10.0, 10.0), (100.0, 100.0)],
                    closed: false,
                    stroke_rgb: (0, 255, 0),
                    stroke_width_pt: 5.0,
                }],
            )
            .unwrap();

        let out_path = std::env::temp_dir().join(format!(
            "pdf_core_flatten_export_test_{}.pdf",
            std::process::id()
        ));
        document.save(&out_path).expect("save should succeed");

        let reopened = engine.open(&out_path, None).expect("flattened export should reopen");
        assert_eq!(reopened.page_count(), original_page_count);

        let _ = std::fs::remove_file(&out_path);
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

        let _guard = real_engine_test_lock().lock().unwrap();
        let engine = PdfiumEngine::new(&lib_path).expect("engine should initialize");
        let document = engine.open(&pdf_path, None).expect("document should open");

        assert!(matches!(
            document.page_size(9999),
            Err(PdfCoreError::PageIndexOutOfRange(9999))
        ));
    }
}
