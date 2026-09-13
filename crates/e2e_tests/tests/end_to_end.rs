//! Cross-crate integration test for the MVP Release Checkpoint workflow
//! named in `docs/PROJECT_PLAN.md`: "open → markup → save → measure →
//! takeoff → export → reopen → verify". Every crate this touches
//! (`document`, `markup`, `measurement`, `takeoff`, `project`, `mds_db`,
//! `geometry`) already has its own unit tests against an in-memory
//! database; what those can't catch is a real close-and-reopen of an
//! on-disk SQLite file, or a mismatch at the boundary between two crates'
//! APIs. This test exercises both, using a real temp file rather than
//! `:memory:` specifically so "reopen" means a fresh `Connection` against
//! the same bytes on disk, not the same live connection.
//!
//! Deliberately NOT covered (not built yet, or not this test's job):
//! actual PDF rendering/UI, autosave/crash recovery (REL-01/02), password
//! protection (SEC-01), and anything requiring `app/src-tauri` (blocked on
//! system deps — see `docs/PROJECT_PLAN.md`).

use geometry::Point;
use measurement::LengthUnit;
use pdf_core::{PdfCoreError, PdfDocument};
use std::path::{Path, PathBuf};

/// Deletes the SQLite file (and its `-wal`/`-shm` siblings, since
/// `mds_db::open_and_migrate` enables WAL mode) when dropped, so a failing
/// assertion still cleans up instead of leaking temp files.
struct TempDbFile(PathBuf);

impl TempDbFile {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("mds_e2e_{}.sqlite", mds_db::new_uuid()));
        Self(path)
    }

    fn path_str(&self) -> &str {
        self.0.to_str().unwrap()
    }
}

impl Drop for TempDbFile {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", self.0.display()));
        }
    }
}

/// A `PdfDocument` test double — this test is about the domain/persistence
/// layers composing correctly, not about `pdf_core`'s PDFium binding
/// (already covered by `pdf_core`'s own tests against a real
/// `libpdfium.so`).
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

#[test]
fn full_workflow_survives_a_real_close_and_reopen() {
    let db_file = TempDbFile::new();

    // -- Session 1: open -> markup -> measure -> takeoff -> export -> save --
    let (
        project_id,
        owner_id,
        document_id,
        page_id,
        markup_id,
        length_measurement_id,
        count_measurement_id,
        takeoff_id,
        csv,
    ) = {
        let mut conn = mds_db::open_and_migrate(db_file.path_str()).unwrap();

        let owner = project::create_user(&conn, "estimator@mds.example", "Estimator").unwrap();
        let field_crew = project::create_user(&conn, "crew@mds.example", "Field Crew").unwrap();
        let proj = project::create_project(&mut conn, "Rebar Job 42", &owner.id).unwrap();
        project::add_member(&conn, &proj.id, &field_crew.id, "viewer").unwrap();

        let fake_pdf = FakePdfDocument {
            page_sizes: vec![(612.0, 792.0)], // one US-Letter page
        };
        let doc = document::import_document(
            &mut conn,
            &fake_pdf,
            "/plans/job42.pdf",
            "Job 42 Foundation Plan",
            Some(&proj.id),
        )
        .unwrap();
        let pages = document::list_pages(&conn, &doc.id).unwrap();
        assert_eq!(pages.len(), 1);
        let page = pages[0].clone();

        // Markup: draw a rectangle via the undo/redo command path (not
        // `markup::create` directly), to prove `UndoStack` composes with
        // real persistence, then leave it in the "created" state.
        let rect_markup = markup::Markup {
            id: mds_db::new_uuid(),
            page_id: page.id.clone(),
            markup_type: markup::MarkupType::Rectangle,
            geometry: markup::MarkupGeometry::new(vec![Point::new(0.0, 0.0), Point::new(2.0, 1.0)]),
            style: markup::MarkupStyle {
                color: Some("#ff0000".into()),
                ..Default::default()
            },
            author: Some(owner.id.clone()),
            locked: false,
            hidden: false,
        };
        let mut undo_stack = markup::UndoStack::new();
        undo_stack
            .execute(&conn, markup::Command::Create(rect_markup.clone()))
            .unwrap();
        // Undo then redo, to prove the round trip actually leaves the
        // expected end state (not just "something is undoable").
        assert!(undo_stack.undo(&conn).unwrap());
        assert!(markup::get(&conn, &rect_markup.id).is_err());
        assert!(undo_stack.redo(&conn).unwrap());
        assert_eq!(markup::get(&conn, &rect_markup.id).unwrap(), rect_markup);

        markup::add_comment(&conn, &rect_markup.id, Some(&field_crew.id), "field verify this wall").unwrap();

        // Measurement: calibrate using the same page, then measure the
        // markup's own geometry — 2 page units == 10 ft (120 in) real-world.
        let scale = measurement::calibrate(
            &conn,
            &page.id,
            Point::new(0.0, 0.0),
            Point::new(2.0, 0.0),
            120.0,
            "imperial",
            Some(&owner.id),
        )
        .unwrap();
        let length = measurement::record_length(
            &conn,
            &page.id,
            &scale.id,
            Point::new(0.0, 0.0),
            Point::new(2.0, 0.0),
            LengthUnit::Feet,
            Some("Wall A length"),
        )
        .unwrap();
        assert!((length.value - 10.0).abs() < 1e-9);
        let count = measurement::record_count(
            &conn,
            &page.id,
            &[Point::new(1.0, 1.0), Point::new(1.5, 1.0), Point::new(2.0, 1.0)],
            Some("Rebar chairs, Wall A"),
        )
        .unwrap();
        assert_eq!(count.value, 3.0);

        // Takeoff: one line item linked to the length measurement.
        let item = takeoff::create(
            &conn,
            "#4 rebar, Wall A",
            length.value,
            "ft",
            Some(&length.id),
            Some(0.85),
            None,
        )
        .unwrap();
        let doc_items = takeoff::list_for_document(&conn, &doc.id).unwrap();
        assert_eq!(doc_items, vec![item.clone()]);
        let csv = takeoff::export_csv(&doc_items);
        assert!(csv.contains("#4 rebar, Wall A"));

        // Save: a version snapshot, never overwriting the original.
        document::create_version(&conn, &doc.id, "/plans/snapshots/job42_v1.pdf", Some(&owner.id)).unwrap();

        (
            proj.id,
            owner.id,
            doc.id,
            page.id,
            rect_markup.id,
            length.id,
            count.id,
            item.id,
            csv,
        )
    }; // `conn` dropped here: simulates the app closing.

    // -- Session 2: reopen the same file fresh, verify everything survived --
    {
        let conn = mds_db::open_and_migrate(db_file.path_str()).unwrap();

        let members = project::list_members(&conn, &project_id).unwrap();
        assert_eq!(members.len(), 2);
        assert!(project::is_member(&conn, &project_id, &owner_id).unwrap());

        let doc = document::get_document(&conn, &document_id).unwrap();
        assert_eq!(doc.project_id.as_deref(), Some(project_id.as_str()));
        let pages = document::list_pages(&conn, &document_id).unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].id, page_id);

        let versions = document::list_versions(&conn, &document_id).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].version_number, 1);

        let markups = markup::list_by_page(&conn, &page_id).unwrap();
        assert_eq!(markups.len(), 1);
        assert_eq!(markups[0].id, markup_id);
        let comments = markup::list_comments(&conn, &markup_id).unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].text, "field verify this wall");

        let measurements = measurement::list_by_page(&conn, &page_id).unwrap();
        assert_eq!(measurements.len(), 2);
        assert!(measurements.iter().any(|m| m.id == length_measurement_id));
        assert!(measurements.iter().any(|m| m.id == count_measurement_id));

        let takeoff_item = takeoff::get(&conn, &takeoff_id).unwrap();
        assert_eq!(takeoff_item.measurement_id.as_deref(), Some(length_measurement_id.as_str()));
        assert_eq!(takeoff_item.total_cost(), Some(takeoff_item.quantity * 0.85));

        let doc_items = takeoff::list_for_document(&conn, &document_id).unwrap();
        assert_eq!(takeoff::export_csv(&doc_items), csv);
    }
}
