use crate::core::GutenCore;
use crate::error::{GutenError, Result};
use std::fs;
use html5ever::tendril::TendrilSink;

impl GutenCore {
    /// Save chapter content with guaranteed XHTML validity
    pub fn save_chapter(&mut self, id: &str, raw_content: &str) -> Result<()> {
        let item = self.get_item(id)?;
        if item.media_type != "application/xhtml+xml" {
            return Err(GutenError::Manifest(format!("{} is not an XHTML document", id)));
        }

        let clean_xhtml = self.sanitize_to_xhtml(raw_content)?;
        let full_path = self.get_resource_path(id)?;

        fs::write(full_path, clean_xhtml)?;
        Ok(())
    }

    /// Clean HTML and produce a full, strictly valid XHTML document.
    ///
    /// - Strips dangerous tags/JS (ammonia)
    /// - Closes orphan tags via html5ever parse+serialize
    /// - Injects XHTML namespace and lang from BookMetadata
    /// - Injects <link> tags for every CSS in the manifest
    pub fn sanitize_to_xhtml(&self, html: &str) -> Result<String> {
        // 1. Strip dangerous tags/JS
        let cleaned = ammonia::clean(html);

        // 2. Parse to fix malformed structure (auto-closes orphan tags, etc.)
        let dom = html5ever::parse_document(
            markup5ever_rcdom::RcDom::default(),
            Default::default(),
        )
        .one(cleaned);

        // 3. Serialize back to a string and convert void elements to XHTML self-closing
        let mut bytes = Vec::new();
        let serializable: markup5ever_rcdom::SerializableHandle = dom.document.clone().into();
        html5ever::serialize(&mut bytes, &serializable, Default::default())?;
        let body_fragment = html5_to_xhtml_void_elements(&String::from_utf8_lossy(&bytes));

        // 4. Extract just the <body> content so we can wrap it in our own template
        let body_content = extract_body(&body_fragment);

        // 5. Collect CSS hrefs from manifest (relative to OEBPS root, so we prefix "../")
        let css_links: Vec<String> = self
            .manifest
            .values()
            .filter(|it| it.media_type == "text/css")
            .map(|it| {
                // nav and chapters are inside Text/, so CSS in Styles/ needs "../Styles/..."
                // We use the stored href (relative to OPF dir) and prepend "../"
                format!(
                    r#"  <link rel="stylesheet" type="text/css" href="../{}"/>"#,
                    it.href
                )
            })
            .collect();

        // 6. Get lang from metadata
        let lang = self
            .metadata
            .as_ref()
            .map(|m| m.language.as_str())
            .unwrap_or("en");

        // 7. Assemble a well-formed XHTML document
        let head_links = if css_links.is_empty() {
            String::new()
        } else {
            format!("\n{}", css_links.join("\n"))
        };

        let result = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" lang="{lang}" xml:lang="{lang}">
<head>
  <meta charset="utf-8"/>{head_links}
</head>
<body>
{body_content}
</body>
</html>"#
        );

        Ok(result)
    }
}

/// Convert HTML5 void elements to XHTML self-closing form.
/// html5ever serializes <br>, <img>, <input>, etc. without closing slash,
/// which breaks XML parsers like roxmltree.
fn html5_to_xhtml_void_elements(html: &str) -> String {
    const VOID: &[&str] = &[
        "area", "base", "br", "col", "embed", "hr", "img", "input",
        "link", "meta", "param", "source", "track", "wbr",
    ];

    let mut result = html.to_string();
    for tag in VOID {
        // Replace <tag> with <tag/> and <tag ...attrs> with <tag ...attrs/>
        // We handle the two cases: self-contained (<br>) and with attributes (<img src="...">)
        let open = format!("<{}>", tag);
        let close_self = format!("<{}/>", tag);
        result = result.replace(&open, &close_self);

        // For tags with attributes: replace `<tag ...>` (not already ending in `/>`) with `<tag .../>`
        let prefix = format!("<{} ", tag);
        let mut out = String::with_capacity(result.len());
        let mut rest = result.as_str();
        while let Some(pos) = rest.find(&prefix) {
            out.push_str(&rest[..pos]);
            rest = &rest[pos..];
            if let Some(end) = rest.find('>') {
                let tag_str = &rest[..end + 1];
                if tag_str.ends_with("/>") {
                    out.push_str(tag_str);
                } else {
                    out.push_str(&rest[..end]);
                    out.push_str("/>");
                }
                rest = &rest[end + 1..];
            } else {
                break;
            }
        }
        out.push_str(rest);
        result = out;
    }
    result
}

/// Extract the content inside <body>...</body>, or return the whole string if not found.
fn extract_body(html: &str) -> String {
    let start = html.find("<body>").or_else(|| html.find("<body "));
    let end = html.rfind("</body>");

    match (start, end) {
        (Some(s), Some(e)) => {
            // skip past the closing ">" of the opening <body> tag
            let after_tag = &html[s..];
            let tag_end = after_tag.find('>').map(|i| s + i + 1).unwrap_or(s + 6);
            html[tag_end..e].trim().to_string()
        }
        _ => html.trim().to_string(),
    }
}
