//! MEAS-01–07 IPC commands, backed by `crates/measurement`.

use crate::dto::{MeasurementDto, ScaleDto};
use crate::AppState;
use geometry::Point;
use measurement::{AreaUnit, LengthUnit};

/// Matches the string tokens `measurement` itself persists in the
/// `measurement.unit` column (see its private `length_unit_to_str`) — kept
/// in sync by hand since that mapping isn't exposed publicly.
fn parse_length_unit(unit: &str) -> Result<LengthUnit, String> {
    match unit {
        "in" => Ok(LengthUnit::Inches),
        "ft" => Ok(LengthUnit::Feet),
        "mm" => Ok(LengthUnit::Millimeters),
        "cm" => Ok(LengthUnit::Centimeters),
        "m" => Ok(LengthUnit::Meters),
        other => Err(format!("unknown length unit `{other}`")),
    }
}

/// Matches `AreaUnit::as_db_str`.
fn parse_area_unit(unit: &str) -> Result<AreaUnit, String> {
    match unit {
        "sq_in" => Ok(AreaUnit::SquareInches),
        "sq_ft" => Ok(AreaUnit::SquareFeet),
        "sq_m" => Ok(AreaUnit::SquareMeters),
        other => Err(format!("unknown area unit `{other}`")),
    }
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn calibrate_scale(
    state: tauri::State<AppState>,
    page_id: String,
    p1: (f64, f64),
    p2: (f64, f64),
    known_real_world_inches: f64,
    unit_system: String,
    calibrated_by: Option<String>,
) -> Result<ScaleDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_page_access(&conn, &state, &page_id)?;
    measurement::calibrate(
        &conn,
        &page_id,
        Point::new(p1.0, p1.1),
        Point::new(p2.0, p2.1),
        known_real_world_inches,
        &unit_system,
        calibrated_by.as_deref(),
    )
    .map(Into::into)
    .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn latest_scale_for_page(state: tauri::State<AppState>, page_id: String) -> Result<Option<ScaleDto>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_page_access(&conn, &state, &page_id)?;
    measurement::latest_scale_for_page(&conn, &page_id)
        .map(|scale| scale.map(Into::into))
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn record_length(
    state: tauri::State<AppState>,
    page_id: String,
    scale_id: String,
    p1: (f64, f64),
    p2: (f64, f64),
    unit: String,
    label: Option<String>,
) -> Result<MeasurementDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_page_access(&conn, &state, &page_id)?;
    let unit = parse_length_unit(&unit)?;
    measurement::record_length(&conn, &page_id, &scale_id, Point::new(p1.0, p1.1), Point::new(p2.0, p2.1), unit, label.as_deref())
        .map(Into::into)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn record_area(
    state: tauri::State<AppState>,
    page_id: String,
    scale_id: String,
    points: Vec<(f64, f64)>,
    unit: String,
    label: Option<String>,
) -> Result<MeasurementDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_page_access(&conn, &state, &page_id)?;
    let unit = parse_area_unit(&unit)?;
    let points: Vec<Point> = points.into_iter().map(|(x, y)| Point::new(x, y)).collect();
    measurement::record_area(&conn, &page_id, &scale_id, &points, unit, label.as_deref())
        .map(Into::into)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn record_count(
    state: tauri::State<AppState>,
    page_id: String,
    markers: Vec<(f64, f64)>,
    label: Option<String>,
) -> Result<MeasurementDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_page_access(&conn, &state, &page_id)?;
    let markers: Vec<Point> = markers.into_iter().map(|(x, y)| Point::new(x, y)).collect();
    measurement::record_count(&conn, &page_id, &markers, label.as_deref())
        .map(Into::into)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn get_measurement(state: tauri::State<AppState>, id: String) -> Result<MeasurementDto, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_measurement_access(&conn, &state, &id)?;
    measurement::get(&conn, &id).map(Into::into).map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn list_measurements_by_page(state: tauri::State<AppState>, page_id: String) -> Result<Vec<MeasurementDto>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_page_access(&conn, &state, &page_id)?;
    measurement::list_by_page(&conn, &page_id)
        .map(|measurements| measurements.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn delete_measurement(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    crate::authz::require_measurement_access(&conn, &state, &id)?;
    measurement::delete(&conn, &id).map_err(|e| e.to_string())
}
