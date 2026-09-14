//! MARK-01–04/07/08 IPC commands, backed by `crates/markup`, plus MARK-06
//! (undo/redo). Scoping decision for `UndoStack`: one stack per page,
//! keyed in `AppState::markup_undo`. Not per-document or app-wide, because
//! the UI is already organized per page (`PdfCanvas`/`PagePanel` both take
//! a single `PageDto`) and undoing an edit on the page you're not looking
//! at would be confusing; not per-user either, since each user runs their
//! own Tauri process — this in-memory state is already scoped to one user
//! by virtue of being one process's memory, so there's no cross-user
//! stack to separate. MARK-05 (markup list/layer panel) is a pure frontend
//! concern once `list_markups_by_page` below exists.

use crate::dto::{MarkupCommentDto, MarkupDto, UndoStatusDto};
use crate::AppState;
use markup::{Command, Markup, MarkupGeometry, MarkupStyle, MarkupType};

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn create_markup(
    state: tauri::State<AppState>,
    page_id: String,
    markup_type: MarkupType,
    geometry: MarkupGeometry,
    style: MarkupStyle,
    author: Option<String>,
) -> Result<MarkupDto, String> {
    geometry.validate(markup_type).map_err(|e| e.to_string())?;
    let markup_row = Markup {
        id: mds_db::new_uuid(),
        page_id: page_id.clone(),
        markup_type,
        geometry,
        style,
        author,
        locked: false,
        hidden: false,
    };
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_page_access(&conn, &state, &page_id)?;
    let mut stacks = state.markup_undo.lock().map_err(|e| e.to_string())?;
    stacks
        .entry(page_id)
        .or_default()
        .execute(&conn, Command::Create(markup_row.clone()))
        .map_err(|e| e.to_string())?;
    Ok(markup_row.into())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn get_markup(state: tauri::State<AppState>, id: String) -> Result<MarkupDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_markup_access(&conn, &state, &id)?;
    markup::get(&conn, &id).map(Into::into).map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn list_markups_by_page(state: tauri::State<AppState>, page_id: String) -> Result<Vec<MarkupDto>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_page_access(&conn, &state, &page_id)?;
    markup::list_by_page(&conn, &page_id)
        .map(|markups| markups.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn update_markup_geometry(
    state: tauri::State<AppState>,
    id: String,
    markup_type: MarkupType,
    geometry: MarkupGeometry,
) -> Result<(), String> {
    geometry.validate(markup_type).map_err(|e| e.to_string())?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_markup_access(&conn, &state, &id)?;
    let before = markup::get(&conn, &id).map_err(|e| e.to_string())?;
    let mut stacks = state.markup_undo.lock().map_err(|e| e.to_string())?;
    stacks
        .entry(before.page_id.clone())
        .or_default()
        .execute(
            &conn,
            Command::SetGeometry { id, markup_type, before: before.geometry, after: geometry },
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn update_markup_style(state: tauri::State<AppState>, id: String, style: MarkupStyle) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_markup_access(&conn, &state, &id)?;
    let before = markup::get(&conn, &id).map_err(|e| e.to_string())?;
    let mut stacks = state.markup_undo.lock().map_err(|e| e.to_string())?;
    stacks
        .entry(before.page_id.clone())
        .or_default()
        .execute(&conn, Command::SetStyle { id, before: before.style, after: style })
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn set_markup_locked(state: tauri::State<AppState>, id: String, locked: bool) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_markup_access(&conn, &state, &id)?;
    let before = markup::get(&conn, &id).map_err(|e| e.to_string())?;
    let mut stacks = state.markup_undo.lock().map_err(|e| e.to_string())?;
    stacks
        .entry(before.page_id.clone())
        .or_default()
        .execute(&conn, Command::SetLocked { id, before: before.locked, after: locked })
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn set_markup_hidden(state: tauri::State<AppState>, id: String, hidden: bool) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_markup_access(&conn, &state, &id)?;
    let before = markup::get(&conn, &id).map_err(|e| e.to_string())?;
    let mut stacks = state.markup_undo.lock().map_err(|e| e.to_string())?;
    stacks
        .entry(before.page_id.clone())
        .or_default()
        .execute(&conn, Command::SetHidden { id, before: before.hidden, after: hidden })
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn delete_markup(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_markup_access(&conn, &state, &id)?;
    let markup_row = markup::get(&conn, &id).map_err(|e| e.to_string())?;
    let mut stacks = state.markup_undo.lock().map_err(|e| e.to_string())?;
    stacks
        .entry(markup_row.page_id.clone())
        .or_default()
        .execute(&conn, Command::Delete(markup_row))
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn undo_markup(state: tauri::State<AppState>, page_id: String) -> Result<bool, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_page_access(&conn, &state, &page_id)?;
    let mut stacks = state.markup_undo.lock().map_err(|e| e.to_string())?;
    stacks.entry(page_id).or_default().undo(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn redo_markup(state: tauri::State<AppState>, page_id: String) -> Result<bool, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_page_access(&conn, &state, &page_id)?;
    let mut stacks = state.markup_undo.lock().map_err(|e| e.to_string())?;
    stacks.entry(page_id).or_default().redo(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn markup_undo_status(state: tauri::State<AppState>, page_id: String) -> Result<UndoStatusDto, String> {
    let stacks = state.markup_undo.lock().map_err(|e| e.to_string())?;
    Ok(match stacks.get(&page_id) {
        Some(stack) => UndoStatusDto { can_undo: stack.can_undo(), can_redo: stack.can_redo() },
        None => UndoStatusDto { can_undo: false, can_redo: false },
    })
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn add_markup_comment(
    state: tauri::State<AppState>,
    markup_id: String,
    author: Option<String>,
    text: String,
) -> Result<MarkupCommentDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_markup_access(&conn, &state, &markup_id)?;
    markup::add_comment(&conn, &markup_id, author.as_deref(), &text)
        .map(Into::into)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn list_markup_comments(state: tauri::State<AppState>, markup_id: String) -> Result<Vec<MarkupCommentDto>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_markup_access(&conn, &state, &markup_id)?;
    markup::list_comments(&conn, &markup_id)
        .map(|comments| comments.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn delete_markup_comment(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_markup_comment_access(&conn, &state, &id)?;
    markup::delete_comment(&conn, &id).map_err(|e| e.to_string())
}
