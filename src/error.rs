//! # Manejo de errores
//!
//! Este módulo define los tipos de error para toda la biblioteca `guten_core`.
//! Todos los errores se encapsulan en el enum [`GutenError`] para facilitar
//! el manejo consistente de errores en toda la aplicación.

use thiserror::Error;

/// Tipos de error que puede producir la biblioteca `gutencore`
///
/// `GutenError` encapsula todos los posibles errores que pueden ocurrir
/// durante las operaciones con archivos EPUB, incluyendo errores de sistema,
/// parseo de XML, manipulación de ZIP, y validaciones específicas del formato EPUB.
///
/// # Ejemplo de manejo de errores
///
/// ```no_run
/// # use gutencore::GutenCore;
/// # use gutencore::error::{GutenError, Result};
/// # fn example() -> Result<()> {
/// match GutenCore::open_folder("./ruta/invalida") {
///     Ok(core) => println!("Proyecto cargado: {:?}", core),
///     Err(GutenError::InvalidProject(msg)) => {
///         eprintln!("Error de validación: {}", msg);
///     }
///     Err(GutenError::Io(e)) => {
///         eprintln!("Error de sistema: {}", e);
///     }
///     Err(e) => eprintln!("Otro error: {}", e),
/// }
/// # Ok(())
/// # }
/// ```
///
/// # Conversión automática
///
/// Gracias a `thiserror`, los errores se convierten automáticamente desde
/// tipos estándar usando `?`:
///
/// ```no_run
/// # use gutencore::error::Result;
/// # use std::fs;
/// fn leer_archivo() -> Result<String> {
///     let contenido = fs::read_to_string("archivo.txt")?;  // Io → GutenError::Io
///     Ok(contenido)
/// }
/// ```
#[derive(Error, Debug)]
pub enum GutenError {
    /// Error de entrada/salida del sistema operativo
    ///
    /// Ocurre cuando fallan operaciones como:
    /// - Leer o escribir archivos
    /// - Crear directorios
    /// - Acceder a rutas inválidas
    ///
    /// Se convierte automáticamente desde [`std::io::Error`] con el operador `?`.
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::error::GutenError;
    /// # use std::fs;
    /// fn test() -> Result<(), GutenError> {
    ///     let _ = fs::read_to_string("/ruta/inexistente")?;
    ///     Ok(())
    /// }
    /// ```
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Error de parseo de XML
    ///
    /// Ocurre cuando un archivo XML (como `container.xml` o `content.opf`)
    /// no está bien formado o no cumple con la sintaxis XML.
    ///
    /// Se convierte automáticamente desde [`quick_xml::Error`].
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::error::GutenError;
    /// fn test() -> Result<(), GutenError> {
    ///     // quick_xml produces error when parsing invalid XML
    ///     let xml_invalido = "<root><unclosed>";
    ///     let mut reader = quick_xml::Reader::from_str(xml_invalido);
    ///     loop {
    ///         match reader.read_event() {
    ///             Ok(quick_xml::events::Event::Eof) => break,
    ///             Ok(_) => (),
    ///             Err(e) => return Err(GutenError::Xml(e)),
    ///         }
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[error("XML error: {0}")]
    Xml(#[from] quick_xml::Error),

    /// Error de compresión/descompresión ZIP
    ///
    /// Ocurre cuando se trabaja con archivos EPUB comprimidos (archivos `.epub`):
    /// - Crear archivos ZIP
    /// - Extraer contenido
    /// - Leer entradas corruptas
    ///
    /// Se convierte automáticamente desde [`zip::result::ZipError`].
    ///
    /// # Nota
    /// Actualmente, esta biblioteca trabaja con EPUB **descomprimidos**.
    /// Este error se reserva para futuras funcionalidades de compresión.
    #[error("Zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    /// Error de validación del proyecto EPUB
    ///
    /// Ocurre cuando un proyecto EPUB no cumple con la estructura requerida:
    ///
    /// - Falta `META-INF/container.xml`
    /// - `container.xml` no tiene el elemento `rootfile`
    /// - Falta el atributo `full-path` en `rootfile`
    /// - El OPF no tiene sección `metadata`, `manifest` o `spine`
    /// - El directorio de destino para `new_project` no está vacío
    /// - `opf_path` no está cargado antes de llamar a `save()`
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::error::{GutenError, Result};
    /// # use gutencore::GutenCore;
    /// match GutenCore::open_folder("./carpeta_sin_epub") {
    ///     Err(GutenError::InvalidProject(msg)) => {
    ///         assert_eq!(msg, "META-INF/container.xml not found");
    ///     }
    ///     _ => {}
    /// }
    /// ```
    #[error("Invalid project: {0}")]
    InvalidProject(String),

    /// Error relacionado con el manifiesto del EPUB
    ///
    /// Ocurre cuando hay problemas con el `<manifest>` del OPF:
    /// - Referencia a un item que no existe en el disco
    /// - ID duplicado en el manifiesto
    /// - Tipo MIME inválido o no soportado
    /// - Falta un item requerido (ej: el archivo `nav`)
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::error::GutenError;
    /// let error = GutenError::Manifest("Item 'chap1' references missing file".to_string());
    /// println!("{}", error); // "Manifest error: Item 'chap1' references missing file"
    /// ```
    #[error("Manifest error: {0}")]
    Manifest(String),
    
    /// Error genérico para otros casos no cubiertos
    ///
    /// Se usa para errores que no encajan en las otras categorías:
    /// - Operaciones no implementadas
    /// - Condiciones inesperadas
    /// - Errores de lógica interna
    ///
    /// # Nota
    /// Si encuentras un caso recurrente que no está cubierto,
    /// considera agregar una variante específica en lugar de usar `Other`.
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::error::GutenError;
    /// let error = GutenError::Other("Unsupported EPUB version".to_string());
    /// println!("{}", error); // "Other error: Unsupported EPUB version"
    /// ```
    #[error("Other error: {0}")]
    Other(String),
}

/// Resultado especializado para operaciones de `gutencore`
///
/// Es un alias de [`Result`](std::result::Result) donde el tipo de error
/// siempre es [`GutenError`].
///
/// # Ejemplo
///
/// ```no_run
/// # use gutencore::error::{Result, GutenError};
/// # use gutencore::GutenCore;
/// fn abrir_epub(ruta: &str) -> Result<GutenCore> {
///     GutenCore::open_folder(ruta)  // Retorna Result<GutenCore, GutenError>
/// }
/// ```
///
/// # Uso con `?`
///
/// ```no_run
/// # use gutencore::error::Result;
/// # use gutencore::GutenCore;
/// fn ejemplo() -> Result<()> {
///     let core = GutenCore::open_folder("./mi_epub")?;  // Propaga GutenError automáticamente
///     Ok(())
/// }
/// ```
pub type Result<T> = std::result::Result<T, GutenError>;