# Technical Documentation - GutenCore

GutenCore is the functional heart of GutenAIR, a robust engine for EPUB management written in Rust. It provides a low-level API for manipulating the OEBPS structure, manifest, spine, and metadata of an EPUB project.

## Architecture Overview

The system is designed with a monolithic core structure (`GutenCore`) that orchestrates several specialized modules.

### Core Modules

- **`core.rs`**: The main entry point. Handles project lifecycle:
    - Scaffolding new projects (`new_project`).
    - Loading existing projects (`open_folder`).
    - Orchestrating the saving process to the OPF file.
- **`manifest.rs`**: Manages the project's internal inventory.
    - **Manifest:** Maps IDs to resource paths (`href`) and media types.
    - **Spine:** Defines the linear reading order of the book.
    - **Atomic Deletion:** Handles the synchronized removal of items from the manifest, spine, and physical disk.
- **`guardian.rs`**: The security and validity layer.
    - **Sanitization:** Uses `ammonia` to strip dangerous tags.
    - **XHTML Validity:** Uses `html5ever` to ensure perfect XHTML structure (closing orphan tags, fixing void elements).
    - **Injection:** Automatically injects CSS links into document headers based on the manifest state.
- **`toc.rs`**: Handles headings scanning and Table of Contents generation.
- **`types.rs`**: Centralized data models and enums.
- **`error.rs`**: Custom error types (`GutenError`) for project-specific failures.

## Data Flow

1. **Ingestion:** Resources are added via `add_resource` or `import_file`.
2. **Persistence:** Files are written to the `workdir` (usually under `OEBPS/`).
3. **Registration:** Items are registered in the `HashMap` manifest and potentially the `Vec` spine.
4. **Synchronization:** Calling `save()` triggers a full rewrite of the `.opf` file and an automatic update of the navigation (`nav.xhtml`).

## Key Dependencies

- `quick-xml`: Fast serialization of the OPF package.
- `roxmltree`: Lightweight XML parsing for ingestion.
- `ammonia`: Secure HTML sanitization.
- `html5ever`: High-performance HTML5/XHTML processing.
- `tempfile`: Used for isolated testing environments.

## File System Structure (Managed)

A project root managed by GutenCore typically follows this structure:

```text
ProjectRoot/
├── META-INF/
│   └── container.xml
├── OEBPS/
│   ├── content.opf
│   ├── Text/        (XHTML Documents)
│   ├── Styles/      (CSS)
│   ├── Images/      (Assets)
│   └── nav.xhtml    (Auto-generated TOC)
└── mimetype
```
