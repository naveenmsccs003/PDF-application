//! VIEW-04 validation (Section 6 of the build prompt): render/tile timing
//! against a large, vector-dense multi-page PDF.
//!
//! This is deliberately **not** the real thing the master prompt asks for —
//! a real 100+ page MDS Rebar construction drawing set, which still has to
//! come from Naveen (see `crates/pdf_engine_spike/README.md`). What this
//! *can* validate without that file: whether the render/tile pipeline
//! degrades catastrophically (not just slowly) as page count and path
//! density grow, using a synthetic stand-in generated in-process (an
//! ARCH-D-sized page with ~1200 independently-stroked line segments plus
//! ~80 text labels per page — meant to approximate a rebar placement
//! sheet's line/label density, not to be mistaken for real content).
//! Generated at test time rather than committed as a fixture so this file
//! doesn't carry a multi-megabyte binary blob for a few KB of Rust.
//!
//! Bounds asserted here are smoke-level (order-of-magnitude), not
//! performance targets — the point is catching a regression that makes
//! this unusable, not chasing a specific millisecond number. Actual timing
//! is printed (`cargo test -p pdf_core --test large_pdf_performance --
//! --nocapture`) for a human to read against real interactive-use
//! expectations.
//!
//! **Run this under `--release` for numbers that mean anything against
//! real interactive-use expectations.** First measured under plain `cargo
//! test` (this workspace's normal invocation everywhere else): ~2.6-6.9s
//! per full-page render, ~13s/tile. Re-measured under `cargo test
//! --release`: ~80-85ms per full-page render (after a ~4.3s one-time
//! first-render warm-up cost per process — not per page), ~250ms/tile.
//! That's roughly a 30-50x gap from the `image` crate's PNG encode/decode
//! path alone running unoptimized — not evidence of a real architectural
//! problem, which the first (debug-mode) run briefly looked like before
//! this was isolated. The bounds below are loose enough to tolerate the
//! debug number and still catch genuine regressions; see
//! `docs/00_SCOPE_REALITY_CHECK.md` for the full writeup including what
//! these numbers do and don't validate.

use pdf_core::{PdfDocument, PdfEngine, PdfiumEngine, TileCache, TileKey};
use std::path::PathBuf;
use std::time::Instant;

const PAGE_W: f32 = 2592.0; // ARCH D, 36x24in @ 72dpi
const PAGE_H: f32 = 1728.0;
const N_PAGES: usize = 30;
const SEGMENTS_PER_PAGE: usize = 1200;
const LABELS_PER_PAGE: usize = 80;

fn spike_lib_path() -> PathBuf {
    // Filename is platform-dependent — see `pdf_core::platform_library_filename`.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("pdf_engine_spike/lib")
        .join(pdf_core::platform_library_filename())
}

/// Deterministic xorshift so this test's output (page count, timings vs.
/// content) is reproducible across runs without pulling in a `rand` dev-dependency.
struct Rng(u64);
impl Rng {
    fn next_f32(&mut self, lo: f32, hi: f32) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        let unit = (self.0 >> 11) as f32 / (1u64 << 53) as f32;
        lo + unit * (hi - lo)
    }
    fn choice<T: Copy>(&mut self, options: &[T]) -> T {
        let idx = (self.next_f32(0.0, options.len() as f32) as usize).min(options.len() - 1);
        options[idx]
    }
}

fn page_content(rng: &mut Rng) -> Vec<u8> {
    let mut ops = String::new();
    ops.push_str("1 w\n");
    for _ in 0..SEGMENTS_PER_PAGE {
        let x1 = rng.next_f32(20.0, PAGE_W - 20.0);
        let y1 = rng.next_f32(20.0, PAGE_H - 20.0);
        let length = rng.next_f32(20.0, 400.0);
        let angle_deg = rng.choice(&[0.0f32, 90.0, 45.0, -45.0]);
        let rad = angle_deg.to_radians();
        let x2 = x1 + length * rad.cos();
        let y2 = y1 + length * rad.sin();
        let gray = rng.choice(&[0.0f32, 0.2, 0.5]);
        ops.push_str(&format!("{gray:.2} G\n{x1:.2} {y1:.2} m {x2:.2} {y2:.2} l S\n"));
    }
    ops.push_str("BT /F1 8 Tf\n");
    for i in 0..LABELS_PER_PAGE {
        let x = rng.next_f32(20.0, PAGE_W - 60.0);
        let y = rng.next_f32(20.0, PAGE_H - 20.0);
        let bar = rng.choice(&[4, 5, 6, 7, 8, 9, 10, 11]);
        let spacing = rng.choice(&[6, 8, 10, 12, 16, 18]);
        ops.push_str(&format!(
            "1 0 0 1 {x:.2} {y:.2} Tm (#{bar}@{spacing}in mark {i}) Tj\n"
        ));
    }
    ops.push_str("ET\n");
    ops.into_bytes()
}

/// Minimal raw PDF writer (no third-party crate — this workspace has no
/// PDF-*authoring* dependency, only reading/rendering ones, and pulling one
/// in just for a test fixture generator wasn't justified). Produces a
/// classic (non-cross-reference-stream) PDF with one `Pages` tree and one
/// `Contents` stream per page, which is all `render_page_to_png` needs.
fn build_synthetic_pdf() -> Vec<u8> {
    let mut objs: Vec<Vec<u8>> = Vec::new();
    let add_obj = |objs: &mut Vec<Vec<u8>>, body: Vec<u8>| -> usize {
        objs.push(body);
        objs.len()
    };

    let font_obj = add_obj(
        &mut objs,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
    );

    let mut rng = Rng(0x2545F4914F6CDD1D);
    let mut content_ids = Vec::with_capacity(N_PAGES);
    for _ in 0..N_PAGES {
        let content = page_content(&mut rng);
        let mut body = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
        body.extend_from_slice(&content);
        body.extend_from_slice(b"\nendstream");
        content_ids.push(add_obj(&mut objs, body));
    }

    let reserved_pages_id = objs.len() + N_PAGES + 1;
    let mut page_ids = Vec::with_capacity(N_PAGES);
    for i in 0..N_PAGES {
        let body = format!(
            "<< /Type /Page /Parent {reserved_pages_id} 0 R /MediaBox [0 0 {PAGE_W} {PAGE_H}] \
             /Resources << /Font << /F1 {font_obj} 0 R >> >> /Contents {} 0 R >>",
            content_ids[i]
        )
        .into_bytes();
        page_ids.push(add_obj(&mut objs, body));
    }

    let kids: String = page_ids
        .iter()
        .map(|id| format!("{id} 0 R"))
        .collect::<Vec<_>>()
        .join(" ");
    let pages_id = add_obj(
        &mut objs,
        format!("<< /Type /Pages /Kids [ {kids} ] /Count {N_PAGES} >>").into_bytes(),
    );
    assert_eq!(pages_id, reserved_pages_id, "pages tree object id must match what page objects reference as /Parent");

    let catalog_id = add_obj(
        &mut objs,
        format!("<< /Type /Catalog /Pages {pages_id} 0 R >>").into_bytes(),
    );

    let mut out = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n");
    let mut offsets = vec![0usize; objs.len() + 1];
    for (i, body) in objs.iter().enumerate() {
        let obj_num = i + 1;
        offsets[obj_num] = out.len();
        out.extend_from_slice(format!("{obj_num} 0 obj\n").as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref_start = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", objs.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for obj_num in 1..=objs.len() {
        out.extend_from_slice(format!("{:010} 00000 n \n", offsets[obj_num]).as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root {catalog_id} 0 R >>\nstartxref\n{xref_start}\n%%EOF",
            objs.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// Serializes with every other real-`PdfiumEngine` test in this workspace
/// (`src/lib.rs`'s own `real_engine_test_lock`, which this integration test
/// binary can't share since it's a separate compilation unit/process) —
/// same underlying constraint (only one live `Pdfium` binding per process),
/// enforced here via `--test-threads=1`-equivalent scoping: this file has
/// exactly one `#[test]`, so there is nothing else in this binary to race
/// against it.
#[test]
fn render_and_tile_timing_against_a_large_vector_dense_pdf() {
    let lib_path = spike_lib_path();
    if !lib_path.exists() {
        eprintln!("skipping: libpdfium.so not present at {lib_path:?} (see crates/pdf_engine_spike/README.md)");
        return;
    }

    let pdf_bytes = build_synthetic_pdf();
    let tmp_path = std::env::temp_dir().join(format!(
        "pdf_core_view04_synthetic_{}.pdf",
        std::process::id()
    ));
    std::fs::write(&tmp_path, &pdf_bytes).expect("write synthetic PDF to temp file");
    println!(
        "synthetic fixture: {} pages, ~{SEGMENTS_PER_PAGE} line segments + ~{LABELS_PER_PAGE} text labels/page, {:.1} MB",
        N_PAGES,
        pdf_bytes.len() as f64 / 1e6
    );

    let engine = PdfiumEngine::new(&lib_path).expect("engine should initialize");

    let open_start = Instant::now();
    let document = engine.open(&tmp_path, None).expect("synthetic PDF should open");
    let open_elapsed = open_start.elapsed();
    println!("open: {open_elapsed:?}");
    assert_eq!(document.page_count(), N_PAGES);
    assert!(open_elapsed.as_secs() < 30, "open took implausibly long: {open_elapsed:?}");

    // Full-page render timing, sampled across first/middle/last page rather
    // than every page (keeps this test fast; content density is uniform by
    // construction, so a sample is representative).
    //
    // Bound is generous (debug-build tolerant, not a real perf target): a
    // first pass at this test measured ~50x faster rendering under
    // `cargo test --release` than plain `cargo test` (e.g. 85ms vs 2.6s for
    // page 15) — the `image` crate's PNG encode/decode path is apparently
    // *far* slower unoptimized. This crate's own tests always run under
    // plain `cargo test` elsewhere in this workspace, so the bound here has
    // to tolerate that, not the release number — see this test's own
    // module doc and `docs/00_SCOPE_REALITY_CHECK.md` for the real
    // (release-mode) numbers this measured.
    for &page_index in &[0usize, N_PAGES / 2, N_PAGES - 1] {
        let render_start = Instant::now();
        let png = document
            .render_page_to_png(page_index, 1600)
            .expect("page should render");
        let elapsed = render_start.elapsed();
        println!("render page {page_index} @ 1600px wide: {elapsed:?}, {} bytes", png.len());
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
        assert!(
            elapsed.as_secs() < 30,
            "rendering page {page_index} took implausibly long: {elapsed:?}"
        );
    }

    // Tile timing at 2x zoom, 512px tiles: compares the sum of tile-render
    // time against one equivalent full-page render, to give real numbers
    // behind the crate doc's existing "re-rasterizes the whole page per
    // tile" caveat rather than leaving it as an unverified claim.
    let zoomed_width = 3200u32;
    let (page_w, page_h) = document.page_size(0).unwrap();
    let zoomed_height = (zoomed_width as f32 * page_h / page_w) as u32;
    let tile_size = 512u32;
    let (cols, rows) = pdf_core::tile_grid(zoomed_width, zoomed_height, tile_size);
    println!("tile grid at {zoomed_width}x{zoomed_height} (2x), {tile_size}px tiles: {cols}x{rows} = {} tiles", cols * rows);

    // Sampled at 2x2 (not the full 7x5 grid): each tile currently
    // re-renders the *entire* zoomed page then crops (the documented,
    // not-yet-fixed limitation on `render_tile_to_png`'s default impl), so
    // tile count directly multiplies test runtime — 16 tiles took over 3
    // minutes under a debug build the first time this ran. 4 is enough to
    // get a real avg/tile number and exercise the cache without that cost.
    let mut cache = TileCache::new(64);
    let tile_start = Instant::now();
    let mut tiles_rendered = 0u32;
    for ty in 0..rows.min(2) {
        for tx in 0..cols.min(2) {
            let key = TileKey {
                page_index: 0,
                zoomed_width,
                tile_x: tx,
                tile_y: ty,
            };
            cache.get_or_render(&document, key, tile_size).expect("tile should render");
            tiles_rendered += 1;
        }
    }
    let tile_elapsed = tile_start.elapsed();
    let avg_per_tile = tile_elapsed / tiles_rendered.max(1);
    println!(
        "rendered {tiles_rendered} tiles (uncached, via TileCache::get_or_render): {tile_elapsed:?} total, {avg_per_tile:?} avg/tile"
    );
    // Bound is per-tile average, debug-build tolerant (see the full-page
    // render bound's comment above for why) — release-mode measured
    // ~250ms/tile; debug-mode measured ~13s/tile. 30s/tile leaves margin
    // over the debug number while still catching a genuine regression.
    assert!(
        avg_per_tile.as_secs() < 30,
        "average tile render time was implausibly long: {avg_per_tile:?}"
    );

    // Re-request the same tiles: should be a cache hit, effectively free,
    // confirming TileCache actually avoids re-render rather than just
    // storing bytes nothing reads back.
    let cached_start = Instant::now();
    for ty in 0..rows.min(2) {
        for tx in 0..cols.min(2) {
            let key = TileKey {
                page_index: 0,
                zoomed_width,
                tile_x: tx,
                tile_y: ty,
            };
            cache
                .get_or_render(&document, key, tile_size)
                .expect("cached tile should still render/return");
        }
    }
    let cached_elapsed = cached_start.elapsed();
    println!("re-requested same {tiles_rendered} tiles (should be cache hits): {cached_elapsed:?} total");
    assert!(
        cached_elapsed < tile_elapsed / 4,
        "cache hits ({cached_elapsed:?}) were not meaningfully faster than the original uncached render ({tile_elapsed:?}) — TileCache may not be caching correctly"
    );

    let _ = std::fs::remove_file(&tmp_path);
}
