//! REL-01/02 IPC commands, backed by `crates/recovery`. See that crate's
//! module doc for the design decision behind what a "snapshot" is here
//! (a flattened PDF copy, reusing EXPORT-01's `export_document_
//! flattened_pdf`) and why "restore" hands the user a copy of the file
//! rather than mutating any live Markup/Measurement rows.

use crate::commands::pdf::export_document_flattened_pdf;
use crate::dto::RecoverySnapshotDto;
use crate::AppState;

/// Snapshots kept per document before `autosave_snapshot` starts pruning
/// the oldest ones — an autosave that never prunes fills the disk.
const MAX_SNAPSHOTS_PER_DOCUMENT: usize = 5;

/// REL-01: renders a flattened snapshot of `document_id`'s current state
/// and records it. Intended to be called on an interval by the frontend
/// while a document is open, not on every single edit — that's what
/// SQLite already durably persisting each edit is for (see the crate doc);
/// this is a coarser safety net on top of that.
#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn autosave_snapshot(state: tauri::State<AppState>, document_id: String) -> Result<RecoverySnapshotDto, String> {
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        crate::authz::require_document_access(&conn, &state, &document_id)?;
    }
    let snapshot_dir = state.data_dir.join("recovery");
    std::fs::create_dir_all(&snapshot_dir).map_err(|e| e.to_string())?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis();
    let snapshot_path = snapshot_dir.join(format!("{document_id}-{timestamp}.pdf"));
    let snapshot_path_str = snapshot_path.to_str().ok_or("snapshot path is not valid UTF-8")?;

    export_document_flattened_pdf(&state, &document_id, snapshot_path_str)?;

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let created = recovery::create(&conn, &document_id, snapshot_path_str).map_err(|e| e.to_string())?;

    for pruned in recovery::prune_oldest(&conn, &document_id, MAX_SNAPSHOTS_PER_DOCUMENT).map_err(|e| e.to_string())? {
        let _ = std::fs::remove_file(&pruned.snapshot_path);
    }

    Ok(created.into())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn list_recovery_snapshots(state: tauri::State<AppState>, document_id: String) -> Result<Vec<RecoverySnapshotDto>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_document_access(&conn, &state, &document_id)?;
    recovery::list_by_document(&conn, &document_id)
        .map(|snapshots| snapshots.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

/// REL-02 (restore): copies the snapshot file to `output_path` (from a
/// save dialog on the frontend) rather than touching any live data — see
/// the crate doc for why.
#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn restore_recovery_snapshot(state: tauri::State<AppState>, id: String, output_path: String) -> Result<(), String> {
    let snapshot_path = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        crate::authz::require_recovery_snapshot_access(&conn, &state, &id)?;
        recovery::get(&conn, &id).map_err(|e| e.to_string())?.snapshot_path
    };
    std::fs::copy(&snapshot_path, &output_path).map_err(|e| e.to_string())?;
    Ok(())
}

/// REL-02 (discard): removes the recovery row and its file.
#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn discard_recovery_snapshot(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_recovery_snapshot_access(&conn, &state, &id)?;
    let deleted = recovery::delete(&conn, &id).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&deleted.snapshot_path);
    Ok(())
}
