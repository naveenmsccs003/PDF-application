//! Measurement domain model (Section 7 layering: Domain "Measurement",
//! backed by the Core Engine `geometry` crate for the actual math).
//! Persists through `mds_db`'s `scale` and `measurement` tables.
//!
//! Covers MEAS-01/02/03/05/06/07 from `docs/features/FEATURE_REGISTRY.md`
//! (scale calibration, length, area, unit conversion, labels,
//! persistence) plus MEAS-04 (count), which `docs/PROJECT_PLAN.md`
//! deferred until a Markup domain model existed to place count markers on
//! — `crates/markup` now exists, but count here only needs marker *points*,
//! not a dependency on the `markup` crate itself, so none was added.

use geometry::units::convert_length;
use geometry::{polygon_area_square_inches, GeometryError, Point, Scale};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

// Re-exported because `record_length` takes this type by value — without
// this, a caller outside `geometry` (found via `e2e_tests`) would have to
// know to reach past `measurement` into `geometry::units::LengthUnit`
// directly to call a `measurement` function.
pub use geometry::units::LengthUnit;

#[derive(Debug, thiserror::Error)]
pub enum MeasurementError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("geometry error: {0}")]
    Geometry(#[from] GeometryError),
    #[error("scale {0} not found")]
    ScaleNotFound(String),
    #[error("measurement {0} not found")]
    MeasurementNotFound(String),
    #[error("decode error: {0}")]
    Decode(String),
}

fn length_unit_to_str(unit: LengthUnit) -> &'static str {
    match unit {
        LengthUnit::Inches => "in",
        LengthUnit::Feet => "ft",
        LengthUnit::Millimeters => "mm",
        LengthUnit::Centimeters => "cm",
        LengthUnit::Meters => "m",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AreaUnit {
    SquareInches,
    SquareFeet,
    SquareMeters,
}

impl AreaUnit {
    /// Linear inches-per-unit, squared — `geometry` only exposes a linear
    /// `convert_length`, so area conversion reuses it rather than adding a
    /// second, parallel unit table.
    fn square_inches_to_unit(self, value_square_inches: f64) -> f64 {
        let linear_factor = match self {
            AreaUnit::SquareInches => convert_length(1.0, LengthUnit::Inches, LengthUnit::Inches),
            AreaUnit::SquareFeet => convert_length(1.0, LengthUnit::Inches, LengthUnit::Feet),
            AreaUnit::SquareMeters => convert_length(1.0, LengthUnit::Inches, LengthUnit::Meters),
        };
        value_square_inches * linear_factor.powi(2)
    }

    fn as_db_str(self) -> &'static str {
        match self {
            AreaUnit::SquareInches => "sq_in",
            AreaUnit::SquareFeet => "sq_ft",
            AreaUnit::SquareMeters => "sq_m",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeasurementType {
    Length,
    Area,
    Count,
}

impl MeasurementType {
    fn as_db_str(self) -> &'static str {
        match self {
            MeasurementType::Length => "length",
            MeasurementType::Area => "area",
            MeasurementType::Count => "count",
        }
    }

    fn from_db_str(s: &str) -> Result<Self, MeasurementError> {
        Ok(match s {
            "length" => MeasurementType::Length,
            "area" => MeasurementType::Area,
            "count" => MeasurementType::Count,
            other => return Err(MeasurementError::Decode(format!("unknown measurement type `{other}`"))),
        })
    }
}

/// A calibrated, persisted scale (MEAS-01). Wraps `geometry::Scale`, which
/// holds the actual ratio but has no persistence of its own.
#[derive(Debug, Clone, PartialEq)]
pub struct CalibratedScale {
    pub id: String,
    pub page_id: String,
    pub scale: Scale,
    pub unit_system: String,
    pub calibrated_by: Option<String>,
}

/// Calibrates a scale from two page-space points a known real-world
/// distance apart, and persists it against `page_id`.
pub fn calibrate(
    conn: &Connection,
    page_id: &str,
    p1: Point,
    p2: Point,
    known_real_world_inches: f64,
    unit_system: &str,
    calibrated_by: Option<&str>,
) -> Result<CalibratedScale, MeasurementError> {
    let scale = Scale::calibrate(p1, p2, known_real_world_inches)?;
    let id = mds_db::new_uuid();
    conn.execute(
        "INSERT INTO scale (id, page_id, ratio, unit_system, calibrated_by) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, page_id, scale.inches_per_page_unit(), unit_system, calibrated_by],
    )?;
    Ok(CalibratedScale {
        id,
        page_id: page_id.to_string(),
        scale,
        unit_system: unit_system.to_string(),
        calibrated_by: calibrated_by.map(str::to_string),
    })
}

pub fn get_scale(conn: &Connection, id: &str) -> Result<CalibratedScale, MeasurementError> {
    conn.query_row(
        "SELECT id, page_id, ratio, unit_system, calibrated_by FROM scale WHERE id = ?1",
        params![id],
        |row| {
            Ok(CalibratedScale {
                id: row.get(0)?,
                page_id: row.get(1)?,
                scale: Scale::from_ratio(row.get(2)?),
                unit_system: row.get(3)?,
                calibrated_by: row.get(4)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| MeasurementError::ScaleNotFound(id.to_string()))
}

/// The most recently calibrated scale for a page, if any — the common
/// lookup a caller makes before recording a length/area measurement.
pub fn latest_scale_for_page(
    conn: &Connection,
    page_id: &str,
) -> Result<Option<CalibratedScale>, MeasurementError> {
    conn.query_row(
        "SELECT id, page_id, ratio, unit_system, calibrated_by FROM scale
         WHERE page_id = ?1 ORDER BY created_at DESC LIMIT 1",
        params![page_id],
        |row| {
            Ok(CalibratedScale {
                id: row.get(0)?,
                page_id: row.get(1)?,
                scale: Scale::from_ratio(row.get(2)?),
                unit_system: row.get(3)?,
                calibrated_by: row.get(4)?,
            })
        },
    )
    .optional()
    .map_err(MeasurementError::from)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Measurement {
    pub id: String,
    pub page_id: String,
    pub scale_id: Option<String>,
    pub measurement_type: MeasurementType,
    pub geometry: Vec<(f64, f64)>,
    pub value: f64,
    pub unit: String,
    pub label: Option<String>,
}

fn insert_measurement(
    conn: &Connection,
    page_id: &str,
    scale_id: Option<&str>,
    measurement_type: MeasurementType,
    geometry: &[Point],
    value: f64,
    unit: &str,
    label: Option<&str>,
) -> Result<Measurement, MeasurementError> {
    let id = mds_db::new_uuid();
    let geometry_tuples: Vec<(f64, f64)> = geometry.iter().map(|p| (p.x, p.y)).collect();
    conn.execute(
        "INSERT INTO measurement (id, page_id, scale_id, type, geometry_json, value, unit, label)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            id,
            page_id,
            scale_id,
            measurement_type.as_db_str(),
            serde_json::to_string(&geometry_tuples)?,
            value,
            unit,
            label,
        ],
    )?;
    Ok(Measurement {
        id,
        page_id: page_id.to_string(),
        scale_id: scale_id.map(str::to_string),
        measurement_type,
        geometry: geometry_tuples,
        value,
        unit: unit.to_string(),
        label: label.map(str::to_string),
    })
}

/// MEAS-02 + MEAS-05: length between two page-space points, converted to
/// `unit`, persisted against `scale_id`.
pub fn record_length(
    conn: &Connection,
    page_id: &str,
    scale_id: &str,
    p1: Point,
    p2: Point,
    unit: LengthUnit,
    label: Option<&str>,
) -> Result<Measurement, MeasurementError> {
    let scale = get_scale(conn, scale_id)?;
    let inches = geometry::length_inches(p1, p2, &scale.scale);
    let value = convert_length(inches, LengthUnit::Inches, unit);
    insert_measurement(
        conn,
        page_id,
        Some(scale_id),
        MeasurementType::Length,
        &[p1, p2],
        value,
        length_unit_to_str(unit),
        label,
    )
}

/// MEAS-03 + MEAS-05: polygon area, converted to `unit`, persisted against
/// `scale_id`.
pub fn record_area(
    conn: &Connection,
    page_id: &str,
    scale_id: &str,
    points: &[Point],
    unit: AreaUnit,
    label: Option<&str>,
) -> Result<Measurement, MeasurementError> {
    let scale = get_scale(conn, scale_id)?;
    let square_inches = polygon_area_square_inches(points, &scale.scale)?;
    let value = unit.square_inches_to_unit(square_inches);
    insert_measurement(
        conn,
        page_id,
        Some(scale_id),
        MeasurementType::Area,
        points,
        value,
        unit.as_db_str(),
        label,
    )
}

/// MEAS-04: count of marker points placed on the page. Scale-independent —
/// unlike length/area, a count has no real-world unit conversion.
pub fn record_count(
    conn: &Connection,
    page_id: &str,
    markers: &[Point],
    label: Option<&str>,
) -> Result<Measurement, MeasurementError> {
    insert_measurement(
        conn,
        page_id,
        None,
        MeasurementType::Count,
        markers,
        markers.len() as f64,
        "each",
        label,
    )
}

fn row_to_measurement(row: &rusqlite::Row) -> rusqlite::Result<Measurement> {
    let geometry_json: String = row.get(4)?;
    let geometry_tuples: Vec<(f64, f64)> = serde_json::from_str(&geometry_json)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e)))?;
    let type_str: String = row.get(3)?;
    let measurement_type = MeasurementType::from_db_str(&type_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e)))?;
    Ok(Measurement {
        id: row.get(0)?,
        page_id: row.get(1)?,
        scale_id: row.get(2)?,
        measurement_type,
        geometry: geometry_tuples,
        value: row.get(5)?,
        unit: row.get(6)?,
        label: row.get(7)?,
    })
}

const SELECT_COLUMNS: &str = "id, page_id, scale_id, type, geometry_json, value, unit, label";

pub fn get(conn: &Connection, id: &str) -> Result<Measurement, MeasurementError> {
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM measurement WHERE id = ?1"),
        params![id],
        row_to_measurement,
    )
    .optional()?
    .ok_or_else(|| MeasurementError::MeasurementNotFound(id.to_string()))
}

pub fn list_by_page(conn: &Connection, page_id: &str) -> Result<Vec<Measurement>, MeasurementError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM measurement WHERE page_id = ?1 ORDER BY created_at"
    ))?;
    let rows = stmt
        .query_map(params![page_id], row_to_measurement)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// MEAS-07 covers persistence, not immutability — a measurement can be
/// deleted (e.g. the underlying markup it was tied to was deleted).
pub fn delete(conn: &Connection, id: &str) -> Result<(), MeasurementError> {
    let n = conn.execute("DELETE FROM measurement WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(MeasurementError::MeasurementNotFound(id.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_test_db() -> Connection {
        mds_db::open_and_migrate(":memory:").unwrap()
    }

    fn fixture_page(conn: &Connection) -> String {
        let user_id = mds_db::new_uuid();
        conn.execute(
            "INSERT INTO user (id, email, display_name) VALUES (?1, 'a@b.com', 'A')",
            params![user_id],
        )
        .unwrap();
        let doc_id = mds_db::new_uuid();
        conn.execute(
            "INSERT INTO document (id, file_path, title) VALUES (?1, '/tmp/x.pdf', 'X')",
            params![doc_id],
        )
        .unwrap();
        let page_id = mds_db::new_uuid();
        conn.execute(
            "INSERT INTO page (id, document_id, page_number, width, height) VALUES (?1, ?2, 1, 100.0, 200.0)",
            params![page_id, doc_id],
        )
        .unwrap();
        page_id
    }

    #[test]
    fn calibrate_persists_and_round_trips() {
        let conn = open_test_db();
        let page_id = fixture_page(&conn);

        // 2 page units == 10 feet (120 inches) real-world.
        let calibrated = calibrate(
            &conn,
            &page_id,
            Point::new(0.0, 0.0),
            Point::new(2.0, 0.0),
            120.0,
            "imperial",
            None,
        )
        .unwrap();

        let fetched = get_scale(&conn, &calibrated.id).unwrap();
        assert_eq!(fetched, calibrated);

        let latest = latest_scale_for_page(&conn, &page_id).unwrap().unwrap();
        assert_eq!(latest, calibrated);
    }

    #[test]
    fn record_length_converts_and_persists() {
        let conn = open_test_db();
        let page_id = fixture_page(&conn);
        let scale = calibrate(
            &conn,
            &page_id,
            Point::new(0.0, 0.0),
            Point::new(2.0, 0.0),
            120.0,
            "imperial",
            None,
        )
        .unwrap();

        let measurement = record_length(
            &conn,
            &page_id,
            &scale.id,
            Point::new(0.0, 0.0),
            Point::new(2.0, 0.0),
            LengthUnit::Feet,
            Some("Wall A"),
        )
        .unwrap();

        assert_eq!(measurement.measurement_type, MeasurementType::Length);
        assert!((measurement.value - 10.0).abs() < 1e-9); // 120 in == 10 ft
        assert_eq!(measurement.unit, "ft");
        assert_eq!(measurement.label.as_deref(), Some("Wall A"));

        let fetched = get(&conn, &measurement.id).unwrap();
        assert_eq!(fetched, measurement);
    }

    #[test]
    fn record_area_converts_and_persists() {
        let conn = open_test_db();
        let page_id = fixture_page(&conn);
        // 1 page unit == 12 inches (1 ft) real-world.
        let scale = calibrate(
            &conn,
            &page_id,
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            12.0,
            "imperial",
            None,
        )
        .unwrap();

        let square = [
            Point::new(0.0, 0.0),
            Point::new(2.0, 0.0),
            Point::new(2.0, 2.0),
            Point::new(0.0, 2.0),
        ];
        let measurement = record_area(
            &conn,
            &page_id,
            &scale.id,
            &square,
            AreaUnit::SquareFeet,
            None,
        )
        .unwrap();

        // 2x2 page units == 2ft x 2ft == 4 sq ft.
        assert!((measurement.value - 4.0).abs() < 1e-9);
        assert_eq!(measurement.unit, "sq_ft");
    }

    #[test]
    fn record_area_rejects_too_few_points() {
        let conn = open_test_db();
        let page_id = fixture_page(&conn);
        let scale = calibrate(
            &conn,
            &page_id,
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            1.0,
            "imperial",
            None,
        )
        .unwrap();

        let err = record_area(
            &conn,
            &page_id,
            &scale.id,
            &[Point::new(0.0, 0.0), Point::new(1.0, 0.0)],
            AreaUnit::SquareInches,
            None,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            MeasurementError::Geometry(GeometryError::PolygonTooFewPoints(2))
        ));
    }

    #[test]
    fn record_count_stores_marker_count_and_no_scale() {
        let conn = open_test_db();
        let page_id = fixture_page(&conn);

        let markers = [
            Point::new(1.0, 1.0),
            Point::new(2.0, 2.0),
            Point::new(3.0, 3.0),
        ];
        let measurement = record_count(&conn, &page_id, &markers, Some("Rebar chairs")).unwrap();

        assert_eq!(measurement.measurement_type, MeasurementType::Count);
        assert_eq!(measurement.value, 3.0);
        assert_eq!(measurement.unit, "each");
        assert_eq!(measurement.scale_id, None);
        assert_eq!(measurement.geometry.len(), 3);
    }

    #[test]
    fn list_by_page_and_delete() {
        let conn = open_test_db();
        let page_id = fixture_page(&conn);
        let m = record_count(&conn, &page_id, &[Point::new(0.0, 0.0)], None).unwrap();

        let listed = list_by_page(&conn, &page_id).unwrap();
        assert_eq!(listed, vec![m.clone()]);

        delete(&conn, &m.id).unwrap();
        assert!(matches!(
            get(&conn, &m.id),
            Err(MeasurementError::MeasurementNotFound(_))
        ));
        assert!(list_by_page(&conn, &page_id).unwrap().is_empty());
    }

    #[test]
    fn record_length_with_unknown_scale_id_errors() {
        let conn = open_test_db();
        let page_id = fixture_page(&conn);
        let err = record_length(
            &conn,
            &page_id,
            "does-not-exist",
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            LengthUnit::Inches,
            None,
        )
        .unwrap_err();
        assert!(matches!(err, MeasurementError::ScaleNotFound(_)));
    }
}
