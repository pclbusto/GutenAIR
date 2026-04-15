use crate::core::GutenCore;
use crate::error::{GutenError, Result};
use crate::types::ManifestItem;
use std::fs;
use std::path::{Path, PathBuf};
use path_slash::{PathBufExt, PathExt};

impl GutenCore {
    // -------------------------
    // Spine Operations
    // -------------------------

    pub fn get_spine(&self) -> &Vec<String> {
        &self.spine
    }

    pub fn set_spine(&mut self, idrefs: Vec<String>) {
        self.spine = idrefs;
    }

    pub fn spine_insert(&mut self, idref: String, index: Option<usize>) {
        if self.spine.contains(&idref) {
            return;
        }
        match index {
            Some(i) => {
                let pos = i.min(self.spine.len());
                self.spine.insert(pos, idref);
            }
            None => self.spine.push(idref),
        }
    }

    pub fn spine_move(&mut self, idref: &str, new_index: usize) -> Result<()> {
        let pos = self.spine.iter().position(|r| r == idref)
            .ok_or_else(|| GutenError::Manifest(format!("{} not in spine", idref)))?;
        
        let idref_owned = self.spine.remove(pos);
        let target_pos = new_index.min(self.spine.len());
        self.spine.insert(target_pos, idref_owned);
        Ok(())
    }

    pub fn spine_remove(&mut self, idref: &str) {
        if let Some(pos) = self.spine.iter().position(|r| r == idref) {
            self.spine.remove(pos);
        }
    }

    // -------------------------
    // Manifest Operations
    // -------------------------

    pub fn add_to_manifest(&mut self, id: String, href: String, media_type: String, properties: String) -> Result<()> {
        if self.manifest.contains_key(&id) {
            return Err(GutenError::Manifest(format!("ID {} already exists", id)));
        }
        
        // Normalize href to Unix style for safety (handle both separators)
        let clean_href = href.replace('\\', "/");
        
        let item = ManifestItem {
            id: id.clone(),
            href: clean_href,
            media_type,
            properties,
        };
        
        self.manifest.insert(id, item);
        Ok(())
    }

    pub fn remove_from_manifest(&mut self, id: &str) -> Result<()> {
        self.spine_remove(id);
        self.manifest.remove(id)
            .ok_or_else(|| GutenError::Manifest(format!("ID {} not found in manifest", id)))?;
        
        Ok(())
    }

    /// Physically deletes an item from the spine, the manifest, and the disk.
    pub fn delete_item(&mut self, id: &str) -> Result<()> {
        let path = self.get_resource_path(id)?;
        
        self.remove_from_manifest(id)?;
        
        if path.exists() {
            fs::remove_file(path)?;
        }
        
        Ok(())
    }

    pub fn get_item(&self, id_or_href: &str) -> Result<&ManifestItem> {
        self.manifest.get(id_or_href)
            .or_else(|| self.manifest.values().find(|it| it.href == id_or_href))
            .ok_or_else(|| GutenError::Manifest(format!("Resource not found: {}", id_or_href)))
    }

    pub fn get_resource_path(&self, id_or_href: &str) -> Result<PathBuf> {
        let item = self.get_item(id_or_href)?;
        let opf_dir = self.opf_dir.as_ref()
            .ok_or_else(|| GutenError::InvalidProject("OPF dir not loaded".into()))?;
        
        Ok(opf_dir.join(&item.href))
    }

    pub fn validate_integrity(&self) -> Vec<String> {
        let mut errors = Vec::new();
        for item in self.manifest.values() {
            match self.get_resource_path(&item.id) {
                Ok(path) => {
                    if !path.exists() {
                        errors.push(format!("Archivo faltante: {}", item.href));
                    }
                }
                Err(e) => errors.push(format!("Error resolviendo {}: {}", item.href, e)),
            }
        }
        errors
    }

    pub fn rename_item(&mut self, id: &str, new_href: String) -> Result<()> {
        let item = self.manifest.get_mut(id)
            .ok_or_else(|| GutenError::Manifest(format!("ID {} not found", id)))?;
        
        // Normalize the new href (handle both separators)
        let clean_href = new_href.replace('\\', "/");
        
        // Physical rename
        if let (Some(opf_dir), old_href) = (&self.opf_dir, &item.href) {
            let old_path = opf_dir.join(old_href);
            // Use native path for FS operations
            let new_path_native = if cfg!(windows) {
                opf_dir.join(&clean_href.replace('/', "\\"))
            } else {
                opf_dir.join(&clean_href)
            };
            
            if old_path.exists() {
                if let Some(parent) = new_path_native.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::rename(old_path, new_path_native)?;
            }
        }
        
        item.href = clean_href;
        Ok(())
    }

    pub fn sanitize_filename(&self, name: &str) -> String {
        name.chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
            .collect::<String>()
    }

    // -------------------------
    // High-Level Orchestration
    // -------------------------

    /// Add a new resource from memory (writes to disk and updates manifest)
    pub fn add_resource(&mut self, id: String, bytes: &[u8], mime_type: &str, target_href: &str) -> Result<()> {
        let clean_href = target_href.replace('\\', "/");
        let opf_dir = self.opf_dir.as_ref().ok_or_else(|| GutenError::InvalidProject("OPF dir not loaded".into()))?;
        let full_path = opf_dir.join(&clean_href);

        if full_path.exists() {
            return Err(GutenError::Manifest(format!("File already exists: {}", clean_href)));
        }

        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(full_path, bytes)?;

        self.add_to_manifest(id, clean_href, mime_type.to_string(), "".to_string())
    }

    /// Import a file from the local system into the project
    pub fn import_file(&mut self, source_path: impl AsRef<Path>, id: String, target_href: &str, mime_type: &str) -> Result<()> {
        let bytes = fs::read(source_path)?;
        self.add_resource(id, &bytes, mime_type, target_href)
    }

    /// Calculate the relative path from one resource to another (for internal links)
    pub fn get_relative_path(&self, from_id: &str, to_id: &str) -> Result<String> {
        let from_item = self.get_item(from_id)?;
        let to_item = self.get_item(to_id)?;
        
        let from_path = Path::new(&from_item.href);
        let to_path = Path::new(&to_item.href);
        
        let from_parent = from_path.parent().unwrap_or(Path::new(""));
        
        let rel = pathdiff::diff_paths(to_path, from_parent)
            .ok_or_else(|| GutenError::Other(format!("Could not calculate relative path from {} to {}", from_item.href, to_item.href)))?;
            
        Ok(rel.to_slash_lossy().to_string())
    }

    /// Deep check: Manifest items must exist, AND files on disk must be in manifest
    pub fn validate_integrity_deep(&self) -> (Vec<String>, Vec<PathBuf>) {
        let manifest_errors = self.validate_integrity();
        let mut orphans = Vec::new();

        if let Some(opf_dir) = &self.opf_dir {
            let known_hrefs: std::collections::HashSet<&str> =
                self.manifest.values().map(|it| it.href.as_str()).collect();

            for entry in walkdir::WalkDir::new(opf_dir).min_depth(1) {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_file() {
                        let rel_path = path.strip_prefix(opf_dir).unwrap();
                        let href = rel_path.to_slash_lossy();

                        if href == "content.opf" || href == "../mimetype" {
                            continue;
                        }

                        if !known_hrefs.contains(href.as_ref()) {
                            orphans.push(path.to_path_buf());
                        }
                    }
                }
            }
        }

        (manifest_errors, orphans)
    }

    /// sanitizes the content, writes the file, and registers it in the manifest.
    /// `id` is the manifest ID (e.g. "chap2"), content is raw HTML from any source.
    /// The file is placed at OEBPS/Text/{id}.xhtml automatically.
    pub fn add_document(&mut self, id: &str, raw_content: &str) -> Result<()> {
        let href = format!("Text/{}.xhtml", id);

        if self.manifest.contains_key(id) {
            return Err(GutenError::Manifest(format!("ID '{}' already exists in manifest", id)));
        }

        let clean_xhtml = self.sanitize_to_xhtml(raw_content)?;

        let opf_dir = self.opf_dir.as_ref()
            .ok_or_else(|| GutenError::InvalidProject("OPF dir not loaded".into()))?;
        let full_path = opf_dir.join(&href);

        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full_path, clean_xhtml)?;

        self.add_to_manifest(
            id.to_string(),
            href,
            "application/xhtml+xml".to_string(),
            "".to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_delete_item() -> Result<()> {
        let dir = tempdir()?;
        let mut core = GutenCore::new_project(dir.path(), "Test Book", "en")?;
        
        // Ensure chap1 exists (auto-created by new_project)
        assert!(core.manifest.contains_key("chap1"));
        let chap1_path = core.get_resource_path("chap1")?;
        assert!(chap1_path.exists());
        assert!(core.spine.contains(&"chap1".to_string()));

        // Delete chap1
        core.delete_item("chap1")?;

        // Verify deletion
        assert!(!core.manifest.contains_key("chap1"));
        assert!(!chap1_path.exists());
        assert!(!core.spine.contains(&"chap1".to_string()));

        Ok(())
    }
}
