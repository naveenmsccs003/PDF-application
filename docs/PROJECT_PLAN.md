# Project Plan — MDS Rebar PDF Review/Measurement/Takeoff Platform

Status of this document: **scaffold**. It restates the phase structure and
gates already defined in the master build prompt so there is one place to
track progress. It does not decide anything the prompt reserves for Naveen,
and it does not contain a fabricated feature list.

## Hard gates still open (block Phase 0 completion)

| Gate | Needed | Status |
|---|---|---|
| Authoritative feature list | `combined-feature-master-list.md` content or file | **MISSING** |
| Existing CI3/PDF.js/Fabric.js/TCPDF/FPDI app | Path, repo URL, or archive to inspect | **MISSING** |
| Build vs. extend decision (Rule 3) | Depends on the above evaluation | **BLOCKED** |
| Business model | Internal / commercial / undecided | Not yet given — defaulting to *internal-first, commercial-capable* per prompt default until told otherwise |
| Available dev time | Hours/week, deadline, full/part-time | Not yet given |

Everything below this line is sequencing, not a green light — no phase after
Phase 0 starts until the table above is clear.

## Phases

- **Phase 0 — Discovery** (in progress, blocked — see above)
  - Deliverables: `docs/00_SCOPE_REALITY_CHECK.md`, `docs/features/FEATURE_REGISTRY.md`,
    `docs/architecture/`, `docs/database/`.
- **Phase 1 — Foundation** *(Path B only)* — **UNBLOCKED (2026-09-13)**:
  - DONE (with evidence): `rustup` toolchain installed (`rustc 1.98.1`,
    `cargo 1.98.1`); git repo initialized at `/home/naveen/pdf_application`;
    Tauri 2 + React + TypeScript scaffold created in `app/` via
    `create-tauri-app`; frontend builds cleanly (`npm run build` → tsc +
    vite build succeeded, `dist/` produced).
  - RESOLVED: the `pkg-config`/webkit2gtk system-deps blocker. Naveen ran
    `sudo apt-get install -y pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev
    libayatana-appindicator3-dev librsvg2-dev libssl-dev file` (one retry
    needed — the first attempt 404'd on 4 packages due to a stale local
    package index; `sudo apt-get update` then a retry fixed it).
    `cargo build -p app` now succeeds end-to-end: `target/debug/app` is a
    real linked ELF binary. First slice of real IPC wiring landed in the
    same pass — `crates/project` (create user, create project, list
    projects, list members) is now called from `app/src-tauri/src/lib.rs`
    via a mutex-guarded `rusqlite::Connection` opened at startup in
    `tauri::Builder::setup` (path: `app.path().app_data_dir()` +
    `mds_rebar.sqlite`), with local DTO structs at the IPC boundary so the
    domain crates themselves stay free of `serde`.
  - **Full IPC wiring, same pass**: extended the same pattern to
    `document`, `markup`, `measurement`, and `takeoff` — 34 `#[tauri::command]`
    functions total now, organized as `app/src-tauri/src/commands/{project,
    document,markup,measurement,takeoff}.rs` with shared DTOs in
    `app/src-tauri/src/dto.rs`. `markup`'s `MarkupType`/`MarkupGeometry`/
    `MarkupStyle` are passed as command parameters directly (they already
    derive `Deserialize` for their own JSON persistence) rather than adding
    redundant input DTOs for them.
    - Deliberately NOT wired: `document::import_document` (DOC-01/open) —
      still blocked on the `libpdfium.so` distribution decision noted
      above; `markup::UndoStack`/`Command` (MARK-06) — in-memory undo/redo
      state needs a scoping decision (per document? per page? per user,
      given confirmed multi-user?) that's a real design call, not a
      mechanical wiring step like the rest of this slice.
    - Verified: `cargo check -p app` — clean, no errors, no warnings.
      Full workspace (`cargo test --workspace --exclude app`) — 78/78
      passing, no regressions.
  - **Frontend UI, same day**: replaced the default Vite/React scaffold in
    `app/src/App.tsx` with a working control panel that calls the real IPC
    layer — sign in/create user (`get_user_by_email` added to `crates/project`
    as a convenience lookup so repeated test runs don't trip the
    `user.email` `UNIQUE` constraint), projects + members, PDF import via
    a native file picker (`tauri-plugin-dialog`), a page thumbnail grid,
    per-page markup CRUD + comments, scale calibration + length/area/count
    measurement, and takeoff line items + CSV export/preview. Typed IPC
    wrappers live in `app/src/api.ts`.
    - **DOC-01 (open) got wired after all**, but not the way first
      planned: `pdf_core::PdfiumEngine` turned out not to be `Send` (its
      `Pdfium` bindings hold a `Box<dyn PdfiumLibraryBindings>`, which
      isn't `Send`) — the compiler caught this, confirming
      `crates/pdf_engine_spike/README.md`'s "must be a singleton" finding
      is even stricter than it first read. Fix: a dedicated OS thread owns
      the one engine for the process's whole lifetime; commands talk to it
      over a channel, and only plain `Send` data (`String`, `Vec<(f32,f32)>`,
      `Vec<u8>`) ever crosses that boundary. `import_pdf_document` gets
      page sizes from that thread, then feeds them into
      `document::import_document` via a `PageSizesDocument` shim — the
      same `FakePdfDocument`-style test-double pattern already used in
      `crates/document`'s and `crates/e2e_tests`' own tests, reused here
      for real data instead of fixtures. Still dev-only: the
      `libpdfium.so` path is resolved via `env!("CARGO_MANIFEST_DIR")`
      relative to this workspace, not bundled for distribution.
    - Verified: `npm run build` (tsc + vite) — clean, no type errors.
      `cargo check -p app` / `cargo build -p app` — clean. Full workspace
      (`cargo test --workspace --exclude app`) — 81/81 passing (adds
      `document::list_documents_for_project` and
      `project::get_user_by_email`, 2 new tests). **Real runtime launch**:
      started the actual compiled binary (`target/debug/app`) against a
      live X11 display and the Vite dev server — it launched, stayed
      running, and (checked directly against the resulting sqlite file)
      correctly created and migrated all 12 tables, confirming
      `AppState::setup()` — DB open+migrate, PDF-engine thread spawn — runs
      correctly for real, not just under `cargo test`. One environment
      wrinkle worth recording: the first launch attempt crashed with a
      `libpthread.so.0` symbol error, caused by this whole session running
      inside a VS Code snap sandbox that leaks `GTK_PATH`/`GIO_MODULE_DIR`/
      etc. pointing at the snap's bundled (incompatible) libraries into
      any child process's environment — fixed by unsetting those vars
      before launch. A real user launching this app normally (not from
      inside this snap-sandboxed session) would not hit this.
    - **NOT verified (at the time)**: actual visual rendering or
      click-through interaction of the UI inside the native window. That
      session's sandbox had no way to screenshot it (Wayland blocks X11
      `XGetImage`-style capture, no screenshot CLI installed, and the
      Claude-in-Chrome extension wasn't connected either). What WAS
      confirmed instead: the dev server serves valid, correctly-transformed
      JSX for every file with no Vite error overlay (checked via `curl`).
    - **Rendering verified, next session (2026-09-13)**: Claude-in-Chrome
      was still not connected, but a real headless Chrome binary
      (`google-chrome --headless --screenshot=...`) was available on the
      machine and worked where the extension-based tools couldn't. Ran
      `npm run dev` (Vite only, not `tauri dev` — no native window in this
      environment either) and screenshotted `http://localhost:1420`: the
      sign-in screen renders correctly — dark theme, styled card, email +
      display-name inputs, "Sign in" button, no blank page or error
      overlay. This confirms React/CSS render correctly; it does NOT
      confirm the Tauri IPC bridge (`window.__TAURI__`) works, since a
      plain browser tab has no bridge injected — that still needs either a
      real native launch or `tauri dev` with a webview. Dev server was
      stopped after the screenshot. Naveen launching the actual app
      (`cd app && npm run tauri dev`, outside any snap/sandboxed
      environment) remains the real end-to-end check, now for IPC + native
      window behavior specifically rather than "does it render at all."
  - **PDF markup canvas added (2026-09-13)**: `app/src/App.tsx` no longer
    creates markup via a raw "points, e.g. 0,0 2,1" text field — `PdfCanvas`
    (new component in the same file) renders the actual page image (reused
    `render_page_thumbnail` at `RENDER_WIDTH = 900` instead of thumbnail
    size — no backend change needed, that command already just calls
    `render_page_to_png` under a different name) with an absolutely
    positioned SVG overlay for click-to-draw Rectangle/Line/Arrow (drag),
    Cloud (click points, then "Finish cloud"), and Text (click, type inline,
    Enter to commit) — all five `MarkupType`s. Existing markups render back
    onto the overlay from `list_markups_by_page`, respecting `hidden`.
    Coordinates: overlay pixel → page-space is a uniform `page.width /
    RENDER_WIDTH` scale (image and page share aspect ratio since
    `pdf_core` renders proportionally), y-axis stays image-top-down rather
    than PDF's native bottom-up — an internal convention, not yet meaningful
    outside this app since nothing exports these coordinates through a real
    PDF round-trip yet. Measurement (calibrate/length/count) and the
    lock/hide/delete/comments list were deliberately left as-is — this pass
    is scoped to markup creation via canvas, not select/move/resize (no
    spatial-index wiring yet, matches the still-open item from Phase 3's
    "Markup, continued" entry above) or a measurement canvas.
    - Verified: `npm run build` (tsc + vite) — clean, no type errors.
      Headless-Chrome screenshot re-confirmed the sign-in screen still
      renders with no regression. **NOT verified**: actually drawing a
      shape and seeing it round-trip through a live `AppState` — that
      needs a signed-in session against a real Tauri IPC bridge, which
      this environment still can't drive (same gap noted just above).
      Naveen exercising the canvas for real (`npm run tauri dev`) is the
      next real check, same as the outstanding IPC-bridge verification.
  - **Select/move/resize added (2026-09-13)**: closed the gap this pass's
    own note called out ("select/move/resize aren't built yet"). New
    `select` mode on `PdfCanvas`: click hit-tests existing markups in page
    space (`hitTest` — segment-distance for Line/Arrow, bounding-box for
    Rectangle/Cloud, a padded box for Text) and highlights the topmost
    unhidden match with a dashed selection box; dragging from inside a
    selected, unlocked shape translates every point by the drag delta
    (`moveState`); dragging one of its `resizeHandles` — the shape's own
    stored points for Rectangle/Line/Arrow, none for Cloud/Text (move-only,
    deliberately not attempting multi-point cloud editing this pass) —
    rewrites that one point (`resizeState`). Both paths render a live
    preview via `livePoints` and commit through the existing
    `update_markup_geometry` IPC command (no backend change needed — it
    already accepted arbitrary geometry) only on mouse-up, and only if the
    geometry actually changed, then reload from `list_markups_by_page` so
    the committed state is always what's shown. Locked markups can be
    selected (to see they exist) but not moved or resized, matching what
    `locked` is supposed to mean. No `rstar`/`SpatialIndex` wiring yet —
    same call as the canvas's first pass: a page's markup count doesn't
    yet justify it, plain O(n) hit-testing is fine here.
    - Verified: `npm run build` (tsc + vite) — clean, no type errors.
      `npm run dev` served `/` with a 200 before being stopped. **NOT
      verified**: same live-IPC gap as every canvas interaction so far —
      actually selecting/dragging a real markup through a signed-in
      `npm run tauri dev` session is still Naveen's check to run, not
      something this environment can drive.
  - **Measurement moved onto the canvas (2026-09-13)**: closed the other
    gap this pass's own notes called out ("everything UI-facing (calibration
    interaction, measurement labels on the canvas)" from Phase 4's early
    pass). `PdfCanvas` gains four Measure tools alongside the five Markup
    ones: Calibrate (drag 2 points, then an inline "real-world inches"
    input — same interaction shape as the Text tool's inline label, commits
    via the existing `calibrate_scale` command), Length (drag 2 points,
    commits via `record_length` in a toolbar-selected unit), Area (click
    points then Finish, commits via `record_area`), Count (click points then
    Finish, commits via `record_count`). All four reuse the drag-pair /
    click-accumulate gesture patterns the markup tools already established
    rather than inventing new interaction shapes. No backend change needed —
    all four IPC commands already existed and took arbitrary geometry; only
    `record_area` had no frontend caller before this. Persisted measurements
    now render back onto the canvas with their value+unit as a text label
    (`MeasurementShape` — line+midpoint label for Length, polygon+centroid
    label for Area, dots+centroid label for Count), closing MEAS-06's
    "labels" half for real this time (the manual-input version stored a
    label but never displayed one on anything resembling a drawing surface).
    Replaced the old free-text "p1 x,y"/"p2 x,y"/"markers: x,y x,y ..."
    inputs entirely — they used arbitrary example coordinates ("0,0"/"2,0")
    that didn't correspond to actual page geometry, whereas canvas clicks
    are real PDF-point coordinates via the same `toPagePoint` conversion
    markup already uses, so a calibration taken here is actually anchored to
    the drawing. `parsePoint`/`parsePoints` helpers (only used by the removed
    inputs) were deleted rather than left dead.
    - Verified: `cargo check`/`cargo build -p app` unaffected (no Rust
      changes this pass — every IPC command used already existed). `npm run
      build` (tsc + vite) — clean, no type errors. `npm run dev` served `/`
      with a 200 before being stopped. **NOT verified**: same live-IPC gap
      as every canvas interaction so far — calibrating a scale and recording
      a real length/area/count through a signed-in `npm run tauri dev`
      session is still Naveen's check to run.
  - **MARK-06 undo/redo wired (2026-09-13)**: closed the scoping question
    this pass's earlier notes deferred as "a real design question, not a
    mechanical wiring step" — resolved as **one `UndoStack` per page**
    (`AppState::markup_undo: Mutex<HashMap<page_id, UndoStack>>`), not per
    document or per user: the UI is already organized per page (`PdfCanvas`/
    `PagePanel` both take one `PageDto`), so undoing while looking at page 3
    shouldn't revert something done on page 1; per-user needed no separate
    handling since each user already runs their own Tauri process, so this
    in-memory state is implicitly scoped to one user by being one process's
    memory. `create_markup`/`update_markup_geometry`/`update_markup_style`/
    `set_markup_locked`/`set_markup_hidden`/`delete_markup` now build a
    `markup::Command` and route it through that page's stack instead of
    calling `markup::create`/`update_geometry`/etc. directly — geometry
    validation (previously only inside `markup::create`/`update_geometry`,
    which `Command::apply` bypasses) had to be exposed as `pub fn
    MarkupGeometry::validate` so the app layer can still enforce it before
    handing a command to the stack. Added `undo_markup`/`redo_markup`/
    `markup_undo_status` commands; the frontend adds Undo/Redo buttons next
    to the markup list, disabled from `markup_undo_status`, refreshed
    after every markup mutation (create/move/resize/lock/hide/delete all
    funnel through the existing `reloadMarkups`).
    - Verified: `cargo check -p markup -p app` — clean. Full workspace
      (`cargo test --workspace --exclude app`) — 80/80 passing, no
      regressions (`markup` still 11/11 — the undo/redo tests already
      covered `UndoStack` itself; this pass only added callers). `npm run
      build` — clean, no type errors. **NOT verified**: same live-IPC gap —
      actually creating a shape, undoing it, and redoing it through a
      signed-in `npm run tauri dev` session is still Naveen's check to run.
  - DONE (with evidence): SQLite + migrations, as a separate pure-Rust
    workspace crate `crates/mds_db` that does not depend on Tauri/webkit —
    this respects the prompt's own layering rule (Core Engine/Domain must
    not depend on the UI) and let the database layer be built and verified
    *before* the webkit2gtk system deps are installed. Uses `rusqlite`
    with the `bundled` SQLite (no system libsqlite3 needed) and
    `rusqlite_migration`. Schema: full table set from
    `docs/database/README.md` (Document, DocumentVersion, Page, Markup,
    MarkupComment, Scale, Measurement, TakeoffItem, RecoveryState) plus
    User/Project/ProjectMember (added for the confirmed collaboration
    requirement — see that doc's "Added 2026-09-13" section). FK enforcement
    and WAL mode enabled on connection open. Verified with `cargo test -p
    mds_db`: 3 tests passing — all expected tables created, FK violation on
    an orphan insert is rejected, cascade delete on `Page → Markup` works.
  - NOT STARTED: IPC command scaffolding, command architecture (undo/redo),
    structured logging, background job infrastructure, updater config, and
    wiring `mds_db`/`pdf_core` into the actual Tauri app (`app/src-tauri`) —
    blocked until the webkit2gtk system packages are installed, since
    `app/src-tauri` itself still can't `cargo check`.

- **Phase 2 — PDF Core** — **STARTED EARLY, groundwork only**: normally
  this phase follows Phase 1 completing, but with `app/src-tauri` blocked on
  system deps, the PDF abstraction layer (Section 5/7: "create an internal
  PDF abstraction/trait ... do not couple the domain layer directly to
  PDFium") was built as another pure-Rust crate, `crates/pdf_core`. Defines
  `PdfEngine`/`PdfDocument` traits (open, page count, page size, render to
  PNG bytes, extract text, save) implemented by `PdfiumEngine`/
  `PdfiumDocument`, which wrap the same `pdfium-render` calls the Section 6
  spike validated. The trait's lifetime design (`Document<'e>` borrows the
  engine) directly encodes the singleton constraint the spike discovered —
  an engine must outlive every document it opens, and only one engine
  (hence one `Pdfium` binding) should exist per process.
  - Verified: `cargo test -p pdf_core` — 2/2 passing (document metadata
    round-trip incl. real PNG magic-byte check on rendered output; a
    deliberately out-of-range page index returns a typed error rather than
    panicking).
  - NOT done (at the time): tile-based rendering, zoom/pan/thumbnails,
    wiring into the Tauri app, and — same caveat as the Section 6 spike —
    validation against a realistic large/vector-heavy construction
    drawing. This remains groundwork, not a finished Phase 2.

- **Phase 2 — PDF Core, continued: tiling + thumbnails added**: found that
  none of this actually needed webkit2gtk either — `pdfium-render` renders
  independent of the Tauri webview, and `crates/pdf_engine_spike/lib/libpdfium.so`
  is already present and working (confirmed real, non-skipped test runs).
  Added to `crates/pdf_core`: `PdfDocument::render_tile_to_png` (default
  trait method — render the full zoomed page via the existing
  `render_page_to_png`, then crop with the `image` crate; edge tiles crop
  narrower/shorter rather than pad), `tile_grid()` (pure column/row math),
  `render_thumbnail_png` (VIEW-03 — names the existing full-render
  capability at a small width rather than adding new logic), and
  `TileCache` (LRU, via the `lru` crate, keyed by page/zoom/tile
  coordinates — Section 8's "LRU cache from the start" requirement).
  - Verified: `cargo test -p pdf_core` — 7/7 passing, including
    interior-vs-edge tile size correctness, an out-of-range tile
    coordinate returning a typed error, thumbnail aspect-ratio
    preservation, and cache hit/miss/eviction behavior against a fake
    `PdfDocument` (so the cache logic doesn't need real PDFium to test).
  - Known limitation, documented in the trait doc comment: `render_tile_to_png`
    re-rasterizes the *entire* page per tile call rather than rasterizing
    only the requested region — PDFium's own `clip()` render option was
    investigated and rejected because it still allocates a full-page bitmap
    and only masks it, so it doesn't save the rendering cost either. This
    is correct today but not yet the performance win tiling is meant to
    provide for large pages; revisit once the Section 6 large/vector-heavy
    PDF validation happens.
  - NOT done: zoom/pan interaction itself (UI), wiring into the Tauri app,
    and the large/vector-heavy PDF validation.

- **Phase 4 — Measurement** — **STARTED EARLY, math engine only**: same
  reasoning as Phase 2 above — the geometry/measurement math is pure
  calculation with no PDF/UI/DB dependency, and Section 18 explicitly
  requires automated unit tests for it regardless of what platform decision
  is made elsewhere. Built as `crates/geometry`: `Scale` calibration
  (two page-space points + a known real-world distance), `length_inches`,
  `polygon_area_square_inches` (shoelace formula), and a `units` module
  with length conversion (inches/feet/mm/cm/m) and architectural
  feet-inches formatting (`12'-6 1/2"`, rounded to the nearest 1/16").
  - Verified: `cargo test -p geometry` — 24/24 passing, covering normal
    values, decimals, unit conversion round-trips, invalid-scale edge cases
    (zero/negative calibration distance), zero/negative-adjacent inputs,
    very large measurements (1e12), and feet-inches formatting edge cases
    (rounding, negatives, whole feet).
  - NOT done: count measurement (trivial once markup objects exist —
    deferred, not skipped, since it needs the Markup domain model from
    Phase 3), measurement persistence (needs `mds_db` wiring), and
    everything UI-facing (calibration interaction, measurement labels on
    the canvas). This is the math core only, not a finished Phase 4.
- **Phase 3 — Markup** — **STARTED EARLY, domain + undo/redo core only**: same
  reasoning as Phase 2/4 above — the markup domain model and Command-pattern
  undo/redo stack don't need webkit2gtk either, so they were built as
  another pure-Rust crate, `crates/markup`, ahead of `app/src-tauri` being
  unblocked. Covers MARK-01–04, MARK-06, MARK-08 from
  `docs/features/FEATURE_REGISTRY.md`: create/get/list/delete against
  `mds_db`'s existing `markup` table, geometry (points, type-checked minimum
  point count per shape) and style stored as JSON, lock/hidden toggles, and
  an `UndoStack` (`Command::{Create,Delete,SetGeometry,SetStyle,SetLocked,
  SetHidden}`) that applies/reverts through the same repository functions —
  a fresh command clears the redo stack, matching standard editor semantics.
  - Verified: `cargo test -p markup` — 9/9 passing, covering CRUD round
    trip, shape point-count validation, lock/hidden toggles, and undo→redo
    for create, delete, and geometry changes (move/resize), plus the
    empty-stack and redo-cleared-by-new-command edge cases.
  - NOT done (at the time): MARK-05 (markup list/layer panel — UI), MARK-07
    (comments/threads — separate `markup_comment` table, untouched),
    spatial indexing (`rstar` hit-testing — needs a canvas to hit-test
    against, deferred), and — same caveat as Phase 2/4 — wiring into the
    actual Tauri app, still blocked on the same system deps as Phase 1.
    This also unblocks MEAS-04 (count measurement), which `docs/PROJECT_PLAN.md`
    previously deferred pending exactly this domain model.

- **Phase 3 — Markup, continued: comments + spatial indexing**: closed
  MARK-07 — `add_comment`/`list_comments`/`delete_comment` in
  `crates/markup`, backed by the existing `markup_comment` table (cascade
  deletes with its parent markup, verified by test). Also added
  `geometry::spatial` — a generic `BoundingBox` + `rstar`-backed
  `SpatialIndex<Id>` for hit-testing (architecture doc's "Spatial
  indexing: `rstar` (R-tree) for markup/measurement hit testing").
  Deliberately generic over an opaque `Id` and placed in `geometry`
  (Core Engine), not `markup` or `measurement` (Domain), so neither domain
  crate needs the other as a dependency just to be indexed the same way.
  - Verified: `cargo test -p markup` — 11/11 passing (adds comment
    add/list/delete and the cascade-delete-with-parent-markup case);
    `cargo test -p geometry` — 31/31 passing (adds bounding-box
    construction, point-query hit-testing including overlap, rect-query
    marquee-select, and exact-match removal).
  - NOT done: actually wiring `SpatialIndex` into `markup`/`measurement`
    (e.g. rebuilding it from `list_by_page` on load, updating it on every
    `Command`) — there's no canvas yet to justify picking a rebuild/update
    strategy, so this stays an available Core Engine capability, not a
    consumed one. MARK-05 (markup list/layer panel) is still UI-only, not
    started.

- **Phase 4 — Measurement, continued** — **domain + persistence layer added**:
  built `crates/measurement` on top of `geometry` (math) and `mds_db`
  (the existing `scale`/`measurement` tables). Covers MEAS-01, 02, 03, 05,
  06, 07: `calibrate()` persists a scale (and two small additions to
  `geometry::Scale` — `inches_per_page_unit()`/`from_ratio()` — so the
  persisted `ratio` column can round-trip back into a usable `Scale`
  without redoing calibration from raw points); `record_length()`/
  `record_area()` compute from `geometry`, convert to a caller-chosen unit,
  and persist against a scale; free-text `label` persists per measurement
  (MEAS-06). Also closes MEAS-04 (count) — turned out not to need a
  dependency on `crates/markup` after all, just marker points and a count
  of them, persisted with `scale_id = NULL` since count has no real-world
  unit conversion.
  - Verified: `cargo test -p measurement` — 7/7 passing (scale round trip,
    length/area conversion and persistence, area's too-few-points error,
    count with no scale, list/delete, and an unknown-scale-id error path);
    `cargo test -p geometry` — 25/25 passing (24 prior + the new ratio
    round-trip test).
  - NOT done: everything UI-facing (calibration interaction, measurement
    labels rendered on the canvas), and CSV export (Phase 5, TAKE-05)
    which will read from this table.

- **Phase 5 — Takeoff, started early** — **domain + persistence layer**:
  built `crates/takeoff` on `mds_db`'s existing `takeoff_item` table.
  Covers TAKE-01–05: manual quantity entry with description/unit/notes,
  optional link to a `measurement_id` (TAKE-02), cost per unit and a
  computed `total_cost()` (TAKE-04 — still flagged in
  `docs/features/FEATURE_REGISTRY.md` as needing Naveen's confirmation
  that cost-per-unit is actually part of MDS Rebar's estimating flow; this
  only stores the field the schema already had, it doesn't resolve that
  question), and CSV export (TAKE-05, hand-rolled RFC 4180 quoting rather
  than a new dependency). `list_for_document()` joins
  `takeoff_item → measurement → page → document` since `takeoff_item` has
  no document/project column of its own — the natural scope for an export.
  - Verified: `cargo test -p takeoff` — 8/8 passing (CRUD round trip,
    update, delete, measurement-scoped and document-scoped listing via the
    join, CSV formatting including comma/quote escaping and the
    empty-list case).
  - NOT done: EXPORT-01/02/03 (flattened PDF export, print, handoff
    package — these need the PDF engine, not just data), and everything
    UI-facing.

- **Phase 1/2 — Document domain, added**: built `crates/document` — Domain
  layer sitting on `pdf_core` (Core Engine) and `mds_db` (Infrastructure),
  the first crate that actually connects the two. Covers DOC-01, 02, 04,
  05: `import_document()` opens a PDF via any `pdf_core::PdfDocument` and
  atomically records one `document` row plus one `page` row per PDF page
  (1-based page numbers, real page sizes); `list_pages()` backs page
  navigation; `set_page_rotation()` validates against the 4 allowed
  90-degree steps; `reorder_pages()` takes a full new page-id ordering and
  renumbers in two passes (everything to a far-negative `page_number`
  first, then to final 1-based numbers) specifically to avoid tripping the
  schema's `UNIQUE (document_id, page_number)` constraint mid-update —
  SQLite has no deferred-constraint equivalent to lean on here; `create_version()`
  always inserts a new `document_version` row (`MAX(version_number) + 1`),
  never overwrites. DOC-03 (close) isn't implemented — closing a
  `PdfDocument` is just dropping the Rust value once this is wired into a
  real app, there's no domain logic to test in isolation. DOC-06 (shared
  project membership) is Project, not Document, and wasn't touched.
  - Verified: `cargo test -p document` — 5/5 passing (import + page
    ordering/sizes, rotation validation and not-found, a full reorder
    round trip including the renumbering edge case, a mismatched-page-set
    reorder correctly rejected with the original order left untouched,
    and version numbering across two versions). Tested against a fake
    `PdfDocument` rather than real PDFium, since this crate's own logic is
    the thing under test, not `pdf_core`'s (already covered separately).
  - NOT done: wiring any of this into `app/src-tauri` (still blocked on
    system deps). DOC-06/Project domain — see below, done in the same pass.

- **Project/Security domain, added**: built `crates/project` for
  `user`/`project`/`project_member`. Covers COLLAB-01 (`create_project`
  atomically adds the creator as an `"owner"` member — a project with zero
  members isn't a useful starting state), DOC-06 (same membership model,
  viewed from the Document side), and part of SEC-03 (membership storage +
  an `is_member` check — not enforcement, since no app layer exists yet to
  enforce anything against a UI action). Role is stored as free text
  (`"owner"`/whatever a caller passes), not a fixed enum — the
  collaboration model and exact role set are still marked "not confirmed"
  by Naveen in `docs/features/FEATURE_REGISTRY.md`, so this doesn't invent
  permission semantics beyond what the schema already commits to.
  COLLAB-02 (async update visibility) needed no new code: every query
  function across all eight crates so far reads current database state on
  each call, so one member's saved change is simply what the next
  member's query returns next time — that's what "async" means here,
  there being no real-time sync layer (COLLAB-03, explicitly BACKLOG).
  - Verified: `cargo test -p project` — 5/5 passing (creator-becomes-owner
    on project creation, add/update/remove membership, the schema's
    `UNIQUE(project_id, user_id)` correctly rejecting a duplicate add, a
    non-member update/remove correctly erroring, and a user's project list
    correctly scoped to only what they're a member of).
  - NOT done: SEC-01 (local password protection) and SEC-02 (secure
    credential handling) — deliberately not attempted this pass; these are
    security-sensitive design decisions (encryption scheme, key handling)
    that deserve a considered choice, not busywork opportunism. Wiring
    into `app/src-tauri` is the same open blocker as everything else.

- **Phase 2 — PDF Core**: open/save/reopen, page nav, tile rendering + cache
  from day one, zoom/pan, thumbnails.
- **Phase 3 — Markup**: text/shapes, select/move/resize/edit/delete, markup
  list, undo/redo, persistence, spatial indexing.
- **Phase 4 — Measurement**: scale calibration, length/area/count, unit
  conversion, labels, persistence, CSV export — geometry math gets unit
  tests, not just UI tests.
- **Phase 5 — Takeoff / Export / Distribution**: manual takeoff linked to
  measurements, CSV/Excel export, flattened PDF export, print, local
  password protection, autosave/recovery, signed Windows installer, updater.
- **MVP Release Checkpoint** (partial — data layer only): added
  `crates/e2e_tests`, a cross-crate integration test
  (`full_workflow_survives_a_real_close_and_reopen`) exercising exactly the
  workflow this checkpoint names — open (via `document::import_document`),
  markup (create through `UndoStack`, undo, redo, a comment), measure
  (calibrate a scale, length + count), takeoff (a line item linked to the
  length measurement, CSV export), save (`document::create_version`) —
  against a **real on-disk SQLite file**, then drops the connection
  entirely and reopens the same file fresh to verify every row survived
  (project membership, document/pages/version, markup/comment,
  measurements, takeoff item + CSV). Unlike every other crate's own tests
  (all against `:memory:`), this is what actually proves a close/reopen
  round-trips correctly, and it's what caught one real bug: `measurement`
  took `geometry::units::LengthUnit` in a public function signature
  without re-exporting it, so a caller outside `geometry` couldn't spell
  the type — fixed with `pub use geometry::units::LengthUnit;`.
  - Verified: `cargo test -p e2e_tests` — 1/1 passing. Full pure-Rust
    workspace (`mds_db`, `geometry`, `pdf_core`, `markup`, `measurement`,
    `takeoff`, `document`, `project`, `e2e_tests`) — 78/78 passing, no
    warnings.
  - This is genuinely partial: no actual PDF rendering/UI is exercised
    (the test uses a fake `PdfDocument`, matching how `document`'s own
    tests do it — `pdf_core`'s real-PDFium path is covered separately),
    and crash recovery (REL-01/02), password protection (SEC-01),
    large-PDF, and long-session testing are all still NOT_STARTED. Backlog
    work still shouldn't start before those close, per this checkpoint's
    own rule — this only closes the data-layer slice of it.
- **Backlog** (post-MVP order): Overlay & Comparison → Revision Lifecycle &
  Projects → OCR & Search → Forms/Signatures/Advanced Security → Advanced
  Export/Import → Collaboration → AI → 3D → Punch/RFI → Plugin/Scripting.

## What happens when the two missing artifacts arrive

1. `combined-feature-master-list.md` → populate `docs/features/FEATURE_REGISTRY.md`
   with real feature IDs, map each to MVP/BACKLOG, recompute the MVP list
   (prompt's provisional MVP categories in Section 4 are a starting filter,
   not a substitute for the real list).
2. Existing app location → run the Rule 3 evaluation (MVP coverage,
   production-usable vs. partial features, structural blockers, browser
   performance question, tile-rendering necessity) and close
   `docs/architecture/adr/ADR-001-build-vs-extend.md` with an actual decision.
