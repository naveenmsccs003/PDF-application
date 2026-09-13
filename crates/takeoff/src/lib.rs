//! Takeoff domain model (Section 7 layering: Domain "Takeoff"). Persists
//! through `mds_db`'s `takeoff_item` table.
//!
//! Covers TAKE-01–05 from `docs/features/FEATURE_REGISTRY.md`: manual
//! quantity entry, optional linking to a `Measurement` row, per-item
//! description/unit/notes, cost per unit, and CSV export. TAKE-04's cost
//! field is persisted because the `takeoff_item.cost_per_unit` column
//! already exists in the schema — `docs/features/FEATURE_REGISTRY.md`
//! still flags it "Needs Naveen confirmation" for whether it's actually
//! used in MDS Rebar's estimating flow, which this doesn't resolve, only
//! stores.

use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, thiserror::Error)]
pub enum TakeoffError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("takeoff item {0} not found")]
    NotFound(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TakeoffItem {
    pub id: String,
    pub measurement_id: Option<String>,
    pub description: String,
    pub quantity: f64,
    pub unit: String,
    pub cost_per_unit: Option<f64>,
    pub notes: Option<String>,
}

impl TakeoffItem {
    /// `quantity * cost_per_unit`, or `None` if no cost was entered —
    /// TAKE-04 is optional per the schema, so this stays optional too.
    pub fn total_cost(&self) -> Option<f64> {
        self.cost_per_unit.map(|cost| cost * self.quantity)
    }
}

const SELECT_COLUMNS: &str = "id, measurement_id, description, quantity, unit, cost_per_unit, notes";

fn row_to_item(row: &rusqlite::Row) -> rusqlite::Result<TakeoffItem> {
    Ok(TakeoffItem {
        id: row.get(0)?,
        measurement_id: row.get(1)?,
        description: row.get(2)?,
        quantity: row.get(3)?,
        unit: row.get(4)?,
        cost_per_unit: row.get(5)?,
        notes: row.get(6)?,
    })
}

/// TAKE-01/02/03/04: create a line item, optionally linked to a
/// measurement, with description/unit/notes/cost.
#[allow(clippy::too_many_arguments)]
pub fn create(
    conn: &Connection,
    description: &str,
    quantity: f64,
    unit: &str,
    measurement_id: Option<&str>,
    cost_per_unit: Option<f64>,
    notes: Option<&str>,
) -> Result<TakeoffItem, TakeoffError> {
    let id = mds_db::new_uuid();
    conn.execute(
        "INSERT INTO takeoff_item (id, measurement_id, description, quantity, unit, cost_per_unit, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, measurement_id, description, quantity, unit, cost_per_unit, notes],
    )?;
    Ok(TakeoffItem {
        id,
        measurement_id: measurement_id.map(str::to_string),
        description: description.to_string(),
        quantity,
        unit: unit.to_string(),
        cost_per_unit,
        notes: notes.map(str::to_string),
    })
}

pub fn get(conn: &Connection, id: &str) -> Result<TakeoffItem, TakeoffError> {
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM takeoff_item WHERE id = ?1"),
        params![id],
        row_to_item,
    )
    .optional()?
    .ok_or_else(|| TakeoffError::NotFound(id.to_string()))
}

/// TAKE-02: line items linked to a specific measurement.
pub fn list_for_measurement(
    conn: &Connection,
    measurement_id: &str,
) -> Result<Vec<TakeoffItem>, TakeoffError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM takeoff_item WHERE measurement_id = ?1 ORDER BY created_at"
    ))?;
    let rows = stmt
        .query_map(params![measurement_id], row_to_item)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// All line items belonging to a document — joins through
/// `measurement -> page -> document` since `takeoff_item` itself has no
/// document/project column. The natural scope for an export (TAKE-05).
pub fn list_for_document(
    conn: &Connection,
    document_id: &str,
) -> Result<Vec<TakeoffItem>, TakeoffError> {
    let sql = format!(
        "SELECT {cols} FROM takeoff_item ti
         JOIN measurement m ON m.id = ti.measurement_id
         JOIN page p ON p.id = m.page_id
         WHERE p.document_id = ?1
         ORDER BY ti.created_at",
        cols = SELECT_COLUMNS
            .split(", ")
            .map(|c| format!("ti.{c}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params![document_id], row_to_item)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// TAKE-01/03/04: edit an existing line item's quantity/unit/cost/notes.
/// Description is intentionally not editable here — the schema treats it
/// as set at creation; splitting create vs. edit is left to the caller.
pub fn update(
    conn: &Connection,
    id: &str,
    quantity: f64,
    unit: &str,
    cost_per_unit: Option<f64>,
    notes: Option<&str>,
) -> Result<(), TakeoffError> {
    let n = conn.execute(
        "UPDATE takeoff_item SET quantity = ?1, unit = ?2, cost_per_unit = ?3, notes = ?4 WHERE id = ?5",
        params![quantity, unit, cost_per_unit, notes, id],
    )?;
    if n == 0 {
        return Err(TakeoffError::NotFound(id.to_string()));
    }
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> Result<(), TakeoffError> {
    let n = conn.execute("DELETE FROM takeoff_item WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(TakeoffError::NotFound(id.to_string()));
    }
    Ok(())
}

/// TAKE-05: CSV export. Hand-rolled rather than pulling in a `csv` crate —
/// the format is a handful of scalar columns, and RFC 4180 quoting for a
/// field is exactly "wrap in quotes if it contains a comma, quote, or
/// newline; double any internal quotes".
pub fn export_csv(items: &[TakeoffItem]) -> String {
    fn csv_field(s: &str) -> String {
        if s.contains(',') || s.contains('"') || s.contains('\n') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    }

    let mut out = String::from("description,quantity,unit,cost_per_unit,total_cost,notes\n");
    for item in items {
        let cost = item
            .cost_per_unit
            .map(|c| c.to_string())
            .unwrap_or_default();
        let total = item
            .total_cost()
            .map(|t| t.to_string())
            .unwrap_or_default();
        let notes = item.notes.as_deref().unwrap_or("");
        out.push_str(&format!(
            "{},{},{},{},{},{}\n",
            csv_field(&item.description),
            item.quantity,
            csv_field(&item.unit),
            cost,
            total,
            csv_field(notes),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_test_db() -> Connection {
        mds_db::open_and_migrate(":memory:").unwrap()
    }

    /// User/document/page/scale/measurement fixture — `takeoff_item` needs
    /// a real `measurement_id` to test the document-scoped join.
    fn fixture_measurement(conn: &Connection) -> (String, String) {
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
        let measurement_id = mds_db::new_uuid();
        conn.execute(
            "INSERT INTO measurement (id, page_id, type, geometry_json, value, unit) VALUES (?1, ?2, 'count', '[]', 3.0, 'each')",
            params![measurement_id, page_id],
        )
        .unwrap();
        (doc_id, measurement_id)
    }

    #[test]
    fn create_get_round_trip() {
        let conn = open_test_db();
        let item = create(
            &conn,
            "#4 rebar, 20ft",
            3.0,
            "each",
            None,
            Some(12.5),
            Some("field verify"),
        )
        .unwrap();

        let fetched = get(&conn, &item.id).unwrap();
        assert_eq!(fetched, item);
        assert_eq!(item.total_cost(), Some(37.5));
    }

    #[test]
    fn total_cost_is_none_without_a_unit_cost() {
        let conn = open_test_db();
        let item = create(&conn, "misc", 5.0, "ea", None, None, None).unwrap();
        assert_eq!(item.total_cost(), None);
    }

    #[test]
    fn update_changes_quantity_unit_cost_and_notes() {
        let conn = open_test_db();
        let item = create(&conn, "rebar chairs", 10.0, "ea", None, Some(1.0), None).unwrap();

        update(&conn, &item.id, 15.0, "ea", Some(1.25), Some("recount")).unwrap();
        let fetched = get(&conn, &item.id).unwrap();
        assert_eq!(fetched.quantity, 15.0);
        assert_eq!(fetched.cost_per_unit, Some(1.25));
        assert_eq!(fetched.notes.as_deref(), Some("recount"));
    }

    #[test]
    fn delete_removes_item() {
        let conn = open_test_db();
        let item = create(&conn, "temp", 1.0, "ea", None, None, None).unwrap();
        delete(&conn, &item.id).unwrap();
        assert!(matches!(get(&conn, &item.id), Err(TakeoffError::NotFound(_))));
    }

    #[test]
    fn list_for_measurement_filters_correctly() {
        let conn = open_test_db();
        let (_, measurement_id) = fixture_measurement(&conn);
        let linked = create(&conn, "linked", 1.0, "ea", Some(&measurement_id), None, None).unwrap();
        create(&conn, "unlinked", 2.0, "ea", None, None, None).unwrap();

        let listed = list_for_measurement(&conn, &measurement_id).unwrap();
        assert_eq!(listed, vec![linked]);
    }

    #[test]
    fn list_for_document_joins_through_measurement_and_page() {
        let conn = open_test_db();
        let (doc_id, measurement_id) = fixture_measurement(&conn);
        let linked = create(&conn, "in scope", 4.0, "ea", Some(&measurement_id), None, None).unwrap();
        create(&conn, "no measurement link", 1.0, "ea", None, None, None).unwrap();

        let listed = list_for_document(&conn, &doc_id).unwrap();
        assert_eq!(listed, vec![linked]);
    }

    #[test]
    fn csv_export_formats_rows_and_escapes_special_characters() {
        let items = vec![
            TakeoffItem {
                id: "1".into(),
                measurement_id: None,
                description: "#4 rebar".into(),
                quantity: 3.0,
                unit: "ea".into(),
                cost_per_unit: Some(12.5),
                notes: None,
            },
            TakeoffItem {
                id: "2".into(),
                measurement_id: None,
                description: "Wall A, north face".into(), // contains a comma
                quantity: 1.0,
                unit: "ea".into(),
                cost_per_unit: None,
                notes: Some("has \"quotes\"".into()),
            },
        ];

        let csv = export_csv(&items);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "description,quantity,unit,cost_per_unit,total_cost,notes");
        assert_eq!(lines[1], "#4 rebar,3,ea,12.5,37.5,");
        assert_eq!(
            lines[2],
            "\"Wall A, north face\",1,ea,,,\"has \"\"quotes\"\"\""
        );
    }

    #[test]
    fn csv_export_of_empty_list_is_just_the_header() {
        assert_eq!(
            export_csv(&[]),
            "description,quantity,unit,cost_per_unit,total_cost,notes\n"
        );
    }
}
