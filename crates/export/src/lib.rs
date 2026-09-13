//! EXPORT-01 (flattened PDF export): burns `Markup` rows into a PDF
//! document's real page content via `pdf_core::PdfDocument::
//! flatten_page_with_annotations`, then saves the result to a new file.
//!
//! Pure composition, no DB access — the caller (an IPC command) already
//! has the `Markup` rows for every page it wants exported; this crate only
//! knows how to translate one page's rows into `pdf_core`'s generic
//! `FlattenAnnotation` shape (see that type's doc for why it's this
//! minimal) and drive the per-page loop.

use markup::{Markup, MarkupType};
use pdf_core::{FlattenAnnotation, PdfCoreError, PdfDocument};
use std::collections::HashMap;
use std::path::Path;

/// `#e53935` — `PdfCanvas`'s own default stroke color in the frontend
/// (`app/src/App.tsx`), reused here so a markup with no explicit color
/// exports looking the same as it renders in the app.
const DEFAULT_COLOR: (u8, u8, u8) = (0xe5, 0x39, 0x35);
const DEFAULT_STROKE_WIDTH_PT: f32 = 2.0;
const TEXT_FONT_SIZE_PT: f32 = 12.0;

fn parse_hex_color(hex: Option<&str>) -> (u8, u8, u8) {
    let Some(hex) = hex.map(|h| h.trim_start_matches('#')) else {
        return DEFAULT_COLOR;
    };
    if hex.len() != 6 {
        return DEFAULT_COLOR;
    }
    match (
        u8::from_str_radix(&hex[0..2], 16),
        u8::from_str_radix(&hex[2..4], 16),
        u8::from_str_radix(&hex[4..6], 16),
    ) {
        (Ok(r), Ok(g), Ok(b)) => (r, g, b),
        _ => DEFAULT_COLOR,
    }
}

/// Converts one `Markup` row to a `pdf_core::FlattenAnnotation`, flipping
/// its stored top-down y coordinates (`markup`'s own module doc: "the
/// y-axis stays image-top-down rather than PDF's native bottom-up") into
/// PDF's native bottom-up page space. `page_height` is that page's height
/// in PDF points. Returns `None` for a markup with too few points to
/// convert (shouldn't happen — `MarkupGeometry::validate` already enforces
/// a minimum at creation time — but this stays defensive rather than
/// panicking on a row some future bug lets through).
pub fn markup_to_annotation(markup: &Markup, page_height: f32) -> Option<FlattenAnnotation> {
    let flip = |&(x, y): &(f64, f64)| (x as f32, page_height - y as f32);
    let stroke_rgb = parse_hex_color(markup.style.color.as_deref());
    let stroke_width_pt = markup
        .style
        .stroke_width
        .map(|w| w as f32)
        .unwrap_or(DEFAULT_STROKE_WIDTH_PT);

    match markup.markup_type {
        MarkupType::Text => {
            let (x, y) = markup.geometry.points.first().map(flip)?;
            Some(FlattenAnnotation::Text {
                x,
                y,
                text: markup.style.text.clone().unwrap_or_default(),
                font_size_pt: TEXT_FONT_SIZE_PT,
                rgb: stroke_rgb,
            })
        }
        MarkupType::Rectangle => {
            let pts: Vec<(f32, f32)> = markup.geometry.points.iter().map(flip).collect();
            let (&(x1, y1), &(x2, y2)) = (pts.first()?, pts.get(1)?);
            Some(FlattenAnnotation::Path {
                points: vec![(x1, y1), (x2, y1), (x2, y2), (x1, y2)],
                closed: true,
                stroke_rgb,
                stroke_width_pt,
            })
        }
        MarkupType::Line | MarkupType::Arrow => Some(FlattenAnnotation::Path {
            points: markup.geometry.points.iter().map(flip).collect(),
            closed: false,
            stroke_rgb,
            stroke_width_pt,
        }),
        MarkupType::Cloud => Some(FlattenAnnotation::Path {
            points: markup.geometry.points.iter().map(flip).collect(),
            closed: true,
            stroke_rgb,
            stroke_width_pt,
        }),
    }
}

/// Flattens every page's markups (skipping hidden ones — they're not
/// visible in the app, so shouldn't appear in the exported file either)
/// into `document`'s real page content, then saves to `output_path`.
/// `markups_by_page_index` is 0-based (`page_number - 1`), matching every
/// other page-index convention in this codebase (see `commands::pdf`).
/// A page with no entry, or only hidden markups, is left untouched — no
/// point calling into PDFium for a no-op flatten.
pub fn export_flattened_pdf<D: PdfDocument>(
    document: &mut D,
    markups_by_page_index: &HashMap<usize, Vec<Markup>>,
    output_path: &Path,
) -> Result<(), PdfCoreError> {
    for page_index in 0..document.page_count() {
        let Some(markups) = markups_by_page_index.get(&page_index) else {
            continue;
        };
        let (_, page_height) = document.page_size(page_index)?;
        let annotations: Vec<FlattenAnnotation> = markups
            .iter()
            .filter(|m| !m.hidden)
            .filter_map(|m| markup_to_annotation(m, page_height))
            .collect();
        if !annotations.is_empty() {
            document.flatten_page_with_annotations(page_index, &annotations)?;
        }
    }
    document.save(output_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use markup::{MarkupGeometry, MarkupStyle};
    use std::cell::RefCell;
    use std::path::PathBuf;

    struct RecordingDocument {
        page_heights: Vec<f32>,
        flatten_calls: Vec<(usize, usize)>,
        saved_to: RefCell<Option<PathBuf>>,
    }

    impl RecordingDocument {
        fn new(page_heights: Vec<f32>) -> Self {
            Self { page_heights, flatten_calls: Vec::new(), saved_to: RefCell::new(None) }
        }
    }

    impl PdfDocument for RecordingDocument {
        fn page_count(&self) -> usize {
            self.page_heights.len()
        }

        fn page_size(&self, page_index: usize) -> Result<(f32, f32), PdfCoreError> {
            self.page_heights
                .get(page_index)
                .map(|&h| (612.0, h))
                .ok_or(PdfCoreError::PageIndexOutOfRange(page_index))
        }

        fn render_page_to_png(&self, _page_index: usize, _target_width: u32) -> Result<Vec<u8>, PdfCoreError> {
            Ok(Vec::new())
        }

        fn extract_text(&self, _page_index: usize) -> Result<String, PdfCoreError> {
            Ok(String::new())
        }

        fn save(&self, path: &Path) -> Result<(), PdfCoreError> {
            *self.saved_to.borrow_mut() = Some(path.to_path_buf());
            Ok(())
        }

        fn flatten_page_with_annotations(
            &mut self,
            page_index: usize,
            annotations: &[FlattenAnnotation],
        ) -> Result<(), PdfCoreError> {
            self.flatten_calls.push((page_index, annotations.len()));
            Ok(())
        }
    }

    fn text_markup(point: (f64, f64), text: &str, hidden: bool) -> Markup {
        Markup {
            id: "m-text".into(),
            page_id: "p1".into(),
            markup_type: MarkupType::Text,
            geometry: MarkupGeometry { points: vec![point] },
            style: MarkupStyle { text: Some(text.to_string()), ..Default::default() },
            author: None,
            locked: false,
            hidden,
        }
    }

    fn rect_markup(p1: (f64, f64), p2: (f64, f64), color: &str) -> Markup {
        Markup {
            id: "m-rect".into(),
            page_id: "p1".into(),
            markup_type: MarkupType::Rectangle,
            geometry: MarkupGeometry { points: vec![p1, p2] },
            style: MarkupStyle { color: Some(color.to_string()), ..Default::default() },
            author: None,
            locked: false,
            hidden: false,
        }
    }

    #[test]
    fn text_markup_flips_y_into_pdf_bottom_up_space() {
        let markup = text_markup((10.0, 20.0), "hello", false);
        let annotation = markup_to_annotation(&markup, 792.0).unwrap();
        match annotation {
            FlattenAnnotation::Text { x, y, text, .. } => {
                assert_eq!(x, 10.0);
                assert_eq!(y, 792.0 - 20.0);
                assert_eq!(text, "hello");
            }
            _ => panic!("expected a Text annotation"),
        }
    }

    #[test]
    fn rectangle_expands_two_corners_into_a_closed_four_point_path() {
        let markup = rect_markup((0.0, 0.0), (100.0, 50.0), "#00ff00");
        let annotation = markup_to_annotation(&markup, 200.0).unwrap();
        match annotation {
            FlattenAnnotation::Path { points, closed, stroke_rgb, .. } => {
                assert!(closed);
                assert_eq!(points.len(), 4);
                assert_eq!(stroke_rgb, (0, 255, 0));
                // (0,0) flips to (0,200); (100,50) flips to (100,150).
                assert_eq!(points[0], (0.0, 200.0));
                assert_eq!(points[2], (100.0, 150.0));
            }
            _ => panic!("expected a Path annotation"),
        }
    }

    #[test]
    fn invalid_or_missing_color_falls_back_to_the_canvas_default() {
        let markup = rect_markup((0.0, 0.0), (1.0, 1.0), "not-a-color");
        let annotation = markup_to_annotation(&markup, 100.0).unwrap();
        let FlattenAnnotation::Path { stroke_rgb, .. } = annotation else {
            panic!("expected a Path annotation")
        };
        assert_eq!(stroke_rgb, DEFAULT_COLOR);
    }

    #[test]
    fn export_skips_hidden_markups_and_pages_with_nothing_to_draw() {
        let mut document = RecordingDocument::new(vec![792.0, 792.0]);
        let mut markups_by_page = HashMap::new();
        markups_by_page.insert(
            0,
            vec![text_markup((1.0, 1.0), "visible", false), text_markup((2.0, 2.0), "hidden", true)],
        );
        // Page 1 has no entry at all.

        let out = PathBuf::from("/tmp/does-not-matter.pdf");
        export_flattened_pdf(&mut document, &markups_by_page, &out).unwrap();

        assert_eq!(document.flatten_calls, vec![(0, 1)]);
        assert_eq!(*document.saved_to.borrow(), Some(out));
    }

    #[test]
    fn export_skips_a_page_whose_only_markups_are_hidden() {
        let mut document = RecordingDocument::new(vec![792.0]);
        let mut markups_by_page = HashMap::new();
        markups_by_page.insert(0, vec![text_markup((1.0, 1.0), "hidden", true)]);

        export_flattened_pdf(&mut document, &markups_by_page, Path::new("/tmp/out.pdf")).unwrap();

        assert!(document.flatten_calls.is_empty());
    }
}
