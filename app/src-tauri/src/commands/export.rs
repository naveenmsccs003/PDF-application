//! EXPORT-03: bundles EXPORT-01's flattened PDF with TAKE-05's takeoff CSV
//! into one output directory — "drawings + markups + takeoff together" is
//! the master list's own wording for this feature. Two sibling files in a
//! folder the user picks, not a zip archive: simpler to produce, and
//! doesn't require a new dependency just to open one file out of the
//! bundle (an actual zip is easy to add later if a real handoff workflow
//! turns out to need one).

use crate::commands::pdf::export_document_flattened_pdf;
use crate::AppState;
use std::path::PathBuf;

fn sanitize_filename_component(s: &str) -> String {
    s.chars()
        .map(|c| if matches!(c, '/' | '\\' | ':') { '_' } else { c })
        .collect()
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn export_handoff_package(
    state: tauri::State<AppState>,
    document_id: String,
    output_dir: String,
) -> Result<(), String> {
    let title = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        crate::authz::require_document_access(&conn, &state, &document_id)?;
        document::get_document(&conn, &document_id).map_err(|e| e.to_string())?.title
    };
    let name = sanitize_filename_component(&title);
    let dir = PathBuf::from(&output_dir);

    let pdf_path = dir.join(format!("{name}-flattened.pdf"));
    let pdf_path = pdf_path.to_str().ok_or("output directory path is not valid UTF-8")?;
    export_document_flattened_pdf(&state, &document_id, pdf_path)?;

    let items = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        takeoff::list_for_document(&conn, &document_id).map_err(|e| e.to_string())?
    };
    let csv = takeoff::export_csv(&items);
    let csv_path = dir.join(format!("{name}-takeoff.csv"));
    std::fs::write(&csv_path, csv).map_err(|e| e.to_string())?;

    Ok(())
}

/// EXPORT-02 (print): this app has no in-webview render of the actual
/// drawing content good enough to print from directly (the canvas is a
/// markup-editing surface, not a print-quality page render), and no
/// native print dialog of its own — so "print" means producing an
/// up-to-date flattened snapshot (current markups included, same as
/// EXPORT-01) and handing it to the OS's default PDF viewer, the same way
/// RFI-03's "Open…" on a past revision does. That viewer's own Print
/// command is the actual print dialog (page range, printer, paper size),
/// which this app doesn't need to reimplement. Writes to the system temp
/// directory rather than `data_dir` — a print snapshot is disposable, not
/// something meant to be kept like a `RecoveryState`/`DocumentVersion`
/// snapshot. Returns the path so the frontend can hand it to
/// `openPath` (`@tauri-apps/plugin-opener`) itself.
#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn print_document(state: tauri::State<AppState>, document_id: String) -> Result<String, String> {
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        crate::authz::require_document_access(&conn, &state, &document_id)?;
    }
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis();
    let print_path = std::env::temp_dir().join(format!("mds-rebar-print-{document_id}-{timestamp}.pdf"));
    let print_path_str = print_path.to_str().ok_or("temp file path is not valid UTF-8")?;

    export_document_flattened_pdf(&state, &document_id, print_path_str)?;

    Ok(print_path_str.to_string())
}
