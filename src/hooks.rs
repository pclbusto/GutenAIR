use crate::core::GutenCore;
use crate::error::Result;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Hook {
    pub file_href: String,
    pub hook_id: String,
    pub tag_name: String,
}

impl GutenCore {
    /// Builds a full index of all IDs (hooks) in the project documents
    pub fn build_hook_index(&self) -> Result<HashMap<String, Vec<Hook>>> {
        let mut index = HashMap::new();
        
        for item in self.manifest.values() {
            if item.media_type == "application/xhtml+xml" || item.media_type == "text/html" {
                let hooks = self.scan_hooks(&item.href)?;
                index.insert(item.href.clone(), hooks);
            }
        }
        
        Ok(index)
    }

    fn scan_hooks(&self, href: &str) -> Result<Vec<Hook>> {
        let full_path = self.opf_dir.as_ref()
            .ok_or_else(|| crate::error::GutenError::InvalidProject("OPF dir not set".to_string()))?
            .join(href);
            
        let content = std::fs::read_to_string(full_path)?;
        let doc = roxmltree::Document::parse(&content)
            .map_err(|e| crate::error::GutenError::InvalidProject(format!("XML error in {}: {}", href, e)))?;
            
        let mut hooks = Vec::new();
        for node in doc.descendants().filter(|n| n.is_element()) {
            if let Some(id) = node.attribute("id") {
                hooks.push(Hook {
                    file_href: href.to_string(),
                    hook_id: id.to_string(),
                    tag_name: node.tag_name().name().to_string(),
                });
            }
        }
        
        Ok(hooks)
    }
}
