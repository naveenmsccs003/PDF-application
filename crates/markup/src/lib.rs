//! Markup domain model + Application-layer undo/redo (Section 7 layering:
//! Application "Commands, Undo/Redo" sits directly on top of Domain
//! "Markup"; bundled into one crate here for the same pragmatic reason
//! `pdf_core` bundled Core Engine trait+impl — there is no separate
//! Application-layer crate yet since the Tauri app (`app/src-tauri`) that
//! would normally host it is still blocked on missing system deps.
//!
//! Persists through `mds_db`'s `markup` table (page_id, type, geometry_json,
//! style_json, author, locked, hidden). Covers MARK-01..06, MARK-08 from
//! `docs/features/FEATURE_REGISTRY.md`: create, select/move/resize/edit,
//! delete, undo/redo, visibility toggle/lock. Does NOT cover MARK-05
//! (markup list/layer panel — UI), MARK-07 (comments/threads — separate
//! `markup_comment` table, not touched here), or spatial indexing
//! (`rstar`, for hit-testing — deferred until there's a canvas to hit-test
//! against).

use geometry::Point;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum MarkupError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("markup {0} not found")]
    NotFound(String),
    #[error("{kind:?} geometry needs at least {min} point(s), got {got}")]
    TooFewPoints {
        kind: MarkupType,
        min: usize,
        got: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarkupType {
    Text,
    Rectangle,
    Cloud,
    Line,
    Arrow,
}

impl MarkupType {
    fn min_points(self) -> usize {
        match self {
            MarkupType::Text => 1,
            MarkupType::Line | MarkupType::Arrow | MarkupType::Rectangle => 2,
            MarkupType::Cloud => 3,
        }
    }

    fn as_db_str(self) -> &'static str {
        match self {
            MarkupType::Text => "text",
            MarkupType::Rectangle => "rectangle",
            MarkupType::Cloud => "cloud",
            MarkupType::Line => "line",
            MarkupType::Arrow => "arrow",
        }
    }

    fn from_db_str(s: &str) -> Result<Self, MarkupError> {
        Ok(match s {
            "text" => MarkupType::Text,
            "rectangle" => MarkupType::Rectangle,
            "cloud" => MarkupType::Cloud,
            "line" => MarkupType::Line,
            "arrow" => MarkupType::Arrow,
            other => return Err(MarkupError::NotFound(format!("unknown markup type `{other}`"))),
        })
    }
}

/// Points in page space. Interpretation depends on `MarkupType`: two
/// opposite corners for `Rectangle`, endpoints for `Line`/`Arrow`, a closed
/// polygon for `Cloud`, a single anchor for `Text`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkupGeometry {
    pub points: Vec<(f64, f64)>,
}

impl MarkupGeometry {
    pub fn new(points: Vec<Point>) -> Self {
        Self {
            points: points.into_iter().map(|p| (p.x, p.y)).collect(),
        }
    }

    /// `pub` so callers routing through `Command`/`UndoStack` (which apply
    /// via `insert_full`/`update_geometry`'s raw writes, not `create`) can
    /// validate before constructing a command — `Command::apply` itself
    /// doesn't, so this is the one place that guarantee still needs to run.
    pub fn validate(&self, kind: MarkupType) -> Result<(), MarkupError> {
        let min = kind.min_points();
        if self.points.len() < min {
            return Err(MarkupError::TooFewPoints {
                kind,
                min,
                got: self.points.len(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MarkupStyle {
    pub color: Option<String>,
    pub stroke_width: Option<f64>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Markup {
    pub id: String,
    pub page_id: String,
    pub markup_type: MarkupType,
    pub geometry: MarkupGeometry,
    pub style: MarkupStyle,
    pub author: Option<String>,
    pub locked: bool,
    pub hidden: bool,
}

fn row_to_markup(row: &rusqlite::Row) -> rusqlite::Result<(String, String, String, String, String, Option<String>, bool, bool)> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get::<_, i64>(6)? != 0,
        row.get::<_, i64>(7)? != 0,
    ))
}

fn decode_markup(
    (id, page_id, type_str, geometry_json, style_json, author, locked, hidden): (
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        bool,
        bool,
    ),
) -> Result<Markup, MarkupError> {
    Ok(Markup {
        id,
        page_id,
        markup_type: MarkupType::from_db_str(&type_str)?,
        geometry: serde_json::from_str(&geometry_json)?,
        style: serde_json::from_str(&style_json)?,
        author,
        locked,
        hidden,
    })
}

const SELECT_COLUMNS: &str =
    "id, page_id, type, geometry_json, style_json, author, locked, hidden";

/// Creates a markup with a fresh id and inserts it. This is the normal
/// entry point for MARK-01/02 (text/shape creation).
pub fn create(
    conn: &Connection,
    page_id: &str,
    markup_type: MarkupType,
    geometry: MarkupGeometry,
    style: MarkupStyle,
    author: Option<&str>,
) -> Result<Markup, MarkupError> {
    geometry.validate(markup_type)?;
    let markup = Markup {
        id: mds_db::new_uuid(),
        page_id: page_id.to_string(),
        markup_type,
        geometry,
        style,
        author: author.map(str::to_string),
        locked: false,
        hidden: false,
    };
    insert_full(conn, &markup)?;
    Ok(markup)
}

/// Inserts a markup that already has an id — used for the normal create
/// path above, and to replay a `Command::Delete` undo or a `Command::Create`
/// redo without changing the markup's identity.
fn insert_full(conn: &Connection, markup: &Markup) -> Result<(), MarkupError> {
    conn.execute(
        "INSERT INTO markup (id, page_id, type, geometry_json, style_json, author, locked, hidden)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            markup.id,
            markup.page_id,
            markup.markup_type.as_db_str(),
            serde_json::to_string(&markup.geometry)?,
            serde_json::to_string(&markup.style)?,
            markup.author,
            markup.locked as i64,
            markup.hidden as i64,
        ],
    )?;
    Ok(())
}

pub fn get(conn: &Connection, id: &str) -> Result<Markup, MarkupError> {
    let row = conn
        .query_row(
            &format!("SELECT {SELECT_COLUMNS} FROM markup WHERE id = ?1"),
            params![id],
            row_to_markup,
        )
        .optional()?
        .ok_or_else(|| MarkupError::NotFound(id.to_string()))?;
    decode_markup(row)
}

pub fn list_by_page(conn: &Connection, page_id: &str) -> Result<Vec<Markup>, MarkupError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM markup WHERE page_id = ?1 ORDER BY created_at"
    ))?;
    let rows = stmt
        .query_map(params![page_id], row_to_markup)?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(decode_markup).collect()
}

/// MARK-03 (move/resize — both just change the geometry).
pub fn update_geometry(
    conn: &Connection,
    id: &str,
    markup_type: MarkupType,
    geometry: &MarkupGeometry,
) -> Result<(), MarkupError> {
    geometry.validate(markup_type)?;
    let n = conn.execute(
        "UPDATE markup SET geometry_json = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
        params![serde_json::to_string(geometry)?, id],
    )?;
    if n == 0 {
        return Err(MarkupError::NotFound(id.to_string()));
    }
    Ok(())
}

/// MARK-03 (edit — style/text content).
pub fn update_style(conn: &Connection, id: &str, style: &MarkupStyle) -> Result<(), MarkupError> {
    let n = conn.execute(
        "UPDATE markup SET style_json = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
        params![serde_json::to_string(style)?, id],
    )?;
    if n == 0 {
        return Err(MarkupError::NotFound(id.to_string()));
    }
    Ok(())
}

/// MARK-08 (lock).
pub fn set_locked(conn: &Connection, id: &str, locked: bool) -> Result<(), MarkupError> {
    let n = conn.execute(
        "UPDATE markup SET locked = ?1 WHERE id = ?2",
        params![locked as i64, id],
    )?;
    if n == 0 {
        return Err(MarkupError::NotFound(id.to_string()));
    }
    Ok(())
}

/// MARK-08 (visibility toggle).
pub fn set_hidden(conn: &Connection, id: &str, hidden: bool) -> Result<(), MarkupError> {
    let n = conn.execute(
        "UPDATE markup SET hidden = ?1 WHERE id = ?2",
        params![hidden as i64, id],
    )?;
    if n == 0 {
        return Err(MarkupError::NotFound(id.to_string()));
    }
    Ok(())
}

/// MARK-04. Returns the deleted row so callers (namely `UndoStack`) can
/// restore it on undo without a round trip.
pub fn delete(conn: &Connection, id: &str) -> Result<Markup, MarkupError> {
    let markup = get(conn, id)?;
    conn.execute("DELETE FROM markup WHERE id = ?1", params![id])?;
    Ok(markup)
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarkupComment {
    pub id: String,
    pub markup_id: String,
    pub author: Option<String>,
    pub text: String,
}

/// MARK-07: a comment thread entry on a markup — needed for the async
/// review workflow (see `docs/architecture/adr/ADR-001-build-vs-extend.md`,
/// "confirmed multi-user collaboration requirement"). Separate from
/// `Command`/`UndoStack` above: comments are a discussion trail, not an
/// editable property someone would undo.
pub fn add_comment(
    conn: &Connection,
    markup_id: &str,
    author: Option<&str>,
    text: &str,
) -> Result<MarkupComment, MarkupError> {
    let id = mds_db::new_uuid();
    conn.execute(
        "INSERT INTO markup_comment (id, markup_id, author, text) VALUES (?1, ?2, ?3, ?4)",
        params![id, markup_id, author, text],
    )?;
    Ok(MarkupComment {
        id,
        markup_id: markup_id.to_string(),
        author: author.map(str::to_string),
        text: text.to_string(),
    })
}

pub fn list_comments(conn: &Connection, markup_id: &str) -> Result<Vec<MarkupComment>, MarkupError> {
    let mut stmt = conn.prepare(
        "SELECT id, markup_id, author, text FROM markup_comment WHERE markup_id = ?1 ORDER BY created_at",
    )?;
    let rows = stmt
        .query_map(params![markup_id], |row| {
            Ok(MarkupComment {
                id: row.get(0)?,
                markup_id: row.get(1)?,
                author: row.get(2)?,
                text: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn delete_comment(conn: &Connection, id: &str) -> Result<(), MarkupError> {
    let n = conn.execute("DELETE FROM markup_comment WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(MarkupError::NotFound(id.to_string()));
    }
    Ok(())
}

/// MARK-06: Application-layer undo/redo, Command pattern. Each command
/// stores enough state to both apply and revert itself, so `UndoStack`
/// doesn't need to know about individual field types.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Create(Markup),
    Delete(Markup),
    SetGeometry {
        id: String,
        markup_type: MarkupType,
        before: MarkupGeometry,
        after: MarkupGeometry,
    },
    SetStyle {
        id: String,
        before: MarkupStyle,
        after: MarkupStyle,
    },
    SetLocked {
        id: String,
        before: bool,
        after: bool,
    },
    SetHidden {
        id: String,
        before: bool,
        after: bool,
    },
}

impl Command {
    fn apply(&self, conn: &Connection) -> Result<(), MarkupError> {
        match self {
            Command::Create(markup) => {
                // `insert_full` itself performs no validation (unlike
                // `create()`, which validates before calling it) — the one
                // production call site (`commands::markup::create_markup`)
                // already validates first, but that's a caller convention,
                // not something this structurally enforces. Validating
                // here too closes that gap for any future `Command::Create`
                // caller (batch import, duplicate/paste, a test helper)
                // that might forget to.
                markup.geometry.validate(markup.markup_type)?;
                insert_full(conn, markup)
            }
            Command::Delete(markup) => conn
                .execute("DELETE FROM markup WHERE id = ?1", params![markup.id])
                .map(|_| ())
                .map_err(MarkupError::from),
            Command::SetGeometry {
                id,
                markup_type,
                after,
                ..
            } => update_geometry(conn, id, *markup_type, after),
            Command::SetStyle { id, after, .. } => update_style(conn, id, after),
            Command::SetLocked { id, after, .. } => set_locked(conn, id, *after),
            Command::SetHidden { id, after, .. } => set_hidden(conn, id, *after),
        }
    }

    fn revert(&self, conn: &Connection) -> Result<(), MarkupError> {
        match self {
            Command::Create(markup) => conn
                .execute("DELETE FROM markup WHERE id = ?1", params![markup.id])
                .map(|_| ())
                .map_err(MarkupError::from),
            Command::Delete(markup) => insert_full(conn, markup),
            Command::SetGeometry {
                id,
                markup_type,
                before,
                ..
            } => update_geometry(conn, id, *markup_type, before),
            Command::SetStyle { id, before, .. } => update_style(conn, id, before),
            Command::SetLocked { id, before, .. } => set_locked(conn, id, *before),
            Command::SetHidden { id, before, .. } => set_hidden(conn, id, *before),
        }
    }
}

#[derive(Debug, Default)]
pub struct UndoStack {
    undo: Vec<Command>,
    redo: Vec<Command>,
}

impl UndoStack {
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies `command` and records it. Clears the redo stack, matching
    /// standard undo/redo semantics: a fresh action invalidates whatever
    /// was undone before it.
    pub fn execute(&mut self, conn: &Connection, command: Command) -> Result<(), MarkupError> {
        command.apply(conn)?;
        self.undo.push(command);
        self.redo.clear();
        Ok(())
    }

    pub fn undo(&mut self, conn: &Connection) -> Result<bool, MarkupError> {
        let Some(command) = self.undo.pop() else {
            return Ok(false);
        };
        command.revert(conn)?;
        self.redo.push(command);
        Ok(true)
    }

    pub fn redo(&mut self, conn: &Connection) -> Result<bool, MarkupError> {
        let Some(command) = self.redo.pop() else {
            return Ok(false);
        };
        command.apply(conn)?;
        self.undo.push(command);
        Ok(true)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal user/document/page fixture, mirroring `mds_db`'s own tests,
    /// since `markup` rows need a real `page_id` under FK enforcement.
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

    fn open_test_db() -> Connection {
        mds_db::open_and_migrate(":memory:").unwrap()
    }

    fn rect(x1: f64, y1: f64, x2: f64, y2: f64) -> MarkupGeometry {
        MarkupGeometry::new(vec![Point::new(x1, y1), Point::new(x2, y2)])
    }

    #[test]
    fn create_get_and_list_round_trip() {
        let conn = open_test_db();
        let page_id = fixture_page(&conn);

        let created = create(
            &conn,
            &page_id,
            MarkupType::Rectangle,
            rect(1.0, 2.0, 3.0, 4.0),
            MarkupStyle {
                color: Some("#ff0000".into()),
                ..Default::default()
            },
            None,
        )
        .unwrap();

        let fetched = get(&conn, &created.id).unwrap();
        assert_eq!(fetched, created);

        let listed = list_by_page(&conn, &page_id).unwrap();
        assert_eq!(listed, vec![created]);
    }

    #[test]
    fn create_rejects_too_few_points_for_shape() {
        let conn = open_test_db();
        let page_id = fixture_page(&conn);

        let err = create(
            &conn,
            &page_id,
            MarkupType::Rectangle,
            MarkupGeometry::new(vec![Point::new(0.0, 0.0)]),
            MarkupStyle::default(),
            None,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            MarkupError::TooFewPoints {
                kind: MarkupType::Rectangle,
                min: 2,
                got: 1
            }
        ));
    }

    #[test]
    fn delete_removes_row_and_returns_snapshot() {
        let conn = open_test_db();
        let page_id = fixture_page(&conn);
        let created = create(
            &conn,
            &page_id,
            MarkupType::Text,
            MarkupGeometry::new(vec![Point::new(5.0, 5.0)]),
            MarkupStyle {
                text: Some("hello".into()),
                ..Default::default()
            },
            None,
        )
        .unwrap();

        let deleted = delete(&conn, &created.id).unwrap();
        assert_eq!(deleted, created);
        assert!(matches!(get(&conn, &created.id), Err(MarkupError::NotFound(_))));
    }

    #[test]
    fn lock_and_hidden_toggle() {
        let conn = open_test_db();
        let page_id = fixture_page(&conn);
        let created = create(
            &conn,
            &page_id,
            MarkupType::Line,
            rect(0.0, 0.0, 1.0, 1.0),
            MarkupStyle::default(),
            None,
        )
        .unwrap();

        set_locked(&conn, &created.id, true).unwrap();
        set_hidden(&conn, &created.id, true).unwrap();
        let fetched = get(&conn, &created.id).unwrap();
        assert!(fetched.locked);
        assert!(fetched.hidden);
    }

    #[test]
    fn undo_create_then_redo() {
        let conn = open_test_db();
        let page_id = fixture_page(&conn);
        let markup = Markup {
            id: mds_db::new_uuid(),
            page_id: page_id.clone(),
            markup_type: MarkupType::Cloud,
            geometry: MarkupGeometry::new(vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(0.0, 1.0),
            ]),
            style: MarkupStyle::default(),
            author: None,
            locked: false,
            hidden: false,
        };

        let mut stack = UndoStack::new();
        stack
            .execute(&conn, Command::Create(markup.clone()))
            .unwrap();
        assert_eq!(get(&conn, &markup.id).unwrap(), markup);

        assert!(stack.undo(&conn).unwrap());
        assert!(matches!(get(&conn, &markup.id), Err(MarkupError::NotFound(_))));

        assert!(stack.redo(&conn).unwrap());
        assert_eq!(get(&conn, &markup.id).unwrap(), markup);
    }

    #[test]
    fn command_create_rejects_too_few_points_for_shape() {
        let conn = open_test_db();
        let page_id = fixture_page(&conn);
        let markup = Markup {
            id: mds_db::new_uuid(),
            page_id,
            markup_type: MarkupType::Rectangle,
            geometry: MarkupGeometry::new(vec![Point::new(0.0, 0.0)]),
            style: MarkupStyle::default(),
            author: None,
            locked: false,
            hidden: false,
        };

        let mut stack = UndoStack::new();
        let err = stack
            .execute(&conn, Command::Create(markup.clone()))
            .unwrap_err();
        assert!(matches!(err, MarkupError::TooFewPoints { .. }));
        assert!(matches!(get(&conn, &markup.id), Err(MarkupError::NotFound(_))));
    }

    #[test]
    fn undo_move_restores_previous_geometry() {
        let conn = open_test_db();
        let page_id = fixture_page(&conn);
        let created = create(
            &conn,
            &page_id,
            MarkupType::Rectangle,
            rect(0.0, 0.0, 1.0, 1.0),
            MarkupStyle::default(),
            None,
        )
        .unwrap();

        let moved = rect(5.0, 5.0, 6.0, 6.0);
        let mut stack = UndoStack::new();
        stack
            .execute(
                &conn,
                Command::SetGeometry {
                    id: created.id.clone(),
                    markup_type: MarkupType::Rectangle,
                    before: created.geometry.clone(),
                    after: moved.clone(),
                },
            )
            .unwrap();
        assert_eq!(get(&conn, &created.id).unwrap().geometry, moved);

        assert!(stack.undo(&conn).unwrap());
        assert_eq!(get(&conn, &created.id).unwrap().geometry, created.geometry);

        assert!(stack.redo(&conn).unwrap());
        assert_eq!(get(&conn, &created.id).unwrap().geometry, moved);
    }

    #[test]
    fn undo_delete_restores_row() {
        let conn = open_test_db();
        let page_id = fixture_page(&conn);
        let created = create(
            &conn,
            &page_id,
            MarkupType::Arrow,
            rect(0.0, 0.0, 2.0, 2.0),
            MarkupStyle::default(),
            None,
        )
        .unwrap();

        let mut stack = UndoStack::new();
        // Command::Delete expects the pre-delete snapshot to already be
        // captured (matching how a caller would use `delete()`'s return
        // value); executing it performs the actual deletion.
        stack
            .execute(&conn, Command::Delete(created.clone()))
            .unwrap();
        assert!(matches!(get(&conn, &created.id), Err(MarkupError::NotFound(_))));

        assert!(stack.undo(&conn).unwrap());
        assert_eq!(get(&conn, &created.id).unwrap(), created);
    }

    #[test]
    fn new_command_after_undo_clears_redo_stack() {
        let conn = open_test_db();
        let page_id = fixture_page(&conn);
        let created = create(
            &conn,
            &page_id,
            MarkupType::Rectangle,
            rect(0.0, 0.0, 1.0, 1.0),
            MarkupStyle::default(),
            None,
        )
        .unwrap();

        let mut stack = UndoStack::new();
        stack
            .execute(
                &conn,
                Command::SetLocked {
                    id: created.id.clone(),
                    before: false,
                    after: true,
                },
            )
            .unwrap();
        stack.undo(&conn).unwrap();
        assert!(stack.can_redo());

        stack
            .execute(
                &conn,
                Command::SetHidden {
                    id: created.id.clone(),
                    before: false,
                    after: true,
                },
            )
            .unwrap();
        assert!(!stack.can_redo());
        assert!(!stack.undo(&conn).is_err());
    }

    #[test]
    fn add_list_and_delete_comments() {
        let conn = open_test_db();
        let page_id = fixture_page(&conn);
        let markup = create(
            &conn,
            &page_id,
            MarkupType::Text,
            MarkupGeometry::new(vec![Point::new(1.0, 1.0)]),
            MarkupStyle::default(),
            None,
        )
        .unwrap();

        let c1 = add_comment(&conn, &markup.id, None, "check this dimension").unwrap();
        let c2 = add_comment(&conn, &markup.id, None, "looks right to me").unwrap();

        let comments = list_comments(&conn, &markup.id).unwrap();
        assert_eq!(comments, vec![c1.clone(), c2]);

        delete_comment(&conn, &c1.id).unwrap();
        let remaining = list_comments(&conn, &markup.id).unwrap();
        assert_eq!(remaining.len(), 1);
    }

    #[test]
    fn deleting_markup_cascades_its_comments() {
        let conn = open_test_db();
        let page_id = fixture_page(&conn);
        let markup = create(
            &conn,
            &page_id,
            MarkupType::Text,
            MarkupGeometry::new(vec![Point::new(1.0, 1.0)]),
            MarkupStyle::default(),
            None,
        )
        .unwrap();
        add_comment(&conn, &markup.id, None, "a comment").unwrap();

        delete(&conn, &markup.id).unwrap();

        assert!(list_comments(&conn, &markup.id).unwrap().is_empty());
    }

    #[test]
    fn undo_and_redo_on_empty_stack_are_no_ops() {
        let conn = open_test_db();
        let mut stack = UndoStack::new();
        assert!(!stack.undo(&conn).unwrap());
        assert!(!stack.redo(&conn).unwrap());
    }
}
