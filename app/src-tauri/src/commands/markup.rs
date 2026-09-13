//! MARK-01–04/07/08 IPC commands, backed by `crates/markup`. MARK-06
//! (undo/redo, `markup::UndoStack`) isn't wired here — an `UndoStack` is
//! in-memory, per-editing-session state, and deciding its scope (per
//! document? per page? per user, for the confirmed multi-user case?) is a
//! real design question, not a mechanical wiring step like the rest of
//! this file. MARK-05 (markup list/layer panel) is a pure frontend concern
//! once `list_markups_by_page` below exists.

use crate::dto::{MarkupCommentDto, MarkupDto};
use crate::AppState;
use markup::{MarkupGeometry, MarkupStyle, MarkupType};

#[tauri::command]
pub fn create_markup(
    state: tauri::State<AppState>,
    page_id: String,
    markup_type: MarkupType,
    geometry: MarkupGeometry,
    style: MarkupStyle,
    author: Option<String>,
) -> Result<MarkupDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    markup::create(&conn, &page_id, markup_type, geometry, style, author.as_deref())
        .map(Into::into)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_markup(state: tauri::State<AppState>, id: String) -> Result<MarkupDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    markup::get(&conn, &id).map(Into::into).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_markups_by_page(state: tauri::State<AppState>, page_id: String) -> Result<Vec<MarkupDto>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    markup::list_by_page(&conn, &page_id)
        .map(|markups| markups.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_markup_geometry(
    state: tauri::State<AppState>,
    id: String,
    markup_type: MarkupType,
    geometry: MarkupGeometry,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    markup::update_geometry(&conn, &id, markup_type, &geometry).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_markup_style(state: tauri::State<AppState>, id: String, style: MarkupStyle) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    markup::update_style(&conn, &id, &style).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_markup_locked(state: tauri::State<AppState>, id: String, locked: bool) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    markup::set_locked(&conn, &id, locked).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_markup_hidden(state: tauri::State<AppState>, id: String, hidden: bool) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    markup::set_hidden(&conn, &id, hidden).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_markup(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    markup::delete(&conn, &id).map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_markup_comment(
    state: tauri::State<AppState>,
    markup_id: String,
    author: Option<String>,
    text: String,
) -> Result<MarkupCommentDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    markup::add_comment(&conn, &markup_id, author.as_deref(), &text)
        .map(Into::into)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_markup_comments(state: tauri::State<AppState>, markup_id: String) -> Result<Vec<MarkupCommentDto>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    markup::list_comments(&conn, &markup_id)
        .map(|comments| comments.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_markup_comment(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    markup::delete_comment(&conn, &id).map_err(|e| e.to_string())
}
