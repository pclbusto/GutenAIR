use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestItem {
    pub id: String,
    pub href: String,
    pub media_type: String,
    pub properties: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadingItem {
    pub level: u8,
    pub title: String,
    pub anchor: String,
    pub include: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocToc {
    pub href: String,
    pub title: String,
    pub items: Vec<HeadingItem>,
    pub include: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookMetadata {
    pub title: String,
    pub language: String,
    pub identifier: String,
    pub modified: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    Document,
    Style,
    Image,
    Font,
    Audio,
    Video,
    Script,
    Vector,
    Navigation,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GutenConfig {
    /// Estilos aplicados por defecto a todos los capítulos si no hay excepción
    pub default_styles: Vec<String>,
    /// Mapa de excepciones para capítulos específicos (ID capítulo -> Lista IDs estilos)
    pub exceptions: std::collections::HashMap<String, Vec<String>>,
    /// Indica si se deben inyectar automáticamente los links CSS en los XHTML
    pub auto_inject: bool,
    /// Estado persistente de la interfaz del editor
    pub editor_state: std::collections::HashMap<String, String>,
}

impl Default for GutenConfig {
    fn default() -> Self {
        Self {
            default_styles: Vec::new(),
            exceptions: std::collections::HashMap::new(),
            auto_inject: true,
            editor_state: std::collections::HashMap::new(),
        }
    }
}
