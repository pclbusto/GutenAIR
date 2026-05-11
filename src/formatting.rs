//! Formateado semántico de fragmentos de texto o HTML.
//!
//! Proporciona [`GutenCore::apply_format`] para envolver contenido en una etiqueta
//! semántica o asignarle una clase CSS, y [`GutenCore::create_list`] para generar
//! listas `<ul>`/`<ol>` desde ítems individuales, produciendo fragmentos XHTML
//! seguros y listos para insertar en un capítulo.

use crate::core::GutenCore;
use crate::error::{GutenError, Result};

/// Tags inline permitidas en `TextFormat::Tag` y como `tag` en `TextFormat::Class`.
const INLINE_TAGS: &[&str] = &[
    "span", "strong", "em", "mark", "small", "sup", "sub", "code", "abbr",
];

/// Tags de bloque permitidas en `TextFormat::Tag` y como `tag` en `TextFormat::Class`.
const BLOCK_TAGS: &[&str] = &["p", "blockquote", "h1", "h2", "h3", "h4", "h5", "h6"];

/// Modo de interpretación del campo `input`.
#[derive(Debug, Clone, PartialEq)]
pub enum FormatInputMode {
    /// El input es texto plano: los caracteres `<`, `>` y `&` se escapan antes de envolver.
    PlainText,
    /// El input es un fragmento HTML: se sanitiza con ammonia antes de envolver.
    HtmlFragment,
}

/// Formato a aplicar al contenido.
#[derive(Debug, Clone, PartialEq)]
pub enum TextFormat {
    /// Envuelve el contenido en la etiqueta semántica indicada: `<tag>…</tag>`.
    Tag {
        /// Etiqueta HTML. Debe estar en [`INLINE_TAGS`] o [`BLOCK_TAGS`].
        tag: String,
    },
    /// Envuelve el contenido en `<tag class="class_name">…</tag>`.
    Class {
        /// Etiqueta contenedora. Si es `None`, se usa `"span"`.
        tag: Option<String>,
        /// Nombre de clase CSS. Se acepta con o sin `.` inicial (`.foo` → `foo`).
        /// Solo se permiten caracteres `[a-zA-Z0-9_-]`.
        class_name: String,
    },
}

/// Opciones para [`GutenCore::apply_format`].
#[derive(Debug, Clone)]
pub struct ApplyFormatOptions {
    /// Texto o fragmento HTML a formatear.
    pub input: String,
    /// Cómo interpretar `input`.
    pub mode: FormatInputMode,
    /// Formato a aplicar.
    pub format: TextFormat,
}

/// Tipo de lista HTML a generar.
#[derive(Debug, Clone, PartialEq)]
pub enum ListKind {
    /// Lista desordenada: `<ul>`.
    Unordered,
    /// Lista ordenada: `<ol>`.
    Ordered,
}

/// Modo de interpretación del `input` en [`GutenCore::create_list`].
#[derive(Debug, Clone, PartialEq)]
pub enum CreateListInputMode {
    /// Detección automática: si `input` (tras `trim_start`) empieza con `<p`,
    /// usa `HtmlParagraphs`; en caso contrario usa `Lines`.
    Auto,
    /// Cada línea no vacía del input se convierte en un ítem. El contenido se escapa.
    Lines,
    /// Cada elemento `<p>` se convierte en un ítem.
    /// El contenido interno (incluyendo tags inline) se preserva; los atributos del `<p>` se descartan.
    HtmlParagraphs,
}

/// Opciones para [`GutenCore::create_list`].
#[derive(Debug, Clone)]
pub struct CreateListOptions {
    /// Texto o HTML a convertir en lista.
    pub input: String,
    /// Tipo de lista (`<ul>` o `<ol>`).
    pub kind: ListKind,
    /// Cómo interpretar `input`. Por defecto usar [`CreateListInputMode::Auto`].
    pub mode: CreateListInputMode,
    /// Clase CSS opcional para el contenedor. Admite `.` inicial; solo `[a-zA-Z0-9_-]`.
    pub class_name: Option<String>,
}

impl GutenCore {
    /// Envuelve `input` en una etiqueta semántica o clase CSS y devuelve un fragmento XHTML seguro.
    ///
    /// # Comportamiento
    ///
    /// - En modo `PlainText` los caracteres especiales del input se escapan (`<` → `&lt;`, etc.).
    /// - En modo `HtmlFragment` el input se sanitiza con ammonia antes de envolver.
    /// - La etiqueta debe pertenecer a la lista de tags inline u de bloque permitidas.
    /// - Para `TextFormat::Class`, el nombre de clase se normaliza (se elimina `.` inicial)
    ///   y solo puede contener `[a-zA-Z0-9_-]`.
    /// - El resultado es un fragmento XHTML auto-cerrado correctamente, apto para insertar
    ///   dentro de un `<body>`.
    ///
    /// # Errores
    ///
    /// - [`GutenError::InvalidProject`] si la etiqueta no está permitida.
    /// - [`GutenError::InvalidProject`] si el nombre de clase contiene caracteres inválidos.
    pub fn apply_format(options: ApplyFormatOptions) -> Result<String> {
        let ApplyFormatOptions { input, mode, format } = options;

        let body = match mode {
            FormatInputMode::PlainText => escape_html(&input),
            FormatInputMode::HtmlFragment => ammonia::clean(&input),
        };

        match format {
            TextFormat::Tag { tag } => {
                let tag = tag.to_lowercase();
                validate_tag(&tag)?;
                Ok(format!("<{tag}>{body}</{tag}>"))
            }
            TextFormat::Class { tag, class_name } => {
                let tag = tag.unwrap_or_else(|| "span".to_string()).to_lowercase();
                validate_tag(&tag)?;
                let class_name = normalize_class(&class_name)?;
                Ok(format!("<{tag} class=\"{class_name}\">{body}</{tag}>"))
            }
        }
    }

    /// Genera una lista XHTML (`<ul>` o `<ol>`) a partir de ítems individuales.
    ///
    /// # Comportamiento
    ///
    /// - Cada ítem produce un `<li>…</li>`.
    /// - En modo `PlainText` el contenido de cada ítem se escapa.
    /// - En modo `HtmlFragment` cada ítem se sanitiza con ammonia antes de envolverlo.
    /// - El resultado pasa por `html5_to_xhtml_void_elements` para garantizar XHTML válido
    ///   (cierra elementos void como `<br>` → `<br/>`).
    /// - Si `class_name` está presente se aplica al contenedor (`<ul class="foo">`).
    ///
    /// # Errores
    ///
    /// - [`GutenError::InvalidProject`] si `items` está vacío.
    /// - [`GutenError::InvalidProject`] si algún ítem es vacío o solo whitespace.
    /// - [`GutenError::InvalidProject`] si `class_name` contiene caracteres inválidos.
    pub fn create_list(options: CreateListOptions) -> Result<String> {
        let CreateListOptions { input, kind, mode, class_name } = options;

        let trimmed = input.trim_start();

        let items = match mode {
            CreateListInputMode::Lines => extract_line_items(trimmed)?,
            CreateListInputMode::HtmlParagraphs => extract_html_paragraph_items(trimmed)?,
            CreateListInputMode::Auto => {
                if trimmed.starts_with("<p") {
                    extract_html_paragraph_items(trimmed)?
                } else {
                    extract_line_items(trimmed)?
                }
            }
        };

        let list_tag = match kind {
            ListKind::Unordered => "ul",
            ListKind::Ordered => "ol",
        };

        let open_tag = if let Some(ref cls) = class_name {
            let cls = normalize_class(cls)?;
            format!("<{list_tag} class=\"{cls}\">")
        } else {
            format!("<{list_tag}>")
        };

        let mut html = open_tag;
        html.push('\n');
        for item in &items {
            html.push_str(&format!("  <li>{item}</li>\n"));
        }
        html.push_str(&format!("</{list_tag}>"));

        Ok(crate::guardian::html5_to_xhtml_void_elements(&html))
    }
}

fn validate_tag(tag: &str) -> Result<()> {
    if INLINE_TAGS.contains(&tag) || BLOCK_TAGS.contains(&tag) {
        Ok(())
    } else {
        Err(GutenError::InvalidProject(format!(
            "tag '{}' is not allowed; permitted tags: {}",
            tag,
            INLINE_TAGS.iter().chain(BLOCK_TAGS.iter()).cloned().collect::<Vec<_>>().join(", ")
        )))
    }
}

fn normalize_class(class_name: &str) -> Result<String> {
    let name = class_name.strip_prefix('.').unwrap_or(class_name);
    if name.is_empty() {
        return Err(GutenError::InvalidProject("class name cannot be empty".into()));
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err(GutenError::InvalidProject(format!(
            "class name '{}' contains invalid characters; only [a-zA-Z0-9_-] are allowed",
            name
        )));
    }
    Ok(name.to_string())
}

fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Divide `input` por saltos de línea; escapa cada línea no vacía.
/// Error si no queda ninguna línea con contenido.
fn extract_line_items(input: &str) -> Result<Vec<String>> {
    let items: Vec<String> = input
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(escape_html)
        .collect();
    if items.is_empty() {
        return Err(GutenError::InvalidProject(
            "input contains no non-empty lines".into(),
        ));
    }
    Ok(items)
}

/// Extrae el contenido interno de cada `<p>` del fragmento HTML.
/// Los atributos del `<p>` se descartan; el contenido inline se sanitiza con ammonia.
/// Error si el fragmento no parsea como XML o no contiene ningún `<p>`.
fn extract_html_paragraph_items(input: &str) -> Result<Vec<String>> {
    let wrapped = format!("<div>{}</div>", input);
    let doc = roxmltree::Document::parse(&wrapped)
        .map_err(|e| GutenError::InvalidProject(format!("failed to parse HTML input: {}", e)))?;

    // Recopilar rangos de byte mientras el doc está en scope
    let ranges: Vec<std::ops::Range<usize>> = doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "p")
        .map(|n| n.range())
        .collect();

    if ranges.is_empty() {
        return Err(GutenError::InvalidProject(
            "no <p> elements found in HTML input".into(),
        ));
    }

    let items: Vec<String> = ranges
        .iter()
        .map(|range| {
            let src = &wrapped[range.clone()];
            // inner = lo que hay entre el cierre de la etiqueta de apertura y la etiqueta de cierre
            let open_end = src.find('>').map(|i| i + 1).unwrap_or(0);
            let close_start = src.rfind("</").unwrap_or(src.len());
            ammonia::clean(src[open_end..close_start].trim())
        })
        .filter(|s| !s.trim().is_empty())
        .collect();

    if items.is_empty() {
        return Err(GutenError::InvalidProject(
            "all <p> elements are empty".into(),
        ));
    }

    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formatting::{ApplyFormatOptions, FormatInputMode, TextFormat};

    fn apply(input: &str, mode: FormatInputMode, format: TextFormat) -> Result<String> {
        GutenCore::apply_format(ApplyFormatOptions {
            input: input.to_string(),
            mode,
            format,
        })
    }

    // --- Tag mode ---

    #[test]
    fn test_tag_inline_strong() {
        let result = apply("bold text", FormatInputMode::PlainText, TextFormat::Tag { tag: "strong".into() }).unwrap();
        assert_eq!(result, "<strong>bold text</strong>");
    }

    #[test]
    fn test_tag_block_p() {
        let result = apply("A paragraph.", FormatInputMode::PlainText, TextFormat::Tag { tag: "p".into() }).unwrap();
        assert_eq!(result, "<p>A paragraph.</p>");
    }

    #[test]
    fn test_tag_blockquote() {
        let result = apply("To be or not to be.", FormatInputMode::PlainText, TextFormat::Tag { tag: "blockquote".into() }).unwrap();
        assert_eq!(result, "<blockquote>To be or not to be.</blockquote>");
    }

    #[test]
    fn test_tag_mark() {
        let result = apply("highlighted", FormatInputMode::PlainText, TextFormat::Tag { tag: "mark".into() }).unwrap();
        assert_eq!(result, "<mark>highlighted</mark>");
    }

    #[test]
    fn test_tag_unknown_rejected() {
        let err = apply("text", FormatInputMode::PlainText, TextFormat::Tag { tag: "script".into() }).unwrap_err();
        assert!(err.to_string().contains("script"), "error should mention the bad tag");
    }

    #[test]
    fn test_tag_div_rejected() {
        let err = apply("text", FormatInputMode::PlainText, TextFormat::Tag { tag: "div".into() }).unwrap_err();
        assert!(err.to_string().contains("div"));
    }

    // --- Class mode ---

    #[test]
    fn test_class_span_default() {
        let result = apply(
            "personaje",
            FormatInputMode::PlainText,
            TextFormat::Class { tag: None, class_name: "personaje".into() },
        ).unwrap();
        assert_eq!(result, r#"<span class="personaje">personaje</span>"#);
    }

    #[test]
    fn test_class_dot_prefix_stripped() {
        let result = apply(
            "text",
            FormatInputMode::PlainText,
            TextFormat::Class { tag: None, class_name: ".mi-clase".into() },
        ).unwrap();
        assert_eq!(result, r#"<span class="mi-clase">text</span>"#);
    }

    #[test]
    fn test_class_custom_tag() {
        let result = apply(
            "quote",
            FormatInputMode::PlainText,
            TextFormat::Class { tag: Some("blockquote".into()), class_name: "epigraph".into() },
        ).unwrap();
        assert_eq!(result, r#"<blockquote class="epigraph">quote</blockquote>"#);
    }

    #[test]
    fn test_class_invalid_chars_rejected() {
        let err = apply(
            "text",
            FormatInputMode::PlainText,
            TextFormat::Class { tag: None, class_name: "my class".into() },
        ).unwrap_err();
        assert!(err.to_string().contains("invalid characters"));
    }

    #[test]
    fn test_class_empty_rejected() {
        let err = apply(
            "text",
            FormatInputMode::PlainText,
            TextFormat::Class { tag: None, class_name: ".".into() },
        ).unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn test_class_invalid_tag_rejected() {
        let err = apply(
            "text",
            FormatInputMode::PlainText,
            TextFormat::Class { tag: Some("table".into()), class_name: "foo".into() },
        ).unwrap_err();
        assert!(err.to_string().contains("table"));
    }

    // --- PlainText escaping ---

    #[test]
    fn test_plain_text_escaping() {
        let result = apply(
            "a < b & c > d",
            FormatInputMode::PlainText,
            TextFormat::Tag { tag: "span".into() },
        ).unwrap();
        assert_eq!(result, "<span>a &lt; b &amp; c &gt; d</span>");
    }

    #[test]
    fn test_plain_text_quotes_escaped() {
        let result = apply(
            r#"say "hello""#,
            FormatInputMode::PlainText,
            TextFormat::Tag { tag: "em".into() },
        ).unwrap();
        assert_eq!(result, "<em>say &quot;hello&quot;</em>");
    }

    // --- HtmlFragment mode ---

    #[test]
    fn test_html_fragment_passthrough_safe() {
        let result = apply(
            "<strong>bold</strong> and <em>italic</em>",
            FormatInputMode::HtmlFragment,
            TextFormat::Tag { tag: "p".into() },
        ).unwrap();
        assert!(result.contains("<strong>bold</strong>"));
        assert!(result.contains("<em>italic</em>"));
        assert!(result.starts_with("<p>") && result.ends_with("</p>"));
    }

    #[test]
    fn test_html_fragment_strips_script() {
        let result = apply(
            r#"hello <script>alert("xss")</script> world"#,
            FormatInputMode::HtmlFragment,
            TextFormat::Tag { tag: "span".into() },
        ).unwrap();
        assert!(!result.contains("<script"), "script tag must be removed by ammonia");
        assert!(result.contains("hello"));
    }

    // --- Case normalization ---

    #[test]
    fn test_tag_uppercase_normalized() {
        let result = apply(
            "text",
            FormatInputMode::PlainText,
            TextFormat::Tag { tag: "STRONG".into() },
        ).unwrap();
        assert_eq!(result, "<strong>text</strong>");
    }

    // ─── create_list ───────────────────────────────────────────────────────────

    fn make_list(input: &str, kind: ListKind, mode: CreateListInputMode, class: Option<&str>) -> Result<String> {
        GutenCore::create_list(CreateListOptions {
            input: input.to_string(),
            kind,
            mode,
            class_name: class.map(str::to_string),
        })
    }

    // Lines mode

    #[test]
    fn test_create_ul_lines_basic() {
        let result = make_list("Alpha\nBeta\nGamma", ListKind::Unordered, CreateListInputMode::Lines, None).unwrap();
        assert_eq!(result, "<ul>\n  <li>Alpha</li>\n  <li>Beta</li>\n  <li>Gamma</li>\n</ul>");
    }

    #[test]
    fn test_create_ol_lines_basic() {
        let result = make_list("One\nTwo", ListKind::Ordered, CreateListInputMode::Lines, None).unwrap();
        assert_eq!(result, "<ol>\n  <li>One</li>\n  <li>Two</li>\n</ol>");
    }

    #[test]
    fn test_create_list_single_item() {
        let result = make_list("Solo", ListKind::Ordered, CreateListInputMode::Lines, None).unwrap();
        assert_eq!(result, "<ol>\n  <li>Solo</li>\n</ol>");
    }

    #[test]
    fn test_create_list_lines_skips_blank_lines() {
        let result = make_list("One\n\nTwo\n   \nThree", ListKind::Unordered, CreateListInputMode::Lines, None).unwrap();
        assert_eq!(result, "<ul>\n  <li>One</li>\n  <li>Two</li>\n  <li>Three</li>\n</ul>");
    }

    #[test]
    fn test_create_list_lines_plain_text_escaping() {
        let result = make_list("a < b & c", ListKind::Unordered, CreateListInputMode::Lines, None).unwrap();
        assert!(result.contains("<li>a &lt; b &amp; c</li>"));
    }

    #[test]
    fn test_create_list_empty_input_rejected() {
        let err = make_list("", ListKind::Unordered, CreateListInputMode::Lines, None).unwrap_err();
        assert!(err.to_string().contains("no non-empty lines"));
    }

    #[test]
    fn test_create_list_all_whitespace_rejected() {
        let err = make_list("   \n   \n", ListKind::Unordered, CreateListInputMode::Lines, None).unwrap_err();
        assert!(err.to_string().contains("no non-empty lines"));
    }

    // HtmlParagraphs mode

    #[test]
    fn test_create_list_html_paragraphs_basic() {
        let result = make_list(
            "<p>Uno</p>\n<p>Dos</p>\n<p>Tres</p>",
            ListKind::Unordered,
            CreateListInputMode::HtmlParagraphs,
            None,
        ).unwrap();
        assert_eq!(result, "<ul>\n  <li>Uno</li>\n  <li>Dos</li>\n  <li>Tres</li>\n</ul>");
    }

    #[test]
    fn test_create_list_html_paragraphs_strips_p_attributes() {
        let result = make_list(
            r#"<p class="foo" id="x">Content</p>"#,
            ListKind::Unordered,
            CreateListInputMode::HtmlParagraphs,
            None,
        ).unwrap();
        assert!(result.contains("<li>Content</li>"), "inner text preserved");
        assert!(!result.contains("class=\"foo\""), "p attributes discarded");
    }

    #[test]
    fn test_create_list_html_paragraphs_preserves_inline_tags() {
        let result = make_list(
            "<p><strong>Bold</strong> and <em>italic</em></p>",
            ListKind::Unordered,
            CreateListInputMode::HtmlParagraphs,
            None,
        ).unwrap();
        assert!(result.contains("<strong>Bold</strong>"));
        assert!(result.contains("<em>italic</em>"));
    }

    #[test]
    fn test_create_list_html_paragraphs_strips_script() {
        let result = make_list(
            r#"<p><script>bad()</script>safe</p>"#,
            ListKind::Unordered,
            CreateListInputMode::HtmlParagraphs,
            None,
        ).unwrap();
        assert!(!result.contains("<script"), "script removed by ammonia");
        assert!(result.contains("safe"));
    }

    #[test]
    fn test_create_list_html_paragraphs_void_elements_xhtml() {
        let result = make_list(
            "<p>line one<br/>line two</p>",
            ListKind::Unordered,
            CreateListInputMode::HtmlParagraphs,
            None,
        ).unwrap();
        assert!(result.contains("<br"), "br present");
        assert!(!result.contains("<br>"), "br must be self-closed in XHTML");
    }

    #[test]
    fn test_create_list_html_paragraphs_no_p_rejected() {
        let err = make_list(
            "<div>no paragraphs here</div>",
            ListKind::Unordered,
            CreateListInputMode::HtmlParagraphs,
            None,
        ).unwrap_err();
        assert!(err.to_string().contains("no <p>"));
    }

    // Auto detection

    #[test]
    fn test_create_list_auto_detects_lines() {
        let lines = make_list("One\nTwo\nThree", ListKind::Unordered, CreateListInputMode::Auto, None).unwrap();
        let explicit = make_list("One\nTwo\nThree", ListKind::Unordered, CreateListInputMode::Lines, None).unwrap();
        assert_eq!(lines, explicit);
    }

    #[test]
    fn test_create_list_auto_detects_html_paragraphs() {
        let auto = make_list("<p>Uno</p><p>Dos</p>", ListKind::Unordered, CreateListInputMode::Auto, None).unwrap();
        let explicit = make_list("<p>Uno</p><p>Dos</p>", ListKind::Unordered, CreateListInputMode::HtmlParagraphs, None).unwrap();
        assert_eq!(auto, explicit);
    }

    #[test]
    fn test_create_list_auto_leading_whitespace_still_detects_html() {
        // trim_start before detection: "\n  <p>..." → starts with "<p"
        let result = make_list("\n  <p>Item</p>", ListKind::Unordered, CreateListInputMode::Auto, None).unwrap();
        assert!(result.contains("<li>Item</li>"));
    }

    // Class and shared validation

    #[test]
    fn test_create_list_with_class() {
        let result = make_list("Item", ListKind::Unordered, CreateListInputMode::Lines, Some("my-list")).unwrap();
        assert!(result.starts_with(r#"<ul class="my-list">"#));
    }

    #[test]
    fn test_create_list_class_dot_stripped() {
        let result = make_list("Item", ListKind::Ordered, CreateListInputMode::Lines, Some(".numbered")).unwrap();
        assert!(result.starts_with(r#"<ol class="numbered">"#));
    }

    #[test]
    fn test_create_list_invalid_class_rejected() {
        let err = make_list("item", ListKind::Unordered, CreateListInputMode::Lines, Some("bad class!")).unwrap_err();
        assert!(err.to_string().contains("invalid characters"));
    }
}
