# Scope Reality Check — Phase 0 Discovery

Status: **BLOCKED** — mandatory discovery gates not satisfied.
Date: 2026-09-13

## What was checked

1. `/home/naveen/pdf_application` (the project working directory)
   - Contents: empty. No source files, no README, no package manifests, no git
     history of its own.
   - It is not its own git repository — there is no `.git` inside it.

2. Search for the authoritative feature source, `combined-feature-master-list.md`
   - Searched recursively under `/home/naveen` (excluding `node_modules`).
   - Result: **not found anywhere on this machine.**

3. Search for the existing CodeIgniter 3 / PDF.js / Fabric.js / TCPDF / FPDI
   PDF annotation application referenced as the thing to evaluate for
   "extend vs. rewrite."
   - Grepped all `.php`/`.html` files under `/home/naveen` for `TCPDF`, `FPDI`,
     `fabric.js`, `fabric.min.js`.
   - Inspected the other project directories under `/home/naveen`
     (`mds_rebar_dynamic_site`, `mds website`, `hrms software`, `rebar calculater`,
     `python_projects`) by structure.
   - Result: **no match found.** `mds_rebar_dynamic_site` has its own git repo
     with a `backend/`/`frontend/`/Docker layout, but no CodeIgniter/PDF.js/
     Fabric.js/TCPDF signatures — it does not appear to be the application
     described in the build prompt.

## Unrelated but important finding — stray git repository at `$HOME`

`/home/naveen` itself (the parent of this project) has a `.git` directory with
no commits, currently showing the entire home directory as untracked,
including `.ssh/`, `.gnupg/`, `.claude.json`, `.bash_history`, and every other
personal/project folder. This was already present at the start of this
session — I did not create it.

Nothing has been committed yet, so nothing has leaked. But this is a
dangerous state: a stray `git add -A && git commit` (or an automated hook)
run from `$HOME` or any of its subdirectories — including this one — could
stage private keys and credentials into a repo that might later be pushed.

**Resolved:** Naveen confirmed no commits existed and asked for it to be
removed. Verified `git log --all` and `git branch -a` were both empty, then
deleted `/home/naveen/.git`. No data was lost.

## Decision per Rule 2 (combined-feature-master-list.md missing)

Per the operating rules for this project: if the authoritative feature list
is not present, **STOP**. Do not reconstruct the feature list from the build
prompt. Do not guess. Do not continue implementation.

This condition is met. Phase 0 cannot proceed past this point:

- `docs/features/FEATURE_REGISTRY.md` cannot be created — it must be derived
  from `combined-feature-master-list.md`, which does not exist here.
- The build-vs-extend decision (Rule 3) cannot be evaluated — the existing
  CI3/PDF.js/Fabric.js application to compare against was not found on this
  machine.
- MVP coverage, architecture docs, and database docs are not started, since
  they depend on the above.

## Open items requiring Naveen's input

1. Where is `combined-feature-master-list.md`? (Still not provided — content
   not pasted, no file has appeared under `/home/naveen/pdf_application`.)
2. Where is the existing CI3/PDF.js/Fabric.js/TCPDF/FPDI application?
   Confirmed by Naveen to exist "elsewhere," but no path, repo URL, or
   archive has been given yet — still not found on this machine.
3. Build vs. extend — cannot be assessed without #2. Tracked in
   [ADR-001](architecture/adr/ADR-001-build-vs-extend.md), currently BLOCKED.
4. Business model: internal tool / commercial product / undecided
   (default assumption per the build prompt: internal-first,
   commercial-capable).
5. Available development time: hours/week, and any real deadline.

## Project plan scaffold (added at Naveen's request, without waiting on #1/#2)

Naveen asked to start building the project plan before items 1–2 above were
resolved. What was added:

- `docs/PROJECT_PLAN.md` — phase roadmap (Phase 0–5, MVP checkpoint, backlog
  order), restated from the master prompt's own phase definitions, not
  invented.
- `docs/features/FEATURE_REGISTRY.md` — table schema only, explicitly marked
  BLOCKED, no feature rows (per Rule 2, nothing was reconstructed from the
  build prompt's provisional category list).
- `docs/architecture/README.md` and `docs/database/README.md` — the
  layering, candidate stack, and entity model the build prompt already
  prescribes for Path B, marked conditional on ADR-001 being resolved as
  Path B.
- `docs/architecture/adr/ADR-001-build-vs-extend.md` — status BLOCKED,
  documents exactly what's needed to close it.

None of this constitutes starting Phase 1 implementation, choosing Path A/B,
or populating the real feature registry — those still wait on items 1 and 2.

## Update — 2026-09-13, scoping conversation

Naveen confirmed:
- `pdf_application` (this directory) is canonical; `pdf_complete_application`
  was a stray duplicate invocation directory.
- Business model: no override given — proceeding with the prompt's default
  (internal-first, commercial-capable; no multi-tenancy/billing/SSO).
- `combined-feature-master-list.md` doesn't exist yet — agreed to build it
  collaboratively rather than wait for a pre-written file. A **draft** now
  exists at the repo root, and `docs/features/FEATURE_REGISTRY.md` has been
  populated from it. Both are explicitly marked DRAFT/provisional pending
  Naveen's review — this does not satisfy Rule 2 as a frozen source yet, but
  unblocks forward motion on documentation while item 2 (existing app) is
  still pending.
- Users are all roles (estimators, detailers, PMs, field crews), and all
  four workflow areas — review/markup, measurement/takeoff, RFI/revision
  tracking, export/handoff — are wanted, not narrowed to a subset.
- **Multi-user collaboration on shared documents/projects is a confirmed
  requirement** (not backlog). Model assumed: async shared review (edits
  visible to others after save/sync), not real-time co-editing — this
  specific detail is still an assumption, not confirmed. See
  `ADR-001-build-vs-extend.md` for why this reopens the build-vs-extend
  question: it favors evaluating the existing (web-based, naturally
  multi-user) CI3 app more seriously once it's located.
- Trigger for the project is greenfield/new capability, not a specific bug
  in the current CI3 app.

Item 2 (existing app location) is still outstanding — asked twice, not yet
provided. ADR-001 remains open.

## Update — 2026-09-13, provisional ADR-001 decision + Phase 1 start

After item 2 remained unanswered through multiple "next step" replies,
**ADR-001 was closed provisionally as Path B (Rust + Tauri rewrite)** —
explicitly marked provisional/revisable, not a final Rule-3-compliant
decision (see `architecture/adr/ADR-001-build-vs-extend.md`). Reasoning:
confirmed greenfield trigger, heavy weight the requirements place on offline
reliability/autosave/crash-recovery and large-PDF rendering performance, and
no way to evaluate "extend" without the app's location despite repeated
requests. This will be revisited immediately if the existing app's location
is ever provided and changes the picture.

Phase 1 foundation work started on that basis: Rust toolchain installed,
git repo initialized, Tauri 2 + React + TypeScript scaffold created in
`app/`, and a SQLite/migrations crate (`crates/mds_db`) built and verified
independently of Tauri (see `PROJECT_PLAN.md` for full detail and test
evidence). `app/src-tauri` itself still cannot `cargo check` — blocked on
`pkg-config`/webkit2gtk system packages that require an interactive sudo
password this session doesn't have; Naveen has been asked to run that
install.

## PDF Engine Validation Spike (Section 6) — results, 2026-09-13

Run as a standalone Rust binary (`crates/pdf_engine_spike`) using
`pdfium-render` 0.8.37 against a prebuilt `libpdfium.so` (chromium/8044,
linux-x64, from bblanchon/pdfium-binaries) — deliberately independent of
Tauri so it didn't need to wait on the webkit2gtk system packages.

**Important caveat: no realistic 100+ page vector-heavy construction/rebar
drawing was available on this machine.** Every PDF found under `/home/naveen`
was a small text-based document (roadmaps, guides, résumés). The spike ran
against `learn road map/UIUX-Exact-10-Day-Roadmap.md.pdf` (7 pages) as a
smoke test of the plumbing only. **This does not satisfy Section 6's actual
requirement** — a real construction/engineering drawing sample from Naveen
is still needed to validate rendering performance and correctness under the
actual target workload (dense vectors, large page count).

| # | Check | Result | Notes |
|---|---|---|---|
| 1 | Open PDF | PASS | 39–49ms |
| 2 | Render representative page | PASS | 106–112ms at 1600px target width |
| 3 | Render large/vector-heavy page | **SKIPPED** | No such sample available — needs Naveen to supply one |
| 4 | Measure render time | PASS | See #2 |
| 5 | Extract text | PASS | 3–5ms, 1275 chars |
| 6 | Add an annotation | PASS | `create_free_text_annotation` — 0.2ms |
| 7 | Attempt incremental save | PASS | `save_to_file`, 75–142ms |
| 8 | Reopen saved result | PASS | 0.03–0.7ms; confirmed the added annotation survived (7 annotations found on page 0 after reopen — the source PDF already had 6, e.g. hyperlinks from its markdown→PDF conversion, plus the 1 we added) |
| 9 | Confirm Windows binary distribution | PARTIAL | bblanchon/pdfium-binaries publishes a `windows-x64` asset for the same pdfium build — availability confirmed, not runtime-tested (this machine is Linux) |
| — | Memory behavior | Not measured | Small PDF, short-lived process — not meaningful without a realistic workload |
| — | Failures | 1 found and fixed | See below |

**Real finding, not theoretical:** the first version of the spike created a
second `Pdfium::bind_to_library`/`Pdfium::new` instance in-process for the
"reopen" step, which **deadlocked** (observed via `/proc/<pid>/wchan` =
`futex_do_wait`, process hung indefinitely, had to be killed). Fixed by
reusing a single `Pdfium` instance for the process lifetime.
**Architectural consequence:** the real app must hold `Pdfium` as a
long-lived singleton (e.g. Tauri managed state initialized once at startup),
never construct it per-operation.

**Overall verdict: PARTIAL.** The engine works correctly for everything
tested, but the one check that actually matters most for this product
(large/vector-heavy construction drawing performance) is unvalidated. Per
Section 6 ("do not build multiple phases on top of a PDF engine that has
failed a required validation"), this isn't a failure, but it is an open
risk — Phase 2 (PDF Core) rendering-performance work should not be
considered de-risked until a real sample is tested.

## Update — 2026-09-14: VIEW-04 synthetic validation (still not the real thing)

With every other feature-registry row closed (SEC-03, then RFI-04 as the
first Backlog item), the only remaining open MVP row was VIEW-04 — exactly
the "#3 SKIPPED" gap the spike results above already called out. Still no
real MDS Rebar drawing available in this environment, so rather than leave
it SKIPPED indefinitely, built a synthetic stand-in and measured against
it: `crates/pdf_core/tests/large_pdf_performance.rs`, a 30-page, ARCH-D-sized
(2592x1728pt) PDF generated at test time (raw PDF bytes written directly —
no PDF-authoring dependency existed in this workspace, and adding one for a
test fixture wasn't justified), ~1200 independently-stroked line segments
plus ~80 text labels per page, meant to approximate a rebar placement
sheet's line/label density. **This is explicitly not a substitute for a
real drawing** — no title block, no real geometry, no embedded raster
images, no complex path fills/gradients a real CAD export might have — only
a stand-in that can't be mistaken for the genuine Section 6 requirement,
which is still open and still needs Naveen to supply a real sample.

What it's good for regardless: catching a *catastrophic* degradation (not
just a slow one) in the render/tile pipeline as page count and path density
scale up, which no test in this workspace exercised before now (every
existing `pdf_core` real-engine test uses a single small page).

**First run, `cargo test -p pdf_core` (this workspace's normal invocation,
debug build): looked alarming** — 2.6-6.9s per full-page render at 1600px,
~13s per tile (512px tile, 2x zoom), 204s to render 16 tiles. That's
unusable for interactive zoom/pan, and briefly looked like confirmation
that the crate's own documented "re-rasterizes the whole page per tile,
not the performance win tiling is meant to provide" caveat was a real,
severe blocker.

**Re-run under `cargo test -p pdf_core --release`: not a real problem, a
debug-build artifact.** Same test, same synthetic PDF, only the build
profile changed: page 0 rendered in 4.27s (a one-time warm-up cost, see
below), but every page after that rendered in 80-85ms; tiles averaged
~250ms each. That's a **~30-50x gap** between debug and release, almost
certainly the `image` crate's PNG encode/decode/crop path (which
`render_tile_to_png`'s default implementation runs on every call) being
far more expensive unoptimized than PDFium's own (already-compiled-release)
rendering call. **This means the first run's numbers were misleading, not
wrong** — they're real debug-build timings, just not representative of
what a compiled Tauri release binary will do. Lesson recorded directly in
the test file's own doc comment so a future debug-mode run of this same
test doesn't cause the same false alarm again.

One more real, worth-keeping finding from the release run: the *first*
full-page render call in a process took 4.27s even in release mode; every
subsequent render (including of different pages) was 80-85ms. This reads
like a one-time PDFium/font/cache warm-up cost, not a per-page cost — worth
a real app calling one throwaway render during startup (or accepting one
slow first page-open) rather than assuming later pages will be equally
slow. Not investigated further this pass (would need profiling PDFium
itself, not just this crate, to say what specifically warms up).

**Numbers this run does support** (release-mode, synthetic PDF, all
figures approximate):
- Full-page render at 1600px target width: ~80-85ms/page after warm-up.
- One-time first-render warm-up cost: ~4.3s, once per process.
- Tile render at 2x zoom, 512px tiles (existing "full-page-render-then-crop"
  implementation, unchanged this pass): ~250ms/tile. A full 7x5=35-tile
  grid for one page would be ~8.75s if rendered serially and completely
  uncached — tolerable for progressive/on-demand tile loading (typical
  viewport only needs a handful of visible tiles at once, and `TileCache`
  makes repeat requests for the same tile effectively free — confirmed:
  4 cached tiles served in 3.3µs vs. ~1s uncached for the same 4), not
  something a user would notice as "broken" the way the debug-mode numbers
  first suggested.

**What this does NOT validate**: real construction-drawing content (raster
images, gradients, complex fills, embedded fonts beyond one base-14 face,
genuinely large page counts like 100+), correctness of the *rendered
output* against a real drawing (only pixel-count/PNG-magic-byte checks ran,
no human or automated visual comparison), and whether 250ms/tile remains
acceptable once real tile-based zoom/pan interaction exists in the UI
(VIEW-01/02 currently re-request the whole page at a new zoom width, not
individual tiles — so this pipeline's tile path isn't actually wired to
anything user-facing yet either). VIEW-04 stays open pending a real sample
from Naveen; see `docs/features/FEATURE_REGISTRY.md`'s VIEW-04 row for
current status.
