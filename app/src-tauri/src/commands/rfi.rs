//! RFI-01/02 IPC commands, backed by `crates/rfi`.

use crate::dto::RfiDto;
use crate::AppState;
use rfi::RfiStatus;

#[tauri::command]
#[tracing::instrument(skip(state), err)]
#[allow(clippy::too_many_arguments)]
pub fn create_rfi(
    state: tauri::State<AppState>,
    document_id: String,
    page_id: Option<String>,
    markup_id: Option<String>,
    title: String,
    description: Option<String>,
    created_by: Option<String>,
) -> Result<RfiDto, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_document_access(&conn, &state, &document_id)?;
    rfi::create(
        &mut conn,
        &document_id,
        page_id.as_deref(),
        markup_id.as_deref(),
        &title,
        description.as_deref(),
        created_by.as_deref(),
    )
    .map(Into::into)
    .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn get_rfi(state: tauri::State<AppState>, id: String) -> Result<RfiDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_rfi_access(&conn, &state, &id)?;
    rfi::get(&conn, &id).map(Into::into).map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn list_rfis_for_document(state: tauri::State<AppState>, document_id: String) -> Result<Vec<RfiDto>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_document_access(&conn, &state, &document_id)?;
    rfi::list_by_document(&conn, &document_id)
        .map(|rfis| rfis.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn set_rfi_status(
    state: tauri::State<AppState>,
    id: String,
    status: RfiStatus,
    response: Option<String>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_rfi_access(&conn, &state, &id)?;
    rfi::set_status(&conn, &id, status, response.as_deref()).map_err(|e| e.to_string())
}
