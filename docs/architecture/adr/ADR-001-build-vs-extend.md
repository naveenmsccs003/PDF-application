# ADR-001: Build (Rust/Tauri rewrite) vs. Extend (existing CI3 app)

## Status

**PROVISIONAL — Path B (Rust + Tauri rewrite).** Not a Rule-3-compliant
final decision (see "Provisional decision" section below). Revisit
immediately if the existing app's location is ever provided.

## Context

The master build prompt (Rule 3) requires evaluating the existing
CodeIgniter 3 / PDF.js / Fabric.js / TCPDF / FPDI PDF annotation application
before committing to a Rust/Tauri rewrite. The evaluation needs to answer:

1. How much of the MVP is already implemented?
2. Which features are already production-usable?
3. Which are partially implemented / cheaply extensible?
4. Which requirements are structurally difficult in the existing system?
5. Is native desktop/offline operation actually necessary?
6. Are large/vector-heavy PDFs a genuine browser performance problem in
   the existing app?
7. Is tile rendering required, and can the existing architecture support it?
8. Is offline-first a hard business requirement?
9. Would extending the existing app produce an acceptable MVP faster?

## Why this is blocked

The existing application was not found anywhere on this machine
(`/home/naveen`) during Phase 0 discovery — searched by directory structure
and by grepping for `TCPDF`, `FPDI`, `fabric.js` signatures. Naveen has
confirmed it exists "elsewhere" but has not yet provided a path, repository
URL, server address, or archive.

Without inspecting the actual code, none of the nine evaluation questions
above can be answered honestly. Choosing Path B (rewrite) right now would
violate Rule 3 ("determine whether extending is more rational" — before
choosing the rewrite) and the master prompt's explicit warning in Section 39
not to assume the rewrite is automatically better.

## Decision

**Deferred** on a genuine Rule-3-compliant basis — the nine questions above
still cannot be answered honestly without the existing app.

## Provisional decision (2026-09-13): proceed with Path B, explicitly caveated

Naveen was asked for the existing app's location three times across the
scoping conversation (see `docs/00_SCOPE_REALITY_CHECK.md` and memory
`project_phase0_blocked.md`) and each time replied "next step" without
providing it. Per the interaction-style feedback captured in memory
(`feedback_interaction_style.md`), the right call was judged to be: state a
reasoned default and keep moving, rather than stall indefinitely on a gate
Naveen isn't in a position to close right now.

**Provisionally proceeding with Path B** based on the evidence actually
available:
- Confirmed greenfield trigger (not a reactive fix to the old app).
- Heavy weight the requirements place on offline reliability (autosave,
  crash recovery), which the master prompt treats as architectural, not
  incidental.
- The unresolved collaboration signal above still cuts the other way
  somewhat — this is acknowledged, not resolved, by this provisional call.

**This explicitly does not satisfy Rule 3.** Questions 1–4, 6, 7, and 9 in
the Context section remain genuinely unanswered — there is no evidence
either app inspection or otherwise. If the existing app's location surfaces
later and inspection favors Path A, this decision reverses, and Phase 1 work
already done (`app/`, `crates/mds_db`, `crates/pdf_engine_spike`) would need
to be evaluated for salvageability rather than assumed wasted (the SQLite
schema and PDF-engine findings are largely stack-agnostic).

Phase 1 foundation work has started on this provisional basis — see
`docs/PROJECT_PLAN.md`.

## Required to unblock

- Access to the existing CI3/PDF.js/Fabric.js/TCPDF/FPDI application's
  source code (path, git remote, server credentials, or exported archive).

Once available: inspect it, answer the nine questions above with evidence,
record findings in `docs/00_SCOPE_REALITY_CHECK.md`, and close this ADR with
an explicit Path A or Path B decision plus reasoning.

## New signal (2026-09-13): confirmed multi-user collaboration requirement

Naveen confirmed that multiple users (estimators, detailers, PMs, field
crews) need to work on the *same document/project*, not isolated copies —
see `combined-feature-master-list.md` (draft) and memory
`project_scope_signals.md`. Assumed model for now: async shared review
(edits visible to others after save/sync), pending confirmation — not
real-time simultaneous co-editing.

This weighs on question 5 ("Is native desktop/offline operation actually
necessary?") and question 8 ("Is offline-first a hard business
requirement?") from the Context section above:

- The existing CI3 app, being a web app, is naturally server-hosted and
  already multi-user by construction (login/session model) — satisfying
  the collaboration requirement may be "already there" rather than new work.
- Path B (offline-first Tauri/Rust desktop) would need a sync/server layer
  *added* to satisfy the same requirement, which is scope the master
  prompt's default Path B architecture (Section 5/7) does not include as
  designed — it assumes local-first single-user with SQLite as the source
  of truth.

This does not close the ADR — the existing app still hasn't been inspected,
so it's unknown how much of the *other* eight questions (rendering
performance, tile-rendering need, etc.) still favor a rewrite regardless of
the collaboration point. But it means "extend" should not be dismissed
before inspection just because Rust/Tauri was the prompt's default
candidate stack.
