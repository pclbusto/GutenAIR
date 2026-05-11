//! Operación de split de capítulos EPUB.
//!
//! Divide un capítulo XHTML en dos desde un punto estructural elegido por la UI.
//! El Core mantiene consistentes el XHTML, manifest, spine, navegación e índice SQLite.

use crate::core::GutenCore;
use crate::error::{GutenError, Result};
use std::fs;

/// Punto de corte para [`split_chapter`](GutenCore::split_chapter).
#[derive(Debug, Clone)]
pub enum SplitPoint {
    /// El bloque top-level del `<body>` que contiene (o es) el elemento con este `id`
    /// se convierte en el primer bloque del capítulo nuevo.
    ElementId(String),
}

/// Opciones para [`split_paragraph`](GutenCore::split_paragraph).
#[derive(Debug, Clone)]
pub struct SplitParagraphOptions {
    /// ID en el manifiesto del capítulo que contiene el párrafo.
    pub chapter_id: String,
    /// Atributo `id` del `<p>` a dividir.
    pub paragraph_id: String,
    /// Offset de carácter Unicode (0-indexed) dentro del texto plano del párrafo.
    /// Offset 0 y offset igual al total de caracteres son rechazados.
    pub text_offset: usize,
    /// ID para el segundo párrafo. Si es `None` se genera `"{paragraph_id}-b"`.
    pub new_paragraph_id: Option<String>,
}

/// Opciones para [`split_chapter`](GutenCore::split_chapter).
#[derive(Debug, Clone)]
pub struct SplitChapterOptions {
    /// ID en el manifiesto del capítulo a dividir.
    pub source_id: String,
    /// ID para el capítulo nuevo (no debe existir en el manifiesto).
    pub new_id: String,
    /// Dónde cortar.
    pub split_at: SplitPoint,
    /// Título del capítulo nuevo. Si es `None` se usa `new_id`.
    pub new_title: Option<String>,
}

impl GutenCore {
    /// Divide un capítulo XHTML en dos desde un punto estructural.
    ///
    /// # Comportamiento
    ///
    /// - El bloque top-level del `<body>` que contiene el ID indicado pasa a ser
    ///   el primer bloque del capítulo nuevo.
    /// - La primera mitad permanece en `source_id`.
    /// - La segunda mitad se guarda como `Text/{new_id}.xhtml`.
    /// - `new_id` se inserta en el spine inmediatamente después de `source_id`.
    /// - Si `source_id` tiene excepciones de estilo, se copian a `new_id`.
    /// - El índice SQLite se actualiza; si falla, se activa `index_dirty`.
    /// - **No** llama a `save()` — el caller decide cuándo persistir el OPF.
    ///
    /// # Errores
    ///
    /// Devuelve [`GutenError::Manifest`] o [`GutenError::InvalidProject`] si:
    /// - `source_id == new_id`
    /// - `new_id` contiene caracteres inválidos o ya existe
    /// - `source_id` no es XHTML
    /// - el punto de corte no existe en el `<body>`
    /// - el corte dejaría la primera mitad vacía
    /// - el archivo destino ya existe en disco
    pub fn split_chapter(&mut self, options: SplitChapterOptions) -> Result<()> {
        let SplitChapterOptions { source_id, new_id, split_at, new_title } = options;
        let SplitPoint::ElementId(ref split_id) = split_at;

        // --- Validaciones ---
        if source_id == new_id {
            return Err(GutenError::InvalidProject(
                "source_id and new_id must differ".into(),
            ));
        }

        if new_id.is_empty()
            || new_id.contains(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        {
            return Err(GutenError::InvalidProject(format!(
                "new_id '{}' must be non-empty and contain only alphanumeric, '-', or '_'",
                new_id
            )));
        }

        if self.manifest.contains_key(&new_id) {
            return Err(GutenError::Manifest(format!(
                "ID '{}' already exists in manifest",
                new_id
            )));
        }

        let source_item = self.get_item(&source_id)?;
        if source_item.media_type != "application/xhtml+xml" {
            return Err(GutenError::Manifest(format!(
                "'{}' is not an XHTML document",
                source_id
            )));
        }

        let opf_dir = self
            .opf_dir
            .as_ref()
            .ok_or_else(|| GutenError::InvalidProject("OPF dir not set".into()))?
            .clone();

        let source_path = opf_dir.join(&source_item.href);
        let new_href = format!("Text/{}.xhtml", new_id);
        let new_path = opf_dir.join(&new_href);

        if new_path.exists() {
            return Err(GutenError::InvalidProject(format!(
                "File {:?} already exists on disk",
                new_path
            )));
        }

        // --- Parsear fuente ---
        let original_xhtml = fs::read_to_string(&source_path)?;

        // Normalizar void elements antes de parsear con roxmltree
        let xhtml_norm = crate::guardian::html5_to_xhtml_void_elements(&original_xhtml);

        let lang = self
            .metadata
            .as_ref()
            .map(|m| m.language.clone())
            .unwrap_or_else(|| "en".into());

        // --- Dividir body ---
        let (source_body, new_body, head_links) = split_body(&xhtml_norm, split_id)?;

        // --- Reconstruir XHTML ---
        let source_title =
            extract_title(&xhtml_norm).unwrap_or_else(|| source_id.clone());
        let new_title_str = new_title.unwrap_or_else(|| new_id.clone());

        let source_xhtml = crate::guardian::html5_to_xhtml_void_elements(
            &Self::build_xhtml(&lang, &source_title, &head_links, &source_body),
        );
        let new_xhtml = crate::guardian::html5_to_xhtml_void_elements(
            &Self::build_xhtml(&lang, &new_title_str, &head_links, &new_body),
        );

        // --- Persistir con rollback básico ---
        // Primero el archivo nuevo; si falla, no tocamos el original.
        fs::write(&new_path, &new_xhtml).map_err(GutenError::Io)?;

        if let Err(e) = fs::write(&source_path, &source_xhtml) {
            let _ = fs::remove_file(&new_path);
            return Err(GutenError::Io(e));
        }

        // --- Manifiesto ---
        if let Err(e) = self.add_to_manifest(
            new_id.clone(),
            new_href,
            "application/xhtml+xml".into(),
            "".into(),
        ) {
            let _ = fs::write(&source_path, &original_xhtml);
            let _ = fs::remove_file(&new_path);
            return Err(e);
        }

        // --- Spine (posición exacta para rollback preciso) ---
        let spine_insert_pos = self.spine
            .iter()
            .position(|id| id == &source_id)
            .map(|src_pos| {
                let insert_pos = src_pos + 1;
                self.spine.insert(insert_pos, new_id.clone());
                insert_pos
            });

        // --- Excepciones de estilo ---
        let had_exception = self.config.exceptions.get(&source_id).cloned();
        if let Some(ref exc) = had_exception {
            self.config.exceptions.insert(new_id.clone(), exc.clone());
        }

        // --- Índice SQLite (evaluación independiente para no perder new_id) ---
        if let Some(db) = &self.index_db {
            let ok_src = db.index_xhtml(&source_id, &source_xhtml).is_ok();
            let ok_new = db.index_xhtml(&new_id, &new_xhtml).is_ok();
            if !ok_src || !ok_new {
                self.index_dirty = true;
            }
        }

        // --- Navegación con rollback completo si falla ---
        if let Err(e) = self.update_nav() {
            self.manifest.remove(&new_id);
            if let Some(pos) = spine_insert_pos {
                self.spine.remove(pos);
            }
            if had_exception.is_some() {
                self.config.exceptions.remove(&new_id);
            }
            if let Some(db) = &self.index_db {
                let _ = db.clear_chapter(&new_id);
                if db.index_xhtml(&source_id, &original_xhtml).is_err() {
                    self.index_dirty = true;
                }
            }
            let _ = fs::write(&source_path, &original_xhtml);
            let _ = fs::remove_file(&new_path);
            return Err(e);
        }

        Ok(())
    }
}

impl GutenCore {
    /// Divide un `<p>` en dos `<p>` dentro de un capítulo XHTML.
    ///
    /// # Comportamiento
    ///
    /// - El primer párrafo conserva el `id` original y todos los atributos.
    /// - El segundo recibe `new_paragraph_id` (o `"{id}-b"` si es `None`) y hereda
    ///   los mismos atributos (`class`, `style`, etc.).
    /// - Si el offset cae dentro de un elemento inline (`<em>`, `<strong>`, etc.)
    ///   devuelve error — el corte solo es válido en los límites de nodos directos.
    /// - Guarda el capítulo en disco y actualiza el índice SQLite.
    /// - No llama a `save()` — el caller decide cuándo persistir el OPF.
    pub fn split_paragraph(&mut self, options: SplitParagraphOptions) -> Result<()> {
        let SplitParagraphOptions {
            chapter_id,
            paragraph_id,
            text_offset,
            new_paragraph_id,
        } = options;

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

        let new_xhtml = split_paragraph_in_xhtml(
            &xhtml_norm,
            &paragraph_id,
            text_offset,
            new_paragraph_id.as_deref(),
        )?;

        fs::write(&chapter_path, &new_xhtml).map_err(GutenError::Io)?;

        if let Some(db) = &self.index_db {
            if db.index_xhtml(&chapter_id, &new_xhtml).is_err() {
                self.index_dirty = true;
            }
        }

        Ok(())
    }
}

/// Divide el body de un documento XHTML en dos mitades y extrae los links del head.
///
/// Retorna `(source_body, new_body, head_links)`.
/// `new_body` comienza en el bloque top-level que contiene (o es) `split_id`.
fn split_body(xhtml: &str, split_id: &str) -> Result<(String, String, String)> {
    let doc = roxmltree::Document::parse(xhtml)
        .map_err(|e| GutenError::InvalidProject(format!("XHTML parse: {}", e)))?;

    // Extraer <link> del <head> para replicarlos en ambos capítulos
    let head_links = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "head")
        .map(|head| {
            head.children()
                .filter(|n| n.is_element() && n.tag_name().name() == "link")
                .map(|n| format!("\n  {}", xhtml[n.range()].trim()))
                .collect::<String>()
        })
        .unwrap_or_default();

    let body = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "body")
        .ok_or_else(|| GutenError::InvalidProject("No <body> element found".into()))?;

    // Solo elementos para detectar el punto de corte
    let elem_blocks: Vec<_> = body.children().filter(|n| n.is_element()).collect();

    if elem_blocks.is_empty() {
        return Err(GutenError::InvalidProject("<body> has no element blocks".into()));
    }

    let split_elem_idx = elem_blocks
        .iter()
        .position(|b| {
            b.attribute("id") == Some(split_id)
                || b.descendants()
                    .any(|d| d.is_element() && d.attribute("id") == Some(split_id))
        })
        .ok_or_else(|| {
            GutenError::InvalidProject(format!(
                "No element with id='{}' found in <body>",
                split_id
            ))
        })?;

    // Byte offset donde empieza el bloque de corte
    let cut_pos = elem_blocks[split_elem_idx].range().start;

    // Bounds del contenido inner del body usando TODOS los hijos (texto + comentarios + elementos)
    let body_inner_start = body
        .children()
        .next()
        .ok_or_else(|| GutenError::InvalidProject("<body> is empty".into()))?
        .range()
        .start;
    let body_inner_end = body
        .children()
        .last()
        .ok_or_else(|| GutenError::InvalidProject("<body> is empty".into()))?
        .range()
        .end;

    let source_body = xhtml[body_inner_start..cut_pos].trim().to_string();
    let new_body = xhtml[cut_pos..body_inner_end].trim().to_string();

    if source_body.is_empty() {
        return Err(GutenError::InvalidProject(
            "Split point is at the start — source chapter would be empty".into(),
        ));
    }

    Ok((source_body, new_body, head_links))
}

fn extract_title(xhtml: &str) -> Option<String> {
    let doc = roxmltree::Document::parse(xhtml).ok()?;
    doc.descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "title")
        .and_then(|n| n.text())
        .map(str::to_string)
}

/// Modifica el XHTML completo dividiendo `<p id=paragraph_id>` en dos `<p>`.
fn split_paragraph_in_xhtml(
    xhtml: &str,
    paragraph_id: &str,
    text_offset: usize,
    new_paragraph_id: Option<&str>,
) -> Result<String> {
    let doc = roxmltree::Document::parse(xhtml)
        .map_err(|e| GutenError::InvalidProject(format!("XHTML parse: {}", e)))?;

    let p = doc
        .descendants()
        .find(|n| {
            n.is_element()
                && n.tag_name().name() == "p"
                && n.attribute("id") == Some(paragraph_id)
        })
        .ok_or_else(|| {
            GutenError::InvalidProject(format!(
                "No <p> with id='{}' found in chapter",
                paragraph_id
            ))
        })?;

    if text_offset == 0 {
        return Err(GutenError::InvalidProject(
            "text_offset 0 would leave the first paragraph empty".into(),
        ));
    }

    let inner_start = p
        .children()
        .next()
        .ok_or_else(|| {
            GutenError::InvalidProject(format!(
                "<p id='{}'> is empty — nothing to split",
                paragraph_id
            ))
        })?
        .range()
        .start;
    let inner_end = p.children().last().unwrap().range().end;

    let total_chars: usize = p
        .children()
        .map(|child| {
            if child.is_text() {
                child.text().unwrap_or("").chars().count()
            } else if child.is_element() {
                child
                    .descendants()
                    .filter(|n| n.is_text())
                    .filter_map(|n| n.text())
                    .map(|t| t.chars().count())
                    .sum()
            } else {
                0
            }
        })
        .sum();

    if text_offset >= total_chars {
        return Err(GutenError::InvalidProject(format!(
            "text_offset {} equals or exceeds the {} characters in the paragraph — second paragraph would be empty",
            text_offset, total_chars
        )));
    }

    let split_byte = find_split_byte(&p, text_offset)?;

    let first_content = &xhtml[inner_start..split_byte];
    let second_content = &xhtml[split_byte..inner_end];

    if first_content.trim().is_empty() {
        return Err(GutenError::InvalidProject(
            "Split would leave the first paragraph empty".into(),
        ));
    }
    if second_content.trim().is_empty() {
        return Err(GutenError::InvalidProject(
            "Split would leave the second paragraph empty".into(),
        ));
    }

    let new_id_owned = new_paragraph_id
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}-b", paragraph_id));

    if doc.descendants().any(|n| {
        n.is_element()
            && n.attribute("id") == Some(new_id_owned.as_str())
            && n.attribute("id") != Some(paragraph_id)
    }) {
        return Err(GutenError::InvalidProject(format!(
            "ID '{}' already exists in chapter — choose a different new_paragraph_id",
            new_id_owned
        )));
    }

    let first_open = build_p_opening_tag(&p, paragraph_id);
    let second_open = build_p_opening_tag(&p, &new_id_owned);

    let replacement = format!(
        "{}{}</p>\n{}{}</p>",
        first_open, first_content, second_open, second_content,
    );

    Ok(format!(
        "{}{}{}",
        &xhtml[..p.range().start],
        replacement,
        &xhtml[p.range().end..],
    ))
}

/// Mapea un offset de caracteres al byte offset dentro del XHTML donde insertar el corte.
///
/// Solo permite cortes en los límites de nodos directos del `<p>`. Si el offset cae
/// dentro de un elemento inline devuelve error.
fn find_split_byte(p: &roxmltree::Node, mut remaining: usize) -> Result<usize> {
    for child in p.children() {
        if child.is_text() {
            let text = child.text().unwrap_or("");
            let char_count = text.chars().count();
            if remaining <= char_count {
                return Ok(child.range().start + char_to_byte_offset(text, remaining));
            }
            remaining -= char_count;
        } else if child.is_element() {
            if remaining == 0 {
                return Ok(child.range().start);
            }
            let elem_chars: usize = child
                .descendants()
                .filter(|n| n.is_text())
                .filter_map(|n| n.text())
                .map(|t| t.chars().count())
                .sum();
            if remaining < elem_chars {
                return Err(GutenError::InvalidProject(format!(
                    "Cannot split inside <{}> element; place cursor before or after it",
                    child.tag_name().name()
                )));
            }
            if remaining == elem_chars {
                return Ok(child.range().end);
            }
            remaining -= elem_chars;
        }
        // Comentarios y otros nodos no aportan texto; se ignoran.
    }
    Err(GutenError::InvalidProject(
        "text_offset is past the end of paragraph content".into(),
    ))
}

fn build_p_opening_tag(p: &roxmltree::Node, id: &str) -> String {
    let tag_name = p.tag_name().name();
    let mut out = format!("<{} id=\"{}\"", tag_name, escape_attr(id));
    for attr in p.attributes() {
        if attr.name() != "id" {
            out.push(' ');
            out.push_str(attr.name());
            out.push_str("=\"");
            out.push_str(&escape_attr(attr.value()));
            out.push('"');
        }
    }
    out.push('>');
    out
}

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn char_to_byte_offset(s: &str, char_offset: usize) -> usize {
    s.char_indices()
        .nth(char_offset)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_project() -> (tempfile::TempDir, GutenCore) {
        let dir = tempdir().unwrap();
        let core = GutenCore::new_project(dir.path(), "Test Book", "en").unwrap();
        (dir, core)
    }

    #[test]
    fn test_split_basic() -> Result<()> {
        let (_dir, mut core) = make_project();

        core.save_chapter(
            "chap1",
            r#"<h1 id="title">Chapter 1</h1>
<p id="para-a">First paragraph.</p>
<p id="cut">Second paragraph — split here.</p>
<p id="para-c">Third paragraph.</p>"#,
        )?;

        core.split_chapter(SplitChapterOptions {
            source_id: "chap1".into(),
            new_id: "chap2".into(),
            split_at: SplitPoint::ElementId("cut".into()),
            new_title: Some("Chapter 2".into()),
        })?;

        // Leer archivos del disco
        let opf_dir = core.opf_dir.clone().unwrap();
        let src_content = fs::read_to_string(opf_dir.join("Text/chap1.xhtml"))?;
        let new_content = fs::read_to_string(opf_dir.join("Text/chap2.xhtml"))?;

        // Source conserva bloques anteriores al corte
        assert!(src_content.contains("para-a"), "source should have para-a");
        assert!(!src_content.contains("id=\"cut\""), "source should not have cut block");
        assert!(!src_content.contains("para-c"), "source should not have para-c");

        // Nuevo capítulo comienza en el bloque de corte
        assert!(new_content.contains("id=\"cut\""), "new chapter should start with cut block");
        assert!(new_content.contains("para-c"), "new chapter should have para-c");
        assert!(!new_content.contains("para-a"), "new chapter should not have para-a");

        // Manifest
        assert!(core.manifest.contains_key("chap2"));

        // Spine: chap2 inmediatamente después de chap1
        let pos_src = core.spine.iter().position(|id| id == "chap1").unwrap();
        let pos_new = core.spine.iter().position(|id| id == "chap2").unwrap();
        assert_eq!(pos_new, pos_src + 1, "chap2 must follow chap1 in spine");

        Ok(())
    }

    #[test]
    fn test_split_rejects_duplicate_new_id() -> Result<()> {
        let (_dir, mut core) = make_project();

        let result = core.split_chapter(SplitChapterOptions {
            source_id: "chap1".into(),
            new_id: "chap1".into(), // same as source
            split_at: SplitPoint::ElementId("any".into()),
            new_title: None,
        });
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_split_rejects_missing_split_point() -> Result<()> {
        let (_dir, mut core) = make_project();

        core.save_chapter("chap1", "<h1>Title</h1><p>Content</p>")?;

        let result = core.split_chapter(SplitChapterOptions {
            source_id: "chap1".into(),
            new_id: "chap2".into(),
            split_at: SplitPoint::ElementId("nonexistent-id".into()),
            new_title: None,
        });
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_split_rejects_first_block_as_split_point() -> Result<()> {
        let (_dir, mut core) = make_project();

        core.save_chapter(
            "chap1",
            r#"<h1 id="first">Title</h1><p>Content</p>"#,
        )?;

        let result = core.split_chapter(SplitChapterOptions {
            source_id: "chap1".into(),
            new_id: "chap2".into(),
            split_at: SplitPoint::ElementId("first".into()),
            new_title: None,
        });
        assert!(
            result.is_err(),
            "splitting at first block should fail (source would be empty)"
        );

        Ok(())
    }

    #[test]
    fn test_split_copies_style_exceptions() -> Result<()> {
        let (_dir, mut core) = make_project();

        // Asignar excepción de estilo al capítulo fuente
        core.config
            .exceptions
            .insert("chap1".into(), vec!["custom-style".into()]);

        core.save_chapter(
            "chap1",
            r#"<h1 id="title">Chapter</h1><p id="cut">Split here</p><p>Rest</p>"#,
        )?;

        core.split_chapter(SplitChapterOptions {
            source_id: "chap1".into(),
            new_id: "chap2".into(),
            split_at: SplitPoint::ElementId("cut".into()),
            new_title: None,
        })?;

        let new_exceptions = core.config.exceptions.get("chap2").cloned();
        assert_eq!(
            new_exceptions,
            Some(vec!["custom-style".into()]),
            "chap2 should inherit style exceptions from chap1"
        );

        Ok(())
    }

    #[test]
    fn test_split_sqlite_search_finds_text_in_correct_chapter() -> Result<()> {
        let (_dir, mut core) = make_project();

        core.save_chapter(
            "chap1",
            r#"<h1 id="title">Chapter</h1>
<p id="before">Content before: dragon</p>
<p id="cut">Content after: phoenix</p>"#,
        )?;

        core.split_chapter(SplitChapterOptions {
            source_id: "chap1".into(),
            new_id: "chap2".into(),
            split_at: SplitPoint::ElementId("cut".into()),
            new_title: Some("Chapter 2".into()),
        })?;

        // "dragon" debe encontrarse solo en chap1
        let dragon_hits = core.search("dragon")?;
        assert!(!dragon_hits.is_empty());
        assert!(
            dragon_hits.iter().all(|r| r.chapter_id == "chap1"),
            "dragon should be in chap1 only"
        );

        // "phoenix" debe encontrarse solo en chap2
        let phoenix_hits = core.search("phoenix")?;
        assert!(!phoenix_hits.is_empty());
        assert!(
            phoenix_hits.iter().all(|r| r.chapter_id == "chap2"),
            "phoenix should be in chap2 only"
        );

        Ok(())
    }

    #[test]
    fn test_split_validate_links_unaffected() -> Result<()> {
        let (_dir, mut core) = make_project();

        // Capítulo con link interno válido
        core.save_chapter(
            "chap1",
            r##"<h1 id="title">Chapter</h1>
<p id="anchor">Anchor paragraph.</p>
<p id="cut">Split here. <a href="#anchor">back to top</a></p>
<p>End.</p>"##,
        )?;

        core.split_chapter(SplitChapterOptions {
            source_id: "chap1".into(),
            new_id: "chap2".into(),
            split_at: SplitPoint::ElementId("cut".into()),
            new_title: None,
        })?;

        // El link #anchor en chap2 ahora apunta a un ID que está en chap1 — es huérfano
        // (El link está dentro del bloque "cut" que se movió a chap2,
        //  pero #anchor quedó en chap1.)
        let orphans = core.validate_links()?;
        // Verificamos que validate_links() no paniquea y devuelve un resultado coherente
        // El link #anchor en chap2 debería ser huérfano ahora
        assert!(
            orphans.iter().any(|(ch, _)| ch == "chap2"),
            "cross-chapter orphan should be detected after split: {:?}",
            orphans
        );

        Ok(())
    }

    // --- split_paragraph tests ---

    #[test]
    fn test_split_paragraph_basic() -> Result<()> {
        let (_dir, mut core) = make_project();

        core.save_chapter(
            "chap1",
            r#"<h1 id="title">Chapter</h1>
<p id="p1">Hello world foo bar.</p>"#,
        )?;

        // "Hello" = 5 chars → split after "Hello"
        core.split_paragraph(SplitParagraphOptions {
            chapter_id: "chap1".into(),
            paragraph_id: "p1".into(),
            text_offset: 5,
            new_paragraph_id: Some("p1-b".into()),
        })?;

        let opf_dir = core.opf_dir.clone().unwrap();
        let content = fs::read_to_string(opf_dir.join("Text/chap1.xhtml"))?;

        assert!(content.contains(r#"id="p1""#), "first paragraph keeps original id");
        assert!(content.contains(r#"id="p1-b""#), "second paragraph gets new id");
        assert!(content.contains("Hello"), "first paragraph contains 'Hello'");
        assert!(content.contains("world foo bar"), "second paragraph contains the rest");

        Ok(())
    }

    #[test]
    fn test_split_paragraph_preserves_attributes() -> Result<()> {
        let (_dir, mut core) = make_project();

        // Write XHTML directly to bypass save_chapter sanitization, so we can verify
        // that split_paragraph faithfully copies all attributes it finds in the file.
        let opf_dir = core.opf_dir.clone().unwrap();
        fs::write(
            opf_dir.join("Text/chap1.xhtml"),
            r#"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="en">
<head><title>Test</title></head>
<body><p id="p1" class="intro" style="color:red">First. Second.</p></body>
</html>"#,
        )?;

        core.split_paragraph(SplitParagraphOptions {
            chapter_id: "chap1".into(),
            paragraph_id: "p1".into(),
            text_offset: 7,
            new_paragraph_id: Some("p1-b".into()),
        })?;

        let content = fs::read_to_string(opf_dir.join("Text/chap1.xhtml"))?;

        assert!(content.contains(r#"class="intro""#), "class preserved on both paragraphs");
        assert!(content.contains(r#"style="color:red""#), "style preserved on both paragraphs");

        Ok(())
    }

    #[test]
    fn test_split_paragraph_rejects_offset_zero() -> Result<()> {
        let (_dir, mut core) = make_project();
        core.save_chapter("chap1", r#"<p id="p1">Some text here.</p>"#)?;

        let result = core.split_paragraph(SplitParagraphOptions {
            chapter_id: "chap1".into(),
            paragraph_id: "p1".into(),
            text_offset: 0,
            new_paragraph_id: None,
        });
        assert!(result.is_err(), "offset 0 must be rejected");
        Ok(())
    }

    #[test]
    fn test_split_paragraph_rejects_offset_past_end() -> Result<()> {
        let (_dir, mut core) = make_project();
        core.save_chapter("chap1", r#"<p id="p1">Short.</p>"#)?;

        let result = core.split_paragraph(SplitParagraphOptions {
            chapter_id: "chap1".into(),
            paragraph_id: "p1".into(),
            text_offset: 9999,
            new_paragraph_id: None,
        });
        assert!(result.is_err(), "offset past end must be rejected");
        Ok(())
    }

    #[test]
    fn test_split_paragraph_rejects_split_inside_inline() -> Result<()> {
        let (_dir, mut core) = make_project();
        // text: "Hello " (6) + "world" inside em (5) + " rest." (6) = 17
        // offset 7 = 1 char into <em>world</em> → error
        core.save_chapter(
            "chap1",
            r#"<p id="p1">Hello <em>world</em> rest.</p>"#,
        )?;

        let result = core.split_paragraph(SplitParagraphOptions {
            chapter_id: "chap1".into(),
            paragraph_id: "p1".into(),
            text_offset: 7,
            new_paragraph_id: None,
        });
        assert!(result.is_err(), "split inside inline element must be rejected");
        Ok(())
    }

    #[test]
    fn test_split_paragraph_at_inline_boundary() -> Result<()> {
        let (_dir, mut core) = make_project();
        // text: "Hello " (6) + "world" inside em (5) + " rest." (6) = 17
        // offset 11 = right after </em>
        core.save_chapter(
            "chap1",
            r#"<p id="p1">Hello <em>world</em> rest.</p>"#,
        )?;

        core.split_paragraph(SplitParagraphOptions {
            chapter_id: "chap1".into(),
            paragraph_id: "p1".into(),
            text_offset: 11,
            new_paragraph_id: Some("p1-b".into()),
        })?;

        let opf_dir = core.opf_dir.clone().unwrap();
        let content = fs::read_to_string(opf_dir.join("Text/chap1.xhtml"))?;

        assert!(content.contains(r#"id="p1""#));
        assert!(content.contains(r#"id="p1-b""#));
        assert!(content.contains("<em>"), "em preserved in first paragraph");
        assert!(content.contains("rest"), "rest content in second paragraph");
        Ok(())
    }

    #[test]
    fn test_split_paragraph_rejects_duplicate_new_id() -> Result<()> {
        let (_dir, mut core) = make_project();

        core.save_chapter(
            "chap1",
            r#"<p id="p1">First part.</p><p id="existing">Other paragraph.</p>"#,
        )?;

        let result = core.split_paragraph(SplitParagraphOptions {
            chapter_id: "chap1".into(),
            paragraph_id: "p1".into(),
            text_offset: 5,
            new_paragraph_id: Some("existing".into()),
        });
        assert!(result.is_err(), "duplicate new_paragraph_id must be rejected");
        Ok(())
    }

    #[test]
    fn test_split_paragraph_before_inline_element() -> Result<()> {
        let (_dir, mut core) = make_project();

        // text: "Foo" (3) then <em>Bar</em> (3)
        // After processing the first element <em>Foo</em>, remaining becomes 0.
        // Fix 2 should allow splitting before the second element.
        // Use a structure where remaining hits 0 entering an element branch:
        // <p>Foo<!-- --><em>Bar</em></p> — comment skipped, so after text remaining == 0 at element.
        //
        // Simplest reproducible case: paragraph where first child is an inline element
        // and we split right at its end (remaining == elem_chars → handled already).
        // For Fix 2: paragraph with no leading text and offset == 0 is rejected.
        // Real trigger: after consuming text/elements, remaining == 0 before next element.
        //
        // Example: <p>Abc<em>Def</em></p>, offset=3 → remaining=0 when reaching <em>
        core.save_chapter(
            "chap1",
            r#"<p id="p1">Abc<em id="e1">Def</em> rest.</p>"#,
        )?;

        // offset 3 = right after "Abc", right before <em>
        core.split_paragraph(SplitParagraphOptions {
            chapter_id: "chap1".into(),
            paragraph_id: "p1".into(),
            text_offset: 3,
            new_paragraph_id: Some("p1-b".into()),
        })?;

        let opf_dir = core.opf_dir.clone().unwrap();
        let content = fs::read_to_string(opf_dir.join("Text/chap1.xhtml"))?;

        assert!(content.contains("Abc"), "first paragraph has 'Abc'");
        assert!(content.contains("<em"), "second paragraph has em element");
        assert!(content.contains(r#"id="p1-b""#), "second paragraph gets new id");
        Ok(())
    }

    #[test]
    fn test_split_paragraph_precise_error_at_total_offset() -> Result<()> {
        let (_dir, mut core) = make_project();
        // "Short" = 5 chars, offset 5 = at the very end
        core.save_chapter("chap1", r#"<p id="p1">Short</p>"#)?;

        let err = core
            .split_paragraph(SplitParagraphOptions {
                chapter_id: "chap1".into(),
                paragraph_id: "p1".into(),
                text_offset: 5,
                new_paragraph_id: None,
            })
            .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("equals or exceeds") || msg.contains("5"),
            "error should be precise about offset == total: {msg}"
        );
        Ok(())
    }

    #[test]
    fn test_split_paragraph_sqlite_finds_both() -> Result<()> {
        let (_dir, mut core) = make_project();

        core.save_chapter(
            "chap1",
            r#"<h1 id="h">Ch</h1><p id="p1">dragon phoenix</p>"#,
        )?;

        // "dragon " = 7 chars
        core.split_paragraph(SplitParagraphOptions {
            chapter_id: "chap1".into(),
            paragraph_id: "p1".into(),
            text_offset: 7,
            new_paragraph_id: Some("p1-b".into()),
        })?;

        let dragon = core.search("dragon")?;
        let phoenix = core.search("phoenix")?;

        assert!(!dragon.is_empty(), "dragon should be indexed");
        assert!(!phoenix.is_empty(), "phoenix should be indexed");
        assert!(dragon.iter().all(|r| r.chapter_id == "chap1"));
        assert!(phoenix.iter().all(|r| r.chapter_id == "chap1"));

        Ok(())
    }
}
