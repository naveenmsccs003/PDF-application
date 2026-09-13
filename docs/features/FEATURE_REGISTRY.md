# Feature Registry

Status: **populated from a DRAFT master list** — see
`combined-feature-master-list.md` at the repo root. That file is not yet
reviewed/frozen by Naveen (per Rule 2), so treat every row's `Tier` column
as provisional. Nothing below is `VERIFIED` — nothing has been built yet.

## Still open (does not block this table existing, but blocks later phases)

- Existing CI3/PDF.js/Fabric.js/TCPDF/FPDI app location — needed to close
  `docs/architecture/adr/ADR-001-build-vs-extend.md` (Path A vs Path B).
  Not yet provided.
- Collaboration model (async shared review vs. real-time) — assumed async
  for this table; not yet confirmed by Naveen.
- Final review/edits to `combined-feature-master-list.md` itself.

## Note (2026-09-13): system deps installed, IPC layer wired

The `pkg-config`/webkit2gtk system packages are installed and
`cargo build -p app` succeeds — see `docs/PROJECT_PLAN.md`, "Phase 1 —
Foundation — UNBLOCKED". `project`, `document`, `markup`, `measurement`,
and `takeoff` are now called from `app/src-tauri/src/commands/*.rs` (34
`#[tauri::command]` functions) — rows below marked "IPC wired
(app/src-tauri); no frontend UI yet" mean exactly that: the Rust side is
callable, but `app/src/App.tsx` is still the default Vite/React scaffold,
so nothing actually calls these commands yet. DOC-01 (`import_document`)
was genuinely unwired for the reasons noted on its own row until it got
wired after all (see `docs/PROJECT_PLAN.md`); MARK-06 (`UndoStack`) is now
IPC-wired too, scoped one stack per page — see its own row and
`docs/PROJECT_PLAN.md`'s "MARK-06 undo/redo wired" entry for the scoping
rationale.

## Registry

| Feature ID | Module | Feature | MVP/BACKLOG | Status | UI component | Domain/service | Core engine dependency | Tests | Evidence | Notes | Blocker |
|---|---|---|---|---|---|---|---|---|---|---|---|
| DOC-01 | Document | Open PDF | MVP | IMPLEMENTED | | Document | PDF abstraction | `cargo test -p document` (import_document_creates_document_and_pages_in_order) | `crates/document/src/lib.rs` (`import_document`) | Domain+DB only, no UI | Not IPC-wired: needs a decision on how the installed app locates/bundles `libpdfium.so` at runtime |
| DOC-02 | Document | Save project/document | MVP | IMPLEMENTED | | Document | PDF abstraction | `cargo test -p document` (create_version_never_overwrites_and_increments) | `crates/document/src/lib.rs` (`create_version`) | Always a new DocumentVersion row, never overwrites | IPC wired (app/src-tauri); no frontend UI yet |
| DOC-03 | Document | Close document | MVP | IMPLEMENTED | "Close document" button in `DocumentsPanel` | Document | | `npm run build` (tsc) | `app/src/App.tsx` (`closeDocument`) | Not domain logic — clears `selectedDocumentId` (and, via the existing effect, pages/thumbnails/selected page) back to the document picker; nothing persisted or deleted, matches the row's own note that there's no domain logic to test in isolation | UI needs Tauri (now wired) |
| DOC-04 | Document | Page navigation | MVP | IMPLEMENTED | | Document | PDF abstraction | `cargo test -p document` (import_document_creates_document_and_pages_in_order) | `crates/document/src/lib.rs` (`list_pages`) | Backend listing only, no viewer UI | IPC wired (app/src-tauri); no frontend UI yet |
| DOC-05 | Document | Page operations (rotate, reorder) | MVP | IMPLEMENTED | | Document | PDF abstraction | `cargo test -p document` (set_page_rotation_validates_and_updates, reorder_pages_renumbers_without_unique_violation, reorder_pages_rejects_a_set_that_does_not_match) | `crates/document/src/lib.rs` | Confirm exact operation set with Naveen still open; rotate=90-deg steps, reorder=full new ordering | IPC wired (app/src-tauri); no frontend UI yet |
| DOC-06 | Document | Shared project membership | MVP | IMPLEMENTED | | Project/Security | | `cargo test -p project` | `crates/project/src/lib.rs` | Same membership model as COLLAB-01, viewed from the Document side | Collaboration model + hosting not confirmed; IPC wired (app/src-tauri); no frontend UI yet |
| VIEW-01 | Viewing | Zoom | MVP | IMPLEMENTED | +/−/Reset buttons in `PdfCanvas` | | Rendering | `cargo test -p pdf_core` (render_tile_to_png tests render at arbitrary zoomed_width); `npm run build` (tsc) | `crates/pdf_core/src/lib.rs`; `app/src/App.tsx` (`zoom` state, `zoomIn`/`zoomOut`/`zoomReset`) | Not tile-based — re-requests the existing `render_page_thumbnail` IPC command at `RENDER_WIDTH * zoom` pixels; markup/measurement coordinate math (`toPagePoint`/`toPixel`) already derives its scale factor from the rendered width, so it stays correct at any zoom level with no separate change | UI needs Tauri (now wired) |
| VIEW-02 | Viewing | Pan | MVP | IMPLEMENTED | Pan tool + native scroll in `PdfCanvas` | | Rendering | `npm run build` (tsc) | `app/src/App.tsx` (`pdf-canvas-viewport` fixed-size scrollable container, `panState`, dedicated `"pan"` tool that click-drags `scrollLeft`/`scrollTop`) | Viewport is a fixed-size `overflow: auto` box independent of zoom, so zooming in creates real overflow to pan through; native scrollbar/trackpad scroll works too, the Pan tool is only for click-drag | UI needs Tauri (now wired) |
| VIEW-03 | Viewing | Thumbnails | MVP | IMPLEMENTED | | | Rendering | `cargo test -p pdf_core` (render_thumbnail_preserves_aspect_ratio_of_full_render) | `crates/pdf_core/src/lib.rs` (`render_thumbnail_png`) | Backend only, no thumbnail strip UI | ADR-001 unresolved; UI needs Tauri |
| VIEW-04 | Viewing | Large/vector-heavy PDF rendering performance | MVP | NOT_STARTED | | | Rendering | | | Needs PDF engine validation spike (Section 6) before an engine is chosen; tile rendering exists but is not yet performance-validated against a large/vector-heavy drawing (see `crates/pdf_core` tiling caveat in `docs/PROJECT_PLAN.md`) | ADR-001 unresolved |
| MARK-01 | Markup | Text annotation | MVP | IMPLEMENTED | | Markup | | `cargo test -p markup` 9/9 | `crates/markup/src/lib.rs` | Domain+DB only, no UI/canvas yet | IPC wired (app/src-tauri); no frontend UI yet |
| MARK-02 | Markup | Basic shapes (rect/cloud/line/arrow) | MVP | IMPLEMENTED | | Markup | | `cargo test -p markup` 9/9 | `crates/markup/src/lib.rs` | Domain+DB only, no UI/canvas yet | IPC wired (app/src-tauri); no frontend UI yet |
| MARK-03 | Markup | Select / move / resize / edit | MVP | IMPLEMENTED | | Markup | | `cargo test -p markup` (undo_move_restores_previous_geometry) | `crates/markup/src/lib.rs` | Move/resize/edit = geometry or style update; "select" is UI-only, not covered here | IPC wired (app/src-tauri); no frontend UI yet |
| MARK-04 | Markup | Delete | MVP | IMPLEMENTED | | Markup | | `cargo test -p markup` (delete_removes_row_and_returns_snapshot) | `crates/markup/src/lib.rs` | | IPC wired (app/src-tauri); no frontend UI yet |
| MARK-05 | Markup | Markup list / layer panel | MVP | IMPLEMENTED | Markup list in `PagePanel`, wired to `PdfCanvas` selection | Markup | | `npm run build` (tsc) | `app/src/App.tsx` (`selectedMarkupId`/`selectRequest` in `PagePanel`, `onSelectionChange`/`selectRequest` props on `PdfCanvas`) | List row click selects/highlights the shape on canvas (`select` button per row) and canvas-driven selection (click, or select-tool hit test) highlights the matching row back; no z-order/reordering — markups have no stored order field, so "layer" here means the existing list view, not a stacking order the user can rearrange | None — real click-through via `npm run tauri dev` still outstanding, same as every other canvas interaction |
| MARK-06 | Markup | Undo / redo | MVP | IMPLEMENTED | Undo/Redo buttons in `PagePanel` | Application (Command) | | `cargo test -p markup` (undo/redo tests) | `crates/markup/src/lib.rs` (`Command`, `UndoStack`); `app/src-tauri/src/commands/markup.rs` (`undo_markup`/`redo_markup`/`markup_undo_status`) | Command pattern, applies/reverts through the same repository fns; IPC-wired with one `UndoStack` per page (`AppState::markup_undo`) | None — real click-through via `npm run tauri dev` still outstanding, same as every other canvas interaction |
| MARK-07 | Markup | Comments/threads on a markup | MVP | IMPLEMENTED | | Markup | | `cargo test -p markup` (add_list_and_delete_comments, deleting_markup_cascades_its_comments) | `crates/markup/src/lib.rs` (`add_comment`/`list_comments`/`delete_comment`) | Domain+DB only, no comment UI/thread view | Collaboration model not confirmed; IPC wired (app/src-tauri); no frontend UI yet |
| MARK-08 | Markup | Visibility toggle / lock | MVP | IMPLEMENTED | | Markup | | `cargo test -p markup` (lock_and_hidden_toggle) | `crates/markup/src/lib.rs` | | IPC wired (app/src-tauri); no frontend UI yet |
| MEAS-01 | Measurement | Manual scale calibration | MVP | IMPLEMENTED | | Measurement | Geometry | `cargo test -p measurement` (calibrate_persists_and_round_trips) | `crates/measurement/src/lib.rs` | Domain+DB only, no calibration UI/canvas yet | IPC wired (app/src-tauri); no frontend UI yet |
| MEAS-02 | Measurement | Length measurement | MVP | IMPLEMENTED | | Measurement | Geometry | `cargo test -p measurement` (record_length_converts_and_persists) | `crates/measurement/src/lib.rs` | Unit-tested per Section 18 | IPC wired (app/src-tauri); no frontend UI yet |
| MEAS-03 | Measurement | Area measurement | MVP | IMPLEMENTED | | Measurement | Geometry | `cargo test -p measurement` (record_area_converts_and_persists, record_area_rejects_too_few_points) | `crates/measurement/src/lib.rs` | Unit-tested | IPC wired (app/src-tauri); no frontend UI yet |
| MEAS-04 | Measurement | Count measurement | MVP | IMPLEMENTED | | Measurement | Geometry | `cargo test -p measurement` (record_count_stores_marker_count_and_no_scale) | `crates/measurement/src/lib.rs` | Scale-independent; no dependency on `crates/markup` was needed | IPC wired (app/src-tauri); no frontend UI yet |
| MEAS-05 | Measurement | Unit conversion | MVP | IMPLEMENTED | | Measurement | Geometry | `cargo test -p geometry` + `cargo test -p measurement` | `crates/geometry/src/lib.rs` (units), `crates/measurement/src/lib.rs` (AreaUnit) | Length units via `geometry::units`; area units added in `measurement` (reuses `convert_length` squared, no separate area table) | IPC wired (app/src-tauri); no frontend UI yet |
| MEAS-06 | Measurement | Measurement labels | MVP | IMPLEMENTED | | Measurement | | `cargo test -p measurement` (record_length_converts_and_persists asserts label) | `crates/measurement/src/lib.rs` | Free-text label field, persisted; no on-canvas rendering yet | IPC wired (app/src-tauri); no frontend UI yet |
| MEAS-07 | Measurement | Measurement persistence | MVP | IMPLEMENTED | | Measurement | | `cargo test -p measurement` (list_by_page_and_delete) | `crates/measurement/src/lib.rs` | | IPC wired (app/src-tauri); no frontend UI yet |
| TAKE-01 | Takeoff | Manual quantity entry | MVP | IMPLEMENTED | | Takeoff | | `cargo test -p takeoff` (create_get_round_trip) | `crates/takeoff/src/lib.rs` | Domain+DB only, no UI yet | IPC wired (app/src-tauri); no frontend UI yet |
| TAKE-02 | Takeoff | Measurement-linked quantities | MVP | IMPLEMENTED | | Takeoff | | `cargo test -p takeoff` (list_for_measurement_filters_correctly) | `crates/takeoff/src/lib.rs` | | IPC wired (app/src-tauri); no frontend UI yet |
| TAKE-03 | Takeoff | Descriptions/units/notes per line item | MVP | IMPLEMENTED | | Takeoff | | `cargo test -p takeoff` (update_changes_quantity_unit_cost_and_notes) | `crates/takeoff/src/lib.rs` | | IPC wired (app/src-tauri); no frontend UI yet |
| TAKE-04 | Takeoff | Cost per unit | MVP | IMPLEMENTED | | Takeoff | | `cargo test -p takeoff` (total_cost_is_none_without_a_unit_cost) | `crates/takeoff/src/lib.rs` | Field persists + `total_cost()` computed; still unconfirmed whether it belongs in MDS Rebar's actual estimating flow | Needs Naveen confirmation |
| TAKE-05 | Takeoff | CSV/Excel export | MVP | IMPLEMENTED (CSV only) | | Takeoff | Export | `cargo test -p takeoff` (csv_export_formats_rows_and_escapes_special_characters) | `crates/takeoff/src/lib.rs` (`export_csv`) | CSV done; Excel (.xlsx) not attempted — needs a real dependency decision, not hand-rolled | IPC wired (app/src-tauri); no frontend UI yet; Excel format TBD |
| RFI-01 | RFI/Revision | Create RFI tied to page/markup | MVP | IMPLEMENTED | RFI panel in `DocumentsPanel` (optional page + markup tie dropdowns) | RFI | | `cargo test -p rfi` (create_get_and_list_round_trip, numbers_increment_per_document, deleting_page_sets_rfi_page_id_null_but_keeps_rfi) | `crates/rfi/src/lib.rs`; `app/src-tauri/src/commands/rfi.rs`; `app/src/App.tsx` (`RfiPanel`) | `page_id`/`markup_id` both optional and independent (`ON DELETE SET NULL`) so deleting the page/markup an RFI pointed at doesn't destroy the RFI itself; numbered sequentially per document ("RFI #14"), not by opaque id | Elevated from backlog #9 per Naveen's stated priority; async collaboration model (project membership) already exists and is what this uses — no live/real-time requirement here |
| RFI-02 | RFI/Revision | RFI status tracking | MVP | IMPLEMENTED | Status dropdown + response field per row in `RfiPanel` | RFI | | `cargo test -p rfi` (set_status_records_response_and_updates_status, set_status_on_missing_rfi_errors) | `crates/rfi/src/lib.rs` (`RfiStatus`, `set_status`); `app/src-tauri/src/commands/rfi.rs` (`set_rfi_status`) | open/answered/closed per the master list; no state machine enforced (a closed RFI can be reopened — real job sites do this) | None — real click-through via `npm run tauri dev` still outstanding, same as every other IPC-wired feature |
| RFI-03 | RFI/Revision | Drawing revision tracking | MVP | NOT_STARTED | | Document/Project | | | | Overlaps DocumentVersion | ADR-001 unresolved |
| RFI-04 | RFI/Revision | Revision comparison/overlay | BACKLOG | NOT_STARTED | | | Rendering | | | Proposed deferral — confirm with Naveen | |
| EXPORT-01 | Export | Flattened PDF export | MVP | IMPLEMENTED | "Export flattened PDF…" button in `DocumentsPanel` (save dialog) | Export | Rendering | `cargo test -p export` (text/rectangle/color-fallback conversion, hidden-markup and empty-page skipping); `cargo test -p pdf_core` (flatten_page_with_annotations_*, flattened_export_saves_and_reopens_with_the_same_page_count, against the real sample PDF + libpdfium) | `crates/pdf_core/src/lib.rs` (`FlattenAnnotation`, `PdfDocument::flatten_page_with_annotations`); `crates/export/src/lib.rs`; `app/src-tauri/src/commands/pdf.rs` (`export_flattened_pdf`, routed through the existing dedicated PDF-engine thread) | Burns non-hidden `Markup` rows into real page content via Pdfium's own `FPDFPage_Flatten`, then saves to a new file — the original is untouched. Rectangle/Line/Cloud become stroked paths, Text becomes a real text object (Helvetica, fixed 12pt); Arrow draws its shaft only, no arrowhead (cosmetic, not required) | While adding this, found and fixed a pre-existing test flake: `cargo test -p pdf_core` runs tests in parallel by default, and 2+ tests each constructing a real `PdfiumEngine` in one process is the exact deadlock `crates/pdf_engine_spike/README.md` already documents — serialized with a `Mutex` in the test module |
| EXPORT-02 | Export | Print | MVP | NOT_STARTED | | | Export | | | | ADR-001 unresolved |
| EXPORT-03 | Export | Export/handoff package | MVP | NOT_STARTED | | | Export | | | Named directly by Naveen | ADR-001 unresolved |
| COLLAB-01 | Collaboration | Shared project access, multiple users | MVP | IMPLEMENTED | | Project/Security | | `cargo test -p project` (create_project_adds_creator_as_owner_member, add_list_update_and_remove_members) | `crates/project/src/lib.rs` | Domain+DB only, no UI; role is free text, not a fixed permission model | Collaboration model + hosting not confirmed; IPC wired (app/src-tauri); no frontend UI yet |
| COLLAB-02 | Collaboration | Async update visibility | MVP | IMPLEMENTED | | Project | | N/A — see note | (no new code) | Falls out of every crate's query-current-DB-state model; nothing to add or test in isolation | Assumption (async vs. real-time) still pending confirmation |
| COLLAB-03 | Collaboration | Real-time simultaneous co-editing | BACKLOG | NOT_STARTED | | | | | | Only if confirmed over async | |
| REL-01 | Reliability | Autosave | MVP | NOT_STARTED | | | Filesystem | | | Separate recovery/snapshot file, not overwrite | ADR-001 unresolved |
| REL-02 | Reliability | Crash recovery (restore/discard) | MVP | NOT_STARTED | | | Filesystem | | | | ADR-001 unresolved |
| SEC-01 | Security | Local password protection | MVP | NOT_STARTED | | Security | Encryption | | | | ADR-001 unresolved |
| SEC-02 | Security | Secure credential handling | MVP | NOT_STARTED | | Security | | | | OS keychain where applicable | ADR-001 unresolved |
| SEC-03 | Security | User accounts / access control for shared projects | MVP | IN_PROGRESS | | Security | | `cargo test -p project` (removing_or_updating_a_non_member_errors) | `crates/project/src/lib.rs` (`is_member`) | Membership storage + a check exist; actual enforcement against an action is an app-layer concern that doesn't exist yet | Not in build prompt's original single-user MVP list; collaboration model + hosting not confirmed |

## Allowed status values

`NOT_STARTED`, `DESIGNING`, `IN_PROGRESS`, `IMPLEMENTED`, `TESTING`,
`VERIFIED`, `BLOCKED`.

A feature is `VERIFIED` only with evidence (test, screenshot, benchmark,
reproducible validation) — never because code merely exists.

Features are never deleted from this table once added, only re-statused.
