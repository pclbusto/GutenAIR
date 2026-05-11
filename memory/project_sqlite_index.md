---
name: SQLite Structural Index
description: Implementación del índice SQLite FTS5 en src/index.rs — búsqueda, hooks, links huérfanos
type: project
---

Implementado el "Espejo Estructural SQLite" en `src/index.rs`.

**Archivo DB:** `workdir/.gutenair.db` (excluido del EPUB exportado).

**Tablas:**
- `virtual_content` (FTS5, tokenize unicode61): chapter_id, block_id, tag, content
- `hook_registry`: todos los `id` attributes del proyecto
- `link_registry`: links internos (no http/https) por capítulo

**Dependencia:** `rusqlite = { version = "0.31", features = ["bundled"] }` — SQLite embebido, FTS5 incluido.

**API pública en GutenCore:**
- `build_index()` — reconstruye desde disco; resetea `index_dirty`; llamado automáticamente en `open_folder`
- `search(query)` → `Vec<SearchResult>` — FTS5 con snippets `<mark>…</mark>`
- `get_links_to(target_chapter, hook_id)` → `Vec<String>` — referencias cruzadas precisas usando resolución de rutas igual a `validate_links`
- `validate_links()` → `Vec<(String, String)>` — links huérfanos con validación local y cross-file
- `index_dirty: bool` — indica que `save_chapter`/`add_document` no pudo actualizar el índice; la UI debe llamar `build_index()`

**Ciclo de vida:** open_folder → build_index (full rebuild). save_chapter → index_xhtml incremental.

**Why:** Habilitar búsqueda instantánea y navegación relacional del libro sin reescanear disco en cada query.

**How to apply:** La fase 3 pendiente es ID injection explícita en párrafos sin `id` attribute.
