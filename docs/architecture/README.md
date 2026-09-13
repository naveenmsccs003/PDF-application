# Architecture

## Status

Conditional / candidate. This describes the architecture **if** Path B
(Rust + Tauri rewrite) is chosen in
[ADR-001](adr/ADR-001-build-vs-extend.md), which is currently unresolved.
Nothing here has been implemented, and none of it applies if Path A (extend
the existing CI3 app) is chosen instead.

## Layering (prescribed by the build prompt, Section 7)

```text
Presentation        React / TypeScript
        ↓
Application          Commands, Undo/Redo, Jobs
        ↓
Domain                Document, Markup, Measurement, Takeoff, Project, Security
        ↓
Core Engine          PDF abstraction, Geometry, Rendering, Export
        ↓
Infrastructure       SQLite, Filesystem, Cache, Encryption, Logging
```

Rules:
- UI never touches low-level PDF infrastructure directly.
- Domain layer has no React dependency.
- Core engine has no UI dependency.
- AI, collaboration, 3D, and plugins are modular add-ons, never hard
  dependencies of core viewing/markup.

## Candidate stack (Section 5) — only if Path B is chosen

- Desktop shell: Tauri 2.x
- Frontend: React + TypeScript
- Core: Rust
- PDF: PDFium via `pdfium-render`, behind an internal trait/abstraction so
  the engine can be swapped later (domain layer must not couple to PDFium
  directly)
- Database: SQLite (WAL mode, prepared statements, migrations, FK
  enforcement, UUID primary keys; FTS5 only once OCR/search is built)
- Async/background: Tokio
- CPU parallelism: Rayon (only where profiling justifies it)
- Spatial indexing: `rstar` (R-tree) for markup/measurement hit testing
- Distribution: Tauri updater

## Performance constraints baked into the architecture (Section 8)

- Persistent rendering surface — the PDF canvas does not get re-rendered by
  React on every pan/zoom/pointer event.
- Tile-based rendering with an LRU cache from the start, not retrofitted.
- Long operations (OCR, export, comparison, batch, AI, large imports) run as
  background jobs with `QUEUED/RUNNING/COMPLETED/FAILED/CANCELLED` states;
  the frontend subscribes rather than blocks.
- IPC carries binary buffers for image/tile data, not JSON-encoded pixels.

## Before this becomes real

A PDF engine validation spike (Section 6) must pass against a realistic
100+ page, vector-heavy construction drawing before any phase is built on
top of PDFium — open, render, extract text, annotate, incremental save,
reopen, Windows binary distribution, memory behavior. Results get recorded
as PASS/FAIL/PARTIAL/BLOCKED in `docs/00_SCOPE_REALITY_CHECK.md`.
