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
