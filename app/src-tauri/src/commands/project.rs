//! COLLAB-01/DOC-06 IPC commands, backed by `crates/project`, plus SEC-03
//! enforcement (see `crate::authz`) on the membership-mutating/reading
//! commands below.
//!
//! `create_user`/`get_user_by_email` are plain identity lookups, not
//! sign-in — deliberately kept side-effect-free, because
//! `get_user_by_email` is also how `AddMemberForm`-style flows resolve a
//! teammate's id from an email to invite them, and a lookup like that must
//! not also switch *this* process's signed-in identity to the person being
//! looked up. `set_current_user`/`sign_out` are the explicit session
//! operations the frontend calls once it has decided (via one of those two
//! lookups) which `UserDto` the person at the keyboard actually is.

use crate::dto::{ProjectDto, ProjectMemberDto, UserDto};
use crate::AppState;

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn create_user(state: tauri::State<AppState>, email: String, display_name: String) -> Result<UserDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    project::create_user(&conn, &email, &display_name)
        .map(Into::into)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn get_user_by_email(state: tauri::State<AppState>, email: String) -> Result<UserDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    project::get_user_by_email(&conn, &email)
        .map(Into::into)
        .map_err(|e| e.to_string())
}

/// SEC-03: the frontend calls this once, right after its sign-in flow
/// (`create_user` or `get_user_by_email`) resolves a `UserDto` — see the
/// module doc for why that's two steps rather than a side effect of the
/// lookup itself. No credential check gates this (same as every other
/// identity operation here — SEC-01/02 are about PDF/keychain passwords,
/// not an app-wide login, and this schema still has no session/password
/// concept), so it's exactly as strong as today's "sign in by typing any
/// known email" — not a new hole, just where membership checks now read
/// the result from.
#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn set_current_user(state: tauri::State<AppState>, user_id: String) -> Result<(), String> {
    crate::authz::set_current_user(&state, &user_id)
}

/// Clears `AppState::current_user` — see `crate::authz`'s module doc for
/// why sign-in identity lives there rather than in a session token.
#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn sign_out(state: tauri::State<AppState>) -> Result<(), String> {
    crate::authz::clear_current_user(&state)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn create_project(state: tauri::State<AppState>, name: String, created_by: String) -> Result<ProjectDto, String> {
    crate::authz::require_self(&state, &created_by)?;
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    project::create_project(&mut conn, &name, &created_by)
        .map(Into::into)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn list_projects_for_user(state: tauri::State<AppState>, user_id: String) -> Result<Vec<ProjectDto>, String> {
    crate::authz::require_self(&state, &user_id)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    project::list_projects_for_user(&conn, &user_id)
        .map(|projects| projects.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn add_project_member(
    state: tauri::State<AppState>,
    project_id: String,
    user_id: String,
    role: String,
) -> Result<ProjectMemberDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_project_membership(&conn, &state, &project_id)?;
    project::add_member(&conn, &project_id, &user_id, &role)
        .map(Into::into)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn list_project_members(state: tauri::State<AppState>, project_id: String) -> Result<Vec<ProjectMemberDto>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_project_membership(&conn, &state, &project_id)?;
    project::list_members(&conn, &project_id)
        .map(|members| members.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn remove_project_member(state: tauri::State<AppState>, project_id: String, user_id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_project_membership(&conn, &state, &project_id)?;
    project::remove_member(&conn, &project_id, &user_id).map_err(|e| e.to_string())
}
