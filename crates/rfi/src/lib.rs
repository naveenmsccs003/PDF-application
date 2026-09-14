//! RFI (Request For Information) domain model (Section 7 layering: Domain
//! "Rfi", built on Infrastructure `mds_db`'s `rfi` table).
//!
//! Covers RFI-01 (create, tied to a document and optionally a specific
//! page/markup) and RFI-02 (status tracking: open/answered/closed) from
//! `docs/features/FEATURE_REGISTRY.md`. RFI-03 (drawing revision tracking)
//! is a separate concern that overlaps `document::DocumentVersionRecord`,
//! not touched here. RFI-04 (revision comparison/overlay) is BACKLOG.
//!
//! `page_id`/`markup_id` are both optional and independent of each other —
//! an RFI can be filed against a whole document, a specific page, or a
//! specific markup on a page (`ON DELETE SET NULL` on both so deleting the
//! page/markup an RFI pointed at doesn't destroy the RFI itself, since the
//! question/answer trail has value independent of the drawing element it
//! was originally raised against).

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum RfiError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("rfi {0} not found")]
    NotFound(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RfiStatus {
    Open,
    Answered,
    Closed,
}

impl RfiStatus {
    fn as_db_str(self) -> &'static str {
        match self {
            RfiStatus::Open => "open",
            RfiStatus::Answered => "answered",
            RfiStatus::Closed => "closed",
        }
    }

    fn from_db_str(s: &str) -> Result<Self, RfiError> {
        Ok(match s {
            "open" => RfiStatus::Open,
            "answered" => RfiStatus::Answered,
            "closed" => RfiStatus::Closed,
            other => return Err(RfiError::NotFound(format!("unknown rfi status `{other}`"))),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rfi {
    pub id: String,
    pub document_id: String,
    pub page_id: Option<String>,
    pub markup_id: Option<String>,
    pub number: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: RfiStatus,
    pub response: Option<String>,
    pub created_by: Option<String>,
}

const SELECT_COLUMNS: &str =
    "id, document_id, page_id, markup_id, number, title, description, status, response, created_by";

#[allow(clippy::type_complexity)]
fn row_to_rfi(
    row: &rusqlite::Row,
) -> rusqlite::Result<(
    String,
    String,
    Option<String>,
    Option<String>,
    i64,
    String,
    Option<String>,
    String,
    Option<String>,
    Option<String>,
)> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
    ))
}

#[allow(clippy::type_complexity)]
fn decode_rfi(
    (id, document_id, page_id, markup_id, number, title, description, status, response, created_by): (
        String,
        String,
        Option<String>,
        Option<String>,
        i64,
        String,
        Option<String>,
        String,
        Option<String>,
        Option<String>,
    ),
) -> Result<Rfi, RfiError> {
    Ok(Rfi {
        id,
        document_id,
        page_id,
        markup_id,
        number,
        title,
        description,
        status: RfiStatus::from_db_str(&status)?,
        response,
        created_by,
    })
}

/// RFI-01: creates an RFI tied to a document and, optionally, a specific
/// page and/or markup. `number` is assigned sequentially per document
/// (1, 2, 3, ...) so RFIs can be referred to the way they are on a real
/// job site ("RFI #14"), not by their opaque id.
/// Takes `&mut Connection` (not `&Connection`, unlike most of this crate)
/// so the number-assignment `SELECT` and the `INSERT` can share one
/// transaction, per the project's own "writes wrapped in transactions"
/// rule (`docs/database/README.md`) — without it, two concurrent callers
/// could both read the same `MAX(number)` and then both insert the same
/// number, hitting the `UNIQUE (document_id, number)` constraint instead
/// of getting the sequential numbers they should.
pub fn create(
    conn: &mut Connection,
    document_id: &str,
    page_id: Option<&str>,
    markup_id: Option<&str>,
    title: &str,
    description: Option<&str>,
    created_by: Option<&str>,
) -> Result<Rfi, RfiError> {
    let tx = conn.transaction()?;
    let number: i64 = tx.query_row(
        "SELECT COALESCE(MAX(number), 0) + 1 FROM rfi WHERE document_id = ?1",
        params![document_id],
        |row| row.get(0),
    )?;
    let rfi = Rfi {
        id: mds_db::new_uuid(),
        document_id: document_id.to_string(),
        page_id: page_id.map(str::to_string),
        markup_id: markup_id.map(str::to_string),
        number,
        title: title.to_string(),
        description: description.map(str::to_string),
        status: RfiStatus::Open,
        response: None,
        created_by: created_by.map(str::to_string),
    };
    tx.execute(
        "INSERT INTO rfi (id, document_id, page_id, markup_id, number, title, description, status, response, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            rfi.id,
            rfi.document_id,
            rfi.page_id,
            rfi.markup_id,
            rfi.number,
            rfi.title,
            rfi.description,
            rfi.status.as_db_str(),
            rfi.response,
            rfi.created_by,
        ],
    )?;
    tx.commit()?;
    Ok(rfi)
}

pub fn get(conn: &Connection, id: &str) -> Result<Rfi, RfiError> {
    let row = conn
        .query_row(
            &format!("SELECT {SELECT_COLUMNS} FROM rfi WHERE id = ?1"),
            params![id],
            row_to_rfi,
        )
        .optional()?
        .ok_or_else(|| RfiError::NotFound(id.to_string()))?;
    decode_rfi(row)
}

pub fn list_by_document(conn: &Connection, document_id: &str) -> Result<Vec<Rfi>, RfiError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM rfi WHERE document_id = ?1 ORDER BY number"
    ))?;
    let rows = stmt
        .query_map(params![document_id], row_to_rfi)?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(decode_rfi).collect()
}

/// RFI-02: moves an RFI through open -> answered -> closed (or back — no
/// state machine enforced, since a closed RFI sometimes gets reopened on a
/// real job and there's no requirement yet to forbid that). `response` is
/// the answer text; passing `None` leaves the RFI's existing response
/// untouched (e.g. moving straight to `Closed` without re-typing the
/// answer already recorded when it was marked `Answered`).
pub fn set_status(
    conn: &Connection,
    id: &str,
    status: RfiStatus,
    response: Option<&str>,
) -> Result<(), RfiError> {
    let n = match response {
        Some(text) => conn.execute(
            "UPDATE rfi SET status = ?1, response = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?3",
            params![status.as_db_str(), text, id],
        )?,
        None => conn.execute(
            "UPDATE rfi SET status = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
            params![status.as_db_str(), id],
        )?,
    };
    if n == 0 {
        return Err(RfiError::NotFound(id.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_document(conn: &Connection) -> (String, String, String) {
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
        (user_id, doc_id, page_id)
    }

    fn open_test_db() -> Connection {
        mds_db::open_and_migrate(":memory:").unwrap()
    }

    #[test]
    fn create_get_and_list_round_trip() {
        let mut conn = open_test_db();
        let (user_id, doc_id, page_id) = fixture_document(&conn);

        let created = create(
            &mut conn,
            &doc_id,
            Some(&page_id),
            None,
            "Rebar spacing at grid C4?",
            Some("Drawing shows 8in o.c. but spec calls for 6in"),
            Some(&user_id),
        )
        .unwrap();
        assert_eq!(created.number, 1);
        assert_eq!(created.status, RfiStatus::Open);

        let fetched = get(&conn, &created.id).unwrap();
        assert_eq!(fetched, created);

        let listed = list_by_document(&conn, &doc_id).unwrap();
        assert_eq!(listed, vec![created]);
    }

    #[test]
    fn numbers_increment_per_document() {
        let mut conn = open_test_db();
        let (_, doc_id, _) = fixture_document(&conn);

        let first = create(&mut conn, &doc_id, None, None, "First question", None, None).unwrap();
        let second = create(&mut conn, &doc_id, None, None, "Second question", None, None).unwrap();
        assert_eq!(first.number, 1);
        assert_eq!(second.number, 2);
    }

    #[test]
    fn set_status_records_response_and_updates_status() {
        let mut conn = open_test_db();
        let (_, doc_id, _) = fixture_document(&conn);
        let created = create(&mut conn, &doc_id, None, None, "Question", None, None).unwrap();

        set_status(&conn, &created.id, RfiStatus::Answered, Some("Use 6in o.c. per spec")).unwrap();
        let answered = get(&conn, &created.id).unwrap();
        assert_eq!(answered.status, RfiStatus::Answered);
        assert_eq!(answered.response.as_deref(), Some("Use 6in o.c. per spec"));

        set_status(&conn, &created.id, RfiStatus::Closed, None).unwrap();
        let closed = get(&conn, &created.id).unwrap();
        assert_eq!(closed.status, RfiStatus::Closed);
        assert_eq!(closed.response.as_deref(), Some("Use 6in o.c. per spec"));
    }

    #[test]
    fn set_status_on_missing_rfi_errors() {
        let conn = open_test_db();
        let err = set_status(&conn, "does-not-exist", RfiStatus::Closed, None).unwrap_err();
        assert!(matches!(err, RfiError::NotFound(_)));
    }

    #[test]
    fn deleting_page_sets_rfi_page_id_null_but_keeps_rfi() {
        let mut conn = open_test_db();
        let (_, doc_id, page_id) = fixture_document(&conn);
        let created = create(&mut conn, &doc_id, Some(&page_id), None, "Question", None, None).unwrap();

        conn.execute("DELETE FROM page WHERE id = ?1", params![page_id]).unwrap();

        let fetched = get(&conn, &created.id).unwrap();
        assert_eq!(fetched.page_id, None);
    }
}
