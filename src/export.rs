use crate::core::GutenCore;
use crate::error::Result;
use std::fs::File;
use std::io::{Write, Read};
use std::path::Path;
use zip::write::FileOptions;
use zip::CompressionMethod;
use walkdir::WalkDir;
use path_slash::PathExt;

impl GutenCore {
        /// Exporta el espacio de trabajo a un archivo EPUB válido
    ///
    /// Este método empaqueta todo el proyecto EPUB descomprimido en un archivo
    /// `.epub` comprimido siguiendo estrictamente la especificación EPUB 3.0.
    /// Realiza validaciones previas, guarda cambios pendientes y construye
    /// el archivo ZIP con el orden y compresión correctos.
    ///
    /// # Proceso de exportación
    ///
    /// El método sigue estos pasos en orden:
    ///
    /// 1. **Validación de integridad** - Verifica que el proyecto sea válido
    ///    llamando a [`validate_integrity`](Self::validate_integrity)
    ///
    /// 2. **Alineación Total** - Llama a [`normalize_all_styles`](Self::normalize_all_styles)
    ///    para asegurar que todos los capítulos tengan los enlaces CSS correctos
    ///    basados en la configuración actual.
    ///
    /// 3. **Guardado automático** - Llama a [`save`](Self::save) para asegurar
    ///    que el OPF esté sincronizado con el estado actual
    ///
    /// 4. **Creación del archivo ZIP** - Construye el archivo `.epub` con:
    ///    - `mimetype` como primer archivo, **sin compresión** (requisito obligatorio)
    ///    - El resto de archivos comprimidos con DEFLATE
    ///    - Preservando la estructura de directorios
    ///    - Usando rutas con separadores `/` (estándar EPUB)
    ///
    /// # Estructura del EPUB generado
    ///
    /// ```text
    /// libro.epub
    /// ├── mimetype                          (primer archivo, sin compresión)
    /// ├── META-INF/
    /// │   └── container.xml
    /// └── OEBPS/
    ///     ├── content.opf
    ///     ├── Text/
    ///     │   ├── chap1.xhtml
    ///     │   └── nav.xhtml
    ///     ├── Styles/
    ///     │   └── style.css
    ///     ├── Images/     (Assets)
    ///     ├── Fonts/
    ///     ├── Audio/
    ///     ├── Video/
    ///     └── Misc/
    /// ```
    ///
    /// # Reglas de compresión
    ///
    /// | Archivo | Compresión | Razón |
    /// |---------|------------|-------|
    /// | `mimetype` | `Stored` (sin compresión) | **Requisito obligatorio** del estándar EPUB |
    /// | Todos los demás | `Deflated` (ZIP DEFLATE) | Optimización de tamaño |
    ///
    /// # Argumentos
    ///
    /// * `output_path` - Ruta donde se guardará el archivo EPUB generado
    ///   (ej: `"./mi_libro.epub"` o `"/ruta/completa/libro.epub"`)
    ///
    /// # Retorna
    ///
    /// * `Result<()>` - `Ok(())` si la exportación es exitosa, o un error
    ///   si falla la validación, el guardado o la compresión.
    ///
    /// # Errores
    ///
    /// Este método puede retornar los siguientes errores:
    ///
    /// * `GutenError::Other` - Si:
    ///   - La validación de integridad encuentra errores (los detalles se incluyen)
    ///   - Hay problemas al recorrer el directorio con `WalkDir`
    /// * `GutenError::Io` - Si falla:
    ///   - La creación del archivo de salida
    ///   - La lectura de archivos del proyecto
    ///   - La escritura en el ZIP
    /// * `GutenError::InvalidProject` - Si `save()` falla (propagado)
    /// * `GutenError::Xml` - Si `save()` falla por errores XML (propagado)
    ///
    /// # Ejemplo básico
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Abrir un proyecto existente
    /// let mut core = GutenCore::open_folder("./mi_proyecto_epub")?;
    ///
    /// // Exportar a archivo EPUB
    /// core.export_epub("./mi_libro.epub")?;
    /// println!("EPUB generado exitosamente!");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Ejemplo con proyecto nuevo
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // Crear un proyecto nuevo
    /// let mut core = GutenCore::new_project("./nuevo_libro", "Mi Novela", "es")?;
    ///
    /// // Hacer modificaciones...
    /// if let Some(meta) = &mut core.metadata {
    ///     meta.title = "El Gran Viaje".to_string();
    /// }
    ///
    /// // Exportar directamente (save() se llama automáticamente)
    /// core.export_epub("./el_gran_viaje.epub")?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Ejemplo con manejo de errores
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// # use gutencore::error::GutenError;
    /// let mut core = GutenCore::open_folder("./proyecto_invalido")?;
    ///
    /// match core.export_epub("./salida.epub") {
    ///     Ok(_) => println!("Exportación exitosa"),
    ///     Err(GutenError::Other(msg)) if msg.contains("Integrity check failed") => {
    ///         eprintln!("El proyecto no es válido: {}", msg);
    ///     }
    ///     Err(e) => eprintln!("Error durante la exportación: {}", e),
    /// }
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Notas importantes sobre el estándar EPUB
    ///
    /// - **`mimetype` primero**: El archivo `mimetype` **debe** ser el primer
    ///   archivo en el ZIP y **no puede estar comprimido**. Los lectores EPUB
    ///   usan esto para identificar el formato rápidamente.
    /// - **Rutas normalizadas**: Se usan barras normales `/` (no `\`) gracias a
    ///   `path_slash::PathExt`, compatible con el estándar EPUB.
    /// - **Permisos Unix**: Se establecen permisos `0o644` (rw-r--r--) para
    ///   compatibilidad multiplataforma.
    /// - **Validación previa**: El proyecto debe pasar `validate_integrity()`
    ///   antes de exportar, asegurando que todos los archivos referenciados existan.
    ///
    /// # Rendimiento
    ///
    /// - **Proyectos pequeños** (< 10MB): Generalmente < 1 segundo
    /// - **Proyectos grandes** (> 100MB): Puede tomar varios segundos debido a
    ///   la compresión DEFLATE de imágenes y archivos grandes.
    /// - **Optimización**: Si exportas frecuentemente, considera mantener el
    ///   proyecto abierto y solo llamar a `export_epub` cuando esté listo.
    ///
    /// # Advertencias
    ///
    /// - **Sobrescritura**: Si el archivo `output_path` ya existe, **será sobrescrito**.
    /// - **Proyecto temporal**: El proyecto original en disco no se modifica
    ///   (excepto por la llamada automática a `save()`, que actualiza el OPF).
    /// - **Memoria**: Los archivos se leen completamente en memoria antes de
    ///   comprimirse. Para archivos muy grandes (>500MB), considera implementar
    ///   streaming.
    ///
    /// # Ver también
    ///
    /// - [`save`](Self::save) - Guarda cambios en el OPF (llamado automáticamente)
    /// - [`validate_integrity`](Self::validate_integrity) - Validación previa a exportación
    /// - [`open_folder`](Self::open_folder) - Abre un proyecto descomprimido
    /// - [EPUB 3.3 Specification - OCF](https://www.w3.org/TR/epub/#sec-ocf) - Especificación oficial
    /// - [ZIP Archive Format](https://pkware.com/documents/casestudies/APPNOTE.TXT)
    pub fn export_epub(&mut self, output_path: impl AsRef<Path>) -> Result<()> {
        // 1. Validate integrity before export
        let errors = self.validate_integrity();
        if !errors.is_empty() {
            return Err(crate::error::GutenError::Other(format!("Integrity check failed: {:?}", errors)));
        }

        // 2. Normalize styles in all chapters (total alignment)
        self.normalize_all_styles()?;

        // 3. Save current state to OPF
        self.save()?;

        let file = File::create(output_path)?;
        let mut zip = zip::ZipWriter::new(file);

        // --- RULE OF GOLD ---
        // 3. mimetype must be the first file and UNCOMPRESSED
        let options_mimetype = FileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .unix_permissions(0o644);
        
        zip.start_file("mimetype", options_mimetype)?;
        zip.write_all(b"application/epub+zip")?;

        // 4. Add the rest of files with compression
        let options_deflated = FileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);

        // We traverse the workdir but skip mimetype (already added)
        for entry in WalkDir::new(&self.workdir).min_depth(1) {
            let entry = entry.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            let path = entry.path();
            
            if path.is_file() {
                let name = path.strip_prefix(&self.workdir).unwrap();
                let name_str = name.to_slash_lossy();

                if name_str == "mimetype"
                    || name_str.starts_with(crate::index::IndexDb::FILE_NAME)
                {
                    continue;
                }

                zip.start_file(name_str, options_deflated)?;
                let mut f = File::open(path)?;
                let mut buffer = Vec::new();
                f.read_to_end(&mut buffer)?;
                zip.write_all(&buffer)?;
            }
        }

        zip.finish()?;
        Ok(())
    }

    /// Realiza una "Alineación Total" de los estilos en todo el proyecto
    ///
    /// Este método recorre todos los documentos XHTML listados en el orden de
    /// lectura (`spine`) y vuelve a aplicar el proceso de sanitización. Esto
    /// asegura que:
    /// 1. Todos los `<link>` de CSS estén actualizados según `default_styles`
    ///    o las `exceptions` actuales.
    /// 2. No existan enlaces a archivos CSS que hayan sido eliminados.
    /// 3. El diseño sea consistente en todo el libro antes de la exportación.
    ///
    /// # Proceso
    /// - Itera sobre el `spine`.
    /// - Ignora recursos que no sean XHTML.
    /// - Lee el archivo completo, extrae su contenido y lo vuelve a guardar
    ///   pasándolo por [`sanitize_to_xhtml`](Self::sanitize_to_xhtml).
    ///
    /// # Errores
    /// - `std::io::Error` si falla la lectura/escritura de algún archivo.
    /// - `GutenError` si falla la sanitización.
    pub fn normalize_all_styles(&mut self) -> Result<()> {
        let spine = self.spine.clone();
        for id in &spine {
            if let Ok(item) = self.get_item(id) {
                if item.media_type == "application/xhtml+xml" {
                    let path = self.get_resource_path(id)?;
                    if path.exists() {
                        let content = std::fs::read_to_string(&path)?;
                        let normalized = self.sanitize_to_xhtml(id, &content)?;
                        std::fs::write(path, normalized)?;
                    }
                }
            }
        }
        Ok(())
    }
}
