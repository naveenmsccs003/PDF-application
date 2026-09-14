# pdf_engine_spike

The PDF engine validation spike required by the master build prompt
(Section 6) before building anything real on top of PDFium. Standalone Rust
binary, deliberately independent of the Tauri app so it could be validated
without the webkit2gtk system packages the Tauri build needs.

## Setup

`lib/` is not committed (the binary is 7-8MB third-party, gitignored). Every
other `libpdfium` path in this workspace (this crate, `pdf_core`'s
real-engine tests, and `app/src-tauri`'s dev-mode PDF loading) reads from
this same `crates/pdf_engine_spike/lib/` directory, so fetching it once here
is enough for all of them.

The filename must match what `pdfium-render` looks for on your OS
(`Pdfium::pdfium_platform_library_name()`, used everywhere in this
workspace that resolves this path — not hardcoded per-platform):
`libpdfium.so` on Linux, `libpdfium.dylib` on macOS, `pdfium.dll` on
Windows.

Linux:
```sh
cd crates/pdf_engine_spike
mkdir -p lib
curl -sL "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8044/pdfium-linux-x64.tgz" -o /tmp/pdfium.tgz
mkdir -p /tmp/pdfium-extract && tar xzf /tmp/pdfium.tgz -C /tmp/pdfium-extract
cp /tmp/pdfium-extract/lib/libpdfium.so lib/libpdfium.so
```

macOS (use `pdfium-mac-x64.tgz` on Intel):
```sh
cd crates/pdf_engine_spike
mkdir -p lib
curl -sL "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8044/pdfium-mac-arm64.tgz" -o /tmp/pdfium.tgz
mkdir -p /tmp/pdfium-extract && tar xzf /tmp/pdfium.tgz -C /tmp/pdfium-extract
cp /tmp/pdfium-extract/lib/libpdfium.dylib lib/libpdfium.dylib
```

Windows (PowerShell):
```powershell
cd crates\pdf_engine_spike
mkdir lib
Invoke-WebRequest "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8044/pdfium-win-x64.tgz" -OutFile pdfium.tgz
tar xzf pdfium.tgz -C pdfium-extract
Copy-Item pdfium-extract\bin\pdfium.dll lib\pdfium.dll
```

**Only the Linux path has actually been run in this project.** The macOS
and Windows steps follow the same released asset layout
(bblanchon/pdfium-binaries) and the code path resolution is now
platform-aware (see `pdf_core::platform_library_filename()`), but neither
has been exercised on a real macOS/Windows machine — see the top-level
`README.md`'s "Known limitations" for the full picture.

## Run

```sh
cargo run -p pdf_engine_spike -- /path/to/some.pdf
```

## Results (2026-09-13)

Run against `UIUX-Exact-10-Day-Roadmap.md.pdf` (7 pages, small text PDF —
**not** the realistic 100+ page vector-heavy construction drawing the master
prompt asks for; that still needs to come from Naveen). See
`docs/00_SCOPE_REALITY_CHECK.md` for the full PASS/FAIL/PARTIAL table and
what this does and doesn't prove.

Key finding: creating a second `Pdfium::bind_to_library` + `Pdfium::new`
instance in the same process (attempted for the "reopen" step) deadlocks
(observed as a futex wait, process hangs indefinitely). Fixed by reusing one
`Pdfium` instance for the whole process lifetime. **Architectural
consequence for the real app:** `Pdfium` must be initialized once and held
as a long-lived singleton (e.g. in Tauri managed state), never constructed
per-operation or per-request.
