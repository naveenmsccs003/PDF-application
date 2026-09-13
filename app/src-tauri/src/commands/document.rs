//! DOC-02/04/05 IPC commands, backed by `crates/document`. DOC-01 (open —
//! `document::import_document`) isn't wired here: it needs a
//! `pdf_core::PdfDocument`, and how the installed app locates/bundles
//! `libpdfium.so` at runtime is still an open decision (see the module
//! doc on `document::import_document` and `crates/pdf_engine_spike/README.md`).

use crate::dto::{DocumentDto, DocumentVersionDto, PageDto};
use crate::AppState;

#[tauri::command]
pub fn get_document(state: tauri::State<AppState>, document_id: String) -> Result<DocumentDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    document::get_document(&conn, &document_id)
        .map(Into::into)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_documents_for_project(state: tauri::State<AppState>, project_id: String) -> Result<Vec<DocumentDto>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    document::list_documents_for_project(&conn, &project_id)
        .map(|docs| docs.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_pages(state: tauri::State<AppState>, document_id: String) -> Result<Vec<PageDto>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    document::list_pages(&conn, &document_id)
        .map(|pages| pages.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_page_rotation(state: tauri::State<AppState>, page_id: String, rotation: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    document::set_page_rotation(&conn, &page_id, rotation).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reorder_pages(state: tauri::State<AppState>, document_id: String, new_order: Vec<String>) -> Result<(), String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    document::reorder_pages(&mut conn, &document_id, &new_order).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_document_version(
    state: tauri::State<AppState>,
    document_id: String,
    file_snapshot_path: String,
    created_by: Option<String>,
) -> Result<DocumentVersionDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    document::create_version(&conn, &document_id, &file_snapshot_path, created_by.as_deref())
        .map(Into::into)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_document_versions(state: tauri::State<AppState>, document_id: String) -> Result<Vec<DocumentVersionDto>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    document::list_versions(&conn, &document_id)
        .map(|versions| versions.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}
