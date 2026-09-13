//! COLLAB-01/DOC-06 IPC commands, backed by `crates/project`.

use crate::dto::{ProjectDto, ProjectMemberDto, UserDto};
use crate::AppState;

#[tauri::command]
pub fn create_user(state: tauri::State<AppState>, email: String, display_name: String) -> Result<UserDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    project::create_user(&conn, &email, &display_name)
        .map(Into::into)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_user_by_email(state: tauri::State<AppState>, email: String) -> Result<UserDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    project::get_user_by_email(&conn, &email)
        .map(Into::into)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_project(state: tauri::State<AppState>, name: String, created_by: String) -> Result<ProjectDto, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    project::create_project(&mut conn, &name, &created_by)
        .map(Into::into)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_projects_for_user(state: tauri::State<AppState>, user_id: String) -> Result<Vec<ProjectDto>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    project::list_projects_for_user(&conn, &user_id)
        .map(|projects| projects.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_project_member(
    state: tauri::State<AppState>,
    project_id: String,
    user_id: String,
    role: String,
) -> Result<ProjectMemberDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    project::add_member(&conn, &project_id, &user_id, &role)
        .map(Into::into)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_project_members(state: tauri::State<AppState>, project_id: String) -> Result<Vec<ProjectMemberDto>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    project::list_members(&conn, &project_id)
        .map(|members| members.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_project_member(state: tauri::State<AppState>, project_id: String, user_id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    project::remove_member(&conn, &project_id, &user_id).map_err(|e| e.to_string())
}
