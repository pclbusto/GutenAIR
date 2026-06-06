use crate::core::GutenCore;
use crate::error::{GutenError, Result};
use html5ever::tendril::TendrilSink;
use regex::Regex;
use std::collections::HashSet;
use std::fs;

impl GutenCore {
    /// Guarda el contenido de un capítulo garantizando validez XHTML
    ///
    /// Este método es la forma segura de guardar o actualizar el contenido
    /// de un capítulo en un proyecto EPUB. Acepta contenido en formato HTML
    /// o texto plano, lo sanitiza para eliminar código peligroso, lo convierte
    /// a XHTML válido y lo guarda en la ubicación correcta del proyecto.
    ///
    /// # Proceso de guardado
    ///
    /// 1. **Validación del item** - Verifica que el ID exista en el manifiesto
    ///    y que corresponda a un documento XHTML
    ///
    /// 2. **Sanitización** - Convierte el contenido a XHTML seguro usando
    ///    [`sanitize_to_xhtml`](Self::sanitize_to_xhtml)
    ///
    /// 3. **Resolución de ruta** - Obtiene la ruta absoluta al archivo del capítulo
    ///
    /// 4. **Escritura** - Guarda el contenido sanitizado en disco
    ///
    /// # Argumentos
    ///
    /// * `id` - Identificador del item en el manifiesto (ej: `"chap1"`, `"introduction"`)
    /// * `raw_content` - Contenido a guardar (HTML, texto plano o XHTML)
    ///
    /// # Retorna
    ///
    /// * `Result<()>` - `Ok(())` si el capítulo se guarda exitosamente
    ///
    /// # Errores
    ///
    /// Este método puede retornar los siguientes errores:
    ///
    /// * `GutenError::Manifest` - Si:
    ///   - El `id` no existe en el manifiesto
    ///   - El item no es de tipo `application/xhtml+xml`
    /// * `GutenError::Other` - Si `sanitize_to_xhtml` falla
    /// * `GutenError::InvalidProject` - Si no se puede resolver la ruta del recurso
    /// * `std::io::Error` - Si falla la escritura del archivo
    ///
    /// # Ejemplo básico
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut core = GutenCore::open_folder("./mi_libro")?;
    ///
    /// // Guardar contenido HTML de un capítulo existente
    /// let contenido = r#"
    ///     <h1>Capítulo 1</h1>
    ///     <p>Esta es la historia de...</p>
    ///     <script>alert('Esto se eliminará automáticamente');</script>
    /// "#;
    ///
    /// core.save_chapter("chap1", contenido)?;
    /// println!("Capítulo guardado y sanitizado");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Ejemplo con texto plano
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut core = GutenCore::open_folder("./mi_libro")?;
    ///
    /// // El texto plano se convertirá automáticamente a XHTML con párrafos
    /// let texto_plano = "Este es el primer párrafo.\n\nY este es el segundo.";
    ///
    /// core.save_chapter("introduccion", texto_plano)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Ejemplo con contenido dinámico (generado por usuario)
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut core = GutenCore::open_folder("./blog_to_epub")?;
    ///
    /// // Contenido de un blog que puede contener HTML inseguro
    /// let comentario_usuario = r#"
    ///     <div class="post">
    ///         <h2>Mi experiencia</h2>
    ///         <p>Excelente libro! <img src="smiley.jpg" onerror="alert('XSS')"></p>
    ///         <script>malicious_code();</script>
    ///     </div>
    /// "#;
    ///
    /// // El método sanitiza automáticamente, eliminando scripts y handlers peligrosos
    /// core.save_chapter("user_story", comentario_usuario)?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Ejemplo con manejo de errores
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// # use gutencore::error::GutenError;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut core = GutenCore::open_folder("./mi_libro")?;
    ///
    /// // Intentar guardar con un ID inválido
    /// match core.save_chapter("id_inexistente", "<p>Contenido</p>") {
    ///     Ok(_) => println!("Guardado exitoso"),
    ///     Err(GutenError::Manifest(msg)) => {
    ///         eprintln!("Error en el manifiesto: {}", msg);
    ///         // Imprime: "id_inexistente not found in manifest"
    ///     }
    ///     Err(e) => eprintln!("Otro error: {}", e),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Sanitización aplicada
    ///
    /// El método `sanitize_to_xhtml` (que internamente llama a este) realiza:
    ///
    /// 1. **Limpieza HTML** - Elimina scripts, iframes, objetos embebidos
    /// 2. **Eliminación de handlers de eventos** - `onclick`, `onload`, etc.
    /// 3. **Validación de URLs** - Elimina `javascript:` y `data:` peligrosos
    /// 4. **Conversión a XHTML** - Asegura etiquetas bien formadas y encoding UTF-8
    /// 5. **Estructura completa** - Agrega `<html>`, `<head>`, `<body>` si falta
    ///
    /// # Notas importantes
    ///
    /// - **Sobrescritura**: Si el capítulo ya existía, **se sobrescribe** completamente
    /// - **Backup automático**: No se crea backup. Considera guardar una copia antes
    /// - **Rendimiento**: Para capítulos muy grandes (>10MB), considera dividirlos
    /// - **Sanitización por defecto**: Siempre se aplica, no hay opción para desactivarla
    ///
    /// # Advertencias de seguridad
    ///
    /// - **Confía pero verifica**: Aunque se aplica sanitización, siempre revisa
    ///   el resultado si el contenido es crítico
    /// - **No usar con datos binarios**: Este método es solo para texto HTML/XHTML
    /// - **Inyección indirecta**: La sanitización no protege contra ataques
    ///   en atributos `style` o `class` personalizados
    ///
    /// # Ver también
    ///
    /// - [`get_item`](Self::get_item) - Obtiene un item del manifiesto
    /// - [`sanitize_to_xhtml`](Self::sanitize_to_xhtml) - Método de sanitización interno
    /// - [`get_resource_path`](Self::get_resource_path) - Resuelve ruta absoluta de un recurso
    /// - [`save`](Self::save) - Guarda metadatos, no contenido de capítulos
    /// - [EPUB XHTML Content Documents](https://www.w3.org/TR/epub/#sec-xhtml) - Especificación
    pub fn save_chapter(&mut self, id: &str, raw_content: &str) -> Result<()> {
        let item = self.get_item(id)?;
        if item.media_type != "application/xhtml+xml" {
            return Err(GutenError::Manifest(format!(
                "{} is not an XHTML document",
                id
            )));
        }

        let clean_xhtml = self.sanitize_to_xhtml(id, raw_content)?;
        let full_path = self.get_resource_path(id)?;

        fs::write(&full_path, &clean_xhtml)?;

        if let Some(db) = &self.index_db {
            if db.index_xhtml(id, &clean_xhtml).is_err() {
                self.index_dirty = true;
            }
        }

        Ok(())
    }

    /// Importa un archivo Markdown, lo convierte a XHTML y lo guarda en el proyecto
    ///
    /// Este método permite integrar contenido escrito en Markdown (.md) directamente
    /// en el EPUB. Realiza la conversión a HTML y luego aplica todo el proceso
    /// de sanitización e inyección de estilos de GutenAIR.
    ///
    /// # Proceso
    /// 1. Lee el archivo Markdown desde el disco.
    /// 2. Lo convierte a HTML usando `pulldown-cmark`.
    /// 3. Pasa el HTML resultante por `sanitize_to_xhtml`.
    /// 4. Escribe el XHTML final en la ruta del recurso.
    ///
    /// # Argumentos
    /// * `id` - ID del capítulo en el manifiesto donde se guardará el contenido.
    /// * `md_path` - Ruta al archivo .md local.
    ///
    /// # Errores
    /// * `GutenError::Io` - Si no se puede leer el archivo .md.
    /// * `GutenError::Manifest` - Si el ID no existe o no es un documento XHTML.
    pub fn import_markdown_as_chapter(&mut self, id: &str, md_path: impl AsRef<std::path::Path>) -> Result<()> {
        let md_content = fs::read_to_string(md_path)?;
        
        // Convertir MD a HTML
        let parser = pulldown_cmark::Parser::new(&md_content);
        let mut html_output = String::new();
        pulldown_cmark::html::push_html(&mut html_output, parser);
        
        // Guardar usando la lógica existente de sanitización
        self.save_chapter(id, &html_output)
    }

    /// Importa un archivo XHTML o HTML externo y lo guarda en el proyecto
    ///
    /// Este método lee un archivo del disco, lo pasa por el proceso de sanitización,
    /// inyección de estilos y validación de clases de GutenAIR, y lo guarda en la 
    /// ubicación correcta dentro de `OEBPS/Text/`.
    ///
    /// # Argumentos
    /// * `id` - ID del capítulo en el manifiesto donde se guardará el contenido.
    /// * `path` - Ruta al archivo .xhtml o .html externo.
    ///
    /// # Errores
    /// * `GutenError::Io` - Si no se puede leer el archivo.
    /// * `GutenError::Manifest` - Si el ID no existe o no es de tipo XHTML.
    /// * `GutenError::Other` - Si el archivo contiene clases CSS no registradas en el proyecto.
    pub fn import_xhtml_as_chapter(&mut self, id: &str, path: impl AsRef<std::path::Path>) -> Result<()> {
        let content = fs::read_to_string(path)?;
        self.save_chapter(id, &content)
    }

    /// Limpia HTML y produce un documento XHTML completo y estrictamente válido
    ///
    /// Este método es el corazón del procesamiento de contenido HTML en `gutencore`.
    /// Toma HTML potencialmente inseguro o mal formado y lo transforma en un
    /// documento XHTML válido, listo para ser incluido en un EPUB.
    ///
    /// # Proceso completo de transformación
    ///
    /// El método aplica una serie de transformaciones en orden:
    ///
    /// 1. **Limpieza de seguridad** - Usa `ammonia` para eliminar:
    ///    - Etiquetas peligrosas (`<script>`, `<iframe>`, `<object>`)
    ///    - Handlers de eventos (`onclick`, `onload`, `onerror`)
    ///    - URLs peligrosas (`javascript:`, `data:`)
    ///    - Comentarios HTML y CDATA no seguros
    ///
    /// 2. **Parseo y corrección estructural** - Usa `html5ever` para:
    ///    - Corregir etiquetas mal anidadas
    ///    - Cerrar etiquetas huérfanas
    ///    - Normalizar la estructura del DOM
    ///
    /// 3. **Conversión a XHTML** - Convierte elementos void HTML5 a formato self-closing:
    ///    - `<br>` → `<br/>`
    ///    - `<img>` → `<img/>`
    ///    - `<hr>` → `<hr/>`
    ///    - Entre otros (ver la función auxiliar html5_to_xhtml_void_elements)
    ///
    /// 4. **Extracción del cuerpo** - Extrae solo el contenido de `<body>`
    ///    para envolverlo en una plantilla XHTML limpia
    ///
    /// 5. **Inyección selectiva de CSS** - Agrega enlaces a las hojas de estilo
    ///    aplicables a este capítulo (según `default_styles` o `exceptions`)
    ///
    ///    La inyección sigue estas reglas:
    ///    - Solo inyecta si `self.config.auto_inject == true`
    ///    - Consulta la lista de estilos vía [`get_chapter_styles`](Self::get_chapter_styles)
    ///    - Busca cada ID en el `manifest` para obtener el `href`
    ///    - Los CSS se inyectan en el orden exacto definido
    ///    - Los IDs que no existen en el manifiesto se ignoran silenciosamente
    ///
    /// 6. **Inyección de idioma** - Establece los atributos `lang` y `xml:lang`
    ///    usando el idioma de los metadatos del libro
    ///
    /// 7. **Ensamblaje final** - Genera un documento XHTML completo con:
    ///    - Declaración XML
    ///    - Namespace XHTML correcto
    ///    - Meta charset UTF-8
    ///    - Enlaces CSS (en orden configurado)
    ///    - Contenido sanitizado en el cuerpo
    ///
    /// # Configuración de inyección CSS
    ///
    /// El método respeta dos opciones de configuración:
    ///
    /// | Configuración | Tipo | Efecto |
    /// |---------------|------|--------|
    /// | `auto_inject` | `bool` | Si `true`, inyecta los CSS; si `false`, no inyecta ninguno |
    /// | `default_styles` | `Vec<String>` | Lista de IDs de CSS a inyectar (respeta orden) |
    ///
    /// # Ejemplo de configuración
    ///
    /// ```json
    /// {
    ///   "default_styles": ["reset", "main", "theme-dark", "print"],
    ///   "auto_inject": true
    /// }
    /// ```
    ///
    /// En este caso, se inyectarán los CSS con IDs `reset`, `main`, `theme-dark` y `print`
    /// en ese orden específico.
    ///
    /// # Importancia del orden de CSS
    ///
    /// El orden en `default_styles` determina el orden de los `<link>` en el `<head>`:
    ///
    /// ```html
    /// <!-- Si default_styles = ["reset", "main", "theme"] -->
    /// <link rel="stylesheet" href="../Styles/reset.css"/>
    /// <link rel="stylesheet" href="../Styles/main.css"/>
    /// <link rel="stylesheet" href="../Styles/theme.css"/>
    /// ```
    ///
    /// Esto es **crítico** para la cascada CSS: las reglas posteriores sobrescriben
    /// a las anteriores. Por ejemplo:
    /// - `reset.css` normaliza estilos
    /// - `main.css` define estilos base
    /// - `theme.css` aplica tema específico (sobrescribe main.css si es necesario)
    ///
    /// # Manejo de CSS faltantes
    ///
    /// Si un ID en `default_styles` no existe en el manifiesto, simplemente se ignora:
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// // Suponiendo: default_styles = ["style", "inexistente", "theme"]
    /// // El manifiesto tiene "style" y "theme", pero no "inexistente"
    ///
    /// // Resultado: solo se inyectan "style" y "theme" (en ese orden)
    /// // "inexistente" se ignora silenciosamente (sin error)
    /// ```
    ///
    /// # Argumentos
    ///
    /// * `id` - ID del capítulo (para consultar excepciones de estilo)
    /// * `html` - Cadena HTML a sanitizar (puede estar mal formada o contener código peligroso)
    ///
    /// # Retorna
    ///
    /// * `Result<String>` - Documento XHTML completo y válido, o un error si falla la serialización
    ///
    /// # Errores
    ///
    /// * `quick_xml::Error` - Si falla la serialización del DOM a XML
    /// * Otros errores de parseo - Propagados desde `html5ever`
    ///
    /// # Ejemplo básico
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let core = GutenCore::open_folder("./mi_epub")?;
    ///
    /// let html_sucio = r#"
    ///     <div>
    ///         <h1>Mi Título</h1>
    ///         <p>Texto normal</p>
    ///         <script>alert('XSS');</script>
    ///         <img src="x" onerror="malicious()">
    ///         <br>
    ///     </div>
    /// "#;
    ///
    /// let xhtml = core.sanitize_to_xhtml("chap1", html_sucio)?;
    ///
    /// // El resultado es XHTML válido, sin scripts ni handlers
    /// assert!(xhtml.contains("<h1>Mi Título</h1>"));
    /// assert!(!xhtml.contains("<script>"));
    /// assert!(!xhtml.contains("onerror"));
    /// assert!(xhtml.contains("<br/>"));  // Convertido a self-closing
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Ejemplo: Corrección de HTML mal formado
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let core = GutenCore::open_folder("./mi_epub")?;
    ///
    /// let html_mal_formado = r#"
    ///     <p>Texto con <strong>negrita sin cerrar
    ///     <p>Otro párrafo</p>
    /// "#;
    ///
    /// let xhtml = core.sanitize_to_xhtml("chap1", html_mal_formado)?;
    ///
    /// // html5ever corrige automáticamente la estructura
    /// assert!(xhtml.contains("<strong>negrita sin cerrar</strong>"));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Ejemplo: Inyección selectiva de CSS
    ///
    /// Suponiendo que tu configuración tiene `default_styles = ["main", "print"]`
    /// y no hay excepciones para "capitulo1":
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let core = GutenCore::open_folder("./mi_epub")?;
    ///
    /// // El manifiesto contiene CSS como:
    /// // - "Styles/main.css" (id="main")
    /// // - "Styles/print.css" (id="print")
    /// // - "Styles/legacy.css" (id="legacy") - NO inyectado porque no está en la lista aplicada
    ///
    /// let xhtml = core.sanitize_to_xhtml("capitulo1", "<p>Hola</p>")?;
    ///
    /// // El resultado incluye solo los CSS configurados, en orden:
    /// assert!(xhtml.contains(r#"<link rel="stylesheet" type="text/css" href="../Styles/main.css"/>"#));
    /// assert!(xhtml.contains(r#"<link rel="stylesheet" type="text/css" href="../Styles/print.css"/>"#));
    /// assert!(!xhtml.contains("legacy.css"));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Ejemplo: Deshabilitar inyección CSS
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    ///
    /// // Deshabilitar inyección automática
    /// core.config.auto_inject = false;
    ///
    /// let xhtml = core.sanitize_to_xhtml("chap1", "<p>Sin CSS</p>")?;
    ///
    /// // El <head> solo tendrá <meta charset="utf-8"/>, sin enlaces CSS
    /// assert!(!xhtml.contains("<link"));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Estructura del documento generado
    ///
    /// ```xml
    /// <?xml version="1.0" encoding="UTF-8"?>
    /// <html xmlns="http://www.w3.org/1999/xhtml" lang="es" xml:lang="es">
    /// <head>
    ///   <meta charset="utf-8"/>
    ///   <link rel="stylesheet" type="text/css" href="../Styles/reset.css"/>
    ///   <link rel="stylesheet" type="text/css" href="../Styles/main.css"/>
    ///   <link rel="stylesheet" type="text/css" href="../Styles/theme.css"/>
    /// </head>
    /// <body>
    ///   <!-- Contenido sanitizado aquí -->
    /// </body>
    /// </html>
    /// ```
    ///
    /// # Características especiales
    ///
    /// - **Elementos self-closing**: Los elementos vacíos HTML (`<br>`, `<hr>`, `<img>`)
    ///   se convierten automáticamente a formato XHTML (`<br/>`, `<hr/>`, `<img/>`)
    /// - **Inyección selectiva de CSS**: Solo inyecta los CSS configurados en
    ///   `default_styles` (no todos los del manifiesto)
    /// - **Orden respetado**: Los CSS se inyectan en el orden exacto definido globalmente o por excepción
    /// - **Inyección opcional**: Se puede deshabilitar con `auto_inject = false`
    /// - **Estilos por capítulo**: Usa [`get_chapter_styles`](Self::get_chapter_styles) para decidir qué inyectar.
    /// - **Rutas CSS relativas**: Los enlaces CSS usan `../` como prefijo porque:
    ///   - Los archivos XHTML están en `OEBPS/Text/`
    ///   - Los CSS están en `OEBPS/Styles/`
    ///   - La ruta relativa correcta es `../Styles/archivo.css`
    /// - **Idioma heredado**: Toma el idioma de `self.metadata.language` o usa `"en"` por defecto
    /// - **Encoding UTF-8**: Siempre se declara UTF-8 (estándar EPUB)
    ///
    /// # Notas de implementación
    ///
    /// - **`ammonia`**: Proporciona la sanitización de seguridad (configuración por defecto)
    /// - **`html5ever`**: Motor de parseo HTML del navegador Servo, garantiza
    ///   corrección estructural incluso con HTML mal formado
    /// - **`html5_to_xhtml_void_elements`**: Función auxiliar que convierte
    ///   elementos void HTML (`<br>`, `<img>`) a formato XHTML self-closing
    /// - **`extract_body`**: Función auxiliar que extrae solo el contenido de `<body>`
    /// - **Búsqueda por ID**: Los CSS se resuelven buscando el ID en `manifest`,
    ///   no la ruta directa
    ///
    /// # Limitaciones conocidas
    ///
    /// - **CSS solo por ID**: No se pueden inyectar CSS por ruta, solo por ID del manifiesto
    /// - **Sin validación de CSS**: No verifica que los archivos CSS existan realmente
    /// - **Rutas fijas**: Asume que los XHTML están en `Text/` y los CSS en `Styles/`
    /// - **Sin soporte de JavaScript**: Todo JS se elimina (por diseño, EPUB no debe tener JS)
    /// - **CSS faltantes se ignoran**: Si un ID en `default_styles` no existe, se ignora silenciosamente
    ///
    /// # Advertencias de seguridad
    ///
    /// - **No es infalible**: Aunque `ammonia` es muy seguro, revisa siempre
    ///   contenido crítico después de la sanitización
    /// - **Atributos `style`**: Los CSS inline se conservan, lo que puede ser
    ///   vector de ataques en algunos navegadores antiguos
    /// - **Contenido externo**: Si el HTML incluye imágenes externas, los `src`
    ///   se conservan. Considera descargar y embeber imágenes localmente
    ///
    /// # Ver también
    ///
    /// - [`clean_html`](Self::clean_html) - Versión simple sin estructura XHTML
    /// - [`save_chapter`](Self::save_chapter) - Usa este método internamente
    /// - [`text_to_xhtml`](Self::text_to_xhtml) - Para convertir texto plano
    /// - [`crate::types::GutenConfig`] - Configuración de inyección CSS
    /// - [Ammonia documentation](https://docs.rs/ammonia)
    /// - [html5ever documentation](https://docs.rs/html5ever)
    /// - [EPUB XHTML Content Documents](https://www.w3.org/TR/epub/#sec-xhtml)
    pub fn sanitize_to_xhtml(&self, id: &str, html: &str) -> Result<String> {
        // 1. Strip dangerous tags/JS but allow standard attributes like class and id
        let cleaned = ammonia::Builder::default()
            .add_generic_attributes(&["class", "id", "title"])
            .add_tags(&["html", "head", "body", "title", "meta", "link"])
            .clean(html)
            .to_string();

        // 2. Parse to fix malformed structure (auto-closes orphan tags, etc.)
        let dom =
            html5ever::parse_document(markup5ever_rcdom::RcDom::default(), Default::default())
                .one(cleaned);

        // 3. Serialize back to a string
        let mut bytes = Vec::new();
        let serializable: markup5ever_rcdom::SerializableHandle = dom.document.clone().into();
        html5ever::serialize(&mut bytes, &serializable, Default::default())?;
        let serialized_html = String::from_utf8_lossy(&bytes);

        // 4. Extract just the <body> content so we can wrap it in our own template
        let body_content = extract_body(&serialized_html);

        // 5. Validation (The Guardian): Verify that used classes exist in the linked CSS files
        if self.config.auto_inject {
            let catalogs = self.get_style_catalog(id)?;
            let mut valid_classes = HashSet::new();
            for cat in catalogs {
                for style in cat.estilos.bloque {
                    valid_classes.insert(style.clase);
                }
                for style in cat.estilos.linea {
                    valid_classes.insert(style.clase);
                }
            }

            // Solo validamos si realmente hay estilos definidos en los CSS vinculados.
            // Si valid_classes está vacío, significa que no hay reglas CSS con clases,
            // por lo que permitimos cualquier clase para no bloquear la importación.
            if !valid_classes.is_empty() {
                // Simple regex to find all class="..." attributes in the body
                let class_re = regex::Regex::new(r#"class="([^"]+)""#).unwrap();
                for cap in class_re.captures_iter(&body_content) {
                    let used_classes_attr = cap.get(1).unwrap().as_str();
                    for used_class in used_classes_attr.split_whitespace() {
                        if !valid_classes.contains(used_class) {
                            return Err(GutenError::Other(format!(
                                "CSS Validation Error: Class '{}' used in chapter '{}' does not exist in linked stylesheets. Valid classes are: {:?}",
                                used_class, id, valid_classes
                            )));
                        }
                    }
                }
            }
        }

        // 6. Collect CSS hrefs (respecting exceptions, order and auto_inject flag)
        let mut css_links = Vec::new();
        if self.config.auto_inject {
            let styles_to_inject = self.get_chapter_styles(id);
            for css_id in &styles_to_inject {
                if let Some(it) = self.manifest.get(css_id) {
                    if it.media_type == "text/css" {
                        css_links.push(format!(
                            r#"  <link rel="stylesheet" type="text/css" href="../{}"/>"#,
                            it.href
                        ));
                    }
                }
            }
        }

        // 7. Get lang from metadata
        let lang = self
            .metadata
            .as_ref()
            .map(|m| m.language.as_str())
            .unwrap_or("en");

        // 8. Extract title or use ID as fallback
        let title_re = regex::Regex::new(r"(?i)<title>(.*?)</title>").unwrap();
        let title = title_re
            .captures(html)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_else(|| id.to_string());

        // 9. Assemble a well-formed XHTML document using the central builder
        let head_links = if css_links.is_empty() {
            String::new()
        } else {
            format!("\n{}", css_links.join("\n"))
        };

        let xhtml = GutenCore::build_xhtml(lang, &title, &head_links, &body_content);
        
        // 10. Final pass to ensure all void elements are XHTML self-closing
        Ok(html5_to_xhtml_void_elements(&xhtml))
    }
}

/// Convierte elementos void de HTML5 a formato self-closing de XHTML
///
/// `html5ever` serializa elementos void (como `<br>`, `<img>`, `<input>`)
/// sin la barra de cierre, lo que **rompe los parsers XML** como `roxmltree`.
/// Esta función corrige ese problema transformando los elementos void al
/// formato XHTML estándar con barra de cierre (`<br/>`, `<img/>`, etc.).
///
/// # ¿Qué son elementos void?
///
/// En HTML5, los elementos void son aquellos que **no pueden tener contenido**
/// ni etiqueta de cierre. Ejemplos comunes:
///
/// | Elemento | HTML5 | XHTML | Uso típico |
/// |----------|-------|-------|------------|
/// | `<br>`   | `<br>` | `<br/>` | Salto de línea |
/// | `<img>`  | `<img src="...">` | `<img src="..."/>` | Imagen |
/// | `<hr>`   | `<hr>` | `<hr/>` | Línea horizontal |
/// | `<link>` | `<link href="...">` | `<link href="..."/>` | CSS, íconos |
/// | `<meta>` | `<meta charset="...">` | `<meta charset="..."/>` | Metadatos |
/// | `<input>`| `<input type="...">` | `<input type="..."/>` | Campos de formulario |
///
/// # Transformaciones realizadas
///
/// La función maneja dos casos principales:
///
/// 1. **Elementos sin atributos** (ej: `<br>`):
///    ```text
///    <br>  →  <br/>
///    <hr>  →  <hr/>
///    ```
///
/// 2. **Elementos con atributos** (ej: `<img src="foto.jpg">`):
///    ```text
///    <img src="foto.jpg">  →  <img src="foto.jpg"/>
///    <link rel="stylesheet" href="style.css">  →  <link rel="stylesheet" href="style.css"/>
///    ```
///
/// 3. **Elementos ya correctos** (con `/>`):
///    ```text
///    <br/>   →   <br/>   (sin cambios)
///    <img src="x"/>  →  <img src="x"/>  (sin cambios)
///    ```
///
/// # Argumentos
///
/// * `html` - Cadena HTML serializada por `html5ever` (puede contener elementos void en formato HTML5)
///
/// # Retorna
///
/// * `String` - Mismo HTML pero con elementos void convertidos a formato XHTML self-closing
///
/// # Ejemplo básico
///
/// ```rust
/// # use gutencore::guardian::html5_to_xhtml_void_elements;
/// let html5 = r#"<p>Línea 1<br>Línea 2</p>
/// <img src="foto.jpg" alt="Descripción">
/// <hr>
/// <link href="style.css" rel="stylesheet">"#;
///
/// let xhtml = html5_to_xhtml_void_elements(html5);
///
/// assert_eq!(xhtml, r#"<p>Línea 1<br/>Línea 2</p>
/// <img src="foto.jpg" alt="Descripción"/>
/// <hr/>
/// <link href="style.css" rel="stylesheet"/>"#);
/// ```
///
/// # Ejemplo con elementos mixtos
///
/// ```rust
/// # use gutencore::guardian::html5_to_xhtml_void_elements;
/// let input = r#"
/// <div>
///   <input type="text" name="nombre">
///   <br>
///   <meta charset="utf-8">
///   <br/>
///   <input type="checkbox" checked>
/// </div>"#;
///
/// let output = html5_to_xhtml_void_elements(input);
///
/// let expected = r#"
/// <div>
///   <input type="text" name="nombre"/>
///   <br/>
///   <meta charset="utf-8"/>
///   <br/>
///   <input type="checkbox" checked/>
/// </div>"#;
///
/// assert_eq!(output, expected);
/// ```
///
/// # Lista completa de elementos procesados
///
/// La función convierte todos los elementos void del estándar HTML5:
///
/// ```text
/// area, base, br, col, embed, hr, img, input,
/// link, meta, param, source, track, wbr
/// ```
///
/// # ¿Por qué es necesaria esta función?
///
/// ```text
/// html5ever (HTML5 parser)  →  <br>, <img>   (formato HTML5)
///                             ↓
///                    ¡XML Parser se rompe!
///                             ↓
///            roxmltree espera  →  <br/>, <img/>  (formato XHTML)
/// ```
///
/// `roxmltree` (y cualquier parser XML conforme) requiere que los elementos
/// vacíos se marquen explícitamente con `/>`. Sin esta conversión, el EPUB
/// resultante sería inválido y no podría ser leído por lectores de EPUB.
///
/// # Notas de implementación
///
/// - **Algoritmo**: La función utiliza expresiones regulares para identificar
///   elementos void y asegurar que terminen con la barra de cierre `/>`.
/// - **Preserva atributos**: Todos los atributos existentes se mantienen intactos.
/// - **No sobre-convierte**: La regex detecta si el elemento ya tiene `/>` para
///   evitar duplicaciones (evita generar `<br//>`).
/// - **Robustez**: Maneja correctamente cualquier tipo de espacio en blanco
///   (espacios, pestañas, saltos de línea) entre el nombre de la etiqueta y
///   sus atributos o el cierre.
/// - **Case-insensitive**: Procesa etiquetas tanto en minúsculas como en
///   mayúsculas (ej: `<BR>` → `<BR/>`).
///
/// # Limitaciones conocidas
///
/// - **No es un parser de estados**: Aunque es muy robusto para el uso en EPUB,
///   una regex no puede manejar todos los casos teóricos de HTML5 (como `>`
///   dentro de un valor de atributo entrecomillado). En la práctica, con el
///   output de `html5ever`, esto no es un problema.
///
/// # Uso típico
///
/// Esta función se usa internamente en [`GutenCore::sanitize_to_xhtml`](crate::core::GutenCore::sanitize_to_xhtml):
///
/// ```ignore
/// let dom = html5ever::parse_document(...);
/// let mut bytes = Vec::new();
/// html5ever::serialize(&mut bytes, &dom, Default::default())?;
/// let html = String::from_utf8_lossy(&bytes);
/// let xhtml = html5_to_xhtml_void_elements(&html);
/// ```
///
/// # Ver también
///
/// - [`GutenCore::sanitize_to_xhtml`](crate::core::GutenCore::sanitize_to_xhtml) - Método principal que usa esta función
/// - [HTML Void Elements (MDN)](https://developer.mozilla.org/en-US/docs/Glossary/Void_element)
/// - [XHTML Empty Elements](https://www.w3.org/TR/xhtml1/#C_2)
/// - [html5ever documentation](https://docs.rs/html5ever)
pub fn html5_to_xhtml_void_elements(html: &str) -> String {
const VOID: &[&str] = &[        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
        "source", "track", "wbr",
    ];

    // Regex explanation:
    // (?i) : case-insensitive
    // <(area|base|...) : match any of the void tags
    // (\b[^>]*?)? : match any attributes (non-greedy), starting with a word boundary to avoid partial matches
    // \s* : optional whitespace before closing
    // /?> : match optional closing slash and then the closing bracket
    let pattern = format!(r"(?i)<({})(\b[^>]*?)?\s*/?>", VOID.join("|"));
    let re = Regex::new(&pattern).unwrap();

    re.replace_all(html, |caps: &regex::Captures| {
        let tag = caps.get(1).unwrap().as_str();
        let attrs = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        // Remove any existing trailing slash to avoid double slashes like <br//>
        let clean_attrs = attrs.trim_end_matches('/');
        format!("<{}{}/>", tag, clean_attrs)
    })
    .to_string()
}

/// Extrae el contenido dentro de `<body>...</body>`, o retorna la cadena completa si no se encuentra
///
/// Esta función auxiliar toma un documento HTML y extrae exclusivamente el
/// contenido que está dentro de la etiqueta `<body>`. Es útil cuando se tiene
/// un documento HTML completo (generado por `html5ever`) pero solo se desea
/// el contenido del cuerpo para insertarlo en una plantilla XHTML personalizada.
///
/// # Comportamiento
///
/// La función maneja dos formatos posibles de la etiqueta `<body>`:
///
/// 1. **`<body>` sin atributos** - Busca la etiqueta exacta `<body>`
/// 2. **`<body>` con atributos** - Busca `<body ` seguido de atributos
///
/// # Algoritmo
///
/// 1. Busca la etiqueta de apertura `<body>` (con o sin atributos)
/// 2. Localiza el cierre `>` de esa etiqueta
/// 3. Busca la etiqueta de cierre `</body>`
/// 4. Extrae el texto entre ambas posiciones
/// 5. Aplica `trim()` para eliminar espacios en blanco innecesarios
///
/// Si no se encuentra `<body>`, retorna el HTML original (sin modificar).
///
/// # Argumentos
///
/// * `html` - Cadena HTML que puede contener o no etiquetas `<body>`
///
/// # Retorna
///
/// * `String` - Contenido del `<body>` si existe, o el HTML original si no
///
/// # Ejemplo básico
///
/// ```rust
/// # use gutencore::guardian::extract_body;
/// let html = r#"<html>
/// <head><title>Test</title></head>
/// <body>
///   <h1>Mi Título</h1>
///   <p>Contenido del cuerpo.</p>
/// </body>
/// </html>"#;
///
/// let body = extract_body(html);
/// assert!(body.contains("<h1>Mi Título</h1>"));
/// assert!(body.contains("<p>Contenido del cuerpo.</p>"));
/// ```
///
/// # Ejemplo con atributos en `<body>`
///
/// ```rust
/// # use gutencore::guardian::extract_body;
/// let html = r#"<html>
/// <body class="main" id="content" data-theme="light">
///   <p>Texto con atributos en body</p>
/// </body>
/// </html>"#;
///
/// let body = extract_body(html);
/// assert!(body.contains("<p>Texto con atributos en body</p>"));
/// ```
///
/// # Ejemplo sin etiqueta `<body>`
///
/// ```rust
/// # use gutencore::guardian::extract_body;
/// let fragmento = "<p>Esto es solo un fragmento HTML</p>";
///
/// let body = extract_body(fragmento);
/// // Como no hay etiqueta <body>, retorna el original
/// assert_eq!(body, "<p>Esto es solo un fragmento HTML</p>");
/// ```
///
/// # Ejemplo con HTML anidado complejo
///
/// ```rust
/// # use gutencore::guardian::extract_body;
/// let html = r#"<!DOCTYPE html>
/// <html xmlns="http://www.w3.org/1999/xhtml">
/// <head>
///   <meta charset="utf-8"/>
///   <title>Página Compleja</title>
/// </head>
/// <body id="top" class="dark-mode">
///   <header>
///     <h1>Bienvenido</h1>
///   </header>
///   <main>
///     <article>
///       <p>Contenido principal</p>
///       <aside>Nota al margen</aside>
///     </article>
///   </main>
///   <footer>Pie de página</footer>
/// </body>
/// </html>"#;
///
/// let body = extract_body(html);
///
/// assert!(body.contains("<header>"));
/// assert!(body.contains("<h1>Bienvenido</h1>"));
/// assert!(body.contains("<p>Contenido principal</p>"));
/// assert!(body.contains("<footer>Pie de página</footer>"));
/// assert!(!body.contains("<head>"));
/// assert!(!body.contains("<title>"));
/// ```
///
/// # Ejemplo con body vacío
///
/// ```rust
/// # use gutencore::guardian::extract_body;
/// let html = "<html><body></body></html>";
/// let body = extract_body(html);
/// assert_eq!(body, "");  // String vacío, no espacios
/// ```
///
/// # Casos extremos
///
/// | Entrada | Salida | Razón |
/// |---------|--------|-------|
/// | `<body></body>` | `""` (vacío) | Cuerpo sin contenido |
/// | `<body>   </body>` | `""` (vacío) | `trim()` elimina espacios |
/// | `<body><p>texto</body>` | `<p>texto` | Funciona aunque falte cierre (caso inválido) |
/// | `<body>` sin cierre | Todo después de `<body>` | `rfind` retorna `None` → HTML original |
/// | `texto sin etiquetas` | `texto sin etiquetas` | Sin `<body>` → original |
///
/// # Notas de implementación
///
/// - **Búsqueda flexible**: Usa `or_else` para encontrar `<body>` con o sin atributos
/// - **Posición después de `>`**: Calcula el índice correcto saltándose el `>` de apertura
/// - **Fallback seguro**: Si no encuentra `</body>`, retorna el HTML original
/// - **`trim()` automático**: Elimina espacios en blanco alrededor del contenido
/// - **Sin parseo XML**: Esta función es intencionalmente simple (basada en strings)
///   para evitar dependencias adicionales. Para casos complejos, confiamos en
///   que `html5ever` ya produjo HTML bien formado.
///
/// # Limitaciones
///
/// - **No es un parser XML**: Usa búsqueda simple de strings, no un parser completo.
///   Esto es suficiente porque `html5ever` ya garantiza etiquetas balanceadas.
/// - **No soporta `<body>` con `>` en atributos**: Si un atributo contiene el
///   carácter `>` (extremadamente raro), el cálculo de `tag_end` podría fallar.
/// - **Case-sensitive**: Solo reconoce `<body>` en minúsculas. `html5ever`
///   normaliza a minúsculas, así que es seguro.
/// - **No procesa etiquetas anidadas**: No es necesario, solo extrae contenido
///   entre la primera apertura y el último cierre.
///
/// # Uso típico
///
/// Esta función se usa en [`sanitize_to_xhtml`](crate::core::GutenCore::sanitize_to_xhtml):
///
/// ```ignore
/// let dom = html5ever::parse_document(...);
/// let serialized = serialize_to_string(&dom);
/// let body_content = extract_body(&serialized);
/// let final_xhtml = format!("<html>...<body>{}</body></html>", body_content);
/// ```
///
/// # Ver también
///
/// - [`sanitize_to_xhtml`](crate::core::GutenCore::sanitize_to_xhtml) - Método principal que usa esta función
/// - `html5_to_xhtml_void_elements` - Otra función auxiliar relacionada (función interna)
/// - [html5ever documentation](https://docs.rs/html5ever)
pub fn extract_body(html: &str) -> String {
let start = html.find("<body>").or_else(|| html.find("<body "));    let end = html.rfind("</body>");

    match (start, end) {
        (Some(s), Some(e)) => {
            // skip past the closing ">" of the opening <body> tag
            let after_tag = &html[s..];
            let tag_end = after_tag.find('>').map(|i| s + i + 1).unwrap_or(s + 6);
            html[tag_end..e].trim().to_string()
        }
        _ => html.trim().to_string(),
    }
}
