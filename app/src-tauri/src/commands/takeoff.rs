//! TAKE-01–05 IPC commands, backed by `crates/takeoff`.

use crate::dto::TakeoffItemDto;
use crate::AppState;

#[tauri::command]
#[tracing::instrument(skip(state), err)]
#[allow(clippy::too_many_arguments)]
pub fn create_takeoff_item(
    state: tauri::State<AppState>,
    description: String,
    quantity: f64,
    unit: String,
    measurement_id: Option<String>,
    cost_per_unit: Option<f64>,
    notes: Option<String>,
) -> Result<TakeoffItemDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    if let Some(measurement_id) = measurement_id.as_deref() {
        crate::authz::require_measurement_access(&conn, &state, measurement_id)?;
    }
    takeoff::create(
        &conn,
        &description,
        quantity,
        &unit,
        measurement_id.as_deref(),
        cost_per_unit,
        notes.as_deref(),
    )
    .map(Into::into)
    .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn get_takeoff_item(state: tauri::State<AppState>, id: String) -> Result<TakeoffItemDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_takeoff_item_access(&conn, &state, &id)?;
    takeoff::get(&conn, &id).map(Into::into).map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn list_takeoff_for_measurement(state: tauri::State<AppState>, measurement_id: String) -> Result<Vec<TakeoffItemDto>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_measurement_access(&conn, &state, &measurement_id)?;
    takeoff::list_for_measurement(&conn, &measurement_id)
        .map(|items| items.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn list_takeoff_for_document(state: tauri::State<AppState>, document_id: String) -> Result<Vec<TakeoffItemDto>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_document_access(&conn, &state, &document_id)?;
    takeoff::list_for_document(&conn, &document_id)
        .map(|items| items.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn update_takeoff_item(
    state: tauri::State<AppState>,
    id: String,
    quantity: f64,
    unit: String,
    cost_per_unit: Option<f64>,
    notes: Option<String>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_takeoff_item_access(&conn, &state, &id)?;
    takeoff::update(&conn, &id, quantity, &unit, cost_per_unit, notes.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn delete_takeoff_item(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_takeoff_item_access(&conn, &state, &id)?;
    takeoff::delete(&conn, &id).map_err(|e| e.to_string())
}

/// TAKE-05: CSV export, scoped to a document (see `takeoff::list_for_document`
/// for why that's the natural scope — it joins through
/// `measurement -> page -> document`).
#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn export_takeoff_csv(state: tauri::State<AppState>, document_id: String) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_document_access(&conn, &state, &document_id)?;
    let items = takeoff::list_for_document(&conn, &document_id).map_err(|e| e.to_string())?;
    Ok(takeoff::export_csv(&items))
}

/// TAKE-05 (Excel half): unlike the CSV row above, this writes straight to
/// a caller-chosen path rather than returning content to preview in a
/// `<pre>` — an `.xlsx` is a zip archive, not text, so there's nothing
/// sensible to display inline; the frontend gets the path from a save
/// dialog first, same pattern as `export_flattened_pdf`.
#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn export_takeoff_xlsx(
    state: tauri::State<AppState>,
    document_id: String,
    output_path: String,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_document_access(&conn, &state, &document_id)?;
    let items = takeoff::list_for_document(&conn, &document_id).map_err(|e| e.to_string())?;
    let bytes = takeoff::export_xlsx(&items).map_err(|e| e.to_string())?;
    std::fs::write(&output_path, bytes).map_err(|e| e.to_string())
}
