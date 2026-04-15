use crate::core::GutenCore;
use crate::error::Result;
use crate::types::{DocToc, HeadingItem};
use std::fs;

impl GutenCore {
    /// Scan a document for headings to build a TOC
    pub fn scan_headings(&self, href: &str) -> Result<DocToc> {
        let full_path = self.opf_dir.as_ref()
            .ok_or_else(|| crate::error::GutenError::InvalidProject("OPF dir not set".to_string()))?
            .join(href);
        
        let content = fs::read_to_string(full_path)?;
        let doc = roxmltree::Document::parse(&content)
            .map_err(|e| crate::error::GutenError::InvalidProject(format!("XML error in {}: {}", href, e)))?;
        
        let mut items = Vec::new();
        
        for node in doc.descendants().filter(|n| n.is_element()) {
            let tag = node.tag_name().name().to_lowercase();
            if tag.len() == 2 && tag.starts_with('h') {
                if let Ok(level) = tag[1..].parse::<u8>() {
                    if (1..=6).contains(&level) {
                        let title = node.text().unwrap_or("").trim().to_string();
                        let anchor = node.attribute("id").unwrap_or("").to_string();
                        
                        items.push(HeadingItem {
                            level,
                            title,
                            anchor,
                            include: true,
                        });
                    }
                }
            }
        }
        
        Ok(DocToc {
            href: href.to_string(),
            title: href.to_string(), // Fallback title
            items,
            include: true,
        })
    }
}
