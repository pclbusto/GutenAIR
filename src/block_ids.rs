//! Asignación e inferencia de IDs estructurales a bloques XHTML.
//!
//! Proporciona:
//! - [`GutenCore::suggest_tag_id`] — devuelve el próximo ID libre para un tag dado
//!   sin modificar el archivo (útil para que la UI sugiera un valor al usuario).
//! - [`GutenCore::ensure_block_ids`] — materializa IDs en todos los bloques que
//!   carezcan de ellos y escribe el resultado en disco.
//!
//! Convención de IDs generados: `{tag}-{n:03}` (ej. `p-001`, `h2-001`, `section-001`).
//! Los IDs existentes de cualquier formato no se modifican.

use crate::core::GutenCore;
use crate::error::{GutenError, Result};
use std::collections::{HashMap, HashSet};
use std::fs;

/// Información sobre un ID asignado por [`GutenCore::ensure_block_ids`].
#[derive(Debug, Clone)]
pub struct AssignedBlockId {
    /// Nombre de la etiqueta HTML (`"p"`, `"h2"`, etc.).
    pub tag: String,
    /// ID generado (ej. `"p-001"`, `"h2-003"`).
    pub id: String,
    /// Primeros 60 caracteres del texto visible del bloque (whitespace normalizado).
    pub text_preview: String,
}

/// Opciones para [`GutenCore::ensure_block_ids`].
#[derive(Debug, Clone)]
pub struct EnsureBlockIdsOptions {
    /// ID en el manifiesto del capítulo a procesar.
    pub chapter_id: String,
    /// Etiquetas a las que asignar IDs si no los tienen.
    /// Si está vacío se usan las etiquetas por defecto: `p`, `h1`–`h6`, `section`, `figure`.
    pub tags: Vec<String>,
}

impl EnsureBlockIdsOptions {
    /// Etiquetas por defecto: bloques de corte más comunes.
    pub fn default_tags() -> Vec<String> {
        ["p", "h1", "h2", "h3", "h4", "h5", "h6", "section", "figure"]
            .iter()
            .map(|&s| s.to_string())
            .collect()
    }
}

impl GutenCore {
    /// Devuelve el próximo ID libre para `tag` en `chapter_id`, sin modificar ningún archivo.
    ///
    /// El ID tiene la forma `{tag}-{n:03}` (ej. `p-001`, `h2-003`). El contador arranca
    /// en 1 y avanza hasta encontrar un valor que no exista como atributo `id` en el XHTML.
    /// Cualquier ID existente — independientemente del formato — bloquea su número si coincide.
    ///
    /// # Errores
    ///
    /// - [`GutenError::Manifest`] si `chapter_id` no existe o no es XHTML.
    /// - [`GutenError::InvalidProject`] si el XHTML no puede parsearse.
    pub fn suggest_tag_id(&self, chapter_id: &str, tag: &str) -> Result<String> {
        let item = self.get_item(chapter_id)?;
        if item.media_type != "application/xhtml+xml" {
            return Err(GutenError::Manifest(format!(
                "'{}' is not an XHTML document",
                chapter_id
            )));
        }

        let opf_dir = self
            .opf_dir
            .as_ref()
            .ok_or_else(|| GutenError::InvalidProject("OPF dir not set".into()))?;
        let chapter_path = opf_dir.join(&item.href);

        let xhtml = fs::read_to_string(&chapter_path)?;
        let xhtml_norm = crate::guardian::html5_to_xhtml_void_elements(&xhtml);
        let existing = collect_existing_ids(&xhtml_norm)?;
        next_available_id(tag, &existing)
    }

    /// Asigna IDs estructurales a los bloques del capítulo que no los tienen.
    ///
    /// # Comportamiento
    ///
    /// - Solo modifica elementos que carezcan de atributo `id`.
    /// - Los IDs existentes (de cualquier formato) no se tocan.
    /// - Los IDs generados usan la forma `{tag}-{n:03}` (ej. `p-001`, `h2-003`).
    ///   Si ese valor ya existe en el capítulo, el contador avanza hasta encontrar uno libre.
    /// - Escribe el XHTML modificado en disco y actualiza el índice SQLite.
    /// - Devuelve solo los IDs recién asignados, en orden de documento.
    /// - Si todos los bloques ya tienen ID, devuelve `Vec` vacío sin escritura en disco.
    ///
    /// # Errores
    ///
    /// - [`GutenError::Manifest`] si `chapter_id` no existe o no es XHTML.
    /// - [`GutenError::InvalidProject`] si el XHTML no puede parsearse.
    pub fn ensure_block_ids(
        &mut self,
        options: EnsureBlockIdsOptions,
    ) -> Result<Vec<AssignedBlockId>> {
        let EnsureBlockIdsOptions {
            chapter_id,
            mut tags,
        } = options;

        if tags.is_empty() {
            tags = EnsureBlockIdsOptions::default_tags();
        }

        let item = self.get_item(&chapter_id)?;
        if item.media_type != "application/xhtml+xml" {
            return Err(GutenError::Manifest(format!(
                "'{}' is not an XHTML document",
                chapter_id
            )));
        }

        let opf_dir = self
            .opf_dir
            .as_ref()
            .ok_or_else(|| GutenError::InvalidProject("OPF dir not set".into()))?
            .clone();
        let chapter_path = opf_dir.join(&item.href);

        let xhtml = fs::read_to_string(&chapter_path)?;
        let xhtml_norm = crate::guardian::html5_to_xhtml_void_elements(&xhtml);

        let (new_xhtml, assigned) = assign_block_ids(&xhtml_norm, &tags)?;

        if assigned.is_empty() {
            return Ok(assigned);
        }

        fs::write(&chapter_path, &new_xhtml).map_err(GutenError::Io)?;

        if let Some(db) = &self.index_db {
            if db.index_xhtml(&chapter_id, &new_xhtml).is_err() {
                self.index_dirty = true;
            }
        }

        Ok(assigned)
    }
}

/// Inserta `id="{tag}-{n:03}"` en los elementos objetivo sin ID.
/// Devuelve el XHTML modificado y la lista de asignaciones en orden de documento.
fn assign_block_ids(xhtml: &str, tags: &[String]) -> Result<(String, Vec<AssignedBlockId>)> {
    let doc = roxmltree::Document::parse(xhtml)
        .map_err(|e| GutenError::InvalidProject(format!("XHTML parse: {}", e)))?;

    let mut existing_ids: HashSet<String> = doc
        .descendants()
        .filter(|n| n.is_element())
        .filter_map(|n| n.attribute("id"))
        .map(str::to_string)
        .collect();

    let tag_set: HashSet<&str> = tags.iter().map(|s| s.as_str()).collect();
    let mut counters: HashMap<String, u32> = HashMap::new();

    // (insert_byte_pos, new_id, tag_name, text_preview) — en orden ascendente de documento
    let mut insertions: Vec<(usize, String, String, String)> = Vec::new();

    for node in doc.descendants() {
        if !node.is_element() {
            continue;
        }
        let tag = node.tag_name().name();
        if !tag_set.contains(tag) || node.attribute("id").is_some() {
            continue;
        }

        let insert_pos = insert_pos_after_tag_name(xhtml, node.range().start);

        let counter = counters.entry(tag.to_string()).or_insert(0);
        let new_id = loop {
            *counter += 1;
            let candidate = format!("{}-{:03}", tag, counter);
            if !existing_ids.contains(&candidate) {
                break candidate;
            }
        };
        existing_ids.insert(new_id.clone());

        let preview: String = node
            .descendants()
            .filter(|n| n.is_text())
            .filter_map(|n| n.text())
            .flat_map(|t| t.split_whitespace())
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(60)
            .collect();

        insertions.push((insert_pos, new_id, tag.to_string(), preview));
    }

    if insertions.is_empty() {
        return Ok((xhtml.to_string(), Vec::new()));
    }

    // Aplicar de atrás hacia adelante para no invalidar posiciones anteriores
    let mut result = xhtml.to_string();
    let mut assigned: Vec<AssignedBlockId> = Vec::with_capacity(insertions.len());

    for (pos, id, tag, preview) in insertions.iter().rev() {
        result.insert_str(*pos, &format!(" id=\"{}\"", id));
        assigned.push(AssignedBlockId {
            tag: tag.clone(),
            id: id.clone(),
            text_preview: preview.clone(),
        });
    }

    assigned.reverse(); // restaurar orden de documento
    Ok((result, assigned))
}

/// Recoge todos los `id` attributes del documento como un conjunto.
fn collect_existing_ids(xhtml: &str) -> Result<HashSet<String>> {
    let doc = roxmltree::Document::parse(xhtml)
        .map_err(|e| GutenError::InvalidProject(format!("XHTML parse: {}", e)))?;
    Ok(doc
        .descendants()
        .filter(|n| n.is_element())
        .filter_map(|n| n.attribute("id"))
        .map(str::to_string)
        .collect())
}

/// Devuelve el primer ID de la forma `{tag}-{n:03}` que no esté en `existing`.
fn next_available_id(tag: &str, existing: &HashSet<String>) -> Result<String> {
    for n in 1u32.. {
        let candidate = format!("{}-{:03}", tag, n);
        if !existing.contains(&candidate) {
            return Ok(candidate);
        }
    }
    unreachable!("ID counter exhausted")
}

/// Byte offset justo después del nombre de la etiqueta en la etiqueta de apertura.
/// Ejemplo: `<p class="x">` → offset apunta al espacio antes de `class`.
fn insert_pos_after_tag_name(xhtml: &str, element_start: usize) -> usize {
    // element_start apunta a '<'; el nombre termina en el primer whitespace, '>' o '/'
    let after_lt = &xhtml[element_start + 1..];
    let name_len = after_lt
        .find(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
        .unwrap_or(after_lt.len());
    element_start + 1 + name_len
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::GutenCore;
    use tempfile::tempdir;

    fn make_project() -> (tempfile::TempDir, GutenCore) {
        let dir = tempdir().unwrap();
        let core = GutenCore::new_project(dir.path(), "Test", "en").unwrap();
        (dir, core)
    }

    #[test]
    fn test_ensure_ids_assigns_to_untagged() -> Result<()> {
        let (_dir, mut core) = make_project();

        core.save_chapter(
            "chap1",
            "<h1>Title</h1><p>First paragraph.</p><p>Second paragraph.</p>",
        )?;

        let assigned = core.ensure_block_ids(EnsureBlockIdsOptions {
            chapter_id: "chap1".into(),
            tags: vec!["p".into(), "h1".into()],
        })?;

        assert_eq!(assigned.len(), 3, "h1 + 2 p should get IDs");
        assert!(assigned.iter().any(|a| a.id.starts_with("h1-")));
        assert!(assigned.iter().any(|a| a.id.starts_with("p-")));

        let opf_dir = core.opf_dir.clone().unwrap();
        let content = fs::read_to_string(opf_dir.join("Text/chap1.xhtml"))?;
        assert!(content.contains(r#"id="p-"#) || content.contains(r#"id="h1-"#),
            "written XHTML must contain generated IDs");

        Ok(())
    }

    #[test]
    fn test_ensure_ids_skips_existing() -> Result<()> {
        let (_dir, mut core) = make_project();

        let opf_dir = core.opf_dir.clone().unwrap();
        fs::write(
            opf_dir.join("Text/chap1.xhtml"),
            r#"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="en">
<head><title>T</title></head>
<body>
<p id="existing">Already has an ID.</p>
<p>No ID here.</p>
</body>
</html>"#,
        )?;

        let assigned = core.ensure_block_ids(EnsureBlockIdsOptions {
            chapter_id: "chap1".into(),
            tags: vec!["p".into()],
        })?;

        assert_eq!(assigned.len(), 1, "only the ID-less paragraph should be processed");
        assert_ne!(assigned[0].id, "existing");

        let content = fs::read_to_string(opf_dir.join("Text/chap1.xhtml"))?;
        assert!(content.contains(r#"id="existing""#), "pre-existing ID must be untouched");

        Ok(())
    }

    #[test]
    fn test_ensure_ids_skips_collision() -> Result<()> {
        let (_dir, mut core) = make_project();

        let opf_dir = core.opf_dir.clone().unwrap();
        fs::write(
            opf_dir.join("Text/chap1.xhtml"),
            r#"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="en">
<head><title>T</title></head>
<body>
<p id="p-001">Pre-existing ID.</p>
<p>Needs a new ID.</p>
</body>
</html>"#,
        )?;

        let assigned = core.ensure_block_ids(EnsureBlockIdsOptions {
            chapter_id: "chap1".into(),
            tags: vec!["p".into()],
        })?;

        assert_eq!(assigned.len(), 1);
        assert_eq!(assigned[0].id, "p-002", "should skip p-001 and use p-002");

        Ok(())
    }

    #[test]
    fn test_ensure_ids_returns_empty_when_all_tagged() -> Result<()> {
        let (_dir, mut core) = make_project();

        let opf_dir = core.opf_dir.clone().unwrap();
        fs::write(
            opf_dir.join("Text/chap1.xhtml"),
            r#"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="en">
<head><title>T</title></head>
<body>
<p id="p1">Has ID.</p>
<p id="p2">Also has ID.</p>
</body>
</html>"#,
        )?;

        let assigned = core.ensure_block_ids(EnsureBlockIdsOptions {
            chapter_id: "chap1".into(),
            tags: vec!["p".into()],
        })?;

        assert!(assigned.is_empty(), "nothing to assign, should return empty vec");

        Ok(())
    }

    #[test]
    fn test_ensure_ids_text_preview() -> Result<()> {
        let (_dir, mut core) = make_project();

        core.save_chapter("chap1", "<p>Hello world from GutenAIR.</p>")?;

        let assigned = core.ensure_block_ids(EnsureBlockIdsOptions {
            chapter_id: "chap1".into(),
            tags: vec!["p".into()],
        })?;

        assert_eq!(assigned.len(), 1);
        assert!(
            assigned[0].text_preview.contains("Hello"),
            "preview should contain paragraph text: {:?}",
            assigned[0].text_preview
        );

        Ok(())
    }

    #[test]
    fn test_ensure_ids_sqlite_updated() -> Result<()> {
        let (_dir, mut core) = make_project();

        core.save_chapter("chap1", "<p>Dragon flies high.</p>")?;

        let assigned = core.ensure_block_ids(EnsureBlockIdsOptions {
            chapter_id: "chap1".into(),
            tags: vec!["p".into()],
        })?;

        assert_eq!(assigned.len(), 1);

        // El ID asignado debe aparecer como block_id en los resultados de búsqueda
        let results = core.search("dragon")?;
        assert!(!results.is_empty(), "dragon should be indexed after ensure_block_ids");
        assert!(
            results.iter().any(|r| r.block_id == assigned[0].id),
            "search result block_id should match assigned ID {:?}: {:?}",
            assigned[0].id,
            results
        );

        Ok(())
    }

    #[test]
    fn test_ensure_ids_default_tags_when_empty() -> Result<()> {
        let (_dir, mut core) = make_project();

        core.save_chapter("chap1", "<h2>A heading</h2><p>A paragraph.</p>")?;

        let assigned = core.ensure_block_ids(EnsureBlockIdsOptions {
            chapter_id: "chap1".into(),
            tags: vec![], // triggers default_tags
        })?;

        // default_tags includes p and h2, so both should get IDs
        assert!(assigned.len() >= 2, "default tags should catch h2 and p");

        Ok(())
    }

    // ─── suggest_tag_id ────────────────────────────────────────────────────────

    #[test]
    fn test_suggest_tag_id_empty_chapter() -> Result<()> {
        let (_dir, mut core) = make_project();
        core.save_chapter("chap1", "<p>Hello.</p>")?;

        // No IDs exist yet — first suggestion must be p-001
        let id = core.suggest_tag_id("chap1", "p")?;
        assert_eq!(id, "p-001");

        Ok(())
    }

    #[test]
    fn test_suggest_tag_id_skips_existing() -> Result<()> {
        let (_dir, core) = make_project();

        let opf_dir = core.opf_dir.clone().unwrap();
        fs::write(
            opf_dir.join("Text/chap1.xhtml"),
            r#"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="en">
<head><title>T</title></head>
<body>
<p id="p-001">First.</p>
<p id="p-002">Second.</p>
<p>Third.</p>
</body>
</html>"#,
        )?;

        let id = core.suggest_tag_id("chap1", "p")?;
        assert_eq!(id, "p-003");

        Ok(())
    }

    #[test]
    fn test_suggest_tag_id_independent_per_tag() -> Result<()> {
        let (_dir, core) = make_project();

        let opf_dir = core.opf_dir.clone().unwrap();
        fs::write(
            opf_dir.join("Text/chap1.xhtml"),
            r#"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="en">
<head><title>T</title></head>
<body>
<h2 id="h2-001">Heading.</h2>
<p>Paragraph.</p>
</body>
</html>"#,
        )?;

        // h2 counter is at 1, so next is h2-002
        assert_eq!(core.suggest_tag_id("chap1", "h2")?, "h2-002");
        // p counter is at 0, so next is p-001
        assert_eq!(core.suggest_tag_id("chap1", "p")?, "p-001");

        Ok(())
    }

    #[test]
    fn test_suggest_tag_id_skips_legacy_gair_ids() -> Result<()> {
        // gair-p-0001 must NOT block p-001 — different format, no collision
        let (_dir, core) = make_project();

        let opf_dir = core.opf_dir.clone().unwrap();
        fs::write(
            opf_dir.join("Text/chap1.xhtml"),
            r#"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="en">
<head><title>T</title></head>
<body>
<p id="gair-p-0001">Legacy ID.</p>
<p>No ID.</p>
</body>
</html>"#,
        )?;

        let id = core.suggest_tag_id("chap1", "p")?;
        assert_eq!(id, "p-001", "legacy gair-* IDs must not block the new format");

        Ok(())
    }

    #[test]
    fn test_suggest_tag_id_does_not_modify_file() -> Result<()> {
        let (_dir, mut core) = make_project();
        core.save_chapter("chap1", "<p>Content.</p>")?;

        let opf_dir = core.opf_dir.clone().unwrap();
        let before = fs::read_to_string(opf_dir.join("Text/chap1.xhtml"))?;

        let _ = core.suggest_tag_id("chap1", "p")?;

        let after = fs::read_to_string(opf_dir.join("Text/chap1.xhtml"))?;
        assert_eq!(before, after, "suggest_tag_id must not write to disk");

        Ok(())
    }

    #[test]
    fn test_suggest_tag_id_consistent_with_ensure_block_ids() -> Result<()> {
        // suggest_tag_id should predict the same ID that ensure_block_ids will assign
        let (_dir, mut core) = make_project();
        core.save_chapter("chap1", "<p>One.</p><p>Two.</p>")?;

        let suggested_first = core.suggest_tag_id("chap1", "p")?;

        let assigned = core.ensure_block_ids(EnsureBlockIdsOptions {
            chapter_id: "chap1".into(),
            tags: vec!["p".into()],
        })?;

        assert_eq!(
            assigned[0].id, suggested_first,
            "first assigned ID must match what suggest_tag_id predicted"
        );

        Ok(())
    }
}
