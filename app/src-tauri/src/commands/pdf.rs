//! DOC-01 (open) + a thumbnail render, wiring `pdf_core`'s PDFium binding
//! into the app.
//!
//! `pdf_core::PdfiumEngine` isn't `Send` (its `Pdfium` bindings hold a
//! `Box<dyn PdfiumLibraryBindings>` trait object, which isn't `Send`
//! either) — confirmed by the compiler, not assumed — so it cannot live in
//! Tauri's `State<AppState>` directly, which requires `Send + Sync` since
//! commands can run on different threads. Combined with the Section 6
//! spike's finding that a second `Pdfium` binding in the same process
//! deadlocks (see `crates/pdf_engine_spike/README.md`), the only sound
//! design is: construct the engine exactly once, on one dedicated thread,
//! and never touch it from anywhere else. Commands talk to that thread
//! over a channel — only plain `Send` data (`String`, `Vec<(f32,f32)>`,
//! `Vec<u8>`) ever crosses the boundary, never the engine or a document
//! borrowing from it.
//!
//! Uses a dev-only `libpdfium.so` path (see `dev_pdfium_lib_path` below) —
//! real distribution needs to bundle this binary with the installed app
//! (a Tauri resource, or platform-specific packaging) and resolve it via
//! `app.path().resource_dir()` instead; that decision hasn't been made
//! yet, so this only works when run from this workspace checkout.

use crate::dto::DocumentDto;
use crate::AppState;
use base64::Engine as _;
use pdf_core::{PdfCoreError, PdfDocument, PdfEngine};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

pub fn dev_pdfium_lib_path() -> PathBuf {
    // app/src-tauri -> app -> pdf_application, then into the spike crate's
    // fetched binary (see crates/pdf_engine_spike/README.md). Filename is
    // platform-dependent (libpdfium.so / .dylib / pdfium.dll) — was
    // hardcoded to `.so` until `pdf_core::platform_library_filename()`
    // existed, which meant this only ever worked on Linux.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("app/src-tauri has a parent")
        .parent()
        .expect("app has a parent")
        .join("crates/pdf_engine_spike/lib")
        .join(pdf_core::platform_library_filename())
}

pub enum PdfEngineRequest {
    GetPageSizes {
        path: String,
        /// SEC-01: `None` for the overwhelming majority of PDFs — only
        /// set when the caller already knows (or is guessing) this PDF is
        /// encrypted.
        password: Option<String>,
        reply: mpsc::Sender<Result<Vec<(f32, f32)>, String>>,
    },
    RenderThumbnail {
        path: String,
        password: Option<String>,
        page_index: usize,
        width: u32,
        reply: mpsc::Sender<Result<Vec<u8>, String>>,
    },
    /// EXPORT-01: `markup::Markup` is plain data (`String`/enum/`Vec<(f64,
    /// f64)>`/`Option<String>`/`bool` fields, no PDFium handles), so it's
    /// `Send` and can cross this channel like every other request payload
    /// here — only the actual `PdfiumDocument` (built and dropped entirely
    /// on this thread, inside `export::export_flattened_pdf`) never does.
    ExportFlattened {
        path: String,
        password: Option<String>,
        output_path: String,
        markups_by_page_index: std::collections::HashMap<usize, Vec<markup::Markup>>,
        reply: mpsc::Sender<Result<(), String>>,
    },
}

/// Spawns the one thread that owns the one `PdfiumEngine` for the process.
/// Returns `None` if `libpdfium.so` isn't found or fails to initialize —
/// the rest of the app still starts, and PDF-specific commands fail with a
/// clear error instead of the whole app refusing to launch.
pub fn spawn_pdf_engine_thread() -> Option<mpsc::Sender<PdfEngineRequest>> {
    let lib_path = dev_pdfium_lib_path();
    if !lib_path.exists() {
        tracing::warn!(
            path = %lib_path.display(),
            "libpdfium.so not found; PDF import/render commands will be unavailable this session (see crates/pdf_engine_spike/README.md)"
        );
        return None;
    }

    let (tx, rx) = mpsc::channel::<PdfEngineRequest>();
    let (ready_tx, ready_rx) = mpsc::channel::<bool>();

    thread::spawn(move || {
        let engine = match pdf_core::PdfiumEngine::new(&lib_path) {
            Ok(engine) => engine,
            Err(e) => {
                tracing::error!(error = %e, "PDF engine failed to initialize");
                let _ = ready_tx.send(false);
                return;
            }
        };
        let _ = ready_tx.send(true);

        for request in rx {
            match request {
                PdfEngineRequest::GetPageSizes { path, password, reply } => {
                    let result = (|| -> Result<Vec<(f32, f32)>, String> {
                        let document = engine.open(Path::new(&path), password.as_deref()).map_err(|e| e.to_string())?;
                        (0..document.page_count())
                            .map(|i| document.page_size(i).map_err(|e| e.to_string()))
                            .collect()
                    })();
                    let _ = reply.send(result);
                }
                PdfEngineRequest::RenderThumbnail { path, password, page_index, width, reply } => {
                    let result = (|| -> Result<Vec<u8>, String> {
                        let document = engine.open(Path::new(&path), password.as_deref()).map_err(|e| e.to_string())?;
                        document.render_thumbnail_png(page_index, width).map_err(|e| e.to_string())
                    })();
                    let _ = reply.send(result);
                }
                PdfEngineRequest::ExportFlattened { path, password, output_path, markups_by_page_index, reply } => {
                    let result = (|| -> Result<(), String> {
                        let mut document =
                            engine.open(Path::new(&path), password.as_deref()).map_err(|e| e.to_string())?;
                        export::export_flattened_pdf(&mut document, &markups_by_page_index, Path::new(&output_path))
                            .map_err(|e| e.to_string())
                    })();
                    let _ = reply.send(result);
                }
            }
        }
    });

    if ready_rx.recv().unwrap_or(false) {
        Some(tx)
    } else {
        None
    }
}

fn get_page_sizes(state: &AppState, path: &str, password: Option<&str>) -> Result<Vec<(f32, f32)>, String> {
    let tx = state.pdf_engine.as_ref().ok_or("PDF engine unavailable (libpdfium.so not found)")?;
    let (reply_tx, reply_rx) = mpsc::channel();
    tx.lock()
        .map_err(|e| e.to_string())?
        .send(PdfEngineRequest::GetPageSizes {
            path: path.to_string(),
            password: password.map(str::to_string),
            reply: reply_tx,
        })
        .map_err(|_| "PDF engine thread is not running".to_string())?;
    reply_rx.recv().map_err(|_| "PDF engine thread dropped the reply channel".to_string())?
}

fn render_thumbnail(
    state: &AppState,
    path: &str,
    password: Option<&str>,
    page_index: usize,
    width: u32,
) -> Result<Vec<u8>, String> {
    let tx = state.pdf_engine.as_ref().ok_or("PDF engine unavailable (libpdfium.so not found)")?;
    let (reply_tx, reply_rx) = mpsc::channel();
    tx.lock()
        .map_err(|e| e.to_string())?
        .send(PdfEngineRequest::RenderThumbnail {
            path: path.to_string(),
            password: password.map(str::to_string),
            page_index,
            width,
            reply: reply_tx,
        })
        .map_err(|_| "PDF engine thread is not running".to_string())?;
    reply_rx.recv().map_err(|_| "PDF engine thread dropped the reply channel".to_string())?
}

fn export_flattened(
    state: &AppState,
    path: &str,
    password: Option<&str>,
    output_path: &str,
    markups_by_page_index: std::collections::HashMap<usize, Vec<markup::Markup>>,
) -> Result<(), String> {
    let tx = state.pdf_engine.as_ref().ok_or("PDF engine unavailable (libpdfium.so not found)")?;
    let (reply_tx, reply_rx) = mpsc::channel();
    tx.lock()
        .map_err(|e| e.to_string())?
        .send(PdfEngineRequest::ExportFlattened {
            path: path.to_string(),
            password: password.map(str::to_string),
            output_path: output_path.to_string(),
            markups_by_page_index,
            reply: reply_tx,
        })
        .map_err(|_| "PDF engine thread is not running".to_string())?;
    reply_rx.recv().map_err(|_| "PDF engine thread dropped the reply channel".to_string())?
}

/// A `PdfDocument` whose only real data is page sizes already fetched from
/// the dedicated PDF-engine thread — lets `document::import_document`
/// (generic over any `PdfDocument`) run on the calling command's own
/// thread against real page metadata, without ever moving the actual
/// PDFium document (or engine) across a thread boundary. Same shape as the
/// `FakePdfDocument` test doubles in `crates/document` and
/// `crates/e2e_tests` — this is that same pattern used for real data
/// instead of test fixtures.
struct PageSizesDocument {
    sizes: Vec<(f32, f32)>,
}

impl PdfDocument for PageSizesDocument {
    fn page_count(&self) -> usize {
        self.sizes.len()
    }

    fn page_size(&self, page_index: usize) -> Result<(f32, f32), PdfCoreError> {
        self.sizes.get(page_index).copied().ok_or(PdfCoreError::PageIndexOutOfRange(page_index))
    }

    fn render_page_to_png(&self, _page_index: usize, _target_width: u32) -> Result<Vec<u8>, PdfCoreError> {
        Ok(Vec::new()) // not used by import_document
    }

    fn extract_text(&self, _page_index: usize) -> Result<String, PdfCoreError> {
        Ok(String::new())
    }

    fn save(&self, _path: &Path) -> Result<(), PdfCoreError> {
        Ok(())
    }
}

/// Looks up any password stored for `document_id`, treating *any* keychain
/// failure — not just "nothing stored" — as "proceed without one". A
/// document that genuinely needs a password still fails informatively
/// with `PdfCoreError::PasswordRequired` when `PdfEngine::open` is tried
/// without it; the alternative (propagating a keychain connectivity
/// error via `?`) would fail thumbnail rendering/export/print/autosave
/// for every document, including unencrypted ones, whenever the OS
/// keychain itself is unreachable (e.g. no Secret Service running).
fn stored_pdf_password(document_id: &str) -> Option<String> {
    match secrets::get_pdf_password(document_id) {
        Ok(password) => password,
        Err(e) => {
            tracing::warn!(document_id, error = %e, "keychain lookup failed, proceeding without a password");
            None
        }
    }
}

/// SEC-01: `password` is only needed for an encrypted PDF (see
/// `pdf_core::PdfCoreError::PasswordRequired`). The frontend's import form
/// has its own password field the user can pre-fill if they already know
/// a PDF is encrypted; there is currently no automatic "detect
/// PasswordRequired and re-prompt" retry flow on that error — the error
/// message alone surfaces in the existing error banner. SEC-02: on
/// success, a supplied password is stored
/// via `secrets::store_pdf_password` keyed by the new document's id, so
/// `render_page_thumbnail`/exports don't need it passed in again every
/// call — see `commands::recovery`'s module doc for why that matters
/// (autosave runs on a timer, not on explicit user action, so there's no
/// natural place to re-prompt for a password there). A failure to store
/// the password is logged, not propagated — the import itself already
/// succeeded, and failing the whole operation over a credential-store
/// hiccup would be a worse outcome than just re-prompting next time.
#[tauri::command]
#[tracing::instrument(skip(state, password), err)]
pub fn import_pdf_document(
    state: tauri::State<AppState>,
    path: String,
    title: String,
    project_id: Option<String>,
    password: Option<String>,
) -> Result<DocumentDto, String> {
    if let Some(project_id) = project_id.as_deref() {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        crate::authz::require_project_membership(&conn, &state, project_id)?;
    }
    let sizes = get_page_sizes(&state, &path, password.as_deref())?;
    let fake_document = PageSizesDocument { sizes };

    let document = {
        let mut conn = state.db.lock().map_err(|e| e.to_string())?;
        document::import_document(&mut conn, &fake_document, &path, &title, project_id.as_deref())
            .map(DocumentDto::from)
            .map_err(|e| e.to_string())?
    };

    if let Some(password) = password.as_deref() {
        if let Err(e) = secrets::store_pdf_password(&document.id, password) {
            tracing::error!(document_id = %document.id, error = %e, "failed to store PDF password in the OS keychain");
        }
    }

    Ok(document)
}

/// Renders one page as a small thumbnail, returned as a `data:` URI so the
/// frontend can drop it straight into an `<img src>` with no extra IPC
/// round trip to fetch bytes separately.
#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn render_page_thumbnail(
    state: tauri::State<AppState>,
    document_id: String,
    page_number: i64,
    width: u32,
) -> Result<String, String> {
    let file_path = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        crate::authz::require_document_access(&conn, &state, &document_id)?;
        document::get_document(&conn, &document_id).map_err(|e| e.to_string())?.file_path
    };
    let password = stored_pdf_password(&document_id);
    let page_index = (page_number - 1).max(0) as usize;
    let png_bytes = render_thumbnail(&state, &file_path, password.as_deref(), page_index, width)?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(png_bytes);
    Ok(format!("data:image/png;base64,{encoded}"))
}

/// SEC-01/02: associates `password` with `document_id` for future
/// renders/exports (via the OS keychain, see `crates/secrets`) without
/// re-importing. Useful if the password wasn't known/entered at import
/// time, or needs to be corrected.
#[tauri::command]
#[tracing::instrument(skip(state, password), err)]
pub fn set_document_pdf_password(state: tauri::State<AppState>, document_id: String, password: String) -> Result<(), String> {
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        crate::authz::require_document_access(&conn, &state, &document_id)?;
    }
    secrets::store_pdf_password(&document_id, &password).map_err(|e| e.to_string())
}

/// SEC-02: forgets any password stored for `document_id`.
#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn clear_document_pdf_password(state: tauri::State<AppState>, document_id: String) -> Result<(), String> {
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        crate::authz::require_document_access(&conn, &state, &document_id)?;
    }
    secrets::delete_pdf_password(&document_id).map_err(|e| e.to_string())
}

/// Does the actual work behind `export_flattened_pdf` below — pulled out
/// as a plain function (not a `#[tauri::command]`) so `commands::export`
/// can also call it as one step of EXPORT-03's handoff package, without
/// going through IPC a second time.
pub(crate) fn export_document_flattened_pdf(
    state: &AppState,
    document_id: &str,
    output_path: &str,
) -> Result<(), String> {
    let (file_path, pages) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let doc = document::get_document(&conn, document_id).map_err(|e| e.to_string())?;
        let pages = document::list_pages(&conn, document_id).map_err(|e| e.to_string())?;
        (doc.file_path, pages)
    };

    let mut markups_by_page_index = std::collections::HashMap::new();
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        for page in &pages {
            let markups = markup::list_by_page(&conn, &page.id).map_err(|e| e.to_string())?;
            let page_index = (page.page_number - 1).max(0) as usize;
            markups_by_page_index.insert(page_index, markups);
        }
    }

    let password = stored_pdf_password(document_id);
    export_flattened(state, &file_path, password.as_deref(), output_path, markups_by_page_index)
}

/// EXPORT-01: burns every page's non-hidden markups into a flattened copy
/// of the document's PDF, saved to `output_path` (the frontend gets that
/// path from a save dialog rather than this command inventing one, same
/// division of responsibility as `import_pdf_document` taking an
/// already-chosen `path` from an open dialog).
#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn export_flattened_pdf(
    state: tauri::State<AppState>,
    document_id: String,
    output_path: String,
) -> Result<(), String> {
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        crate::authz::require_document_access(&conn, &state, &document_id)?;
    }
    export_document_flattened_pdf(&state, &document_id, &output_path)
}

/// RFI-04: renders the same page from two saved revisions (DOC-02/RFI-03's
/// `DocumentVersion` snapshots) and returns a pixel-diff overlay (see
/// `pdf_core::render_comparison_overlay`) as a `data:` URI, same
/// no-extra-round-trip reasoning as `render_page_thumbnail`. Each
/// snapshot is a flattened copy already produced without a password (see
/// `export_document_flattened_pdf`'s callers), so unlike importing the
/// original source PDF there's no password to look up or pass through
/// here.
#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn compare_document_versions(
    state: tauri::State<AppState>,
    version_a_id: String,
    version_b_id: String,
    page_number: i64,
    width: u32,
) -> Result<String, String> {
    let (path_a, path_b) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let version_a = document::get_version(&conn, &version_a_id).map_err(|e| e.to_string())?;
        let version_b = document::get_version(&conn, &version_b_id).map_err(|e| e.to_string())?;
        if version_a.document_id != version_b.document_id {
            return Err("cannot compare versions belonging to two different documents".to_string());
        }
        crate::authz::require_document_access(&conn, &state, &version_a.document_id)?;
        (version_a.file_snapshot_path, version_b.file_snapshot_path)
    };

    let page_index = (page_number - 1).max(0) as usize;
    let png_a = render_thumbnail(&state, &path_a, None, page_index, width)?;
    let png_b = render_thumbnail(&state, &path_b, None, page_index, width)?;
    let overlay_png = pdf_core::render_comparison_overlay(&png_a, &png_b).map_err(|e| e.to_string())?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(overlay_png);
    Ok(format!("data:image/png;base64,{encoded}"))
}
