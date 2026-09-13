mod commands;
mod dto;

use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::Manager;

/// Tauri-managed state: one shared SQLite connection, guarded by a mutex
/// since `rusqlite::Connection` isn't `Sync` and Tauri commands can run on
/// different threads. A connection pool is the obvious next step once this
/// is under real concurrent load; a single mutex-guarded connection is the
/// right starting point, not a shortcut — see Section 8 in the
/// architecture doc, which only calls out background jobs and rendering
/// as the things that must not block, not every DB access.
pub struct AppState {
    db: Mutex<Connection>,
    /// A channel to the one dedicated thread that owns the one
    /// `PdfiumEngine` for the process — see the module doc on
    /// `commands::pdf` for why that indirection is required (the engine
    /// itself isn't `Send`). `None` when `libpdfium.so` isn't found or
    /// failed to initialize — the app still starts and every non-PDF
    /// command still works, but `import_pdf_document`/
    /// `render_page_thumbnail` return a clear error instead of the whole
    /// app failing to launch.
    pdf_engine: Option<Mutex<std::sync::mpsc::Sender<commands::pdf::PdfEngineRequest>>>,
    /// MARK-06: one `UndoStack` per page, created on first use. See
    /// `commands::markup`'s module doc for why this is scoped per page
    /// rather than per document or per user.
    markup_undo: Mutex<HashMap<String, markup::UndoStack>>,
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("app data dir should resolve");
            std::fs::create_dir_all(&data_dir).expect("app data dir should be creatable");
            let db_path = data_dir.join("mds_rebar.sqlite");
            let conn = mds_db::open_and_migrate(
                db_path.to_str().expect("app data dir path should be valid UTF-8"),
            )
            .expect("database should open and migrate");

            let pdf_engine = commands::pdf::spawn_pdf_engine_thread().map(Mutex::new);

            app.manage(AppState {
                db: Mutex::new(conn),
                pdf_engine,
                markup_undo: Mutex::new(HashMap::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            commands::pdf::import_pdf_document,
            commands::pdf::render_page_thumbnail,
            commands::project::create_user,
            commands::project::get_user_by_email,
            commands::project::create_project,
            commands::project::list_projects_for_user,
            commands::project::add_project_member,
            commands::project::list_project_members,
            commands::project::remove_project_member,
            commands::document::get_document,
            commands::document::list_documents_for_project,
            commands::document::list_pages,
            commands::document::set_page_rotation,
            commands::document::reorder_pages,
            commands::document::create_document_version,
            commands::document::list_document_versions,
            commands::markup::create_markup,
            commands::markup::get_markup,
            commands::markup::list_markups_by_page,
            commands::markup::update_markup_geometry,
            commands::markup::update_markup_style,
            commands::markup::set_markup_locked,
            commands::markup::set_markup_hidden,
            commands::markup::delete_markup,
            commands::markup::undo_markup,
            commands::markup::redo_markup,
            commands::markup::markup_undo_status,
            commands::markup::add_markup_comment,
            commands::markup::list_markup_comments,
            commands::markup::delete_markup_comment,
            commands::measurement::calibrate_scale,
            commands::measurement::latest_scale_for_page,
            commands::measurement::record_length,
            commands::measurement::record_area,
            commands::measurement::record_count,
            commands::measurement::get_measurement,
            commands::measurement::list_measurements_by_page,
            commands::measurement::delete_measurement,
            commands::takeoff::create_takeoff_item,
            commands::takeoff::get_takeoff_item,
            commands::takeoff::list_takeoff_for_measurement,
            commands::takeoff::list_takeoff_for_document,
            commands::takeoff::update_takeoff_item,
            commands::takeoff::delete_takeoff_item,
            commands::takeoff::export_takeoff_csv,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
