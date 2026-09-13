use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};

#[derive(thiserror::Error, Debug)]
pub enum DbError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("migration error: {0}")]
    Migration(#[from] rusqlite_migration::Error),
}

fn migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(include_str!("../migrations/0001_initial.sql"))])
}

/// Opens a SQLite connection at `path`, enables foreign key enforcement and
/// WAL mode, and applies all pending migrations.
pub fn open_and_migrate(path: &str) -> Result<Connection, DbError> {
    let mut conn = Connection::open(path)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    migrations().to_latest(&mut conn)?;
    Ok(conn)
}

pub fn new_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_names(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap();
        stmt.query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    }

    #[test]
    fn migration_creates_all_expected_tables() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        migrations().to_latest(&mut conn).unwrap();

        let tables = table_names(&conn);
        for expected in [
            "user",
            "project",
            "project_member",
            "document",
            "document_version",
            "page",
            "markup",
            "markup_comment",
            "scale",
            "measurement",
            "takeoff_item",
            "recovery_state",
        ] {
            assert!(
                tables.iter().any(|t| t == expected),
                "expected table `{expected}` to exist, got tables: {tables:?}"
            );
        }
    }

    #[test]
    fn foreign_key_enforcement_rejects_orphan_page() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        migrations().to_latest(&mut conn).unwrap();

        let result = conn.execute(
            "INSERT INTO page (id, document_id, page_number, width, height) VALUES (?1, ?2, 1, 100.0, 200.0)",
            rusqlite::params![new_uuid(), "does-not-exist"],
        );
        assert!(result.is_err(), "expected FK violation to be rejected");
    }

    #[test]
    fn cascade_delete_removes_child_markup() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        migrations().to_latest(&mut conn).unwrap();

        let user_id = new_uuid();
        conn.execute(
            "INSERT INTO user (id, email, display_name) VALUES (?1, 'a@b.com', 'A')",
            rusqlite::params![user_id],
        )
        .unwrap();
        let doc_id = new_uuid();
        conn.execute(
            "INSERT INTO document (id, file_path, title) VALUES (?1, '/tmp/x.pdf', 'X')",
            rusqlite::params![doc_id],
        )
        .unwrap();
        let page_id = new_uuid();
        conn.execute(
            "INSERT INTO page (id, document_id, page_number, width, height) VALUES (?1, ?2, 1, 100.0, 200.0)",
            rusqlite::params![page_id, doc_id],
        )
        .unwrap();
        let markup_id = new_uuid();
        conn.execute(
            "INSERT INTO markup (id, page_id, type, geometry_json, style_json) VALUES (?1, ?2, 'text', '{}', '{}')",
            rusqlite::params![markup_id, page_id],
        )
        .unwrap();

        conn.execute("DELETE FROM page WHERE id = ?1", rusqlite::params![page_id])
            .unwrap();

        let remaining: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM markup WHERE id = ?1",
                rusqlite::params![markup_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            remaining, 0,
            "markup should be cascade-deleted with its page"
        );
    }
}
