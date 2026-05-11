use crate::core::GutenCore;
use crate::error::Result;
use crate::GutenError;
use std::fs;

/// Trait para transformar el contenido de un capítulo en otro formato.
pub trait ExportTransformer {
    /// Transforma el contenido XHTML en el formato destino.
    fn transform(&self, id: &str, xhtml_content: &str) -> Result<String>;
    
    /// Retorna la extensión de archivo sugerida para este formato (ej: "txt").
    fn extension(&self) -> &'static str;
}

/// Transformador básico que extrae texto plano de los capítulos.
pub struct TextTransformer;

impl ExportTransformer for TextTransformer {
    fn transform(&self, _id: &str, xhtml_content: &str) -> Result<String> {
        // Usamos ammonia para extraer solo el texto
        let clean_text = ammonia::Builder::new()
            .tags(std::collections::HashSet::new())
            .clean(xhtml_content)
            .to_string();
            
        Ok(clean_text)
    }
    
    fn extension(&self) -> &'static str {
        "txt"
    }
}

impl GutenCore {
    /// Exporta una lista de capítulos usando un transformador específico.
    ///
    /// # Argumentos
    /// * `chapter_ids` - Lista de IDs de capítulos a exportar (en el orden deseado).
    /// * `transformer` - Una implementación de `ExportTransformer`.
    ///
    /// # Retorna
    /// * `String` - El contenido total exportado y concatenado.
    pub fn export_custom<T: ExportTransformer>(
        &self,
        chapter_ids: &[String],
        transformer: &T
    ) -> Result<String> {
        let mut output = String::new();
        let opf_dir = self.opf_dir.as_ref()
            .ok_or_else(|| GutenError::InvalidProject("OPF directory not loaded".to_string()))?;

        for id in chapter_ids {
            let item = self.get_item(id)?;
            let file_path = opf_dir.join(&item.href);
            let content = fs::read_to_string(&file_path)?;
            
            let transformed = transformer.transform(id, &content)?;
            output.push_str(&transformed);
            output.push('\n'); // Separador entre capítulos
        }

        Ok(output)
    }

    /// Exporta los capítulos indicados a un único archivo de texto plano.
    ///
    /// Si la lista de IDs está vacía, exporta todo el `spine`.
    pub fn export_to_text(&self, chapter_ids: Option<Vec<String>>) -> Result<String> {
        let ids = match chapter_ids {
            Some(vec) => vec,
            None => self.spine.clone(),
        };
        
        self.export_custom(&ids, &TextTransformer)
    }

    /// Exporta el contenido a un archivo físico en el disco.
    /// 
    /// # Argumentos
    /// * `output_dir` - Directorio donde se guardará el archivo.
    /// * `filename` - Nombre del archivo (opcional). Si es None, usa "{titulo}.txt".
    /// * `chapter_ids` - Lista de capítulos (opcional). Si es None, usa todo el spine.
    pub fn export_to_text_file(
        &self,
        output_dir: impl AsRef<std::path::Path>,
        filename: Option<String>,
        chapter_ids: Option<Vec<String>>
    ) -> Result<std::path::PathBuf> {
        let content = self.export_to_text(chapter_ids)?;
        
        let final_filename = match filename {
            Some(name) => name,
            None => {
                let title = self.get_metadata()
                    .map(|m| m.title.clone())
                    .unwrap_or_else(|| "libro".to_string());
                format!("{}.txt", title)
            }
        };

        let output_path = output_dir.as_ref().join(final_filename);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&output_path, content)?;
        
        Ok(output_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_export_to_text() -> Result<()> {
        let dir = tempdir()?;
        let mut core = GutenCore::new_project(dir.path(), "Test Export", "en")?;
        
        core.add_document("exp1", "<p>Texto del capítulo 1.</p>")?;
        core.add_document("exp2", "<p>Texto del capítulo 2.</p>")?;
        
        let exported = core.export_to_text(Some(vec!["exp1".to_string(), "exp2".to_string()]))?;
        
        assert!(exported.contains("Texto del capítulo 1."));
        assert!(exported.contains("Texto del capítulo 2."));
        
        Ok(())
    }

    #[test]
    fn test_export_to_text_file() -> Result<()> {
        let dir = tempdir()?;
        let output_dir = dir.path().join("exports");
        let mut core = GutenCore::new_project(dir.path().join("book"), "Mi Libro", "es")?;
        
        core.add_document("exp1", "<p>Contenido</p>")?;
        
        // Test 1: Nombre por defecto
        let path1 = core.export_to_text_file(&output_dir, None, Some(vec!["exp1".to_string()]))?;
        assert!(path1.ends_with("Mi Libro.txt"));
        assert!(path1.exists());
        
        // Test 2: Nombre personalizado
        let path2 = core.export_to_text_file(&output_dir, Some("especial.txt".to_string()), None)?;
        assert!(path2.ends_with("especial.txt"));
        assert!(path2.exists());
        
        Ok(())
    }
}
