use crate::core::GutenCore;
use crate::error::Result;
use crate::types::{DocToc, HeadingItem};
use std::fs;

impl GutenCore {
    /// Escanea un documento XHTML en busca de encabezados (headings) para construir una TOC
    ///
    /// Este método analiza un documento XHTML y extrae todos los encabezados
    /// (`<h1>` a `<h6>`) para generar una tabla de contenidos (Table of Contents).
    /// Es utilizado internamente por [`update_nav`](Self::update_nav) para construir
    /// la navegación automática del EPUB.
    ///
    /// # Proceso de escaneo
    ///
    /// 1. **Resuelve la ruta** - Construye la ruta absoluta al documento usando `opf_dir`
    /// 2. **Lee el archivo** - Carga el contenido XHTML desde el disco
    /// 3. **Parsea el XML** - Usa `roxmltree` para parsear el documento
    /// 4. **Busca encabezados** - Recorre todos los elementos del DOM
    /// 5. **Filtra etiquetas** - Identifica elementos con nombres `<h1>` a `<h6>`
    /// 6. **Extrae información** - Para cada encabezado, extrae:
    ///    - Nivel (1-6)
    ///    - Título (texto interno)
    ///    - Anclaje (atributo `id` para enlaces internos)
    ///
    /// # Argumentos
    ///
    /// * `href` - Ruta relativa al documento XHTML (desde el directorio OPF)
    ///   Ejemplo: `"Text/capitulo1.xhtml"`, `"Text/chapter2.xhtml"`
    ///
    /// # Retorna
    ///
    /// * `Result<DocToc>` - Estructura con la ruta del documento y la lista de encabezados
    ///
    /// # Errores
    ///
    /// * `GutenError::InvalidProject` - Si:
    ///   - `self.opf_dir` es `None` (proyecto no cargado)
    ///   - El archivo no existe o no se puede leer
    ///   - El archivo contiene XML mal formado
    /// * `std::io::Error` - Si falla la lectura del archivo
    ///
    /// # Ejemplo básico
    ///
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let core = GutenCore::open_folder("./mi_epub")?;
    ///
    /// // Escanear un capítulo
    /// let toc = core.scan_headings("Text/capitulo1.xhtml")?;
    ///
    /// println!("Documento: {}", toc.href);
    /// for heading in toc.items {
    ///     println!("  H{}: {} (id: {})", heading.level, heading.title, heading.anchor);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Ejemplo: Estructura de un documento
    ///
    /// Dado este XHTML:
    ///
    /// ```html
    /// <body>
    ///   <h1 id="intro">Introducción</h1>
    ///   <p>Texto introductorio...</p>
    ///   
    ///   <h2 id="section1">Primera Sección</h2>
    ///   <p>Contenido de la sección...</p>
    ///   
    ///   <h2 id="section2">Segunda Sección</h2>
    ///   <p>Más contenido...</p>
    ///   
    ///   <h3 id="subsection">Subsección (ignorada en TOC principal)</h3>
    ///   <p>Detalles...</p>
    /// </body>
    /// ```
    ///
    /// El método retornará:
    ///
    /// ```text
    /// DocToc {
    ///     href: "Text/capitulo1.xhtml",
    ///     items: [
    ///         HeadingItem { level: 1, title: "Introducción", anchor: "intro", include: true },
    ///         HeadingItem { level: 2, title: "Primera Sección", anchor: "section1", include: true },
    ///         HeadingItem { level: 2, title: "Segunda Sección", anchor: "section2", include: true },
    ///         HeadingItem { level: 3, title: "Subsección", anchor: "subsection", include: true },
    ///     ]
    /// }
    /// ```
    ///
    /// # Ejemplo: Uso interno en `update_nav`
    ///
    /// Este método es llamado automáticamente durante la generación de la TOC:
    ///
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    ///
    /// // update_nav llama a scan_headings para cada documento en el spine
    /// core.update_nav()?;  // Aquí se escanean todos los documentos automáticamente
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Ejemplo: Escaneo personalizado
    ///
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let core = GutenCore::open_folder("./mi_epub")?;
    ///
    /// // Escanear múltiples documentos
    /// let documentos = ["Text/intro.xhtml", "Text/cap1.xhtml", "Text/cap2.xhtml"];
    /// let mut indice = Vec::new();
    ///
    /// for href in documentos {
    ///     let toc = core.scan_headings(href)?;
    ///     indice.push(toc);
    /// }
    ///
    /// // Generar TOC personalizada
    /// for doc in indice {
    ///     println!("--- {} ---", doc.href);
    ///     for heading in doc.items {
    ///         if heading.level <= 2 {  // Solo H1 y H2
    ///             println!("  {}{}", "  ".repeat((heading.level - 1) as usize), heading.title);
    ///         }
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Reglas de extracción
    ///
    /// | Etiqueta | Nivel | ¿Se extrae? | Uso típico |
    /// |----------|-------|-------------|------------|
    /// | `<h1>` | 1 | ✅ Sí | Título principal del capítulo |
    /// | `<h2>` | 2 | ✅ Sí | Secciones principales |
    /// | `<h3>` | 3 | ✅ Sí | Subsecciones (pueden filtrarse) |
    /// | `<h4>` | 4 | ✅ Sí | Detalles (rara vez en TOC) |
    /// | `<h5>` | 5 | ✅ Sí | Muy detallado |
    /// | `<h6>` | 6 | ✅ Sí | Nivel más profundo |
    ///
    /// # Notas de implementación
    ///
    /// - **Detección de encabezados**: Busca etiquetas que comiencen con 'h'
    ///   seguido de un dígito (ej: `h1`, `h2`, etc.). Usa `to_lowercase()`
    ///   para manejar mayúsculas/minúsculas.
    /// - **Título**: Extrae el texto del nodo con `node.text()` (solo texto directo,
    ///   no HTML interno). Si está vacío, usa string vacío.
    /// - **Anclaje**: Usa el atributo `id` del elemento. Es crucial para enlaces
    ///   internos como `#section1`.
    /// - **Campo `include`**: Siempre se establece a `true`. Es usado por el
    ///   código que llama para filtrar niveles.
    /// - **Título fallback**: El campo `title` de `DocToc` se establece al href
    ///   como fallback. Idealmente debería extraerse del `<title>` del documento.
    ///
    /// # Limitaciones conocidas
    ///
    /// - **Texto plano**: No extrae contenido HTML interno del encabezado
    ///   (ej: `<h1>Hola <em>mundo</em></h1>` devuelve `"Hola mundo"` sin formato)
    /// - **Sin detección de anclajes alternativos**: Solo usa `id`, no `name`
    /// - **Título del documento**: No extrae el `<title>` de la cabecera
    ///   (campo `title` de `DocToc` es solo fallback)
    /// - **Solo XHTML**: Asume que el documento es XHTML válido; errores XML
    ///   causan fallo completo
    ///
    /// # Posibles mejoras
    ///
    /// ```rust
    /// // Podrías mejorar la extracción del título del documento:
    /// if let Some(title_node) = doc.descendants().find(|n| n.has_tag_name("title")) {
    ///     doc_toc.title = title_node.text().unwrap_or(href).to_string();
    /// }
    /// ```
    ///
    /// # Ver también
    ///
    /// - [`update_nav`](Self::update_nav) - Usa este método para generar la TOC
    /// - [`DocToc`](crate::types::DocToc) - Estructura contenedora de la TOC
    /// - [`HeadingItem`](crate::types::HeadingItem) - Estructura de cada encabezado
    /// - [HTML Heading Elements (MDN)](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/Heading_Elements)
    pub fn scan_headings(&self, href: &str) -> Result<DocToc> {
        let full_path = self
            .opf_dir
            .as_ref()
            .ok_or_else(|| crate::error::GutenError::InvalidProject("OPF dir not set".to_string()))?
            .join(href);

        let content = fs::read_to_string(full_path)?;
        let doc = roxmltree::Document::parse(&content).map_err(|e| {
            crate::error::GutenError::InvalidProject(format!("XML error in {}: {}", href, e))
        })?;

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
