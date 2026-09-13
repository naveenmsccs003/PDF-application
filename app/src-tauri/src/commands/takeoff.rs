//! TAKE-01–05 IPC commands, backed by `crates/takeoff`.

use crate::dto::TakeoffItemDto;
use crate::AppState;

#[tauri::command]
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
pub fn get_takeoff_item(state: tauri::State<AppState>, id: String) -> Result<TakeoffItemDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    takeoff::get(&conn, &id).map(Into::into).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_takeoff_for_measurement(state: tauri::State<AppState>, measurement_id: String) -> Result<Vec<TakeoffItemDto>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    takeoff::list_for_measurement(&conn, &measurement_id)
        .map(|items| items.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_takeoff_for_document(state: tauri::State<AppState>, document_id: String) -> Result<Vec<TakeoffItemDto>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    takeoff::list_for_document(&conn, &document_id)
        .map(|items| items.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_takeoff_item(
    state: tauri::State<AppState>,
    id: String,
    quantity: f64,
    unit: String,
    cost_per_unit: Option<f64>,
    notes: Option<String>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    takeoff::update(&conn, &id, quantity, &unit, cost_per_unit, notes.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_takeoff_item(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    takeoff::delete(&conn, &id).map_err(|e| e.to_string())
}

/// TAKE-05: CSV export, scoped to a document (see `takeoff::list_for_document`
/// for why that's the natural scope — it joins through
/// `measurement -> page -> document`).
#[tauri::command]
pub fn export_takeoff_csv(state: tauri::State<AppState>, document_id: String) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let items = takeoff::list_for_document(&conn, &document_id).map_err(|e| e.to_string())?;
    Ok(takeoff::export_csv(&items))
}
