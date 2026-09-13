# Database Model

## Status

Conditional / candidate — same caveat as `docs/architecture/README.md`.
This applies only if Path B is chosen in [ADR-001](../architecture/adr/ADR-001-build-vs-extend.md).

## Core entities (Section 11 of the build prompt)

```text
Document
    ├── DocumentVersion
    └── Page
          ├── Markup
          │      └── MarkupComment
          ├── Measurement
          │      └── Scale
          └── ...

TakeoffItem
RecoveryState
```

| Table | Fields |
|---|---|
| Document | id, file_path, title, created_at, updated_at |
| DocumentVersion | id, document_id, version_number, file_snapshot_path, created_at, created_by |
| Page | id, document_id, page_number, width, height, rotation |
| Markup | id, page_id, type, geometry_json, style_json, author, created_at, updated_at, locked, hidden |
| MarkupComment | id, markup_id, author, text, created_at |
| Measurement | id, page_id, scale_id, type, geometry_json, value, unit, label, created_at |
| Scale | id, page_id, ratio, unit_system, calibrated_by, created_at |
| TakeoffItem | id, measurement_id, description, quantity, unit, cost_per_unit, notes, created_at |
| RecoveryState | id, document_id, snapshot_path, created_at |

## Rules (Section 12)

- UUID primary keys.
- Foreign-key enforcement on, migrations tracked, indexes where justified,
  timestamps everywhere, writes wrapped in transactions.
- `DocumentVersion` rows are append-only — a save creates a new version, it
  never overwrites/destroys history.
- `Page → Markup` and `Page → Measurement` use `ON DELETE CASCADE`.
- Document/version relationships avoid destructive deletes that could
  silently erase history.

## Added 2026-09-13 — tables required by the confirmed collaboration requirement

Not in the build prompt's original schema (which assumed single-user).
Added as an essential dependency of the confirmed multi-user requirement
(see `docs/00_SCOPE_REALITY_CHECK.md` and ADR-001), not silent scope
expansion — flagged here per Section 26/36.

| Table | Fields |
|---|---|
| User | id, email, display_name, created_at |
| Project | id, name, created_at, created_by |
| ProjectMember | id, project_id, user_id, role, created_at |

- `Document` gains a `project_id` FK (nullable initially, since a
  single-user document may not belong to a shared project yet).
- `ProjectMember.role` is a plain text field for now (e.g. `owner`,
  `member`) — no formal RBAC designed yet; kept minimal until the
  collaboration model (async vs. real-time) is confirmed.

## Added 2026-09-13 — Rfi (RFI-01/02)

| Table | Fields |
|---|---|
| Rfi | id, document_id, page_id, markup_id, number, title, description, status, response, created_by, created_at, updated_at |

- `page_id`/`markup_id` are both nullable and independent —
  `ON DELETE SET NULL` on both, so deleting the page/markup an RFI pointed
  at doesn't cascade-delete the RFI (the question/answer trail outlives the
  drawing element it was raised against).
- `number` is sequential per `document_id` (`UNIQUE (document_id, number)`),
  assigned as `MAX(number) + 1` at insert time — so RFIs can be referred to
  the way they are on a real job site ("RFI #14"), not by opaque id.
- `status` is a plain text field (`open`/`answered`/`closed`, matching
  RFI-02's three states from the master list) — no state machine enforced
  at the DB layer; `response` holds the answer text once one exists.

## Implementation status

Initial migration (`crates/mds_db/migrations/0001_initial.sql`) implements
the full table set above plus the original schema, with UUID (text) primary
keys, `FOREIGN KEY` constraints, `ON DELETE CASCADE` for `Page → Markup` and
`Page → Measurement` per Rule 12, and timestamp columns. Verified with a
`cargo test` that runs the migration against an in-memory SQLite database
and asserts every table exists — see `crates/mds_db/src/lib.rs`.

Not yet designed: indexes beyond FK columns (deferred until real query
patterns are known), and the FTS5 schema (deferred until OCR/search is
built, per the build prompt).
