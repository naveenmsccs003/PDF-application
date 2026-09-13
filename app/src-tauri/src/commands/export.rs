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
pub fn export_handoff_package(
    state: tauri::State<AppState>,
    document_id: String,
    output_dir: String,
) -> Result<(), String> {
    let title = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
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
