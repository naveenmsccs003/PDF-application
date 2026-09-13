# pdf_engine_spike

The PDF engine validation spike required by the master build prompt
(Section 6) before building anything real on top of PDFium. Standalone Rust
binary, deliberately independent of the Tauri app so it could be validated
without the webkit2gtk system packages the Tauri build needs.

## Setup

`lib/libpdfium.so` is not committed (7.8MB third-party binary, gitignored).
Fetch it before running:

```sh
cd crates/pdf_engine_spike
mkdir -p lib
curl -sL "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8044/pdfium-linux-x64.tgz" \
  | tar xz -C /tmp/pdfium-extract --strip-components=0 2>/dev/null || \
  (mkdir -p /tmp/pdfium-extract && curl -sL "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8044/pdfium-linux-x64.tgz" -o /tmp/pdfium.tgz && tar xzf /tmp/pdfium.tgz -C /tmp/pdfium-extract)
cp /tmp/pdfium-extract/lib/libpdfium.so lib/libpdfium.so
```

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
