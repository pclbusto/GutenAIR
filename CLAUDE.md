# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build

# Check for errors (faster than build)
cargo check

# Run tests
cargo test

# Run a single test
cargo test <test_name>

# Run an example
cargo run --example usage
cargo run --example guardian_demo

# Lint warnings auto-fixable
cargo fix --lib -p gutencore
```

## Architecture

`gutencore` is a Rust library (no binary) that manages EPUB books as unpacked folder workspaces. It is the **core** of the GutenAIR project — all logic lives here, and any future UI layer must delegate to this core rather than implement logic independently.

### Central struct: `GutenCore`

Defined in `src/core.rs`. Holds the entire in-memory state of an open EPUB project:

- `workdir` — root folder of the unpacked EPUB
- `opf_path` / `opf_dir` — path to `content.opf` and its parent
- `metadata` — title, language, identifier, modified date
- `manifest` — `HashMap<String, ManifestItem>` keyed by item `id`
- `spine` — ordered `Vec<String>` of manifest IDs

Two entry points: `GutenCore::new_project()` creates a skeleton EPUB on disk; `GutenCore::open_folder()` loads an existing one by parsing `META-INF/container.xml` → `content.opf`.

### Module layout

Each file is an `impl GutenCore` block — there is no trait system, just methods grouped by concern:

| File | Responsibility |
|---|---|
| `core.rs` | Struct definition, `open_folder`, `new_project`, OPF parsing/saving, `update_nav` |
| `manifest.rs` | Manifest and spine CRUD, `add_resource`, `import_file`, `validate_integrity` |
| `metadata.rs` | `get/set_metadata`, `update_modified_date` |
| `toc.rs` | `scan_headings` → `DocToc` / `HeadingItem` |
| `hooks.rs` | `build_hook_index` — indexes all HTML element `id` attributes across the project |
| `cleaner.rs` | `clean_html` (ammonia), `text_to_xhtml` |
| `guardian.rs` | `save_chapter` / `sanitize_to_xhtml` — sanitizes HTML through ammonia + html5ever parse/serialize |
| `export.rs` | `export_epub` — zips the workspace into a valid `.epub` (mimetype first, uncompressed) |
| `error.rs` | `GutenError` enum + `Result<T>` alias |
| `types.rs` | Shared data structs: `ManifestItem`, `BookMetadata`, `DocToc`, `HeadingItem`, `ResourceKind` |

### Key invariants

- `save()` always calls `update_nav()` before writing the OPF, so `nav.xhtml` is always rebuilt from the spine.
- `mimetype` must be the first file in the ZIP and stored uncompressed — enforced in `export_epub`.
- All hrefs are normalized to Unix-style (`/`) regardless of the host OS.
- `guardian.rs` uses `markup5ever_rcdom::SerializableHandle` (not `&dom.document` directly) to avoid a version mismatch between `markup5ever` 0.35 and 0.38.

## UI principle

When adding a UI layer, always implement logic through `gutencore` methods. The UI must be thin — if a feature requires new behavior, expose it from the core first.
