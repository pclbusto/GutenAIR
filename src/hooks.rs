use crate::core::GutenCore;
use crate::error::Result;
use std::collections::HashMap;
/// Representa un "hook" o punto de anclaje dentro de un documento del proyecto
///
/// Un hook es cualquier elemento HTML que tenga un atributo `id`. Estos IDs
/// pueden ser usados como puntos de referencia para:
/// - Enlaces internos (fragmentos URL como `#capitulo1`)
/// - Navegación personalizada
/// - Inyección de contenido en posiciones específicas
/// - Análisis de estructura del documento
///
/// # Ejemplo
///
/// Dado este HTML:
/// ```html
/// <section id="introduccion">
///   <h2 id="titulo-intro">Introducción</h2>
///   <p id="primer-parrafo">Texto...</p>
/// </section>
/// ```
///
/// Se generarán tres hooks:
/// - `{ file_href: "doc.xhtml", hook_id: "introduccion", tag_name: "section" }`
/// - `{ file_href: "doc.xhtml", hook_id: "titulo-intro", tag_name: "h2" }`
/// - `{ file_href: "doc.xhtml", hook_id: "primer-parrafo", tag_name: "p" }`
#[derive(Debug, Clone)]
pub struct Hook {
    /// Ruta relativa al archivo que contiene este hook
    ///
    /// Es la misma ruta que aparece en el manifiesto (ej: `"Text/capitulo1.xhtml"`)
    pub file_href: String,

    /// Valor del atributo `id` del elemento
    ///
    /// Debe ser único dentro del documento (según estándar HTML)
    pub hook_id: String,

    /// Nombre de la etiqueta HTML (ej: `"p"`, `"h1"`, `"div"`, `"section"`)
    pub tag_name: String,
}

impl GutenCore {
    /// Construye un índice completo de todos los IDs (hooks) en los documentos del proyecto
    ///
    /// Escanea todos los documentos XHTML/HTML del proyecto y extrae todos los
    /// elementos que tienen un atributo `id`, organizándolos en un mapa donde
    /// la clave es la ruta del archivo y el valor es la lista de hooks de ese archivo.
    ///
    /// # ¿Qué documentos se escanean?
    ///
    /// Solo se escanean los items del manifiesto con `media_type`:
    /// - `application/xhtml+xml` (documentos XHTML estándar de EPUB)
    /// - `text/html` (documentos HTML, para compatibilidad)
    ///
    /// # Estructura del índice resultante
    ///
    /// ```text
    /// HashMap<String, Vec<Hook>>
    ///     │           │
    ///     │           └── Lista de hooks encontrados en ese archivo
    ///     └── Ruta del archivo (ej: "Text/capitulo1.xhtml")
    /// ```
    ///
    /// # Retorna
    ///
    /// * `Result<HashMap<String, Vec<Hook>>>` - Mapa de rutas a lista de hooks,
    ///   o un error si falla la lectura/parseo de algún documento.
    ///
    /// # Errores
    ///
    /// * `GutenError::InvalidProject` - Si:
    ///   - `self.opf_dir` es `None` (proyecto no cargado)
    ///   - Algún archivo XHTML no se puede parsear (XML mal formado)
    /// * `std::io::Error` - Si falla la lectura de algún archivo
    ///
    /// # Ejemplo básico
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// # use gutencore::error::Result;
    /// # fn example() -> Result<()> {
    /// let core = GutenCore::open_folder("./mi_epub")?;
    /// let index = core.build_hook_index()?;
    ///
    /// // Iterar sobre todos los hooks encontrados
    /// for (file, hooks) in &index {
    ///     println!("Archivo: {}", file);
    ///     for hook in hooks {
    ///         println!("  - #{} ({})", hook.hook_id, hook.tag_name);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Ejemplo: Buscar un hook específico
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// # use gutencore::error::Result;
    /// # fn example() -> Result<()> {
    /// let core = GutenCore::open_folder("./mi_epub")?;
    /// let index = core.build_hook_index()?;
    ///
    /// // Encontrar el hook con id "introduccion"
    /// for (file, hooks) in &index {
    ///     if let Some(hook) = hooks.iter().find(|h| h.hook_id == "introduccion") {
    ///         println!("Hook encontrado en: {}", file);
    ///         println!("Etiqueta: {}", hook.tag_name);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Ejemplo: Verificar unicidad de IDs
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// # use gutencore::error::Result;
    /// # use std::collections::HashSet;
    /// # fn example() -> Result<()> {
    /// let core = GutenCore::open_folder("./mi_epub")?;
    /// let index = core.build_hook_index()?;
    ///
    /// let mut all_ids = HashSet::new();
    /// for hooks in index.values() {
    ///     for hook in hooks {
    ///         if !all_ids.insert(&hook.hook_id) {
    ///             eprintln!("ID duplicado encontrado: #{}", hook.hook_id);
    ///         }
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Ejemplo: Generar tabla de contenidos personalizada
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// # use gutencore::error::Result;
    /// # fn example() -> Result<()> {
    /// let core = GutenCore::open_folder("./mi_epub")?;
    /// let index = core.build_hook_index()?;
    ///
    /// // Generar TOC solo con encabezados que tengan ID
    /// let mut toc = Vec::new();
    /// for (file, hooks) in &index {
    ///     for hook in hooks {
    ///         if hook.tag_name.starts_with('h') && hook.tag_name.len() == 2 {
    ///             toc.push(format!("<li><a href=\"{}\">{}</a></li>",
    ///                 format!("{}#{}", file, hook.hook_id), hook.hook_id));
    ///         }
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Rendimiento
    ///
    /// - **Complejidad**: O(N × M) donde N = número de documentos, M = elementos por documento
    /// - **Memoria**: Almacena todos los IDs de todos los documentos
    /// - **Uso típico**: Proyectos con <100 documentos y <10k elementos totales
    ///
    /// Para proyectos muy grandes, considera:
    /// - Cachear el resultado después de la primera llamada
    /// - Usar `scan_hooks` para escanear documentos individualmente
    ///
    /// # Notas de implementación
    ///
    /// - **Método público**: Forma parte de la API para análisis de estructura
    /// - **Solo atributo `id`**: No escanea otros identificadores como `name`
    /// - **Documentos no XHTML**: Ignora imágenes, CSS, etc.
    /// - **Orden de hooks**: El orden dentro de cada vector es el orden de aparición
    ///   en el documento (recorrido en profundidad del DOM)
    ///
    /// # Limitaciones conocidas
    ///
    /// - **Solo `id`**: No detecta hooks con `name` (atributo obsoleto en HTML5)
    /// - **Sin validación de unicidad**: No verifica que los IDs sean únicos dentro del documento
    /// - **Documentos grandes**: Puede consumir mucha memoria si hay miles de elementos con ID
    ///
    /// # Ver también
    ///
    /// - `scan_hooks` - Escanea un solo documento
    /// - [`Hook`] - Estructura que representa cada hook encontrado
    /// - [`get_item`](Self::get_item) - Obtiene información de un item del manifiesto
    /// - [HTML id attribute (MDN)](https://developer.mozilla.org/en-US/docs/Web/HTML/Global_attributes/id)
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
    /// Escanea un documento individual y extrae todos sus hooks (elementos con `id`)
    ///
    /// Este método interno parsea un archivo XHTML/HTML y busca todos los elementos
    /// que tienen un atributo `id`, devolviendo una lista de [`Hook`] encontrados.
    ///
    /// # Argumentos
    ///
    /// * `href` - Ruta relativa al documento (desde `self.opf_dir`)
    ///   Ejemplo: `"Text/capitulo1.xhtml"`
    ///
    /// # Retorna
    ///
    /// * `Result<Vec<Hook>>` - Lista de hooks encontrados en el documento,
    ///   en orden de aparición (recorrido DFS del DOM).
    ///
    /// # Errores
    ///
    /// * `GutenError::InvalidProject` - Si:
    ///   - `self.opf_dir` es `None` (proyecto no cargado)
    ///   - El archivo no existe o no se puede leer
    ///   - El archivo contiene XML mal formado
    /// * `std::io::Error` - Si falla la lectura del archivo
    ///
    /// # Ejemplo de uso interno
    ///
    /// ```ignore
    /// # use gutencore::GutenCore;
    /// # use gutencore::error::Result;
    /// # fn example() -> Result<()> {
    /// let core = GutenCore::open_folder("./mi_epub")?;
    ///
    /// // Escanear un solo documento
    /// let hooks = core.scan_hooks("Text/capitulo1.xhtml")?;
    /// for hook in hooks {
    ///     println!("ID: {}, Etiqueta: {}", hook.hook_id, hook.tag_name);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Nota
    ///
    /// Este método es **privado** y se usa internamente en [`build_hook_index`](Self::build_hook_index).
    /// Normalmente no necesitas llamarlo directamente.
    ///
    /// # Ver también
    ///
    /// - [`build_hook_index`](Self::build_hook_index) - Escanea todos los documentos
    /// - [`Hook`] - Estructura resultante  
    fn scan_hooks(&self, href: &str) -> Result<Vec<Hook>> {
        let full_path = self
            .opf_dir
            .as_ref()
            .ok_or_else(|| crate::error::GutenError::InvalidProject("OPF dir not set".to_string()))?
            .join(href);

        let content = std::fs::read_to_string(full_path)?;
        let doc = roxmltree::Document::parse(&content).map_err(|e| {
            crate::error::GutenError::InvalidProject(format!("XML error in {}: {}", href, e))
        })?;

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
