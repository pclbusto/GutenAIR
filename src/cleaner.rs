use crate::core::GutenCore;

impl GutenCore {
        /// Limpia y sanitiza contenido HTML para prevenir vulnerabilidades XSS
    ///
    /// Este método utiliza la biblioteca [`ammonia`](https://crates.io/crates/ammonia)
    /// para eliminar scripts maliciosos y atributos peligrosos del HTML,
    /// manteniendo solo las etiquetas y atributos seguros.
    ///
    /// # ¿Qué elimina?
    ///
    /// - Etiquetas `<script>`, `<iframe>`, `<object>`, etc.
    /// - Atributos como `onclick`, `onload`, `onerror`
    /// - URLs peligrosas (`javascript:`, `data:`)
    /// - Comentarios HTML y CDATA no seguros
    ///
    /// # ¿Qué preserva?
    ///
    /// Por defecto, `ammonia` preserva etiquetas seguras como:
    /// - `<p>`, `<div>`, `<span>`, `<h1>`-`<h6>`
    /// - `<a>`, `<img>`, `<ul>`, `<ol>`, `<li>`
    /// - `<strong>`, `<em>`, `<br>`, `<hr>`
    /// - Atributos seguros como `href`, `src`, `alt`, `class`, `id`
    ///
    /// # Argumentos
    ///
    /// * `html` - Cadena HTML que puede contener contenido peligroso
    ///
    /// # Retorna
    ///
    /// * `String` - HTML sanitizado y seguro para incluir en un EPUB
    ///
    /// # Ejemplo
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let core = GutenCore::new("./proyecto");
    ///
    /// let html_peligroso = r#"
    ///     <p>Texto seguro</p>
    ///     <script>alert('XSS');</script>
    ///     <img src="x" onerror="alert('malicioso')">
    /// "#;
    ///
    /// let limpio = core.clean_html(html_peligroso);
    /// 
    /// // El script y el onerror se eliminan, pero el <p> se conserva
    /// assert!(!limpio.contains("<script>"));
    /// assert!(!limpio.contains("onerror"));
    /// assert!(limpio.contains("<p>Texto seguro</p>"));
    /// ```
    ///
    /// # Casos de uso típicos
    ///
    /// 1. **Sanitizar contenido generado por usuarios** antes de incluirlo en un EPUB
    /// 2. **Limpiar HTML importado** de fuentes no confiables
    /// 3. **Prevenir inyección de código** al convertir formatos externos a EPUB
    ///
    /// # Notas de seguridad
    ///
    /// - **No confíes solo en esto**: Aunque `ammonia` es muy seguro,
    ///   siempre valida que el resultado sea el esperado.
    /// - **Rendimiento**: Para textos muy largos (>1MB), considera ejecutarlo
    ///   en un hilo separado o con chunks.
    /// - **Configuración personalizada**: Si necesitas reglas diferentes,
    ///   considera crear tu propia instancia de `ammonia::Builder`.
    ///
    /// # Ver también
    ///
    /// - [`text_to_xhtml`](Self::text_to_xhtml) - Convierte texto plano a XHTML
    /// - [Ammonia documentation](https://docs.rs/ammonia) - Para configuraciones avanzadas
    /// - [OWASP XSS Prevention Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Cross_Site_Scripting_Prevention_Cheat_Sheet.html)
    pub fn clean_html(&self, html: &str) -> String {
        ammonia::clean(html)
    }

        /// Convierte texto plano a un documento XHTML con párrafos
    ///
    /// Este método toma texto plano (como el contenido de un libro sin formato)
    /// y lo convierte en un documento XHTML válido para incluir en un EPUB.
    /// Los párrafos se detectan mediante dobles saltos de línea (`\n\n`).
    ///
    /// # Reglas de conversión
    ///
    /// | Entrada | Salida |
    /// |---------|--------|
    /// | `"Texto\n\nOtro párrafo"` | `<p>Texto</p><p>Otro párrafo</p>` |
    /// | Saltos de línea simples (`\n`) | `<br/>` dentro del párrafo |
    /// | Espacios al inicio/final | Se eliminan con `trim()` |
    /// | Líneas vacías | Se ignoran (no crean párrafos vacíos) |
    ///
    /// # Argumentos
    ///
    /// * `text` - Texto plano a convertir (puede contener múltiples líneas y párrafos)
    /// * `title` - Título que se usará en la etiqueta `<title>` del documento
    ///
    /// # Retorna
    ///
    /// * `String` - Documento XHTML completo con declaración XML, DOCTYPE y estructura HTML5
    ///
    /// # Ejemplo básico
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let core = GutenCore::new("./proyecto");
    ///
    /// let texto = "Este es el primer párrafo.\nTiene dos líneas.\n\nEste es el segundo párrafo.";
    /// let xhtml = core.text_to_xhtml(texto, "Capítulo 1");
    ///
    /// println!("{}", xhtml);
    /// // Resultado:
    /// // <?xml version="1.0" encoding="UTF-8"?>
    /// // <!DOCTYPE html>
    /// // <html xmlns="http://www.w3.org/1999/xhtml">
    /// // <head><title>Capítulo 1</title></head>
    /// // <body>
    /// // <p>Este es el primer párrafo.<br/>Tiene dos líneas.</p>
    /// // <p>Este es el segundo párrafo.</p>
    /// // </body>
    /// // </html>
    /// ```
    ///
    /// # Ejemplo con formato complejo
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let core = GutenCore::new("./proyecto");
    ///
    /// let poema = r#"Roses are red,
    /// Violets are blue.
    ///
    /// Sugar is sweet,
    /// And so are you."#;
    ///
    /// let xhtml = core.text_to_xhtml(poema, "Un Poema");
    /// 
    /// // Cada línea dentro del párrafo tiene <br/> excepto la última
    /// assert!(xhtml.contains("Roses are red,<br/>Violets are blue."));
    /// assert!(xhtml.contains("Sugar is sweet,<br/>And so are you."));
    /// ```
    ///
    /// # Ejemplo de integración con EPUB
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut core = GutenCore::new_project("./mi_libro", "Mi Libro", "es")?;
    ///
    /// // Texto del capítulo
    /// let texto_capitulo = "Había una vez...\n\nY colorín colorado.";
    /// let xhtml = core.text_to_xhtml(texto_capitulo, "Capítulo 1");
    ///
    /// // Guardar el archivo XHTML
    /// let opf_dir = core.opf_dir.as_ref().unwrap();
    /// let ruta_capitulo = opf_dir.join("Text/capitulo1.xhtml");
    /// std::fs::write(ruta_capitulo, xhtml)?;
    ///
    /// // Agregar al manifiesto y spine
    /// # use gutencore::ManifestItem;
    /// core.manifest.insert("cap1".to_string(), ManifestItem {
    ///     id: "cap1".to_string(),
    ///     href: "Text/capitulo1.xhtml".to_string(),
    ///     media_type: "application/xhtml+xml".to_string(),
    ///     properties: String::new(),
    /// });
    /// core.spine.push("cap1".to_string());
    ///
    /// core.save()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Limitaciones conocidas
    ///
    /// - **No soporta listas o tablas**: Solo detecta párrafos. Para estructuras
    ///   más complejas, necesitarás usar `clean_html` con HTML pre-formateado.
    /// - **No preserva sangrías**: Los espacios al inicio de línea se eliminan
    ///   con `trim()`.
    /// - **Encoding fijo**: Siempre genera UTF-8 (estándar de EPUB).
    /// - **Sin detección de encabezados**: Las líneas que parecen títulos no
    ///   se convierten automáticamente a `<h1>`-`<h6>`.
    ///
    /// # Notas de implementación
    ///
    /// - **Separador de párrafos**: Se usa `\n\n` (doble salto de línea).
    ///   Los párrafos separados por un solo `\n` se mantienen dentro del mismo `<p>`.
    /// - **Reemplazo de saltos**: Los `\n` dentro del párrafo se convierten a `<br/>`
    /// - **Escape automático**: El texto se inserta directamente; no se aplica
    ///   escape HTML adicional. Si el texto contiene `<` o `>`, se interpretarán
    ///   como HTML. Para texto con caracteres especiales, usa `clean_html` primero.
    ///
    /// # Advertencia de seguridad
    ///
    /// Este método **no sanitiza** el texto de entrada. Si el texto proviene
    /// de fuentes no confiables, **debes llamar a `clean_html` primero**:
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let core = GutenCore::new("./proyecto");
    /// let texto_peligroso = "Texto <script>alert('xss')</script> normal";
    /// let sanitizado = core.clean_html(texto_peligroso);
    /// let xhtml = core.text_to_xhtml(&sanitizado, "Título");
    /// ```
    ///
    /// # Ver también
    ///
    /// - [`clean_html`](Self::clean_html) - Para sanitizar HTML antes de convertirlo
    /// - [`ammonia` crate](https://crates.io/crates/ammonia) - Motor de sanitización subyacente
    /// - [EPUB XHTML Content Documents](https://www.w3.org/TR/epub/#sec-xhtml) - Especificación oficial
    pub fn text_to_xhtml(&self, text: &str, title: &str) -> String {
        let paragraphs: Vec<String> = text
            .split("\n\n")
            .map(|p| format!("<p>{}</p>", p.trim().replace("\n", "<br/>")))
            .collect();

        format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>{}</title></head>
<body>
{}
</body>
</html>"#, title, paragraphs.join("\n"))
    }

    /// Extrae texto plano de HTML o XHTML.
    ///
    /// - Elimina todas las etiquetas.
    /// - Decodifica entidades HTML (`&amp;` → `&`, `&#160;` → espacio, etc.).
    /// - Convierte bloques (`<p>`, headings, `<div>`, …) en párrafos separados por `\n\n`.
    /// - Convierte `<br>` en `\n`.
    /// - Convierte `<li>` en `- item\n`.
    /// - Ignora el contenido de `<script>`, `<style>` y `<head>`.
    /// - Normaliza espacios múltiples en la misma línea.
    /// - Colapsa más de una línea en blanco consecutiva.
    ///
    /// Si el input no es XML/XHTML válido aplica un strip de etiquetas de emergencia
    /// carácter a carácter (sin regex).
    ///
    /// # Ejemplo
    ///
    /// ```rust
    /// # use gutencore::GutenCore;
    /// let html = r#"<p>Hola <strong>mundo</strong>.</p>
    /// <p>Segundo&#160;párrafo<br/>otra línea.</p>"#;
    /// let text = GutenCore::extract_text(html);
    /// assert!(text.contains("Hola mundo."));
    /// assert!(text.contains("Segundo"));
    /// ```
    pub fn extract_text(html: &str) -> String {
        extract_text_impl(html)
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

const BLOCK_TAGS: &[&str] = &[
    "p", "h1", "h2", "h3", "h4", "h5", "h6",
    "div", "blockquote", "section", "article",
    "figure", "figcaption", "pre",
    "ul", "ol", "dl", "dt", "dd",
    "table", "thead", "tbody", "tr",
    "address", "aside", "footer", "header", "main", "nav",
];

const SKIP_TAGS: &[&str] = &["script", "style", "head"];

fn extract_text_impl(html: &str) -> String {
    // 1. Try parsing as-is (full XHTML document)
    if let Ok(doc) = roxmltree::Document::parse(html) {
        return extract_from_xml_doc(&doc);
    }
    // 2. Wrap in <div> and retry (HTML fragment with multiple roots)
    let wrapped = format!("<div>{}</div>", html);
    if let Ok(doc) = roxmltree::Document::parse(&wrapped) {
        return extract_from_xml_doc(&doc);
    }
    // 3. Fallback: strip tags character-by-character (broken HTML syntax)
    let stripped = strip_tags_naive(html);
    normalize_text(&stripped)
}

fn extract_from_xml_doc(doc: &roxmltree::Document) -> String {
    let mut buf = String::new();
    let start = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "body")
        .unwrap_or_else(|| doc.root_element());
    for child in start.children() {
        walk_extract(&child, &mut buf);
    }
    normalize_text(&buf)
}

fn walk_extract(node: &roxmltree::Node, buf: &mut String) {
    if node.is_text() {
        if let Some(t) = node.text() {
            buf.push_str(t);
        }
        return;
    }
    if !node.is_element() {
        return;
    }

    let tag = node.tag_name().name();

    if SKIP_TAGS.contains(&tag) {
        return;
    }

    if tag == "br" {
        buf.push('\n');
        return;
    }

    let is_block = BLOCK_TAGS.contains(&tag);
    if is_block && !buf.is_empty() {
        ensure_paragraph_break(buf);
    }

    if tag == "li" {
        ensure_newline(buf);
        buf.push_str("- ");
    }

    for child in node.children() {
        walk_extract(&child, buf);
    }

    if is_block {
        ensure_paragraph_break(buf);
    }
}

/// Asegura que `buf` termine en `\n\n` sin duplicar separadores.
fn ensure_paragraph_break(buf: &mut String) {
    if buf.ends_with("\n\n") {
        // already ok
    } else if buf.ends_with('\n') {
        buf.push('\n');
    } else {
        buf.push_str("\n\n");
    }
}

/// Asegura que `buf` termine en al menos un `\n`.
fn ensure_newline(buf: &mut String) {
    if !buf.ends_with('\n') {
        buf.push('\n');
    }
}

fn normalize_text(text: &str) -> String {
    // 1. Non-breaking spaces → regular space
    let text = text.replace('\u{00A0}', " ");

    // 2. Normalize each line: collapse tabs/spaces, trim trailing space
    let lines: Vec<String> = text
        .split('\n')
        .map(|line| {
            let mut out = String::new();
            let mut prev_space = false;
            for c in line.chars() {
                if c == ' ' || c == '\t' {
                    if !prev_space && !out.is_empty() {
                        out.push(' ');
                    }
                    prev_space = true;
                } else {
                    out.push(c);
                    prev_space = false;
                }
            }
            out.trim_end().to_string()
        })
        .collect();

    // 3. Collapse consecutive blank lines to at most one
    let mut result = String::new();
    let mut blank_run = 0usize;
    for line in &lines {
        if line.is_empty() {
            blank_run += 1;
            if blank_run == 1 {
                result.push('\n');
            }
        } else {
            blank_run = 0;
            result.push_str(line);
            result.push('\n');
        }
    }

    result.trim().to_string()
}

/// Strip de emergencia carácter a carácter sin regex. Decodifica las entidades
/// HTML más comunes para que el texto sea legible aunque el XML no sea válido.
fn strip_tags_naive(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }
    // Decode common entities in the stripped text
    result
        .replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::GutenCore;

    #[test]
    fn test_extract_text_paragraphs() {
        let html = r#"<p>Hola <strong>mundo</strong>.</p>
<p>Segundo&#160;párrafo<br/>otra línea.</p>"#;
        let text = GutenCore::extract_text(html);
        assert!(text.contains("Hola mundo."), "inline tags stripped: {text}");
        assert!(text.contains("Segundo párrafo"), "nbsp decoded: {text}");
        assert!(text.contains("otra línea"), "br converted: {text}");
        // paragraphs separated by blank line
        assert!(text.contains(".\n\n"), "paragraph break present: {text}");
    }

    #[test]
    fn test_extract_text_headings() {
        let html = "<h1>Título</h1><p>Contenido del capítulo.</p>";
        let text = GutenCore::extract_text(html);
        assert!(text.contains("Título"), "h1 text preserved");
        assert!(text.contains("Contenido"), "p text preserved");
    }

    #[test]
    fn test_extract_text_skips_script_and_style() {
        let html = r#"<p>Visible.</p>
<script>alert('xss');</script>
<style>.x { color: red; }</style>"#;
        let text = GutenCore::extract_text(html);
        assert!(text.contains("Visible"), "regular text kept");
        assert!(!text.contains("alert"), "script content removed");
        assert!(!text.contains("color"), "style content removed");
    }

    #[test]
    fn test_extract_text_list_items() {
        let html = "<ul><li>Primero</li><li>Segundo</li><li>Tercero</li></ul>";
        let text = GutenCore::extract_text(html);
        assert!(text.contains("- Primero"), "li bullet: {text}");
        assert!(text.contains("- Segundo"), "li bullet: {text}");
    }

    #[test]
    fn test_extract_text_collapses_whitespace() {
        let html = "<p>Texto  con    espacios   extras.</p>";
        let text = GutenCore::extract_text(html);
        assert!(!text.contains("  "), "multiple spaces collapsed: {text}");
    }

    #[test]
    fn test_extract_text_fallback_on_broken_html() {
        // Not valid XML — should still return readable text
        let html = "<p>Texto sin cerrar<br>siguiente línea</p>";
        let text = GutenCore::extract_text(html);
        assert!(text.contains("Texto"), "fallback should recover text: {text}");
    }

    #[test]
    fn test_extract_text_full_xhtml_document() {
        let xhtml = r#"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="es">
<head><title>Test</title><style>.x{}</style></head>
<body>
<h1>Capítulo 1</h1>
<p>Primer párrafo con <em>énfasis</em>.</p>
<p>Segundo párrafo.</p>
</body>
</html>"#;
        let text = GutenCore::extract_text(xhtml);
        assert!(text.contains("Capítulo 1"), "heading present");
        assert!(text.contains("Primer párrafo con énfasis"), "inline stripped");
        assert!(text.contains("Segundo párrafo"), "second p present");
        assert!(!text.contains("style"), "head/style excluded");
        assert!(!text.contains("<"), "no HTML tags in output");
    }
}
