---
name: GutenAIR Architecture
description: Arquitectura general de gutencore — lib Rust para gestión de EPUBs como workspaces
type: project
---

gutencore es una librería Rust (no binario) que gestiona libros EPUB como carpetas de trabajo descomprimidas.

Struct central: `GutenCore` en `src/core.rs`. Cada archivo src es un bloque `impl GutenCore` por área funcional.

Módulos: core, manifest, metadata, toc, hooks, cleaner, guardian, export, export_custom, error, types, stats, index.

Entry points: `GutenCore::new_project()` crea skeleton EPUB en disco; `GutenCore::open_folder()` carga uno existente.

**Why:** El core es el "cerebro" — toda la lógica vive aquí; cualquier UI futura debe ser thin y delegar al core.

**How to apply:** Si hay que agregar una feature con lógica, va en core, no en la UI hipotética.
