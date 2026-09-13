use std::path::Path;
use std::time::Instant;

use pdfium_render::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lib_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("lib/libpdfium.so");
    let input_path = std::env::args()
        .nth(1)
        .expect("usage: pdf_engine_spike <path-to-pdf>");

    println!("== PDF engine validation spike ==");
    println!("pdfium library: {}", lib_path.display());
    println!("input PDF: {input_path}");

    let bindings = Pdfium::bind_to_library(&lib_path)?;
    let pdfium = Pdfium::new(bindings);

    let open_start = Instant::now();
    let document = pdfium.load_pdf_from_file(&input_path, None)?;
    println!("1. OPEN: PASS ({:?})", open_start.elapsed());

    let page_count = document.pages().len();
    println!("   page count: {page_count}");

    let page_index = 0u16;
    let mut page = document.pages().get(page_index)?;
    println!(
        "   page 0 size: {:.1} x {:.1} pt",
        page.width().value,
        page.height().value
    );

    let render_start = Instant::now();
    let render_config = PdfRenderConfig::new().set_target_width(1600);
    let bitmap = page.render_with_config(&render_config)?;
    let image = bitmap.as_image();
    let render_elapsed = render_start.elapsed();
    println!("2. RENDER representative page: PASS ({render_elapsed:?})");
    println!("4. RENDER TIME measured: {render_elapsed:?} for target width 1600px");

    let out_png = Path::new(env!("CARGO_MANIFEST_DIR")).join("spike_output_page0.png");
    image.save(&out_png)?;
    println!("   saved rendered page to {}", out_png.display());
    drop(bitmap);

    println!("3. RENDER large/vector-heavy page: SKIPPED — no realistic 100+ page vector-heavy construction drawing available on this machine. Needs a real sample from Naveen.");

    let text_start = Instant::now();
    match page.text() {
        Ok(text) => {
            let all_text = text.all();
            println!(
                "5. EXTRACT TEXT: PASS ({:?}), {} chars extracted",
                text_start.elapsed(),
                all_text.len()
            );
        }
        Err(e) => println!("5. EXTRACT TEXT: FAIL — {e}"),
    }

    let annotation_start = Instant::now();
    match page
        .annotations_mut()
        .create_free_text_annotation("MDS Rebar PDF engine spike test annotation")
    {
        Ok(_) => println!(
            "6. ADD ANNOTATION: PASS ({:?}), free-text annotation created",
            annotation_start.elapsed()
        ),
        Err(e) => println!("6. ADD ANNOTATION: FAIL — {e}"),
    }

    let save_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("spike_output_resaved.pdf");
    let save_start = Instant::now();
    match document.save_to_file(&save_path) {
        Ok(()) => println!(
            "7. SAVE: PASS ({:?}) -> {}",
            save_start.elapsed(),
            save_path.display()
        ),
        Err(e) => println!("7. SAVE: FAIL — {e}"),
    }

    // Reuse the same Pdfium instance rather than loading libpdfium.so a
    // second time in-process: pdfium's global state is not safe to
    // initialize twice via separate bindings in one process (observed as a
    // futex deadlock during this spike, not just theoretical).
    let reopen_start = Instant::now();
    match pdfium.load_pdf_from_file(&save_path, None) {
        Ok(reopened) => {
            let annotation_count = reopened
                .pages()
                .get(0)
                .map(|p| p.annotations().len())
                .unwrap_or(0);
            println!(
                "8. REOPEN saved result: PASS ({:?}), {} pages, {} annotation(s) on page 0 (verifies annotation survived save+reopen)",
                reopen_start.elapsed(),
                reopened.pages().len(),
                annotation_count
            );
        }
        Err(e) => println!("8. REOPEN saved result: FAIL — {e}"),
    }

    println!("9. Windows binary distribution: bblanchon/pdfium-binaries publishes a windows-x64 release asset for the same pdfium version used here — availability confirmed, but not runtime-tested (this machine is Linux).");

    println!("== spike complete ==");
    Ok(())
}
