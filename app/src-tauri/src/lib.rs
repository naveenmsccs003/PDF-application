mod authz;
mod commands;
mod dto;

use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;
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
    /// REL-01/02: where `commands::recovery`'s autosave snapshots get
    /// written (a `recovery/` subdirectory of this). Resolved once at
    /// startup from Tauri's own app data dir, same place the SQLite file
    /// lives.
    data_dir: PathBuf,
    /// SEC-03: the user id currently signed in to this process, set by
    /// `commands::project::{create_user,get_user_by_email}` on success and
    /// read by `authz`'s membership checks. See `authz`'s module doc for
    /// why one slot (not a session map) is the right model here.
    current_user: Mutex<Option<String>>,
}

/// Structured logging (Section 18): every `#[tauri::command]` is annotated
/// `#[tracing::instrument(err)]`, so each call and its arguments (with
/// `state`/password fields explicitly skipped — the former isn't `Debug`,
/// the latter shouldn't ever land in a log file) are recorded as structured
/// spans, and a returned `Err` is logged automatically rather than only
/// surfacing as a string in the frontend. Two sinks: human-readable to
/// stdout (so `npm run tauri dev` shows activity live) and JSON lines to a
/// daily-rotating file under the app data dir's `logs/` folder (so a crash
/// or a bug report has something to inspect after the fact — this app has
/// no telemetry/crash-reporting service to send logs to). Returns the
/// non-blocking writer's guard, which must be held for the process's whole
/// lifetime (dropping it stops the background flush thread and silently
/// truncates buffered log lines) — `run()` binds it to a local that lives
/// until `.run()` returns.
fn init_logging(data_dir: &PathBuf) -> tracing_appender::non_blocking::WorkerGuard {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let log_dir = data_dir.join("logs");
    std::fs::create_dir_all(&log_dir).expect("log dir should be creatable");
    let file_appender = tracing_appender::rolling::daily(&log_dir, "mds_rebar.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().with_target(false))
        .with(fmt::layer().json().with_writer(non_blocking).with_ansi(false))
        .init();

    guard
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

            // Leaked deliberately: a desktop app's logging guard has no
            // natural drop point before process exit (there's no
            // `on_window_event`/shutdown hook this setup already uses), and
            // leaking one `WorkerGuard` for the process's lifetime is the
            // documented way to keep the non-blocking writer flushing —
            // the alternative (storing it in `AppState` for someone to drop
            // "later") has the same lifetime in practice but hides it.
            Box::leak(Box::new(init_logging(&data_dir)));
            tracing::info!(?data_dir, "MDS Rebar starting up");

            let db_path = data_dir.join("mds_rebar.sqlite");
            let conn = mds_db::open_and_migrate(
                db_path.to_str().expect("app data dir path should be valid UTF-8"),
            )
            .expect("database should open and migrate");
            tracing::info!(?db_path, "database opened and migrated");

            let pdf_engine = commands::pdf::spawn_pdf_engine_thread().map(Mutex::new);
            match &pdf_engine {
                Some(_) => tracing::info!("pdfium engine thread started"),
                None => tracing::warn!(
                    "pdfium engine thread did not start — libpdfium.so not found or failed to \
                     initialize; PDF-specific commands will return errors, everything else still works"
                ),
            }

            app.manage(AppState {
                db: Mutex::new(conn),
                pdf_engine,
                markup_undo: Mutex::new(HashMap::new()),
                data_dir,
                current_user: Mutex::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            commands::pdf::import_pdf_document,
            commands::pdf::render_page_thumbnail,
            commands::pdf::export_flattened_pdf,
            commands::pdf::compare_document_versions,
            commands::pdf::set_document_pdf_password,
            commands::pdf::clear_document_pdf_password,
            commands::export::export_handoff_package,
            commands::export::print_document,
            commands::project::create_user,
            commands::project::get_user_by_email,
            commands::project::set_current_user,
            commands::project::sign_out,
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
            commands::document::save_document_revision,
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
            commands::takeoff::export_takeoff_xlsx,
            commands::rfi::create_rfi,
            commands::rfi::get_rfi,
            commands::rfi::list_rfis_for_document,
            commands::rfi::set_rfi_status,
            commands::recovery::autosave_snapshot,
            commands::recovery::list_recovery_snapshots,
            commands::recovery::restore_recovery_snapshot,
            commands::recovery::discard_recovery_snapshot,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
