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
    pub author: Option<String>,
    pub language: String,
    pub identifier: String,
    pub modified: String,
    pub series: Option<String>,
    pub series_index: Option<f32>,
    pub tags: Vec<String>,
    pub description: Option<String>,
    /// Metadatos custom con prefijo (ej: `rubrica:source`, `rubrica:imported_at`).
    /// Clave = nombre completo del atributo property/name, valor = contenido.
    pub custom_meta: std::collections::HashMap<String, String>,
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
pub struct StyleEntry {
    pub clase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descripcion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_sugerido: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleGroup {
    pub bloque: Vec<StyleEntry>,
    pub linea: Vec<StyleEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleCatalog {
    pub archivo_origen: String,
    pub estilos: StyleGroup,
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

/// Entrada unificada de tabla de contenidos (EPUB2 toc.ncx + EPUB3 nav.xhtml)
#[derive(Debug, Clone)]
pub struct TocEntry {
    pub title: String,
    pub href: String,
    pub level: u8,
}

/// Esquema de organización de carpetas para reorganizar un EPUB en disco
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderSchema {
    /// Author/Series/Title.epub
    AuthorSeriesTitle,
    /// Author/Title.epub
    AuthorTitle,
    /// Title.epub
    Flat,
}

/// Reporte completo de salud estructural de un EPUB
#[derive(Debug, Clone)]
pub struct EpubHealthReport {
    /// Lista de (from_chapter, href) con enlaces rotos
    pub broken_links: Vec<(String, String)>,
    /// Anclajes (id) huérfanos que no son referenciados por ningún enlace
    pub orphan_anchors: Vec<String>,
    /// Entradas en el manifiesto que apuntan a archivos inexistentes
    pub missing_manifest_entries: Vec<String>,
    /// true si el OPF está bien formado y es parseable
    pub opf_well_formed: bool,
}
