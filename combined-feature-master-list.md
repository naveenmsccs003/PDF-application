# Combined Feature Master List — MDS Rebar PDF Platform

**STATUS: DRAFT — proposed by Claude, not yet reviewed/approved by Naveen.**
This file becomes the authoritative feature source (per the master build
prompt's Rule 2) only once Naveen confirms or edits it. Until then, treat
`docs/features/FEATURE_REGISTRY.md` as still blocked.

## Basis for this draft

Built from a scoping conversation on 2026-09-13, not invented from the build
prompt's own placeholder categories:

- Users: "all types" — estimators, detailers, PMs, field crews all use the
  tool (not a single-role tool).
- Workflow priority: all four of review/markup, measurement/takeoff,
  RFI/revision tracking, and export/handoff were named as needed — none
  deferred to backlog by Naveen.
- Collaboration: **confirmed requirement.** Multiple users work on the same
  document/project, not isolated copies. Model assumed for this draft:
  **async shared review** (edits become visible to others when saved/synced;
  no live simultaneous-editing/cursors) — this is an assumption pending
  confirmation, not a decision.
- Trigger: greenfield — not a reactive fix to a specific bug in the existing
  CI3 app.
- Existing CI3/PDF.js/Fabric.js/TCPDF/FPDI app: location not yet provided,
  so Rule 3's build-vs-extend evaluation is still open. **The confirmed
  collaboration requirement is a point in favor of extending that app**
  (a web app is naturally server-hosted/multi-user already) rather than an
  offline-first desktop rewrite, which would need a sync layer added to meet
  the same requirement. See `docs/architecture/adr/ADR-001-build-vs-extend.md`.

## How to read this table

Same schema the registry will use. `Tier` is a *proposal* — confirm or
correct each row's tier with Naveen before it's frozen.

| ID | Module | Feature | Tier | Notes |
|---|---|---|---|---|
| DOC-01 | Document | Open PDF | MVP | |
| DOC-02 | Document | Save project/document | MVP | Must not destroy prior version — see DocumentVersion model |
| DOC-03 | Document | Close document | MVP | |
| DOC-04 | Document | Page navigation | MVP | |
| DOC-05 | Document | Page operations (rotate, reorder) | MVP | Confirm which operations are actually needed |
| DOC-06 | Document | Shared project membership (who can open/edit a project) | MVP | Direct consequence of confirmed collaboration requirement |
| VIEW-01 | Viewing | Zoom | MVP | |
| VIEW-02 | Viewing | Pan | MVP | |
| VIEW-03 | Viewing | Thumbnails | MVP | |
| VIEW-04 | Viewing | Page rendering performance (large/vector-heavy PDFs) | MVP | Needs the PDF engine validation spike before committing to an engine |
| MARK-01 | Markup | Text annotation | MVP | |
| MARK-02 | Markup | Basic shapes (rectangle, cloud, line, arrow) | MVP | Cloud is standard for construction redlines — confirm shape set |
| MARK-03 | Markup | Select / move / resize / edit | MVP | |
| MARK-04 | Markup | Delete | MVP | |
| MARK-05 | Markup | Markup list / layer panel | MVP | |
| MARK-06 | Markup | Undo / redo | MVP | |
| MARK-07 | Markup | Comments/threads on a markup | MVP | Needed for async review — someone marks up, another replies |
| MARK-08 | Markup | Visibility toggle / lock | MVP | |
| MEAS-01 | Measurement | Manual scale calibration | MVP | |
| MEAS-02 | Measurement | Length measurement | MVP | |
| MEAS-03 | Measurement | Area measurement | MVP | |
| MEAS-04 | Measurement | Count measurement | MVP | |
| MEAS-05 | Measurement | Unit conversion | MVP | |
| MEAS-06 | Measurement | Measurement labels | MVP | |
| MEAS-07 | Measurement | Measurement persistence | MVP | |
| TAKE-01 | Takeoff | Manual quantity entry | MVP | |
| TAKE-02 | Takeoff | Measurement-linked quantities | MVP | |
| TAKE-03 | Takeoff | Descriptions / units / notes per line item | MVP | |
| TAKE-04 | Takeoff | Cost per unit | MVP | Confirm — estimating-relevant, may be needed day one for MDS Rebar |
| TAKE-05 | Takeoff | CSV/Excel export | MVP | |
| RFI-01 | RFI / Revision | Create RFI tied to a page/markup | MVP | Elevated from the build prompt's backlog position #9 per Naveen's priority |
| RFI-02 | RFI / Revision | RFI status tracking (open/answered/closed) | MVP | |
| RFI-03 | RFI / Revision | Drawing revision tracking (version history per sheet) | MVP | Overlaps with DocumentVersion |
| RFI-04 | RFI / Revision | Revision comparison/overlay | BACKLOG | Genuinely heavier feature (image diffing) — propose deferring unless Naveen disagrees |
| EXPORT-01 | Export | Flattened PDF export | MVP | |
| EXPORT-02 | Export | Print | MVP | |
| EXPORT-03 | Export | Export/handoff package (drawings + markups + takeoff together) | MVP | Named directly by Naveen as a priority ("export/handoff") |
| COLLAB-01 | Collaboration | Shared project access for multiple users | MVP | Confirmed requirement — model TBD (async assumed) |
| COLLAB-02 | Collaboration | Async update visibility (see others' markups/RFIs after save/sync) | MVP | Assumption pending confirmation |
| COLLAB-03 | Collaboration | Real-time simultaneous co-editing (live cursors) | BACKLOG | Only if Naveen confirms this over async — much higher cost |
| REL-01 | Reliability | Autosave | MVP | |
| REL-02 | Reliability | Crash recovery (restore/discard) | MVP | |
| SEC-01 | Security | Local password protection | MVP | |
| SEC-02 | Security | Secure credential handling | MVP | |
| SEC-03 | Security | User accounts / access control for shared projects | MVP | Direct consequence of confirmed collaboration requirement — not in the build prompt's original MVP list, which assumed single-user |

## Explicitly open questions this draft does NOT resolve

1. Async vs. real-time collaboration — assumed async for this draft.
2. Where shared data lives (server MDS Rebar hosts, vs. something else) —
   not decided; depends on #1 and on the existing app inspection.
3. Existing CI3 app location — still needed to close ADR-001.
4. Whether "cost per unit" (TAKE-04) is actually used by MDS Rebar's
   estimating process, or costing lives in a separate system entirely.

## Next step

Naveen reviews/edits this table (add, remove, re-tier rows — this is a
starting proposal, not a finished spec). Once he confirms it, it gets
frozen and `docs/features/FEATURE_REGISTRY.md` gets populated from it with
per-feature status tracking.
