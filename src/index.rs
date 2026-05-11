//! Índice estructural SQLite del proyecto EPUB.
//!
//! Mantiene un espejo en SQLite del contenido del libro para habilitar
//! búsqueda de texto completo (FTS5), registro de hooks y detección de
//! links huérfanos. El archivo `.gutenair.db` se crea en el `workdir`
//! y se excluye del EPUB exportado.

use crate::error::{GutenError, Result};
use rusqlite::{params, Connection};
use std::fmt;
use std::path::Path;

/// Wrapper sobre `rusqlite::Connection` con `Debug` manual.
pub struct IndexDb(Connection);

impl fmt::Debug for IndexDb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IndexDb").finish_non_exhaustive()
    }
}

/// Un resultado de búsqueda devuelto por [`IndexDb::search`].
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// ID del item en el manifiesto (ej. `"chap1"`).
    pub chapter_id: String,
    /// Atributo `id` del bloque, o `"block-N"` si no tenía ID.
    pub block_id: String,
    /// Nombre de la etiqueta HTML (`"p"`, `"h2"`, etc.).
    pub tag: String,
    /// Fragmento de texto con la coincidencia envuelta en `<mark>...</mark>`.
    pub snippet: String,
}

impl IndexDb {
    /// Nombre del archivo de base de datos dentro del `workdir`.
    pub const FILE_NAME: &'static str = ".gutenair.db";

    /// Abre (o crea) la base de datos en `workdir/.gutenair.db` e inicializa el esquema.
    pub fn open_or_create(workdir: &Path) -> Result<Self> {
        let db_path = workdir.join(Self::FILE_NAME);
        let conn = Connection::open(&db_path)
            .map_err(|e| GutenError::Other(format!("SQLite open: {}", e)))?;
        let db = Self(conn);
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.0
            .execute_batch(
                r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous  = NORMAL;

            CREATE VIRTUAL TABLE IF NOT EXISTS virtual_content USING fts5(
                chapter_id UNINDEXED,
                block_id   UNINDEXED,
                tag        UNINDEXED,
                content,
                tokenize   = 'unicode61 remove_diacritics 1'
            );

            CREATE TABLE IF NOT EXISTS hook_registry (
                chapter_id TEXT NOT NULL,
                hook_id    TEXT NOT NULL,
                tag_name   TEXT NOT NULL,
                PRIMARY KEY (chapter_id, hook_id)
            );

            CREATE TABLE IF NOT EXISTS link_registry (
                from_chapter TEXT NOT NULL,
                href         TEXT NOT NULL,
                PRIMARY KEY  (from_chapter, href)
            );
        "#,
            )
            .map_err(|e| GutenError::Other(format!("SQLite schema: {}", e)))
    }

    /// Elimina todas las filas de las tres tablas. Usado al inicio de `build_index`
    /// para evitar entradas fantasma de capítulos borrados del manifiesto.
    pub(crate) fn clear_all(&self) -> Result<()> {
        self.0
            .execute_batch(
                "DELETE FROM virtual_content; \
                 DELETE FROM hook_registry; \
                 DELETE FROM link_registry;",
            )
            .map_err(|e| GutenError::Other(format!("SQLite clear_all: {}", e)))
    }

    /// Elimina todas las filas asociadas a un capítulo de las tres tablas.
    pub(crate) fn clear_chapter(&self, chapter_id: &str) -> Result<()> {
        for sql in &[
            "DELETE FROM virtual_content WHERE chapter_id = ?1",
            "DELETE FROM hook_registry    WHERE chapter_id = ?1",
            "DELETE FROM link_registry    WHERE from_chapter = ?1",
        ] {
            self.0
                .execute(sql, params![chapter_id])
                .map_err(|e| GutenError::Other(e.to_string()))?;
        }
        Ok(())
    }

    /// Indexa el contenido XHTML de un capítulo.
    ///
    /// Reemplaza atómicamente cualquier dato previo del mismo `chapter_id`.
    /// Los elementos de texto sin atributo `id` reciben un bloque posicional
    /// `block-N` para que la búsqueda pueda referenciarlos.
    pub fn index_xhtml(&self, chapter_id: &str, xhtml: &str) -> Result<()> {
        self.clear_chapter(chapter_id)?;

        let doc = match roxmltree::Document::parse(xhtml) {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };

        const TEXT_TAGS: &[&str] = &[
            "p", "h1", "h2", "h3", "h4", "h5", "h6",
            "li", "blockquote", "td", "dt", "dd",
        ];

        let mut ins_content = self
            .0
            .prepare_cached(
                "INSERT INTO virtual_content(chapter_id, block_id, tag, content) \
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .map_err(|e| GutenError::Other(e.to_string()))?;

        let mut ins_hook = self
            .0
            .prepare_cached(
                "INSERT OR IGNORE INTO hook_registry(chapter_id, hook_id, tag_name) \
                 VALUES (?1, ?2, ?3)",
            )
            .map_err(|e| GutenError::Other(e.to_string()))?;

        let mut ins_link = self
            .0
            .prepare_cached(
                "INSERT OR IGNORE INTO link_registry(from_chapter, href) \
                 VALUES (?1, ?2)",
            )
            .map_err(|e| GutenError::Other(e.to_string()))?;

        let mut block_n = 0usize;

        for node in doc.descendants().filter(|n| n.is_element()) {
            let tag = node.tag_name().name();

            // Registrar hooks (elementos con id)
            if let Some(id_val) = node.attribute("id") {
                ins_hook
                    .execute(params![chapter_id, id_val, tag])
                    .map_err(|e| GutenError::Other(e.to_string()))?;
            }

            // Registrar links internos
            if tag == "a" {
                if let Some(href) = node.attribute("href") {
                    let is_external = href.starts_with("http://")
                        || href.starts_with("https://")
                        || href.starts_with("mailto:");
                    if !is_external {
                        ins_link
                            .execute(params![chapter_id, href])
                            .map_err(|e| GutenError::Other(e.to_string()))?;
                    }
                }
            }

            // Indexar contenido de texto
            if TEXT_TAGS.contains(&tag) {
                let text: String = node
                    .descendants()
                    .filter(|n| n.is_text())
                    .filter_map(|n| n.text())
                    .collect::<Vec<_>>()
                    .join(" ");

                let text = text.trim().to_string();
                if text.is_empty() {
                    continue;
                }

                let block_id = if let Some(id_val) = node.attribute("id") {
                    id_val.to_string()
                } else {
                    let id = format!("block-{}", block_n);
                    block_n += 1;
                    id
                };

                ins_content
                    .execute(params![chapter_id, block_id, tag, text])
                    .map_err(|e| GutenError::Other(e.to_string()))?;
            }
        }

        Ok(())
    }

    /// Busca `query` usando FTS5 y devuelve hasta 50 resultados con snippet.
    ///
    /// El snippet envuelve la coincidencia en `<mark>…</mark>`.
    pub fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let mut stmt = self
            .0
            .prepare(
                "SELECT chapter_id, block_id, tag, \
                        snippet(virtual_content, 3, '<mark>', '</mark>', '…', 20) \
                 FROM virtual_content \
                 WHERE content MATCH ?1 \
                 ORDER BY rank \
                 LIMIT 50",
            )
            .map_err(|e| GutenError::Other(e.to_string()))?;

        let results = stmt
            .query_map(params![query], |row| {
                Ok(SearchResult {
                    chapter_id: row.get(0)?,
                    block_id: row.get(1)?,
                    tag: row.get(2)?,
                    snippet: row.get(3)?,
                })
            })
            .map_err(|e| GutenError::Other(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    /// Devuelve todos los links internos registrados como `(from_chapter, href)`.
    ///
    /// Usado por `GutenCore::validate_links()` para la validación con acceso al manifiesto.
    pub(crate) fn get_all_links(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .0
            .prepare("SELECT from_chapter, href FROM link_registry")
            .map_err(|e| GutenError::Other(e.to_string()))?;

        let results = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| GutenError::Other(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    /// Comprueba si un hook `hook_id` existe en el capítulo `chapter_id`.
    pub(crate) fn hook_exists(&self, chapter_id: &str, hook_id: &str) -> Result<bool> {
        let count: i64 = self
            .0
            .query_row(
                "SELECT COUNT(*) FROM hook_registry \
                 WHERE chapter_id = ?1 AND hook_id = ?2",
                params![chapter_id, hook_id],
                |row| row.get(0),
            )
            .map_err(|e| GutenError::Other(e.to_string()))?;

        Ok(count > 0)
    }
}
