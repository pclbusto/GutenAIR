//! # EPUB Core Module
//!
//! Constantes y estructuras fundamentales para manipular archivos EPUB 3.0.
//!
//! ## Namespaces XML
//! Las constantes `NS_*` definen los namespaces estándar usados en los archivos
//! de configuración EPUB.
//!
//! ## GutenCore
//! La estructura principal que representa un proyecto EPUB cargado en memoria.

use crate::error::{GutenError, Result};
use crate::types::*;
use chrono::{SecondsFormat, Utc};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Namespace OPF (Open Packaging Format) - elemento raíz de content.opf
pub const NS_OPF: &str = "http://www.idpf.org/2007/opf";

/// Namespace Dublin Core - metadatos como título, autor, idioma
pub const NS_DC: &str = "http://purl.org/dc/elements/1.1/";

/// Namespace OCF (Open Container Format) - usado en META-INF/container.xml
pub const NS_OCF: &str = "urn:oasis:names:tc:opendocument:xmlns:container";

/// Representa un proyecto EPUB completo cargado en memoria
///
/// `GutenCore` es el punto de entrada principal para trabajar con archivos EPUB.
/// Almacena toda la estructura del libro después de parsear el archivo OPF.
///
/// # Ejemplo
///
/// ```no_run
/// # use guten_core::GutenCore;
/// let mut core = GutenCore::open_folder("mi_epub/")?;
///
/// // Acceder al directorio de trabajo
/// println!("Raíz: {}", core.workdir.display());
///
/// // Verificar si hay metadatos
/// if let Some(meta) = &core.metadata {
///     println!("Título: {}", meta.title);
/// }
///
/// // Recorrer el orden de lectura
/// for id in &core.spine {
///     if let Some(item) = core.manifest.get(id) {
///         println!("Archivo: {}", item.href);
///     }
/// }
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub struct GutenCore {
    /// Directorio raíz del proyecto EPUB descomprimido
    ///
    /// Esta es la carpeta que contiene `META-INF/` y `mimetype`.
    /// Todas las rutas son relativas a este directorio.
    pub workdir: PathBuf,

    /// Ruta completa al archivo OPF (Package Document)
    ///
    /// Normalmente es `workdir/OEBPS/content.opf`.
    /// Es `None` hasta que se carga exitosamente con `load_container_and_opf()`.
    pub opf_path: Option<PathBuf>,

    /// Directorio que contiene el archivo OPF
    ///
    /// Útil para resolver rutas relativas de los items en el manifiesto.
    /// Generalmente es `workdir/OEBPS/`.
    pub opf_dir: Option<PathBuf>,

    /// Metadatos del libro según el estándar Dublin Core
    ///
    /// Incluye título, idioma, identificador único y fecha de modificación.
    /// Es `None` hasta que se parsea exitosamente el OPF.
    pub metadata: Option<BookMetadata>,

    /// Manifiesto de recursos del EPUB
    ///
    /// Mapea un ID único (String) a cada item del EPUB:
    /// - Archivos XHTML de contenido
    /// - Hojas de estilo CSS
    /// - Imágenes
    /// - El archivo de navegación (nav)
    ///
    /// Los IDs se definen en el archivo OPF y deben ser únicos.
    pub manifest: HashMap<String, ManifestItem>,

    /// Orden de lectura (Spine) del EPUB
    ///
    /// Lista ordenada de IDs que referencia items del `manifest`.
    /// Define el orden secuencial en que un lector de EPUB debe presentar
    /// el contenido.
    ///
    /// # Ejemplo
    /// ```text
    /// spine = ["cover", "introduction", "chapter1", "chapter2", "colophon"]
    /// ```
    pub spine: Vec<String>,

    /// Configuración específica de GutenAIR (no estándar EPUB)
    ///
    /// Almacena preferencias de estilo, estado del editor y metadatos internos
    /// que no forman parte del estándar EPUB 3.0.
    ///
    /// # Contenido típico
    ///
    /// - Estilos predeterminados (IDs de CSS en el manifiesto)
    /// - Preferencias de edición (zoom, modo oscuro, etc.)
    /// - Estado de la interfaz (último capítulo abierto, posición de scroll)
    /// - Metadatos del proyecto (fecha de última apertura, versión del editor)
    ///
    /// # Persistencia
    ///
    /// La configuración se guarda automáticamente en:
    /// `META-INF/gutenAIR.config` (formato JSON)
    ///
    /// # Nota
    ///
    /// Este campo es **extensión propia** y no afecta la compatibilidad EPUB.
    /// Los lectores de EPUB estándar ignoran este archivo.
    pub config: GutenConfig,
}

impl GutenCore {
    /// Este método solo inicializa la estructura con el directorio de trabajo,
    /// pero **no** carga ningún archivo EPUB existente. Los campos `metadata`,
    /// `manifest` y `spine` quedarán vacíos.
    ///
    /// # Argumentos
    ///
    /// * `workdir` - Directorio raíz donde se encuentra o se creará el proyecto EPUB
    ///
    /// # Ejemplo
    ///
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// let core = GutenCore::new("./mi_proyecto");
    ///
    /// // En este punto, el core está vacío:
    /// assert!(core.metadata.is_none());
    /// assert!(core.manifest.is_empty());
    /// ```
    ///
    /// # Nota
    ///
    /// Para cargar un proyecto existente, usa [`open_folder`](Self::open_folder).
    /// Para crear un proyecto nuevo, usa [`new_project`](Self::new_project).
    pub fn new(workdir: impl AsRef<Path>) -> Self {
        Self {
            workdir: workdir.as_ref().to_path_buf(),
            opf_path: None,
            opf_dir: None,
            metadata: None,
            manifest: HashMap::new(),
            spine: Vec::new(),
            config: GutenConfig::default(),
        }
    }

    /// Abre una carpeta existente como proyecto EPUB
    ///
    /// Este método inicializa un `GutenCore` a partir de un directorio que contiene
    /// un proyecto EPUB válido. Realiza los siguientes pasos:
    ///
    /// 1. Crea una instancia vacía con [`new`](Self::new)
    /// 2. Carga y parsea el archivo `META-INF/container.xml`
    /// 3. Localiza y parsea el archivo OPF (normalmente `OEBPS/content.opf`)
    /// 4. Extrae todos los metadatos, manifiesto y orden de lectura
    /// 5. **Carga la configuración de GutenAIR** desde `META-INF/gutenAIR.config` (si existe)
    ///
    /// # Argumentos
    ///
    /// * `workdir` - Ruta al directorio raíz del proyecto EPUB.
    ///   Este directorio debe contener la carpeta `META-INF/` y el archivo `mimetype`.
    ///
    /// # Retorna
    ///
    /// * `Result<Self>` - Una instancia de `GutenCore` con todos los datos cargados,
    ///   o un error si el proyecto es inválido.
    ///
    /// # Errores
    ///
    /// Este método puede retornar los siguientes errores:
    ///
    /// * `GutenError::InvalidProject` - Si:
    ///   - No existe `META-INF/container.xml`
    ///   - El XML de `container.xml` está mal formado
    ///   - Falta el atributo `full-path` en el elemento `rootfile`
    ///   - No se encuentra o no se puede parsear el archivo OPF
    ///   - El OPF falta elementos requeridos (`metadata`, `manifest`, `spine`)
    /// * `GutenError::Other` - Si el archivo de configuración existe pero es JSON inválido
    /// * `std::io::Error` - Si hay problemas leyendo archivos del sistema
    ///
    /// # Ejemplo básico
    ///
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let core = GutenCore::open_folder("./mi_libro_epub")?;
    ///
    /// // Verificar que se cargaron los metadatos
    /// if let Some(metadata) = &core.metadata {
    ///     println!("Título: {}", metadata.title);
    ///     println!("Idioma: {}", metadata.language);
    /// }
    ///
    /// // La configuración se carga automáticamente
    /// println!("Estilos predeterminados: {:?}", core.config.default_styles);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Notas importantes
    ///
    /// - El directorio debe ser un **EPUB descomprimido** válido (no un archivo `.epub` comprimido)
    /// - Este método **modifica el estado interno** cargando todos los datos en memoria
    /// - Después de cargar, puedes modificar los datos y usar [`save`](Self::save) para persistir los cambios
    /// - Si no existe el archivo de configuración, `config` mantiene sus valores por defecto
    ///
    /// # Ver también
    ///
    /// - [`new`](Self::new) - Para crear una instancia vacía sin cargar datos
    /// - [`new_project`](Self::new_project) - Para crear un proyecto nuevo desde cero
    /// - [`save`](Self::save) - Para guardar cambios después de cargar
    /// - [`load_config`](Self::load_config) - Método interno que carga la configuración
    pub fn open_folder(workdir: impl AsRef<Path>) -> Result<Self> {
        let mut core = Self::new(workdir);
        core.load_container_and_opf()?;
        core.parse_opf()?;
        core.load_config()?;
        Ok(core)
    }

    /// Crea un nuevo proyecto EPUB desde cero
    ///
    /// Este método genera la estructura completa de carpetas y archivos necesarios
    /// para un EPUB 3.0 válido. Crea un proyecto mínimo pero funcional que puede
    /// usarse como punto de partida para desarrollar un ebook completo.
    ///
    /// # Estructura generada
    ///
    /// El método crea la siguiente estructura de directorios y archivos:
    ///
    /// ```text
    /// root/
    /// ├── mimetype
    /// ├── META-INF/
    /// │   ├── container.xml
    /// │   └── gutenAIR.config      (configuración del editor)
    /// └── OEBPS/
    ///     ├── content.opf
    ///     ├── Text/
    ///     │   ├── chap1.xhtml
    ///     │   └── nav.xhtml
    ///     ├── Styles/
    ///     │   └── style.css
    ///     └── Images/
    /// ```
    ///
    /// # Archivos creados automáticamente
    ///
    /// * **`mimetype`** - Identificador MIME del EPUB (`application/epub+zip`)
    /// * **`container.xml`** - Punto de entrada que localiza el OPF
    /// * **`gutenAIR.config`** - Archivo JSON con configuración del editor
    /// * **`content.opf`** - Package document con metadatos, manifiesto y spine
    /// * **`chap1.xhtml`** - Archivo de ejemplo con contenido inicial
    /// * **`nav.xhtml`** - Tabla de contenidos básica
    /// * **`style.css`** - Hoja de estilos minimalista
    ///
    /// # Configuración inicial
    ///
    /// El proyecto se crea con una configuración predeterminada que:
    /// - Registra `style` como estilo predeterminado en `default_styles`
    /// - Incluye metadatos básicos del editor
    ///
    /// # Argumentos
    ///
    /// * `root` - Directorio raíz donde se creará el proyecto.
    ///   **Debe estar vacío** o no existir previamente.
    /// * `title` - Título del libro. Se usará en los metadatos y navegación.
    /// * `lang` - Código de idioma según RFC 5646 (ej: `"es"`, `"en"`, `"fr-CA"`).
    ///
    /// # Retorna
    ///
    /// * `Result<Self>` - Una instancia de `GutenCore` con el proyecto recién creado
    ///   ya cargado en memoria (equivalente a llamar [`open_folder`](Self::open_folder)
    ///   en el directorio recién creado).
    ///
    /// # Errores
    ///
    /// Este método puede retornar los siguientes errores:
    ///
    /// * `GutenError::InvalidProject` - Si el directorio de destino **no está vacío**
    /// * `std::io::Error` - Si falla alguna operación de creación de archivos/carpetas:
    ///   - Permisos insuficientes
    ///   - Disco lleno
    ///   - Rutas inválidas
    /// * `GutenError::Other` - Si falla la serialización del archivo de configuración
    ///
    /// Además, puede propagar errores de [`open_folder`](Self::open_folder) si la
    /// carga posterior del proyecto falla (aunque esto es poco probable).
    ///
    /// # Ejemplo básico
    ///
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Crear un nuevo libro en español
    /// let mut core = GutenCore::new_project("./mi_novela", "El Gran Viaje", "es")?;
    ///
    /// // Verificar que se creó correctamente
    /// if let Some(metadata) = &core.metadata {
    ///     println!("Título: {}", metadata.title);     // "El Gran Viaje"
    ///     println!("Idioma: {}", metadata.language);  // "es"
    /// }
    ///
    /// // La configuración ya tiene el estilo predeterminado
    /// println!("Estilos: {:?}", core.config.default_styles); // ["style"]
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Ejemplo con diferentes idiomas
    ///
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Libro en inglés
    /// let en_book = GutenCore::new_project("./english_book", "My Story", "en")?;
    ///
    /// // Libro en francés con variante regional
    /// let fr_book = GutenCore::new_project("./french_book", "Mon Histoire", "fr-CA")?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Ejemplo con manejo de errores
    ///
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// # use guten_core::error::GutenError;
    /// # fn main() {
    /// match GutenCore::new_project("./directorio_existente", "Mi Libro", "es") {
    ///     Ok(core) => println!("Proyecto creado exitosamente"),
    ///     Err(GutenError::InvalidProject(msg)) => {
    ///         eprintln!("Error: {}", msg);  // "Target directory is not empty"
    ///     }
    ///     Err(e) => eprintln!("Error de IO: {}", e),
    /// }
    /// # }
    /// ```
    ///
    /// # Notas importantes
    ///
    /// * **El directorio no debe contener archivos previos** - Si el directorio
    ///   existe y no está vacío, el método fallará para evitar sobrescribir datos.
    /// * **UUID único** - Se genera automáticamente un identificador UUID v4
    ///   para el libro.
    /// * **Fecha de modificación** - Se establece automáticamente a la hora actual
    ///   en formato RFC 3339.
    /// * **Configuración persistente** - Se guarda automáticamente en `META-INF/gutenAIR.config`
    /// * **Proyecto cargado automáticamente** - A diferencia de [`new`](Self::new),
    ///   este método deja el core en estado cargado (con metadata, manifest, etc.).
    ///
    /// # Diferencias con otros constructores
    ///
    /// | Método | Uso | Estado resultante |
    /// |--------|-----|-------------------|
    /// | [`new`](Self::new) | Crear estructura vacía | Sin datos cargados |
    /// | [`open_folder`](Self::open_folder) | Abrir proyecto existente | Datos cargados |
    /// | **`new_project`** | Crear desde cero | Datos cargados |
    ///
    /// # Ver también
    ///
    /// - [`new`](Self::new) - Para crear una instancia vacía manualmente
    /// - [`open_folder`](Self::open_folder) - Para abrir un proyecto existente
    /// - [`save`](Self::save) - Para guardar cambios después de modificar
    /// - [`save_config_file`](Self::save_config_file) - Para guardar configuración manualmente
    /// - [EPUB 3.0 Specification](https://www.w3.org/publishing/epub3/) - Estándar oficial
    pub fn new_project(root: impl AsRef<Path>, title: &str, lang: &str) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        if root.exists() && fs::read_dir(&root)?.next().is_some() {
            return Err(GutenError::InvalidProject(
                "Target directory is not empty".to_string(),
            ));
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

        let opf_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
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
</package>"#,
            lang = lang,
            title = title,
            book_uuid = book_uuid,
            modified = modified
        );

        fs::write(root.join("OEBPS/content.opf"), opf_xml)?;

        // Basic files
        fs::write(
            root.join("OEBPS/Styles/style.css"),
            "body { font-family: serif; }",
        )?;

        let chap1 = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" lang="{lang}">
<head>
  <title>Chapter 1</title>
  <link rel="stylesheet" type="text/css" href="../Styles/style.css"/>
</head>
<body>
  <h1>Chapter 1</h1>
  <p>Hello, EPUB!</p>
</body>
</html>"#,
            lang = lang
        );
        fs::write(root.join("OEBPS/Text/chap1.xhtml"), chap1)?;

        let nav = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="{lang}">
<head><title>TOC</title></head>
<body>
  <nav epub:type="toc" id="toc">
    <ol>
      <li><a href="chap1.xhtml">Chapter 1</a></li>
    </ol>
  </nav>
</body>
</html>"#,
            lang = lang
        );
        fs::write(root.join("OEBPS/Text/nav.xhtml"), nav)?;

        let mut core = Self::open_folder(root)?;
        core.config.default_styles.push("style".to_string());
        core.save_config_file()?;

        Ok(core)
    }

    /// Carga y parsea el archivo `container.xml` para localizar el OPF
    ///
    /// Este método privado es parte del proceso de inicialización de un proyecto EPUB.
    /// Lee el archivo `META-INF/container.xml`, extrae la ruta al archivo OPF
    /// (Package Document) y actualiza los campos internos `opf_path` y `opf_dir`.
    ///
    /// # Estructura del archivo container.xml
    ///
    /// El archivo `container.xml` debe seguir este formato:
    /// ```xml
    /// <?xml version="1.0" encoding="UTF-8"?>
    /// <container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
    ///   <rootfiles>
    ///     <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
    ///   </rootfiles>
    /// </container>
    /// ```
    ///
    /// # Proceso
    ///
    /// 1. Verifica que `META-INF/container.xml` existe en el directorio de trabajo
    /// 2. Lee y parsea el XML usando `roxmltree`
    /// 3. Busca el elemento `<rootfile>` con el namespace OCF
    /// 4. Extrae el atributo `full-path` que apunta al archivo OPF
    /// 5. Construye la ruta completa al OPF y la guarda en `self.opf_path`
    /// 6. Extrae el directorio padre del OPF en `self.opf_dir`
    ///
    /// # Modificaciones al estado interno
    ///
    /// Este método modifica los siguientes campos de `GutenCore`:
    /// - `self.opf_path` - Se establece a `Some(PathBuf)` con la ruta completa al OPF
    /// - `self.opf_dir` - Se establece a `Some(PathBuf)` con el directorio que contiene el OPF
    ///
    /// # Errores
    ///
    /// Este método puede retornar los siguientes errores:
    ///
    /// * `GutenError::InvalidProject` - Si:
    ///   - No existe el archivo `META-INF/container.xml`
    ///   - El archivo XML está mal formado
    ///   - No se encuentra el elemento `<rootfile>`
    ///   - El elemento `<rootfile>` no tiene el atributo `full-path`
    /// * `std::io::Error` - Si hay problemas leyendo el archivo
    ///
    /// # Ejemplo de uso interno
    ///
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// # use guten_core::Result;
    /// # fn example() -> Result<()> {
    /// let mut core = GutenCore::new("./mi_epub");
    /// core.load_container_and_opf()?;
    ///
    /// // Después de llamar este método:
    /// assert!(core.opf_path.is_some());   // Ruta al OPF encontrada
    /// assert!(core.opf_dir.is_some());    // Directorio del OPF
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Notas de implementación
    ///
    /// - **Método privado**: Este método no forma parte de la API pública y solo debe
    ///   ser llamado internamente durante la inicialización (por ejemplo, desde
    ///   [`open_folder`](Self::open_folder)).
    /// - **Namespace OCF**: Se usa la constante [`NS_OCF`] para identificar el elemento
    ///   `<rootfile>` correctamente.
    /// - **`unwrap()` seguro**: El uso de `unwrap()` en `opf_path.parent().unwrap()`
    ///   es seguro porque `full-path` debe ser una ruta relativa válida que contiene
    ///   al menos un componente (ej: `"OEBPS/content.opf"`).
    ///
    /// # Ver también
    ///
    /// - [`open_folder`](Self::open_folder) - Método público que llama a este
    /// - [`parse_opf`](Self::parse_opf) - Método que se llama después para parsear el OPF
    /// - [EPUB Container Format](https://www.w3.org/TR/epub/#sec-container) - Especificación oficial
    fn load_container_and_opf(&mut self) -> Result<()> {
        let container_path = self.workdir.join("META-INF").join("container.xml");
        if !container_path.exists() {
            return Err(GutenError::InvalidProject(
                "META-INF/container.xml not found".to_string(),
            ));
        }

        let content = fs::read_to_string(&container_path)?;
        let doc = roxmltree::Document::parse(&content).map_err(|e| {
            GutenError::InvalidProject(format!("XML error in container.xml: {}", e))
        })?;

        let rootfile = doc
            .descendants()
            .find(|n| n.has_tag_name((NS_OCF, "rootfile")))
            .ok_or_else(|| {
                GutenError::InvalidProject("container.xml invalid: missing rootfile".to_string())
            })?;

        let full_path_attr = rootfile.attribute("full-path").ok_or_else(|| {
            GutenError::InvalidProject(
                "container.xml invalid: rootfile missing full-path".to_string(),
            )
        })?;

        let opf_path = self.workdir.join(full_path_attr);
        self.opf_dir = Some(opf_path.parent().unwrap().to_path_buf());
        self.opf_path = Some(opf_path);

        Ok(())
    }

    /// Parsea el archivo OPF (Open Packaging Format) y carga su contenido en memoria
    ///
    /// Este método privado es el corazón del proceso de carga de un proyecto EPUB.
    /// Lee el archivo `content.opf`, extrae toda la información estructural del libro
    /// y actualiza los campos internos `metadata`, `manifest` y `spine`.
    ///
    /// # Estructura del archivo OPF
    ///
    /// El archivo OPF debe seguir el estándar EPUB 3.0 con esta estructura básica:
    ///
    /// ```xml
    /// <?xml version="1.0" encoding="UTF-8"?>
    /// <package version="3.0" unique-identifier="bookid" xmlns="http://www.idpf.org/2007/opf">
    ///   <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    ///     <dc:identifier id="bookid">urn:uuid:...</dc:identifier>
    ///     <dc:title>Mi Libro</dc:title>
    ///     <dc:language>es</dc:language>
    ///     <meta property="dcterms:modified">2024-01-01T00:00:00Z</meta>
    ///   </metadata>
    ///   <manifest>
    ///     <item id="chap1" href="Text/chap1.xhtml" media-type="application/xhtml+xml"/>
    ///   </manifest>
    ///   <spine>
    ///     <itemref idref="chap1"/>
    ///   </spine>
    /// </package>
    /// ```
    ///
    /// # Proceso de parseo
    ///
    /// El método realiza tres pasos principales:
    ///
    /// ## 1. Parseo de metadatos
    /// - Extrae `<dc:title>` (título del libro)
    /// - Extrae `<dc:language>` (código de idioma)
    /// - Extrae `<dc:identifier>` (identificador único, normalmente un UUID)
    /// - Extrae `<meta property="dcterms:modified">` (fecha de última modificación)
    /// - Si falta algún campo, usa valores predeterminados sensatos
    ///
    /// ## 2. Parseo del manifiesto
    /// - Itera sobre todos los elementos `<item>`
    /// - Extrae `id`, `href`, `media-type` y `properties` de cada uno
    /// - Los almacena en `self.manifest` (HashMap con `id` como clave)
    /// - Omite items sin `id` (no válidos según el estándar)
    ///
    /// ## 3. Parseo del spine (orden de lectura)
    /// - Itera sobre todos los elementos `<itemref>`
    /// - Extrae el atributo `idref` de cada uno
    /// - Los almacena en `self.spine` (Vector que mantiene el orden)
    ///
    /// # Modificaciones al estado interno
    ///
    /// Este método modifica los siguientes campos de `GutenCore`:
    ///
    /// | Campo | Cambio | Descripción |
    /// |-------|--------|-------------|
    /// | `self.metadata` | `Some(BookMetadata)` | Metadatos extraídos del OPF |
    /// | `self.manifest` | Limpia y llena | HashMap con todos los items del EPUB |
    /// | `self.spine` | Limpia y llena | Vector con el orden de lectura |
    ///
    /// # Valores predeterminados
    ///
    /// Cuando faltan elementos en el OPF, se usan estos valores:
    ///
    /// | Elemento faltante | Valor predeterminado |
    /// |-------------------|---------------------|
    /// | `<dc:title>` | `"Untitled"` |
    /// | `<dc:language>` | `"en"` (inglés) |
    /// | `<dc:identifier>` | `""` (string vacío) |
    /// | `<meta modified>` | Fecha/hora actual (UTC) |
    ///
    /// # Errores
    ///
    /// Este método puede retornar los siguientes errores:
    ///
    /// * `GutenError::InvalidProject` - Si:
    ///   - `self.opf_path` es `None` (no se llamó a `load_container_and_opf` primero)
    ///   - El archivo OPF no existe o no se puede leer
    ///   - El XML está mal formado
    ///   - Falta la sección `<metadata>`
    ///   - Falta la sección `<manifest>`
    ///   - Falta la sección `<spine>`
    /// * `std::io::Error` - Si hay problemas leyendo el archivo
    ///
    /// # Ejemplo de uso interno
    ///
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// # use guten_core::Result;
    /// # fn example() -> Result<()> {
    /// let mut core = GutenCore::new("./mi_epub");
    /// core.load_container_and_opf()?;  // Primero carga container.xml
    /// core.parse_opf()?;                // Luego parsea el OPF
    ///
    /// // Después del parseo:
    /// assert!(core.metadata.is_some());
    /// assert!(!core.manifest.is_empty());
    /// assert!(!core.spine.is_empty());
    ///
    /// let metadata = core.metadata.as_ref().unwrap();
    /// println!("Título: {}", metadata.title);
    /// println!("Número de recursos: {}", core.manifest.len());
    /// println!("Orden de lectura: {} items", core.spine.len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Notas de implementación
    ///
    /// - **Método privado**: Este método no forma parte de la API pública y solo debe
    ///   ser llamado internamente después de `load_container_and_opf()`.
    /// - **Namespaces**: Se usan las constantes [`NS_OPF`] y [`NS_DC`] para identificar
    ///   correctamente los elementos XML.
    /// - **Tolerancia a errores**: El método es tolerante con metadatos faltantes,
    ///   pero estricto con la estructura requerida (metadata, manifest, spine).
    /// - **Limpieza previa**: Los campos `manifest` y `spine` se limpian antes de
    ///   cargar nuevos datos para evitar acumulación de información obsoleta.
    ///
    /// # Advertencias
    ///
    /// - Este método **sobrescribe** cualquier dato previo en `manifest` y `spine`
    /// - No valida que los items referenciados en `spine` existan en `manifest`
    ///   (esa validación debería hacerse en otro método)
    ///
    /// # Ver también
    ///
    /// - [`load_container_and_opf`](Self::load_container_and_opf) - Método que debe llamarse antes
    /// - [`open_folder`](Self::open_folder) - Método público que orquesta ambos
    /// - [`save`](Self::save) - Para guardar cambios en el OPF
    /// - [EPUB Open Packaging Format (OPF)](https://www.w3.org/TR/epub/#sec-package) - Especificación oficial
    fn parse_opf(&mut self) -> Result<()> {
        let opf_path = self
            .opf_path
            .as_ref()
            .ok_or_else(|| GutenError::InvalidProject("OPF path not loaded".to_string()))?;
        let content = fs::read_to_string(opf_path)?;
        let doc = roxmltree::Document::parse(&content)
            .map_err(|e| GutenError::InvalidProject(format!("XML error in content.opf: {}", e)))?;

        let root = doc.root_element();

        // Metadata
        let metadata_node = root
            .children()
            .find(|n| n.has_tag_name((NS_OPF, "metadata")))
            .ok_or_else(|| GutenError::InvalidProject("OPF missing metadata".to_string()))?;

        let title = metadata_node
            .children()
            .find(|n| n.has_tag_name((NS_DC, "title")))
            .map(|n| n.text().unwrap_or("").to_string())
            .unwrap_or_else(|| "Untitled".to_string());

        let language = metadata_node
            .children()
            .find(|n| n.has_tag_name((NS_DC, "language")))
            .map(|n| n.text().unwrap_or("").to_string())
            .unwrap_or_else(|| "en".to_string());

        let identifier = metadata_node
            .children()
            .find(|n| n.has_tag_name((NS_DC, "identifier")))
            .map(|n| n.text().unwrap_or("").to_string())
            .unwrap_or_else(|| "".to_string());

        let modified = metadata_node
            .children()
            .find(|n| {
                n.has_tag_name((NS_OPF, "meta"))
                    && n.attribute("property") == Some("dcterms:modified")
            })
            .map(|n| n.text().unwrap_or("").to_string())
            .unwrap_or_else(|| Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true));

        self.metadata = Some(BookMetadata {
            title,
            language,
            identifier,
            modified,
        });

        // Manifest
        let manifest_node = root
            .children()
            .find(|n| n.has_tag_name((NS_OPF, "manifest")))
            .ok_or_else(|| GutenError::InvalidProject("OPF missing manifest".to_string()))?;

        self.manifest.clear();
        for item in manifest_node
            .children()
            .filter(|n| n.has_tag_name((NS_OPF, "item")))
        {
            let id = item.attribute("id").unwrap_or("").to_string();
            let href = item.attribute("href").unwrap_or("").to_string();
            let media_type = item.attribute("media-type").unwrap_or("").to_string();
            let properties = item.attribute("properties").unwrap_or("").to_string();

            if !id.is_empty() {
                self.manifest.insert(
                    id.clone(),
                    ManifestItem {
                        id,
                        href,
                        media_type,
                        properties,
                    },
                );
            }
        }

        // Spine
        let spine_node = root
            .children()
            .find(|n| n.has_tag_name((NS_OPF, "spine")))
            .ok_or_else(|| GutenError::InvalidProject("OPF missing spine".to_string()))?;

        self.spine.clear();
        for itemref in spine_node
            .children()
            .filter(|n| n.has_tag_name((NS_OPF, "itemref")))
        {
            if let Some(idref) = itemref.attribute("idref") {
                self.spine.push(idref.to_string());
            }
        }

        Ok(())
    }

    /// Guarda el estado actual del proyecto en el archivo OPF
    ///
    /// Este método serializa toda la información del `GutenCore` de vuelta al
    /// archivo `content.opf`. Es la contraparte de `parse_opf` y debe llamarse
    /// después de realizar modificaciones en los datos del proyecto.
    ///
    /// # Proceso de guardado
    ///
    /// El método realiza las siguientes operaciones en orden:
    ///
    /// 1. **Sincroniza la navegación** - Llama a [`update_nav`](Self::update_nav)
    ///    para regenerar `nav.xhtml` basado en el spine y los encabezados
    ///
    /// 2. **Actualiza la fecha de modificación** - Llama a `update_modified_date()`
    ///    para establecer la fecha actual en los metadatos
    ///
    /// 3. **Genera el XML** - Construye el archivo OPF desde cero usando `quick_xml`:
    ///    - Declaración XML
    ///    - Elemento `<package>` con atributos requeridos
    ///    - Sección `<metadata>` con todos los metadatos Dublin Core
    ///    - Sección `<manifest>` con todos los items ordenados por ID
    ///    - Sección `<spine>` con el orden de lectura
    ///
    /// 4. **Escribe el archivo** - Guarda el XML generado en `self.opf_path`
    ///
    /// # Formato del archivo generado
    ///
    /// El OPF generado sigue el estándar EPUB 3.0:
    ///
    /// ```xml
    /// <?xml version="1.0" encoding="UTF-8"?>
    /// <package version="3.0" unique-identifier="bookid"
    ///          xmlns="http://www.idpf.org/2007/opf" xml:lang="es">
    ///   <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"
    ///             xmlns:dcterms="http://purl.org/dc/terms/">
    ///     <dc:identifier id="bookid">urn:uuid:...</dc:identifier>
    ///     <dc:title>Mi Libro</dc:title>
    ///     <dc:language>es</dc:language>
    ///     <meta property="dcterms:modified">2024-01-01T00:00:00Z</meta>
    ///   </metadata>
    ///   <manifest>
    ///     <item id="chap1" href="Text/chap1.xhtml" media-type="application/xhtml+xml"/>
    ///     <!-- ... más items ordenados alfabéticamente por ID ... -->
    ///   </manifest>
    ///   <spine>
    ///     <itemref idref="chap1"/>
    ///     <!-- ... más itemrefs en el orden del spine ... -->
    ///   </spine>
    /// </package>
    /// ```
    ///
    /// # Características especiales
    ///
    /// - **Items ordenados**: Los items en el `<manifest>` se ordenan alfabéticamente
    ///   por ID para generar un diff más legible en control de versiones
    /// - **Propiedades opcionales**: El atributo `properties` solo se incluye si no está vacío
    /// - **Formato indentado**: El XML se genera con indentación de 2 espacios para mejor legibilidad
    ///
    /// # Requisitos previos
    ///
    /// Antes de llamar a `save`, el `GutenCore` debe estar en un estado válido:
    /// - `self.opf_path` debe ser `Some` (proyecto cargado o creado)
    /// - `self.metadata` debe ser `Some` (metadatos existentes)
    ///
    /// # Errores
    ///
    /// Este método puede retornar los siguientes errores:
    ///
    /// * `GutenError::InvalidProject` - Si:
    ///   - `self.opf_path` es `None` (proyecto no cargado)
    ///   - `self.metadata` es `None` (no hay metadatos)
    ///   - `update_nav` falla (problemas con la navegación)
    /// * `quick_xml::Error` - Si falla la serialización XML
    /// * `std::io::Error` - Si falla la escritura del archivo
    ///
    /// # Panics
    ///
    /// Este método **no debería entrar en pánico** bajo condiciones normales,
    /// pero los siguientes casos podrían causar pánico:
    /// - Si `self.opf_path` no tiene un padre válido (muy improbable)
    /// - Si hay errores internos en `quick_xml` (deberían propagarse como `Result`)
    ///
    /// # Ejemplo básico
    ///
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    ///
    /// // Modificar el título
    /// if let Some(metadata) = &mut core.metadata {
    ///     metadata.title = "Nuevo Título".to_string();
    /// }
    ///
    /// // Agregar un nuevo capítulo al manifiesto
    /// core.manifest.insert("chap2".to_string(), ManifestItem {
    ///     id: "chap2".to_string(),
    ///     href: "Text/chap2.xhtml".to_string(),
    ///     media_type: "application/xhtml+xml".to_string(),
    ///     properties: String::new(),
    /// });
    ///
    /// // Agregar al spine
    /// core.spine.push("chap2".to_string());
    ///
    /// // Guardar los cambios
    /// core.save()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Ejemplo con manejo de errores
    ///
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// # use guten_core::error::GutenError;
    /// let mut core = GutenCore::new("./proyecto_vacio");
    ///
    /// // Intentar guardar sin haber cargado un proyecto
    /// if let Err(e) = core.save() {
    ///     match e {
    ///         GutenError::InvalidProject(msg) => {
    ///             println!("Error esperado: {}", msg); // "OPF path not loaded"
    ///         }
    ///         _ => eprintln!("Error inesperado: {}", e),
    ///     }
    /// }
    /// ```
    ///
    /// # Notas de implementación
    ///
    /// - **`quick_xml`**: Se usa para generar XML de manera eficiente y segura.
    ///   El writer se configura con indentación automática.
    /// - **Ordenamiento**: Los items se ordenan alfabéticamente por ID para
    ///   garantizar que archivos idénticos generen el mismo XML (útil para Git).
    /// - **`update_modified_date()`**: Este método debe existir y actualizar
    ///   `self.metadata.modified` con la fecha/hora actual en formato RFC 3339.
    /// - **`update_nav()`**: Se llama automáticamente; no necesitas llamarlo
    ///   explícitamente antes de `save`.
    ///
    /// # Advertencias
    ///
    /// - **Sobrescribe el archivo**: Este método reemplaza completamente el
    ///   archivo OPF existente. No hay copia de seguridad automática.
    /// - **Operación costosa**: Para proyectos grandes con muchos recursos,
    ///   la serialización puede ser lenta. No llames `save` en loops innecesarios.
    /// - **Dependencia de `update_nav`**: Si `update_nav` falla, el guardado
    ///   se cancela y el OPF no se modifica.
    ///
    /// # Ver también
    ///
    /// - [`parse_opf`](Self::parse_opf) - Método que carga el OPF (operación inversa)
    /// - [`update_nav`](Self::update_nav) - Actualiza la tabla de contenidos
    /// - [`open_folder`](Self::open_folder) - Carga un proyecto para poder guardarlo
    /// - [`new_project`](Self::new_project) - Crea un proyecto nuevo listo para guardar
    pub fn save(&mut self) -> Result<()> {
        let opf_path = self
            .opf_path
            .clone()
            .ok_or_else(|| GutenError::InvalidProject("OPF path not loaded".to_string()))?;

        // 1. Sync Nav (TOC) automatically
        self.update_nav()?;

        // 2. Update modified date before saving
        self.update_modified_date();
        let metadata = self
            .metadata
            .as_ref()
            .ok_or_else(|| GutenError::InvalidProject("Metadata missing".to_string()))?;

        use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
        use quick_xml::writer::Writer;
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

        // 3. Sync and save GutenAIR config
        let config_id = "gutenair-config";
        let config_href = "../META-INF/gutenAIR.config";
        if !self.manifest.contains_key(config_id) {
            self.add_to_manifest(
                config_id.to_string(),
                config_href.to_string(),
                "application/json".to_string(),
                String::new(),
            )?;
        }
        self.save_config_file()?;

        let result = writer.into_inner().into_inner();
        fs::write(opf_path, result)?;

        Ok(())
    }

    /// Actualiza automáticamente el archivo de navegación (`nav.xhtml`)
    ///
    /// Este método regenera la tabla de contenidos (TOC) del EPUB basándose en
    /// los encabezados (headings) de los archivos XHTML listados en el `spine`.
    /// Es llamado automáticamente por [`save`](Self::save), pero también puede
    /// invocarse manualmente si se necesita regenerar la navegación sin guardar.
    ///
    /// # Proceso de generación
    ///
    /// 1. **Escanea los documentos** - Itera sobre cada `idref` en `self.spine`
    /// 2. **Filtra XHTML** - Solo procesa items con `media_type = "application/xhtml+xml"`
    /// 3. **Extrae encabezados** - Llama a [`scan_headings`](Self::scan_headings) para
    ///    cada documento, obteniendo sus encabezados (H1, H2, H3...)
    /// 4. **Construye rutas relativas** - Calcula rutas desde `Text/` hacia cada documento
    /// 5. **Genera HTML** - Crea una lista anidada (`<ol>`) con enlaces a los encabezados
    /// 6. **Guarda el archivo** - Escribe `Text/nav.xhtml` en el directorio OPF
    /// 7. **Actualiza el manifiesto** - Agrega el item `nav` al manifest si no existe
    ///
    /// # Jerarquía de navegación
    ///
    /// Actualmente, solo se incluyen en la navegación principal los encabezados
    /// de nivel 1 y 2 (H1 y H2). Los niveles inferiores (H3-H6) se ignoran para
    /// mantener la TOC concisa.
    ///
    /// | Nivel | Incluido | Indentación |
    /// |-------|----------|-------------|
    /// | H1    | ✅ Sí    | Ninguna     |
    /// | H2    | ✅ Sí    | 2 espacios  |
    /// | H3-H6 | ❌ No    | -           |
    ///
    /// # Estructura generada
    ///
    /// El archivo `nav.xhtml` generado tiene este formato:
    ///
    /// ```html
    /// <?xml version="1.0" encoding="UTF-8"?>
    /// <html xmlns="http://www.w3.org/1999/xhtml"
    ///       xmlns:epub="http://www.idpf.org/2007/ops" lang="es">
    /// <head><title>Título del Libro</title></head>
    /// <body>
    ///   <nav epub:type="toc" id="toc">
    ///     <h1>Título del Libro</h1>
    ///     <ol>
    ///       <li><a href="chap1.xhtml">Capítulo 1</a></li>
    ///       <li><a href="chap2.xhtml">Capítulo 2</a>
    ///         <ol>
    ///           <li><a href="chap2.xhtml#section1">Sección 1.1</a></li>
    ///         </ol>
    ///       </li>
    ///     </ol>
    ///   </nav>
    /// </body>
    /// </html>
    /// ```
    ///
    /// # Requisitos previos
    ///
    /// Para que `update_nav` funcione correctamente, se necesita:
    /// - `self.spine` debe contener los IDs de los documentos en orden de lectura
    /// - `self.manifest` debe mapear esos IDs a items con `href` válidos
    /// - `self.opf_dir` debe estar definido (dónde guardar `nav.xhtml`)
    /// - `self.scan_headings` debe estar implementado (lee y parsea XHTML)
    ///
    /// # Errores
    ///
    /// Este método puede retornar los siguientes errores:
    ///
    /// * `GutenError::InvalidProject` - Si:
    ///   - `self.opf_dir` es `None` (proyecto no cargado correctamente)
    ///   - Algún documento XHTML no puede ser leído o parseado
    ///   - `scan_headings` falla para algún documento
    /// * `std::io::Error` - Si falla la escritura del archivo `nav.xhtml`
    ///
    /// # Ejemplo de uso manual
    ///
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    ///
    /// // Agregar un nuevo capítulo
    /// core.spine.push("nuevo_capitulo".to_string());
    ///
    /// // Regenerar la navegación sin guardar todo el OPF
    /// core.update_nav()?;
    ///
    /// // Los cambios en nav.xhtml ya están guardados en disco
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Ejemplo de anclajes (anchors)
    ///
    /// Si `scan_headings` extrae anclajes de los encabezados (ej: `id="section1"`),
    /// los enlaces incluirán el fragmento:
    ///
    /// ```html
    /// <li><a href="capitulo2.xhtml#section1">Sección 1.1</a></li>
    /// ```
    ///
    /// # Notas de implementación
    ///
    /// - **Rutas relativas**: Se usa `pathdiff::diff_paths` para calcular rutas
    ///   relativas desde el directorio `Text/` hacia cada documento. Esto es
    ///   necesario porque `nav.xhtml` está en `Text/` y puede referenciar
    ///   documentos en subdirectorios.
    /// - **`unwrap_or_else` seguro**: Si `diff_paths` falla (no debería), se usa
    ///   la ruta original como fallback.
    /// - **Propiedad `nav`**: El item del manifiesto se marca con `properties="nav"`
    ///   según el estándar EPUB 3.0.
    /// - **Dynamic Language**: The `lang` attribute is taken from `metadata.language`.
    ///
    /// # Limitaciones conocidas
    ///
    /// - **Solo H1 y H2**: Los niveles inferiores no se incluyen en la TOC principal
    /// - **Idioma dinámico**: Usa el idioma definido en los metadatos del libro.
    /// - **Ruta fija**: `nav.xhtml` siempre se guarda en `Text/`, asumiendo esa estructura
    /// - **Sin soporte de nav anidada**: La estructura de `<ol>` es plana para H1/H2,
    ///   no se crean sublistas anidadas
    ///
    /// # Ver también
    ///
    /// - [`scan_headings`](Self::scan_headings) - Método que extrae encabezados de un XHTML
    /// - [`save`](Self::save) - Método que llama automáticamente a `update_nav`
    /// - [`add_to_manifest`](Self::add_to_manifest) - Agrega el nav al manifiesto
    /// - [EPUB Navigation Document](https://www.w3.org/TR/epub/#sec-nav) - Especificación oficial
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
        let mut in_sublist = false;
        let mut h1_open = false;

        for doc in nav_items {
            let doc_path = std::path::Path::new(&doc.href);
            let rel =
                pathdiff::diff_paths(doc_path, nav_dir).unwrap_or_else(|| doc_path.to_path_buf());
            let rel_str = rel.to_string_lossy();

            for heading in doc.items {
                if heading.level == 1 {
                    if in_sublist {
                        list_items.push("      </ol>".to_string());
                        in_sublist = false;
                    }
                    if h1_open {
                        list_items.push("    </li>".to_string());
                    }

                    let href = if heading.anchor.is_empty() {
                        rel_str.to_string()
                    } else {
                        format!("{}#{}", rel_str, heading.anchor)
                    };
                    list_items.push(format!(
                        "    <li><a href=\"{}\">{}</a>",
                        href, heading.title
                    ));
                    h1_open = true;
                } else if heading.level == 2 {
                    let href = if heading.anchor.is_empty() {
                        rel_str.to_string()
                    } else {
                        format!("{}#{}", rel_str, heading.anchor)
                    };

                    if !h1_open {
                        // Stray H2 without H1, treat as top-level
                        list_items.push(format!(
                            "    <li><a href=\"{}\">{}</a></li>",
                            href, heading.title
                        ));
                    } else {
                        if !in_sublist {
                            list_items.push("      <ol>".to_string());
                            in_sublist = true;
                        }
                        list_items.push(format!(
                            "        <li><a href=\"{}\">{}</a></li>",
                            href, heading.title
                        ));
                    }
                }
            }
        }

        if in_sublist {
            list_items.push("      </ol>".to_string());
        }
        if h1_open {
            list_items.push("    </li>".to_string());
        }

        let title = self
            .metadata
            .as_ref()
            .map(|m| m.title.as_str())
            .unwrap_or("Table of Contents");
        let lang = self
            .metadata
            .as_ref()
            .map(|m| m.language.as_str())
            .unwrap_or("en");
        let nav_xhtml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="{}">
<head><title>{}</title></head>
<body>
  <nav epub:type="toc" id="toc">
    <h1>{}</h1>
    <ol>
{}
    </ol>
  </nav>
</body>
</html>"#,
            lang,
            title,
            title,
            list_items.join("\n")
        );

        // Save nav.xhtml and ensure it's in the manifest
        let nav_href = "Text/nav.xhtml";
        let opf_dir = self
            .opf_dir
            .as_ref()
            .ok_or_else(|| GutenError::InvalidProject("OPF dir not loaded".into()))?;
        fs::write(opf_dir.join(nav_href), nav_xhtml)?;

        if !self.manifest.values().any(|it| it.href == nav_href) {
            self.add_to_manifest(
                "nav".to_string(),
                nav_href.to_string(),
                "application/xhtml+xml".to_string(),
                "nav".to_string(),
            )?;
        }

        Ok(())
    }

    /// Carga la configuración de GutenAIR desde `META-INF/gutenAIR.config`
    ///
    /// Este método interno busca el archivo de configuración en el directorio
    /// `META-INF/` del proyecto. Si existe, lo parsea como JSON y actualiza
    /// `self.config`. Si no existe, mantiene la configuración actual
    /// (que por defecto es [`GutenConfig::default()`]).
    ///
    /// # Ubicación del archivo
    ///
    /// ```text
    /// workdir/
    /// └── META-INF/
    ///     └── gutenAIR.config    <-- Archivo de configuración
    /// ```
    ///
    /// # Formato del archivo
    ///
    /// El archivo `gutenAIR.config` es JSON con formato pretty-print:
    ///
    /// ```json
    /// {
    ///   "default_styles": ["style", "custom-theme"],
    ///   "editor_preferences": {
    ///     "dark_mode": false,
    ///     "font_size": 14,
    ///     "auto_save": true
    ///   },
    ///   "last_open_chapter": "chap1",
    ///   "scroll_position": 42
    /// }
    /// ```
    ///
    /// # Comportamiento
    ///
    /// | Caso | Acción |
    /// |------|--------|
    /// | Archivo existe y es JSON válido | Carga y reemplaza `self.config` |
    /// | Archivo existe pero JSON inválido | Retorna `GutenError::Other` con detalles del error |
    /// | Archivo no existe | No hace nada, mantiene `self.config` actual |
    ///
    /// # Errores
    ///
    /// * `GutenError::Other` - Si el archivo existe pero contiene JSON inválido.
    ///   El mensaje incluye la razón del error de parseo.
    /// * `std::io::Error` - Si hay problemas leyendo el archivo (permisos, etc.)
    ///
    /// # Ejemplo de uso interno
    ///
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// # use guten_core::error::Result;
    /// # fn example() -> Result<()> {
    /// let mut core = GutenCore::new("./mi_epub");
    /// core.load_config()?;
    ///
    /// // Si el archivo existía, ahora core.config tiene esos valores
    /// if !core.config.default_styles.is_empty() {
    ///     println!("Estilos cargados: {:?}", core.config.default_styles);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Notas de implementación
    ///
    /// - **Método privado**: No forma parte de la API pública.
    /// - **Tolerante a ausencia**: Si el archivo no existe, no es un error.
    /// - **Sobrescribe**: Si el archivo existe, reemplaza completamente `self.config`.
    /// - **Serialización**: Usa `serde_json` para parsear el JSON.
    ///
    /// # Ver también
    ///
    /// - [`save_config_file`](Self::save_config_file) - Guarda la configuración
    /// - [`open_folder`](Self::open_folder) - Método que llama a este
    /// - [`GutenConfig`](crate::types::GutenConfig) - Estructura de configuración
    fn load_config(&mut self) -> Result<()> {
        let config_path = self.workdir.join("META-INF").join("gutenAIR.config");
        if config_path.exists() {
            let content = fs::read_to_string(config_path)?;
            self.config = serde_json::from_str(&content)
                .map_err(|e| GutenError::Other(format!("Config parse error: {}", e)))?;
        }
        Ok(())
    }

    /// Guarda la configuración de GutenAIR en `META-INF/gutenAIR.config`
    ///
    /// Este método serializa `self.config` a JSON y lo escribe en el archivo
    /// `META-INF/gutenAIR.config` dentro del proyecto. Crea el directorio
    /// `META-INF/` si no existe.
    ///
    /// # Ubicación del archivo
    ///
    /// ```text
    /// workdir/
    /// └── META-INF/
    ///     └── gutenAIR.config    <-- Archivo creado/actualizado
    /// ```
    ///
    /// # Formato de salida
    ///
    /// El archivo se guarda con formato JSON pretty-print (indentado) para
    /// facilitar la edición manual y el control de versiones:
    ///
    /// ```json
    /// {
    ///   "default_styles": [
    ///     "style",
    ///     "custom"
    ///   ],
    ///   "editor_preferences": {
    ///     "dark_mode": true,
    ///     "font_size": 16
    ///   }
    /// }
    /// ```
    ///
    /// # Cuándo se guarda automáticamente
    ///
    /// Este método es llamado automáticamente en:
    ///
    /// 1. **[`save`](Self::save)** - Al guardar el OPF, también guarda la configuración
    /// 2. **[`new_project`](Self::new_project)** - Al crear un proyecto nuevo
    ///
    /// # Cuándo usarlo manualmente
    ///
    /// Puedes llamarlo directamente si:
    /// - Modificaste `self.config` y quieres persistir los cambios **sin** reescribir el OPF
    /// - Quieres guardar la configuración en medio de una sesión larga de edición
    /// - Necesitas forzar la escritura por razones de respaldo
    ///
    /// # Errores
    ///
    /// * `GutenError::Other` - Si falla la serialización a JSON.
    ///   El mensaje incluye la razón del error de serialización.
    /// * `std::io::Error` - Si falla:
    ///   - La creación del directorio `META-INF/` (problemas de permisos)
    ///   - La escritura del archivo (disco lleno, permisos, etc.)
    ///
    /// # Ejemplo de uso manual
    ///
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// # use guten_core::error::Result;
    /// # fn example() -> Result<()> {
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    ///
    /// // Modificar configuración
    /// core.config.default_styles.push("mi-estilo-personal".to_string());
    /// core.config.editor_preferences.insert("zoom".to_string(), 120.0);
    ///
    /// // Guardar solo la configuración (sin reescribir el OPF)
    /// core.save_config_file()?;
    /// println!("Configuración guardada");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Ejemplo con manejo de errores
    ///
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// # use guten_core::error::GutenError;
    /// let core = GutenCore::new("./proyecto_sin_permisos");
    ///
    /// match core.save_config_file() {
    ///     Ok(_) => println!("Configuración guardada"),
    ///     Err(GutenError::Other(msg)) if msg.contains("serialize") => {
    ///         eprintln!("Error serializando configuración: {}", msg);
    ///     }
    ///     Err(e) => eprintln!("Error de IO: {}", e),
    /// }
    /// ```
    ///
    /// # Notas de implementación
    ///
    /// - **Método privado**: No forma parte de la API pública.
    /// - **Sobrescribe**: Reemplaza completamente el archivo existente.
    /// - **Crea directorios**: `fs::write` crea el archivo, pero los directorios
    ///   padres deben existir. En un proyecto EPUB válido, `META-INF/` ya existe.
    /// - **Formato pretty**: Usa `to_string_pretty()` para JSON legible.
    /// - **Sin backup**: No se crea copia de seguridad del archivo anterior.
    ///
    /// # Advertencia
    ///
    /// Este método **no** actualiza el manifiesto del OPF. El item de configuración
    /// se agrega al manifiesto en [`save`](Self::save), no aquí. Si guardas solo
    /// la configuración sin llamar a `save`, el archivo existirá en disco pero
    /// no estará referenciado en `content.opf` (lo cual es aceptable porque no
    /// es un recurso estándar de EPUB).
    ///
    /// # Ver también
    ///
    /// - [`load_config`](Self::load_config) - Carga la configuración
    /// - [`save`](Self::save) - Guarda OPF + configuración (llama a este método)
    /// - [`new_project`](Self::new_project) - Crea proyecto con configuración inicial
    /// - [`GutenConfig`](crate::types::GutenConfig) - Estructura de configuración
    /// - [serde_json documentation](https://docs.rs/serde_json)
    fn save_config_file(&self) -> Result<()> {
        let config_path = self.workdir.join("META-INF").join("gutenAIR.config");
        let content = serde_json::to_string_pretty(&self.config)
            .map_err(|e| GutenError::Other(format!("Config serialize error: {}", e)))?;
        fs::write(config_path, content)?;
        Ok(())
    }

    /// Establece una hoja de estilo como predeterminada en la configuración
    ///
    /// Este método modifica las preferencias de `gutenAIR.config` para incluir
    /// el ID del estilo en la lista de inyección automática (`default_styles`).
    /// Esta lista se aplica a todos los capítulos que **no** tengan una excepción
    /// definida.
    ///
    /// # Comportamiento
    /// - Verifica que el ID exista en el manifiesto.
    /// - Si el ID ya está en la lista de favoritos, no hace nada (evita duplicados).
    /// - Solo modifica la memoria; usa [`save`](Self::save) o [`save_config_file`](Self::save_config_file)
    ///   para persistir el cambio.
    ///
    /// # Argumentos
    /// * `id` - ID del recurso CSS registrado en el manifiesto
    ///
    /// # Errores
    /// * `GutenError::Manifest` - Si el ID no existe en el manifiesto
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    ///
    /// // Registrar y activar un estilo global
    /// core.add_style("moderno", "body { font-size: 1.2em; }")?;
    /// core.set_style_as_default("moderno")?;
    ///
    /// // Guardar cambios
    /// core.save()?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Ver también
    /// - [`add_style`](Self::add_style) - Para crear el archivo CSS primero
    /// - [`remove_style_from_chapter`](Self::remove_style_from_chapter) - Para crear una excepción
    /// - [`get_chapter_styles`](Self::get_chapter_styles) - Para ver qué estilos se aplican
    pub fn set_style_as_default(&mut self, id: &str) -> Result<()> {
        // 1. Validar que el ID existe
        if !self.manifest.contains_key(id) {
            return Err(GutenError::Manifest(format!(
                "ID '{}' not found in manifest. Add it first with add_style.",
                id
            )));
        }

        // 2. Agregar a la lista si no está
        if !self.config.default_styles.contains(&id.to_string()) {
            self.config.default_styles.push(id.to_string());
        }

        Ok(())
    }

    /// Obtiene la lista de estilos (IDs) que se aplican a un capítulo específico
    ///
    /// Este método resuelve la jerarquía de estilos de GutenAIR:
    /// 1. Consulta el mapa de `exceptions` en la configuración.
    /// 2. Si el capítulo tiene una lista personalizada, la retorna.
    /// 3. Si no hay excepción, retorna la lista global de `default_styles`.
    ///
    /// # Argumentos
    /// * `id_chapter` - ID del capítulo (XHTML) en el manifiesto.
    ///
    /// # Retorna
    /// * `Vec<String>` - Lista ordenada de IDs de recursos CSS.
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// let core = GutenCore::open_folder("./mi_epub")?;
    /// let estilos = core.get_chapter_styles("chap1");
    /// println!("Estilos aplicados a chap1: {:?}", estilos);
    /// ```
    pub fn get_chapter_styles(&self, id_chapter: &str) -> Vec<String> {
        self.config
            .exceptions
            .get(id_chapter)
            .cloned()
            .unwrap_or_else(|| self.config.default_styles.clone())
    }

    /// Elimina el vínculo de una hoja de estilo de un capítulo específico
    ///
    /// Este método crea o actualiza automáticamente una entrada en el mapa de
    /// excepciones para el capítulo indicado. A partir de este momento, el capítulo
    /// dejará de heredar la lista global de estilos (`default_styles`) y usará
    /// su propia lista personalizada.
    ///
    /// # Proceso
    /// 1. Obtiene la lista actual de estilos para el capítulo (vía [`get_chapter_styles`]).
    /// 2. Elimina todas las ocurrencias del `id_style` solicitado.
    /// 3. Registra el resultado en `config.exceptions` bajo la clave `id_chapter`.
    ///
    /// # Argumentos
    /// * `id_chapter` - ID del capítulo al que se le quitará el estilo.
    /// * `id_style` - ID del estilo a eliminar.
    ///
    /// # Errores
    /// * `GutenError::Manifest` - Si el capítulo no existe en el manifiesto.
    ///
    /// # Notas
    /// - Si el capítulo ya era una excepción, simplemente se elimina el estilo de su lista.
    /// - Si el capítulo no era una excepción, se crea una copia de `default_styles`
    ///   sin el estilo indicado.
    /// - Este cambio solo afecta a la memoria; llama a [`save`] para persistir.
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    ///
    /// // El estilo 'oscuro' es global, pero no lo queremos en la portada
    /// core.remove_style_from_chapter("cover", "oscuro")?;
    /// core.save()?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn remove_style_from_chapter(&mut self, id_chapter: &str, id_style: &str) -> Result<()> {
        // Validar que el capítulo existe
        if !self.manifest.contains_key(id_chapter) {
            return Err(GutenError::Manifest(format!(
                "Chapter ID '{}' not found in manifest",
                id_chapter
            )));
        }

        // Obtener la "capa" actual de estilos y filtrar
        let current_styles = self.get_chapter_styles(id_chapter);
        let new_styles: Vec<String> = current_styles
            .into_iter()
            .filter(|s| s != id_style)
            .collect();

        // Registrar como excepción
        self.config
            .exceptions
            .insert(id_chapter.to_string(), new_styles);

        Ok(())
    }
}
