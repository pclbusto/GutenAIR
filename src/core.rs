use std::path::{Path, PathBuf};
use std::collections::HashMap;
use crate::error::{GutenError, Result};
use crate::types::*;
use std::fs;
use chrono::{SecondsFormat, Utc};

pub const NS_OPF: &str = "http://www.idpf.org/2007/opf";
pub const NS_DC: &str = "http://purl.org/dc/elements/1.1/";
pub const NS_OCF: &str = "urn:oasis:names:tc:opendocument:xmlns:container";

pub struct GutenCore {
    pub workdir: PathBuf,
    pub opf_path: Option<PathBuf>,
    pub opf_dir: Option<PathBuf>,
    pub metadata: Option<BookMetadata>,
    pub manifest: HashMap<String, ManifestItem>,
    pub spine: Vec<String>,
}

impl GutenCore {
    pub fn new(workdir: impl AsRef<Path>) -> Self {
        Self {
            workdir: workdir.as_ref().to_path_buf(),
            opf_path: None,
            opf_dir: None,
            metadata: None,
            manifest: HashMap::new(),
            spine: Vec::new(),
        }
    }

    /// Open an existing folder as a workspace
    pub fn open_folder(workdir: impl AsRef<Path>) -> Result<Self> {
        let mut core = Self::new(workdir);
        core.load_container_and_opf()?;
        core.parse_opf()?;
        Ok(core)
    }

    /// Create a new project skeleton
    pub fn new_project(
        root: impl AsRef<Path>,
        title: &str,
        lang: &str,
    ) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        if root.exists() && fs::read_dir(&root)?.next().is_some() {
            return Err(GutenError::InvalidProject("Target directory is not empty".to_string()));
        }

        // Create folders
        fs::create_dir_all(root.join("META-INF"))?;
        fs::create_dir_all(root.join("OEBPS/Text"))?;
        fs::create_dir_all(root.join("OEBPS/Styles"))?;
        fs::create_dir_all(root.join("OEBPS/Images"))?;

        // mimetype
        fs::write(root.join("mimetype"), "application/epub+zip")?;

        // container.xml
        let container_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;
        fs::write(root.join("META-INF/container.xml"), container_xml)?;

        // OPF minimum
        let book_uuid = uuid::Uuid::new_v4().to_string();
        let modified = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
        
        let opf_xml = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<package version="3.0" unique-identifier="bookid" xmlns="http://www.idpf.org/2007/opf" xml:lang="{lang}">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/">
    <dc:identifier id="bookid">urn:uuid:{book_uuid}</dc:identifier>
    <dc:title>{title}</dc:title>
    <dc:language>{lang}</dc:language>
    <meta property="dcterms:modified">{modified}</meta>
  </metadata>
  <manifest>
    <item id="style" href="Styles/style.css" media-type="text/css"/>
    <item id="chap1" href="Text/chap1.xhtml" media-type="application/xhtml+xml"/>
    <item id="nav" href="Text/nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
  </manifest>
  <spine>
    <itemref idref="chap1"/>
  </spine>
</package>"#, lang=lang, title=title, book_uuid=book_uuid, modified=modified);

        fs::write(root.join("OEBPS/content.opf"), opf_xml)?;

        // Basic files
        fs::write(root.join("OEBPS/Styles/style.css"), "body { font-family: serif; }")?;
        
        let chap1 = r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" lang="en">
<head>
  <title>Chapter 1</title>
  <link rel="stylesheet" type="text/css" href="../Styles/style.css"/>
</head>
<body>
  <h1>Chapter 1</h1>
  <p>Hello, EPUB!</p>
</body>
</html>"#;
        fs::write(root.join("OEBPS/Text/chap1.xhtml"), chap1)?;

        let nav = r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="en">
<head><title>TOC</title></head>
<body>
  <nav epub:type="toc" id="toc">
    <ol>
      <li><a href="chap1.xhtml">Chapter 1</a></li>
    </ol>
  </nav>
</body>
</html>"#;
        fs::write(root.join("OEBPS/Text/nav.xhtml"), nav)?;

        Self::open_folder(root)
    }

    /// Load container.xml and find the OPF path
    fn load_container_and_opf(&mut self) -> Result<()> {
        let container_path = self.workdir.join("META-INF").join("container.xml");
        if !container_path.exists() {
            return Err(GutenError::InvalidProject("META-INF/container.xml not found".to_string()));
        }

        let content = fs::read_to_string(&container_path)?;
        let doc = roxmltree::Document::parse(&content)
            .map_err(|e| GutenError::InvalidProject(format!("XML error in container.xml: {}", e)))?;

        let rootfile = doc.descendants()
            .find(|n| n.has_tag_name((NS_OCF, "rootfile")))
            .ok_or_else(|| GutenError::InvalidProject("container.xml invalid: missing rootfile".to_string()))?;

        let full_path_attr = rootfile.attribute("full-path")
            .ok_or_else(|| GutenError::InvalidProject("container.xml invalid: rootfile missing full-path".to_string()))?;

        let opf_path = self.workdir.join(full_path_attr);
        self.opf_dir = Some(opf_path.parent().unwrap().to_path_buf());
        self.opf_path = Some(opf_path);

        Ok(())
    }

    /// Parse the OPF file into memory
    fn parse_opf(&mut self) -> Result<()> {
        let opf_path = self.opf_path.as_ref().ok_or_else(|| GutenError::InvalidProject("OPF path not loaded".to_string()))?;
        let content = fs::read_to_string(opf_path)?;
        let doc = roxmltree::Document::parse(&content)
            .map_err(|e| GutenError::InvalidProject(format!("XML error in content.opf: {}", e)))?;

        let root = doc.root_element();
        
        // Metadata
        let metadata_node = root.children().find(|n| n.has_tag_name((NS_OPF, "metadata")))
            .ok_or_else(|| GutenError::InvalidProject("OPF missing metadata".to_string()))?;
        
        let title = metadata_node.children()
            .find(|n| n.has_tag_name((NS_DC, "title")))
            .map(|n| n.text().unwrap_or("").to_string())
            .unwrap_or_else(|| "Untitled".to_string());
            
        let language = metadata_node.children()
            .find(|n| n.has_tag_name((NS_DC, "language")))
            .map(|n| n.text().unwrap_or("").to_string())
            .unwrap_or_else(|| "en".to_string());
            
        let identifier = metadata_node.children()
            .find(|n| n.has_tag_name((NS_DC, "identifier")))
            .map(|n| n.text().unwrap_or("").to_string())
            .unwrap_or_else(|| "".to_string());

        let modified = metadata_node.children()
            .find(|n| n.has_tag_name((NS_OPF, "meta")) && n.attribute("property") == Some("dcterms:modified"))
            .map(|n| n.text().unwrap_or("").to_string())
            .unwrap_or_else(|| Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true));

        self.metadata = Some(BookMetadata { title, language, identifier, modified });

        // Manifest
        let manifest_node = root.children().find(|n| n.has_tag_name((NS_OPF, "manifest")))
            .ok_or_else(|| GutenError::InvalidProject("OPF missing manifest".to_string()))?;
            
        self.manifest.clear();
        for item in manifest_node.children().filter(|n| n.has_tag_name((NS_OPF, "item"))) {
            let id = item.attribute("id").unwrap_or("").to_string();
            let href = item.attribute("href").unwrap_or("").to_string();
            let media_type = item.attribute("media-type").unwrap_or("").to_string();
            let properties = item.attribute("properties").unwrap_or("").to_string();
            
            if !id.is_empty() {
                self.manifest.insert(id.clone(), ManifestItem { id, href, media_type, properties });
            }
        }

        // Spine
        let spine_node = root.children().find(|n| n.has_tag_name((NS_OPF, "spine")))
            .ok_or_else(|| GutenError::InvalidProject("OPF missing spine".to_string()))?;
            
        self.spine.clear();
        for itemref in spine_node.children().filter(|n| n.has_tag_name((NS_OPF, "itemref"))) {
            if let Some(idref) = itemref.attribute("idref") {
                self.spine.push(idref.to_string());
            }
        }

        Ok(())
    }

    /// Save the current state to the OPF file
    pub fn save(&mut self) -> Result<()> {
        let opf_path = self.opf_path.clone().ok_or_else(|| GutenError::InvalidProject("OPF path not loaded".to_string()))?;
        
        // 1. Sync Nav (TOC) automatically
        self.update_nav()?;

        // 2. Update modified date before saving
        self.update_modified_date();
        let metadata = self.metadata.as_ref().ok_or_else(|| GutenError::InvalidProject("Metadata missing".to_string()))?;

        use quick_xml::writer::Writer;
        use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
        use std::io::Cursor;

        let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

        // Header
        writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

        // <package>
        let mut package = BytesStart::new("package");
        package.push_attribute(("version", "3.0"));
        package.push_attribute(("unique-identifier", "bookid"));
        package.push_attribute(("xmlns", "http://www.idpf.org/2007/opf"));
        package.push_attribute(("xml:lang", metadata.language.as_str()));
        writer.write_event(Event::Start(package))?;

        //   <metadata>
        let mut meta_start = BytesStart::new("metadata");
        meta_start.push_attribute(("xmlns:dc", "http://purl.org/dc/elements/1.1/"));
        meta_start.push_attribute(("xmlns:dcterms", "http://purl.org/dc/terms/"));
        writer.write_event(Event::Start(meta_start))?;

        //     dc:identifier
        let mut id_start = BytesStart::new("dc:identifier");
        id_start.push_attribute(("id", "bookid"));
        writer.write_event(Event::Start(id_start))?;
        writer.write_event(Event::Text(BytesText::new(&metadata.identifier)))?;
        writer.write_event(Event::End(BytesEnd::new("dc:identifier")))?;

        //     dc:title
        writer.write_event(Event::Start(BytesStart::new("dc:title")))?;
        writer.write_event(Event::Text(BytesText::new(&metadata.title)))?;
        writer.write_event(Event::End(BytesEnd::new("dc:title")))?;

        //     dc:language
        writer.write_event(Event::Start(BytesStart::new("dc:language")))?;
        writer.write_event(Event::Text(BytesText::new(&metadata.language)))?;
        writer.write_event(Event::End(BytesEnd::new("dc:language")))?;

        //     dcterms:modified
        let mut mod_start = BytesStart::new("meta");
        mod_start.push_attribute(("property", "dcterms:modified"));
        writer.write_event(Event::Start(mod_start))?;
        writer.write_event(Event::Text(BytesText::new(&metadata.modified)))?;
        writer.write_event(Event::End(BytesEnd::new("meta")))?;

        writer.write_event(Event::End(BytesEnd::new("metadata")))?;

        //   <manifest>
        writer.write_event(Event::Start(BytesStart::new("manifest")))?;
        
        let mut sorted_manifest: Vec<_> = self.manifest.values().collect();
        sorted_manifest.sort_by(|a, b| a.id.cmp(&b.id));

        for item in sorted_manifest {
            let mut it = BytesStart::new("item");
            it.push_attribute(("id", item.id.as_str()));
            it.push_attribute(("href", item.href.as_str()));
            it.push_attribute(("media-type", item.media_type.as_str()));
            if !item.properties.is_empty() {
                it.push_attribute(("properties", item.properties.as_str()));
            }
            writer.write_event(Event::Empty(it))?;
        }
        writer.write_event(Event::End(BytesEnd::new("manifest")))?;

        //   <spine>
        writer.write_event(Event::Start(BytesStart::new("spine")))?;
        for idref in &self.spine {
            let mut ir = BytesStart::new("itemref");
            ir.push_attribute(("idref", idref.as_str()));
            writer.write_event(Event::Empty(ir))?;
        }
        writer.write_event(Event::End(BytesEnd::new("spine")))?;

        writer.write_event(Event::End(BytesEnd::new("package")))?;

        let result = writer.into_inner().into_inner();
        fs::write(opf_path, result)?;

        Ok(())
    }

    /// Automatically rebuild the nav.xhtml based on Spine and Headings
    pub fn update_nav(&mut self) -> Result<()> {
        let mut nav_items = Vec::new();
        for idref in &self.spine {
            if let Some(item) = self.manifest.get(idref) {
                if item.media_type == "application/xhtml+xml" {
                    let doc_toc = self.scan_headings(&item.href)?;
                    nav_items.push(doc_toc);
                }
            }
        }

        // nav.xhtml lives at "Text/nav.xhtml", so links must be relative to "Text/"
        let nav_dir = std::path::Path::new("Text");

        let mut list_items = Vec::new();
        for doc in nav_items {
            let doc_path = std::path::Path::new(&doc.href);
            let rel = pathdiff::diff_paths(doc_path, nav_dir)
                .unwrap_or_else(|| doc_path.to_path_buf());
            let rel_str = rel.to_string_lossy();

            for heading in doc.items {
                if heading.level <= 2 { // Only H1 and H2 for the main nav
                    let indent = if heading.level > 1 { "  " } else { "" };
                    let href = if heading.anchor.is_empty() {
                        rel_str.to_string()
                    } else {
                        format!("{}#{}", rel_str, heading.anchor)
                    };
                    list_items.push(format!("{}<li><a href=\"{}\">{}</a></li>", indent, href, heading.title));
                }
            }
        }

        let title = self.metadata.as_ref().map(|m| m.title.as_str()).unwrap_or("Table of Contents");
        let nav_xhtml = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="en">
<head><title>{}</title></head>
<body>
  <nav epub:type="toc" id="toc">
    <h1>{}</h1>
    <ol>
{}
    </ol>
  </nav>
</body>
</html>"#, title, title, list_items.join("\n"));

        // Save nav.xhtml and ensure it's in the manifest
        let nav_href = "Text/nav.xhtml";
        let opf_dir = self.opf_dir.as_ref().ok_or_else(|| GutenError::InvalidProject("OPF dir not loaded".into()))?;
        fs::write(opf_dir.join(nav_href), nav_xhtml)?;

        if !self.manifest.values().any(|it| it.href == nav_href) {
            self.add_to_manifest("nav".to_string(), nav_href.to_string(), "application/xhtml+xml".to_string(), "nav".to_string())?;
        }

        Ok(())
    }
}
