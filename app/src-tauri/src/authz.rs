//! SEC-03 enforcement: the app-layer half of `crates/project`'s membership
//! check — that crate stores membership and can answer "is this user a
//! member," but (per `docs/features/FEATURE_REGISTRY.md`) had nothing to
//! enforce it against an actual action until now. Every command whose
//! target row eventually belongs to a `document` (directly, or via
//! `page`/`markup`/`measurement`/`rfi`/`recovery_state`/`takeoff_item`)
//! calls one of the `require_*` functions below before touching the
//! database. A project-less document or an id that doesn't resolve at all
//! isn't restricted — see `project::user_can_access_via_document`'s doc
//! comment for why an unresolved chain allows through rather than denying.
//!
//! "Current user" here is one slot in `AppState`, not a session token —
//! this schema has no session concept (SEC-01/02 are about PDF/credential
//! passwords, not an app-wide login), and one Tauri process is already one
//! desktop session for one person at a time, the same reasoning
//! `commands::markup`'s module doc gives for scoping `UndoStack` per page
//! rather than inventing per-user keys nothing else in this app has.

use crate::AppState;
use rusqlite::Connection;

pub fn current_user(state: &AppState) -> Result<String, String> {
    state
        .current_user
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or_else(|| "not signed in".to_string())
}

pub fn set_current_user(state: &AppState, user_id: &str) -> Result<(), String> {
    *state.current_user.lock().map_err(|e| e.to_string())? = Some(user_id.to_string());
    Ok(())
}

pub fn clear_current_user(state: &AppState) -> Result<(), String> {
    *state.current_user.lock().map_err(|e| e.to_string())? = None;
    Ok(())
}

fn deny_unless(allowed: bool) -> Result<(), String> {
    if allowed {
        Ok(())
    } else {
        Err("not a member of the project this belongs to".to_string())
    }
}

pub fn require_document_access(conn: &Connection, state: &AppState, document_id: &str) -> Result<(), String> {
    let user_id = current_user(state)?;
    let allowed = project::user_can_access_document(conn, document_id, &user_id).map_err(|e| e.to_string())?;
    deny_unless(allowed)
}

pub fn require_page_access(conn: &Connection, state: &AppState, page_id: &str) -> Result<(), String> {
    let user_id = current_user(state)?;
    let doc_id = project::document_id_for_page(conn, page_id).map_err(|e| e.to_string())?;
    deny_unless(project::user_can_access_via_document(conn, doc_id, &user_id).map_err(|e| e.to_string())?)
}

pub fn require_markup_access(conn: &Connection, state: &AppState, markup_id: &str) -> Result<(), String> {
    let user_id = current_user(state)?;
    let doc_id = project::document_id_for_markup(conn, markup_id).map_err(|e| e.to_string())?;
    deny_unless(project::user_can_access_via_document(conn, doc_id, &user_id).map_err(|e| e.to_string())?)
}

pub fn require_markup_comment_access(conn: &Connection, state: &AppState, comment_id: &str) -> Result<(), String> {
    let user_id = current_user(state)?;
    let doc_id = project::document_id_for_markup_comment(conn, comment_id).map_err(|e| e.to_string())?;
    deny_unless(project::user_can_access_via_document(conn, doc_id, &user_id).map_err(|e| e.to_string())?)
}

pub fn require_measurement_access(conn: &Connection, state: &AppState, measurement_id: &str) -> Result<(), String> {
    let user_id = current_user(state)?;
    let doc_id = project::document_id_for_measurement(conn, measurement_id).map_err(|e| e.to_string())?;
    deny_unless(project::user_can_access_via_document(conn, doc_id, &user_id).map_err(|e| e.to_string())?)
}

pub fn require_rfi_access(conn: &Connection, state: &AppState, rfi_id: &str) -> Result<(), String> {
    let user_id = current_user(state)?;
    let doc_id = project::document_id_for_rfi(conn, rfi_id).map_err(|e| e.to_string())?;
    deny_unless(project::user_can_access_via_document(conn, doc_id, &user_id).map_err(|e| e.to_string())?)
}

pub fn require_recovery_snapshot_access(conn: &Connection, state: &AppState, snapshot_id: &str) -> Result<(), String> {
    let user_id = current_user(state)?;
    let doc_id = project::document_id_for_recovery_snapshot(conn, snapshot_id).map_err(|e| e.to_string())?;
    deny_unless(project::user_can_access_via_document(conn, doc_id, &user_id).map_err(|e| e.to_string())?)
}

/// TAKE-02's `measurement_id` is optional; an item with none has no
/// reachable document/project (see `project::document_id_for_takeoff_item`)
/// and so isn't restricted, same as a project-less document.
pub fn require_takeoff_item_access(conn: &Connection, state: &AppState, item_id: &str) -> Result<(), String> {
    let user_id = current_user(state)?;
    let doc_id = project::document_id_for_takeoff_item(conn, item_id).map_err(|e| e.to_string())?;
    deny_unless(project::user_can_access_via_document(conn, doc_id, &user_id).map_err(|e| e.to_string())?)
}

pub fn require_project_membership(conn: &Connection, state: &AppState, project_id: &str) -> Result<(), String> {
    let user_id = current_user(state)?;
    let allowed = project::is_member(conn, project_id, &user_id).map_err(|e| e.to_string())?;
    deny_unless(allowed)
}

/// COLLAB-01: a user may only act "as" themself (create a project under
/// their own id, list their own project list) — the one check that makes
/// sense before any project/document id exists to check membership on.
pub fn require_self(state: &AppState, user_id: &str) -> Result<(), String> {
    let current = current_user(state)?;
    if current == user_id {
        Ok(())
    } else {
        Err("cannot act as another user".to_string())
    }
}
