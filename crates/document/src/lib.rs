//! Document domain model (Section 7 layering: Domain "Document", built on
//! Core Engine `pdf_core` for reading PDF metadata and Infrastructure
//! `mds_db` for the `document`/`document_version`/`page` tables).
//!
//! Covers DOC-01, 02, 04, 05 from `docs/features/FEATURE_REGISTRY.md`:
//! opening/importing a PDF and recording its pages, versioning on save
//! (never overwrite), page listing (navigation), and page rotation/reorder.
//! DOC-03 (close) isn't domain logic — closing a `pdf_core::PdfDocument` is
//! just dropping the Rust value once this is wired into a real app, nothing
//! to persist or test in isolation. DOC-06 (shared project membership) is a
//! separate concern (Project, not Document) and isn't touched here.

use pdf_core::{PdfCoreError, PdfDocument};
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, thiserror::Error)]
pub enum DocumentError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("pdf engine error: {0}")]
    PdfCore(#[from] PdfCoreError),
    #[error("document {0} not found")]
    DocumentNotFound(String),
    #[error("page {0} not found")]
    PageNotFound(String),
    #[error("document version {0} not found")]
    VersionNotFound(String),
    #[error("rotation must be one of 0/90/180/270 degrees, got {0}")]
    InvalidRotation(i64),
    #[error("new page order must be a permutation of the document's existing pages")]
    ReorderMismatch,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentRecord {
    pub id: String,
    pub project_id: Option<String>,
    pub file_path: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageRecord {
    pub id: String,
    pub document_id: String,
    pub page_number: i64,
    pub width: f64,
    pub height: f64,
    pub rotation: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentVersionRecord {
    pub id: String,
    pub document_id: String,
    pub version_number: i64,
    pub file_snapshot_path: String,
    pub created_by: Option<String>,
}

/// DOC-01: opens a PDF via any `pdf_core::PdfDocument` and records it — one
/// `document` row plus one `page` row per PDF page, 1-based page numbers
/// matching `pdf_document`'s page order. Atomic: a failure partway through
/// leaves neither the document nor any of its pages behind.
pub fn import_document<D: PdfDocument>(
    conn: &mut Connection,
    pdf_document: &D,
    file_path: &str,
    title: &str,
    project_id: Option<&str>,
) -> Result<DocumentRecord, DocumentError> {
    let tx = conn.transaction()?;
    let document_id = mds_db::new_uuid();
    tx.execute(
        "INSERT INTO document (id, project_id, file_path, title) VALUES (?1, ?2, ?3, ?4)",
        params![document_id, project_id, file_path, title],
    )?;
    for page_index in 0..pdf_document.page_count() {
        let (width, height) = pdf_document.page_size(page_index)?;
        tx.execute(
            "INSERT INTO page (id, document_id, page_number, width, height) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                mds_db::new_uuid(),
                document_id,
                (page_index + 1) as i64,
                width as f64,
                height as f64,
            ],
        )?;
    }
    tx.commit()?;
    Ok(DocumentRecord {
        id: document_id,
        project_id: project_id.map(str::to_string),
        file_path: file_path.to_string(),
        title: title.to_string(),
    })
}

pub fn get_document(conn: &Connection, id: &str) -> Result<DocumentRecord, DocumentError> {
    conn.query_row(
        "SELECT id, project_id, file_path, title FROM document WHERE id = ?1",
        params![id],
        |row| {
            Ok(DocumentRecord {
                id: row.get(0)?,
                project_id: row.get(1)?,
                file_path: row.get(2)?,
                title: row.get(3)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| DocumentError::DocumentNotFound(id.to_string()))
}

/// Every document belonging to a project — the natural listing a
/// "documents" screen needs once DOC-01 (import) has produced any.
pub fn list_documents_for_project(
    conn: &Connection,
    project_id: &str,
) -> Result<Vec<DocumentRecord>, DocumentError> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, file_path, title FROM document
         WHERE project_id = ?1 ORDER BY created_at",
    )?;
    let rows = stmt
        .query_map(params![project_id], |row| {
            Ok(DocumentRecord {
                id: row.get(0)?,
                project_id: row.get(1)?,
                file_path: row.get(2)?,
                title: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// DOC-04: pages in navigation order.
pub fn list_pages(conn: &Connection, document_id: &str) -> Result<Vec<PageRecord>, DocumentError> {
    let mut stmt = conn.prepare(
        "SELECT id, document_id, page_number, width, height, rotation FROM page
         WHERE document_id = ?1 ORDER BY page_number",
    )?;
    let rows = stmt
        .query_map(params![document_id], |row| {
            Ok(PageRecord {
                id: row.get(0)?,
                document_id: row.get(1)?,
                page_number: row.get(2)?,
                width: row.get(3)?,
                height: row.get(4)?,
                rotation: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// DOC-05: rotate a page in 90-degree steps.
pub fn set_page_rotation(conn: &Connection, page_id: &str, rotation: i64) -> Result<(), DocumentError> {
    if ![0, 90, 180, 270].contains(&rotation) {
        return Err(DocumentError::InvalidRotation(rotation));
    }
    let n = conn.execute(
        "UPDATE page SET rotation = ?1 WHERE id = ?2",
        params![rotation, page_id],
    )?;
    if n == 0 {
        return Err(DocumentError::PageNotFound(page_id.to_string()));
    }
    Ok(())
}

/// DOC-05: reorders a document's pages to match `new_order` (page ids, in
/// the desired sequence). `new_order` must contain exactly the document's
/// current page ids, in some order — this reorders existing pages, it
/// doesn't add or remove any.
///
/// Renumbers in two passes to avoid the `UNIQUE (document_id,
/// page_number)` constraint colliding mid-update: first move every page
/// far outside the valid range, then assign final 1-based numbers.
pub fn reorder_pages(
    conn: &mut Connection,
    document_id: &str,
    new_order: &[String],
) -> Result<(), DocumentError> {
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare("SELECT id FROM page WHERE document_id = ?1")?;
        let mut existing: Vec<String> = stmt
            .query_map(params![document_id], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        existing.sort();
        let mut wanted = new_order.to_vec();
        wanted.sort();
        if existing != wanted {
            return Err(DocumentError::ReorderMismatch);
        }
    }

    for (offset, page_id) in new_order.iter().enumerate() {
        tx.execute(
            "UPDATE page SET page_number = ?1 WHERE id = ?2",
            params![-(1_000_000 + offset as i64), page_id],
        )?;
    }
    for (index, page_id) in new_order.iter().enumerate() {
        tx.execute(
            "UPDATE page SET page_number = ?1 WHERE id = ?2",
            params![(index + 1) as i64, page_id],
        )?;
    }

    tx.commit()?;
    Ok(())
}

/// DOC-02: a new snapshot version — never overwrites a prior one.
pub fn create_version(
    conn: &Connection,
    document_id: &str,
    file_snapshot_path: &str,
    created_by: Option<&str>,
) -> Result<DocumentVersionRecord, DocumentError> {
    let next_version: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version_number), 0) + 1 FROM document_version WHERE document_id = ?1",
        params![document_id],
        |row| row.get(0),
    )?;
    let id = mds_db::new_uuid();
    conn.execute(
        "INSERT INTO document_version (id, document_id, version_number, file_snapshot_path, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, document_id, next_version, file_snapshot_path, created_by],
    )?;
    Ok(DocumentVersionRecord {
        id,
        document_id: document_id.to_string(),
        version_number: next_version,
        file_snapshot_path: file_snapshot_path.to_string(),
        created_by: created_by.map(str::to_string),
    })
}

/// RFI-04: a single version by id — `list_versions` above already exists
/// for the "browse revisions" UI, but comparing two specific revisions
/// needs to fetch each one by the id the caller picked, not re-list and
/// filter.
pub fn get_version(conn: &Connection, version_id: &str) -> Result<DocumentVersionRecord, DocumentError> {
    conn.query_row(
        "SELECT id, document_id, version_number, file_snapshot_path, created_by
         FROM document_version WHERE id = ?1",
        params![version_id],
        |row| {
            Ok(DocumentVersionRecord {
                id: row.get(0)?,
                document_id: row.get(1)?,
                version_number: row.get(2)?,
                file_snapshot_path: row.get(3)?,
                created_by: row.get(4)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| DocumentError::VersionNotFound(version_id.to_string()))
}

pub fn list_versions(
    conn: &Connection,
    document_id: &str,
) -> Result<Vec<DocumentVersionRecord>, DocumentError> {
    let mut stmt = conn.prepare(
        "SELECT id, document_id, version_number, file_snapshot_path, created_by FROM document_version
         WHERE document_id = ?1 ORDER BY version_number",
    )?;
    let rows = stmt
        .query_map(params![document_id], |row| {
            Ok(DocumentVersionRecord {
                id: row.get(0)?,
                document_id: row.get(1)?,
                version_number: row.get(2)?,
                file_snapshot_path: row.get(3)?,
                created_by: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// A `PdfDocument` test double with fixed page sizes — lets
    /// `import_document` be tested without a real `libpdfium.so`.
    struct FakePdfDocument {
        page_sizes: Vec<(f32, f32)>,
    }

    impl PdfDocument for FakePdfDocument {
        fn page_count(&self) -> usize {
            self.page_sizes.len()
        }

        fn page_size(&self, page_index: usize) -> Result<(f32, f32), PdfCoreError> {
            self.page_sizes
                .get(page_index)
                .copied()
                .ok_or(PdfCoreError::PageIndexOutOfRange(page_index))
        }

        fn render_page_to_png(&self, _page_index: usize, _target_width: u32) -> Result<Vec<u8>, PdfCoreError> {
            Ok(Vec::new())
        }

        fn extract_text(&self, _page_index: usize) -> Result<String, PdfCoreError> {
            Ok(String::new())
        }

        fn save(&self, _path: &Path) -> Result<(), PdfCoreError> {
            Ok(())
        }
    }

    fn open_test_db() -> Connection {
        mds_db::open_and_migrate(":memory:").unwrap()
    }

    fn three_page_doc() -> FakePdfDocument {
        FakePdfDocument {
            page_sizes: vec![(612.0, 792.0), (612.0, 792.0), (792.0, 612.0)],
        }
    }

    #[test]
    fn import_document_creates_document_and_pages_in_order() {
        let mut conn = open_test_db();
        let fake = three_page_doc();

        let doc = import_document(&mut conn, &fake, "/tmp/x.pdf", "X", None).unwrap();
        assert_eq!(get_document(&conn, &doc.id).unwrap(), doc);

        let pages = list_pages(&conn, &doc.id).unwrap();
        assert_eq!(pages.len(), 3);
        assert_eq!(pages[0].page_number, 1);
        assert_eq!(pages[1].page_number, 2);
        assert_eq!(pages[2].page_number, 3);
        assert_eq!((pages[2].width, pages[2].height), (792.0, 612.0)); // landscape 3rd page
        assert!(pages.iter().all(|p| p.rotation == 0));
    }

    #[test]
    fn list_documents_for_project_scopes_correctly() {
        let mut conn = open_test_db();
        let fake = three_page_doc();

        let user_id = mds_db::new_uuid();
        conn.execute(
            "INSERT INTO user (id, email, display_name) VALUES (?1, 'a@b.com', 'A')",
            rusqlite::params![user_id],
        )
        .unwrap();
        let project_id = mds_db::new_uuid();
        conn.execute(
            "INSERT INTO project (id, name, created_by) VALUES (?1, 'P', ?2)",
            rusqlite::params![project_id, user_id],
        )
        .unwrap();

        let in_project = import_document(&mut conn, &fake, "/tmp/a.pdf", "A", Some(&project_id)).unwrap();
        import_document(&mut conn, &fake, "/tmp/b.pdf", "B", None).unwrap(); // no project

        let listed = list_documents_for_project(&conn, &project_id).unwrap();
        assert_eq!(listed, vec![in_project]);
    }

    #[test]
    fn set_page_rotation_validates_and_updates() {
        let mut conn = open_test_db();
        let fake = three_page_doc();
        let doc = import_document(&mut conn, &fake, "/tmp/x.pdf", "X", None).unwrap();
        let page_id = list_pages(&conn, &doc.id).unwrap()[0].id.clone();

        set_page_rotation(&conn, &page_id, 90).unwrap();
        assert_eq!(list_pages(&conn, &doc.id).unwrap()[0].rotation, 90);

        assert!(matches!(
            set_page_rotation(&conn, &page_id, 45),
            Err(DocumentError::InvalidRotation(45))
        ));
        assert!(matches!(
            set_page_rotation(&conn, "does-not-exist", 90),
            Err(DocumentError::PageNotFound(_))
        ));
    }

    #[test]
    fn reorder_pages_renumbers_without_unique_violation() {
        let mut conn = open_test_db();
        let fake = three_page_doc();
        let doc = import_document(&mut conn, &fake, "/tmp/x.pdf", "X", None).unwrap();
        let original = list_pages(&conn, &doc.id).unwrap();
        let (p1, p2, p3) = (original[0].id.clone(), original[1].id.clone(), original[2].id.clone());

        // Reverse the order: p3, p1, p2.
        reorder_pages(&mut conn, &doc.id, &[p3.clone(), p1.clone(), p2.clone()]).unwrap();

        let reordered = list_pages(&conn, &doc.id).unwrap();
        assert_eq!(reordered[0].id, p3);
        assert_eq!(reordered[0].page_number, 1);
        assert_eq!(reordered[1].id, p1);
        assert_eq!(reordered[1].page_number, 2);
        assert_eq!(reordered[2].id, p2);
        assert_eq!(reordered[2].page_number, 3);
    }

    #[test]
    fn reorder_pages_rejects_a_set_that_does_not_match() {
        let mut conn = open_test_db();
        let fake = three_page_doc();
        let doc = import_document(&mut conn, &fake, "/tmp/x.pdf", "X", None).unwrap();
        let original = list_pages(&conn, &doc.id).unwrap();

        // Missing the third page id, plus a bogus one.
        let bad_order = vec![original[0].id.clone(), original[1].id.clone(), "bogus".to_string()];
        assert!(matches!(
            reorder_pages(&mut conn, &doc.id, &bad_order),
            Err(DocumentError::ReorderMismatch)
        ));

        // Original order/numbers must be untouched after a rejected reorder.
        let unchanged = list_pages(&conn, &doc.id).unwrap();
        assert_eq!(unchanged, original);
    }

    #[test]
    fn create_version_never_overwrites_and_increments() {
        let mut conn = open_test_db();
        let fake = three_page_doc();
        let doc = import_document(&mut conn, &fake, "/tmp/x.pdf", "X", None).unwrap();

        let v1 = create_version(&conn, &doc.id, "/snap/v1.pdf", None).unwrap();
        let v2 = create_version(&conn, &doc.id, "/snap/v2.pdf", None).unwrap();
        assert_eq!(v1.version_number, 1);
        assert_eq!(v2.version_number, 2);

        let versions = list_versions(&conn, &doc.id).unwrap();
        assert_eq!(versions, vec![v1, v2]);
    }

    #[test]
    fn get_version_finds_by_id_and_errors_on_unknown() {
        let mut conn = open_test_db();
        let fake = three_page_doc();
        let doc = import_document(&mut conn, &fake, "/tmp/x.pdf", "X", None).unwrap();
        let v1 = create_version(&conn, &doc.id, "/snap/v1.pdf", None).unwrap();

        assert_eq!(get_version(&conn, &v1.id).unwrap(), v1);
        assert!(matches!(get_version(&conn, "missing"), Err(DocumentError::VersionNotFound(_))));
    }
}
