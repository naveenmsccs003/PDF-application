# MDS Rebar — PDF Review / Measurement / Takeoff Platform

A desktop app (Rust + Tauri, React frontend) for the construction-drawing
review workflow: open a PDF, mark it up (text/shapes/redlines), measure it
(scale calibration → length/area/count), turn measurements into a takeoff,
track RFIs and drawing revisions, and export/hand off a flattened package —
all offline-first, with async multi-user project sharing.

See `docs/PROJECT_PLAN.md` for the full build history and
`docs/features/FEATURE_REGISTRY.md` for per-feature status (43 of 45
tracked features implemented as of 2026-09-14; see "Known limitations"
below for what's genuinely still open).

**Verified platform: Linux only.** The steps below for macOS/Windows follow
the same tooling Tauri documents for those platforms and the code no longer
hardcodes a Linux-only file path (see below), but neither has actually been
built or run on this project — say so plainly if you hit something that
doesn't match here.

## 1. Prerequisites

| Tool | Used in this project | Get it |
|---|---|---|
| Rust (via rustup) | 1.98.1 | https://rustup.rs |
| Node.js + npm | Node 22.22.1 / npm 9.2.0 | https://nodejs.org (any current LTS should work) |

The Tauri CLI is a devDependency of `app/package.json` — no separate global
install needed, just `npm install`.

### Linux (Debian/Ubuntu) — the only OS this has actually run on

```sh
sudo apt-get update
sudo apt-get install -y pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev libssl-dev file
```

Optional, only needed for SEC-01/02 (opening password-protected PDFs, which
stores the password in the OS keychain rather than the database): a running
Secret Service provider (GNOME Keyring or KWallet). Present by default on
most desktop Linux sessions; headless/minimal installs may need
`gnome-keyring` installed and unlocked.

### macOS — untested, expected to work

- Xcode Command Line Tools: `xcode-select --install`
- Everything else below is identical to Linux, minus the `apt-get` step.

### Windows — untested, expected to work

- Microsoft C++ Build Tools (Visual Studio Installer → "Desktop development
  with C++" workload)
- WebView2 Runtime (preinstalled on Windows 11 and most Windows 10
  machines; Tauri's own docs link the standalone installer if it's missing)

## 2. Clone and install JS dependencies

```sh
git clone <repo-url> pdf_application
cd pdf_application/app
npm install
```

## 3. Fetch the PDFium binary (required, not committed to git)

PDF opening/rendering is done via Google's PDFium, loaded dynamically at
runtime — it's a 7-8MB third-party binary, deliberately kept out of git.
Every `libpdfium` lookup in this workspace (the app itself, the validation
spike binary, and `pdf_core`'s real-engine tests) reads from the same
`crates/pdf_engine_spike/lib/` directory, so this is a one-time step that
covers all of them. The filename must match your OS's native shared-library
name — `libpdfium.so` (Linux), `libpdfium.dylib` (macOS), or `pdfium.dll`
(Windows).

From the repo root:

**Linux**
```sh
mkdir -p crates/pdf_engine_spike/lib
curl -sL "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8044/pdfium-linux-x64.tgz" -o /tmp/pdfium.tgz
mkdir -p /tmp/pdfium-extract && tar xzf /tmp/pdfium.tgz -C /tmp/pdfium-extract
cp /tmp/pdfium-extract/lib/libpdfium.so crates/pdf_engine_spike/lib/libpdfium.so
```

**macOS** (swap `arm64` for `x64` on Intel Macs)
```sh
mkdir -p crates/pdf_engine_spike/lib
curl -sL "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8044/pdfium-mac-arm64.tgz" -o /tmp/pdfium.tgz
mkdir -p /tmp/pdfium-extract && tar xzf /tmp/pdfium.tgz -C /tmp/pdfium-extract
cp /tmp/pdfium-extract/lib/libpdfium.dylib crates/pdf_engine_spike/lib/libpdfium.dylib
```

**Windows** (PowerShell)
```powershell
mkdir crates\pdf_engine_spike\lib
Invoke-WebRequest "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8044/pdfium-win-x64.tgz" -OutFile pdfium.tgz
tar xzf pdfium.tgz -C pdfium-extract
Copy-Item pdfium-extract\bin\pdfium.dll crates\pdf_engine_spike\lib\pdfium.dll
```

If you skip this step, the app still launches — every PDF-touching command
just fails with a clear "PDF engine unavailable (libpdfium.so not found)"
error instead of the app refusing to start.

## 4. Run it

```sh
cd app
npm run tauri dev
```

This compiles the Rust backend, starts the Vite dev server, and opens the
native app window. First launch is slower (full Rust compile); subsequent
launches are fast.

**Sign in** just needs an email — a new one creates an account, an
existing one signs you back in. There's no password on the app itself
(SEC-01/02 are about opening PDFs that are *themselves* password-protected,
not an app-level login).

**First-time tour**: the app opens a short guided walkthrough automatically
the first time it's launched (tracked in the webview's local storage, so it
won't reappear after that). Click **"Take the tour"** in the header any
time to see it again — useful for showing someone else the app, or after
you've forgotten a corner of it.

## 5. Run the test suite

```sh
# Pure-Rust workspace (all domain crates — fast, no webkit2gtk needed)
cargo test --workspace --exclude app

# Tauri backend typecheck
cargo check -p app

# Frontend build
cd app && npm run build
```

One test is worth knowing is slow: `cargo test -p pdf_core --test
large_pdf_performance` renders a synthetic large PDF and reports real
timing. It's ~1 minute under a normal debug build; run it with `--release`
if you actually care about the numbers it prints (debug-mode PNG encoding
is ~30-50x slower than release and makes the printed timings meaningless —
see the test's own doc comment).

## 6. Build a release binary

```sh
cd app
npm run tauri build
```

This has **not been run or verified** in this project — `tauri.conf.json`
still has the scaffold's default bundle config, and the dev-mode
`libpdfium` path (step 3 above) hasn't been switched over to Tauri's
resource-bundling story (`app.path().resource_dir()`) that a real installer
would need. Treat this as the next real step if you want a distributable
build, not something already proven to work.

## Troubleshooting

**`symbol lookup error: ...libpthread.so.0: undefined symbol
__libc_pthread_init, version GLIBC_PRIVATE`** — this means `npm run tauri
dev` is running inside a *snap-sandboxed* terminal (e.g. VS Code installed
via `snap`), which leaks its own `GTK_PATH`/`GIO_MODULE_DIR`/`LOCPATH` into
child processes and points the compiled binary at an incompatible bundled
libc. Run from a normal (non-snap) terminal, or `env -i` with just
`PATH`/`HOME`/`DISPLAY`/`WAYLAND_DISPLAY`/`XDG_RUNTIME_DIR`/
`DBUS_SESSION_BUS_ADDRESS` set.

**"PDF engine unavailable (libpdfium.so not found)"** — step 3 above
wasn't done, or the file isn't at the exact platform-specific filename the
app looks for. Check `crates/pdf_engine_spike/lib/`.

**A password-protected PDF's password doesn't seem to be remembered
between sessions (Linux)** — SEC-02 stores it via the OS's Secret Service.
Confirm one is actually running: `dbus-send --session --print-reply
--dest=org.freedesktop.secrets /org/freedesktop/secrets
org.freedesktop.DBus.Peer.Ping` (no reply / error means nothing's
listening — install and unlock `gnome-keyring` or equivalent).

**`cargo test -p pdf_core` hangs** — a second `PdfiumEngine` alive in the
same process deadlocks (see `crates/pdf_engine_spike/README.md`); every
real-engine test in this workspace already serializes through a shared
`Mutex` for this reason, so this shouldn't happen unless a new real-engine
test is added without going through it.

## Known limitations (honest as of 2026-09-14 — see `docs/` for detail)

- **Linux-only, run-from-source only.** No packaged installer has been
  built or tested; distribution (bundling `libpdfium` as a Tauri resource,
  code signing, an updater) is unstarted.
- **Two open product decisions block calling this "done":** the existing
  CI3/PDF.js/Fabric.js app this was meant to evaluate against (Rule 3,
  `docs/architecture/adr/ADR-001-build-vs-extend.md`) was never located, so
  this whole rewrite is built on a *provisional* decision, not a confirmed
  one; and `combined-feature-master-list.md` — the source for every feature
  row this app is built from — is still marked as an unreviewed draft in
  its own header.
- **VIEW-04** (large/vector-heavy PDF rendering performance) has only been
  validated against a synthetic stand-in PDF, not a real construction
  drawing — see `docs/00_SCOPE_REALITY_CHECK.md`.
- **COLLAB-03** (real-time simultaneous co-editing) is intentionally not
  built — the feature list itself gates it behind an explicit go/no-go
  decision, since it's a much larger architectural commitment than
  everything else here (which all assumes async/save-to-sync sharing).
- **Real interactive click-through is mostly unverified.** Nearly every
  feature has been checked via `cargo test`/`npm run build`/`cargo check`
  and, at most, one pass of manual `npm run tauri dev` testing — not a
  full feature-by-feature walkthrough in the running app.

Full detail, including exact measured numbers and what was and wasn't
tested for every feature: `docs/PROJECT_PLAN.md` and
`docs/features/FEATURE_REGISTRY.md`.
