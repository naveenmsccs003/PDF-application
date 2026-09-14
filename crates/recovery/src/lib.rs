//! REL-01/02 (Autosave, Crash recovery) domain model, backed by `mds_db`'s
//! `recovery_state` table (`id`, `document_id`, `snapshot_path`,
//! `created_at`).
//!
//! Design decision, since neither the master list nor the build prompt
//! spell out what "autosave"/"restore" mean once you notice every
//! Markup/Measurement/RFI mutation in this app already writes straight to
//! SQLite on the action that made it (no in-memory "unsaved changes"
//! buffer to lose on a crash the way a traditional editor has): a snapshot
//! here is a periodic **flattened PDF copy** of a document (see
//! `crates/export`, which this crate deliberately does not depend on —
//! the app layer builds the PDF and just hands this crate a path to
//! record), not a backup of the live database. "Autosave" = the app layer
//! periodically renders one of these and calls `create` to record it.
//! "Restore" = hand the user their own copy of a recent snapshot file
//! (e.g. via a save dialog) rather than this crate — or the IPC layer
//! above it — silently overwriting any live Markup/Measurement rows; the
//! one thing a restore could plausibly do that genuinely isn't durable
//! elsewhere is recover the in-memory-only `markup::UndoStack` history,
//! and a flattened PDF can't reconstruct that either, so "restore" staying
//! non-destructive (get the file back, don't touch the database) is the
//! safe interpretation until a real incident says otherwise. "Discard" =
//! `delete`.

use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, thiserror::Error)]
pub enum RecoveryError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("recovery snapshot {0} not found")]
    NotFound(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoverySnapshot {
    pub id: String,
    pub document_id: String,
    pub snapshot_path: String,
    pub created_at: String,
}

const SELECT_COLUMNS: &str = "id, document_id, snapshot_path, created_at";

fn row_to_snapshot(row: &rusqlite::Row) -> rusqlite::Result<RecoverySnapshot> {
    Ok(RecoverySnapshot {
        id: row.get(0)?,
        document_id: row.get(1)?,
        snapshot_path: row.get(2)?,
        created_at: row.get(3)?,
    })
}

/// REL-01: records that a snapshot file was written for `document_id`.
/// Writing the file itself is the app layer's job (it needs the PDF
/// engine); this just records where it landed.
pub fn create(conn: &Connection, document_id: &str, snapshot_path: &str) -> Result<RecoverySnapshot, RecoveryError> {
    let snapshot = RecoverySnapshot {
        id: mds_db::new_uuid(),
        document_id: document_id.to_string(),
        snapshot_path: snapshot_path.to_string(),
        created_at: String::new(), // overwritten by the DB default; refetched below
    };
    conn.execute(
        "INSERT INTO recovery_state (id, document_id, snapshot_path) VALUES (?1, ?2, ?3)",
        params![snapshot.id, snapshot.document_id, snapshot.snapshot_path],
    )?;
    get(conn, &snapshot.id)
}

pub fn get(conn: &Connection, id: &str) -> Result<RecoverySnapshot, RecoveryError> {
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM recovery_state WHERE id = ?1"),
        params![id],
        row_to_snapshot,
    )
    .optional()?
    .ok_or_else(|| RecoveryError::NotFound(id.to_string()))
}

/// Newest first, for a recovery picker.
pub fn list_by_document(conn: &Connection, document_id: &str) -> Result<Vec<RecoverySnapshot>, RecoveryError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM recovery_state WHERE document_id = ?1 ORDER BY created_at DESC"
    ))?;
    let rows = stmt.query_map(params![document_id], row_to_snapshot)?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// REL-02 (discard). Returns the deleted row so the caller can also remove
/// its file — this crate only ever touches the database.
pub fn delete(conn: &Connection, id: &str) -> Result<RecoverySnapshot, RecoveryError> {
    let snapshot = get(conn, id)?;
    conn.execute("DELETE FROM recovery_state WHERE id = ?1", params![id])?;
    Ok(snapshot)
}

/// REL-01's disk-bounding half: an autosave that never prunes anything
/// will fill the disk. Deletes every row for `document_id` past the
/// `keep` most recent, returning the deleted rows so the caller can also
/// remove their now-orphaned files.
pub fn prune_oldest(conn: &Connection, document_id: &str, keep: usize) -> Result<Vec<RecoverySnapshot>, RecoveryError> {
    let all = list_by_document(conn, document_id)?;
    let to_prune = all.into_iter().skip(keep).collect::<Vec<_>>();
    for snapshot in &to_prune {
        conn.execute("DELETE FROM recovery_state WHERE id = ?1", params![snapshot.id])?;
    }
    Ok(to_prune)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_document(conn: &Connection) -> String {
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
        doc_id
    }

    fn open_test_db() -> Connection {
        mds_db::open_and_migrate(":memory:").unwrap()
    }

    #[test]
    fn create_get_and_list_round_trip() {
        let conn = open_test_db();
        let doc_id = fixture_document(&conn);

        let created = create(&conn, &doc_id, "/tmp/recovery/snap1.pdf").unwrap();
        assert_eq!(created.document_id, doc_id);
        assert!(!created.created_at.is_empty());

        let fetched = get(&conn, &created.id).unwrap();
        assert_eq!(fetched, created);

        let listed = list_by_document(&conn, &doc_id).unwrap();
        assert_eq!(listed, vec![created]);
    }

    #[test]
    fn list_orders_newest_first() {
        let conn = open_test_db();
        let doc_id = fixture_document(&conn);

        let first = create(&conn, &doc_id, "/tmp/1.pdf").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let second = create(&conn, &doc_id, "/tmp/2.pdf").unwrap();

        let listed = list_by_document(&conn, &doc_id).unwrap();
        assert_eq!(listed, vec![second, first]);
    }

    #[test]
    fn delete_removes_row_and_returns_it() {
        let conn = open_test_db();
        let doc_id = fixture_document(&conn);
        let created = create(&conn, &doc_id, "/tmp/1.pdf").unwrap();

        let deleted = delete(&conn, &created.id).unwrap();
        assert_eq!(deleted, created);
        assert!(matches!(get(&conn, &created.id), Err(RecoveryError::NotFound(_))));
    }

    #[test]
    fn prune_oldest_keeps_only_the_most_recent() {
        let conn = open_test_db();
        let doc_id = fixture_document(&conn);

        let mut created = Vec::new();
        for i in 0..5 {
            created.push(create(&conn, &doc_id, &format!("/tmp/{i}.pdf")).unwrap());
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        let pruned = prune_oldest(&conn, &doc_id, 2).unwrap();
        assert_eq!(pruned.len(), 3);

        let remaining = list_by_document(&conn, &doc_id).unwrap();
        assert_eq!(remaining.len(), 2);
        // The 2 most recent (last created) should survive.
        assert_eq!(remaining[0], created[4]);
        assert_eq!(remaining[1], created[3]);
    }

    #[test]
    fn deleting_document_cascades_its_recovery_snapshots() {
        let conn = open_test_db();
        let doc_id = fixture_document(&conn);
        create(&conn, &doc_id, "/tmp/1.pdf").unwrap();

        conn.execute("DELETE FROM document WHERE id = ?1", params![doc_id]).unwrap();

        assert!(list_by_document(&conn, &doc_id).unwrap().is_empty());
    }
}
