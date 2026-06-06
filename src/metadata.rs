use crate::core::GutenCore;
use crate::types::BookMetadata;
use chrono::{SecondsFormat, Utc};

impl GutenCore {
    /// Obtiene una referencia a los metadatos del libro, si están cargados
    ///
    /// Los metadatos incluyen título, idioma, identificador único y fecha de modificación.
    /// Devuelve `None` si el proyecto no se ha cargado correctamente
    /// (ej: se usó [`new`](Self::new) sin cargar un proyecto).
    ///
    /// # Retorna
    /// * `Option<&BookMetadata>` - Referencia a los metadatos o `None` si no están disponibles
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let core = GutenCore::open_folder("./mi_epub")?;
    ///
    /// if let Some(metadata) = core.get_metadata() {
    ///     println!("Título: {}", metadata.title);
    ///     println!("Idioma: {}", metadata.language);
    ///     println!("Identificador: {}", metadata.identifier);
    ///     println!("Última modificación: {}", metadata.modified);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Ver también
    /// - [`set_metadata`](Self::set_metadata) - Para modificar los metadatos
    /// - [`BookMetadata`] - Estructura completa de metadatos
    ///
    pub fn get_metadata(&self) -> Option<&BookMetadata> {
        self.metadata.as_ref()
    }

    /// Alias semántico de [`get_metadata`](Self::get_metadata) que expone todos los metadatos
    /// extendidos (autor, serie, etiquetas, descripción, metadatos custom, etc.).
    pub fn get_extended_metadata(&self) -> Option<&BookMetadata> {
        self.get_metadata()
    }

    /// Actualiza selectivamente los metadatos del libro
    ///
    /// Este método permite modificar uno o más campos de los metadatos.
    /// Solo los campos proporcionados (`Some`) se actualizan; los campos `None`
    /// se ignoran y mantienen su valor actual.
    ///
    /// # Comportamiento especial
    ///
    /// - **Actualización automática de fecha**: Si al menos un campo cambia,
    ///   la fecha de modificación (`modified`) se actualiza automáticamente
    ///   a la hora actual en formato RFC 3339.
    /// - **No inicializa metadatos**: Si `self.metadata` es `None`, este método
    ///   no hace nada (no crea metadatos automáticamente).
    ///
    /// # Argumentos
    ///
    /// * `title` - Nuevo título del libro (`None` = no cambiar)
    /// * `language` - Nuevo código de idioma (`None` = no cambiar)
    /// * `identifier` - Nuevo identificador único (`None` = no cambiar)
    ///
    /// # Ejemplo básico
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    ///
    /// // Cambiar solo el título
    /// core.set_metadata(Some("Nuevo Título".to_string()), None, None);
    ///
    /// // Cambiar título e idioma
    /// core.set_metadata(
    ///     Some("El Gran Viaje".to_string()),
    ///     Some("es".to_string()),
    ///     None
    /// );
    ///
    /// // Cambiar todos los campos
    /// core.set_metadata(
    ///     Some("Mi Libro".to_string()),
    ///     Some("en".to_string()),
    ///     Some("urn:uuid:1234-5678".to_string())
    /// );
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Ejemplo con verificación de cambios
    ///
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    ///
    /// let fecha_antes = core.get_metadata().unwrap().modified.clone();
    /// std::thread::sleep(std::time::Duration::from_secs(1));
    ///
    /// // Actualizar metadatos
    /// core.set_metadata(Some("Título Actualizado".to_string()), None, None);
    ///
    /// let fecha_despues = core.get_metadata().unwrap().modified.clone();
    /// assert_ne!(fecha_antes, fecha_despues); // La fecha cambió automáticamente
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Notas importantes
    ///
    /// - **Persistencia**: Este método solo modifica la memoria. Para guardar
    ///   los cambios en disco, debes llamar a [`save`](Self::save) después.
    /// - **Metadatos no inicializados**: Si el proyecto no se ha cargado
    ///   correctamente (ej: usando [`new`](Self::new) sin `open_folder`),
    ///   este método no tendrá efecto.
    /// - **Formato de fecha**: La fecha se guarda en formato RFC 3339 con
    ///   precisión de segundos (ej: `"2024-01-15T10:30:00Z"`).
    ///
    /// # Ver también
    ///
    /// - [`get_metadata`](Self::get_metadata) - Para obtener los metadatos actuales
    /// - [`save`](Self::save) - Para persistir los cambios en disco
    /// - `update_modified_date` - Para actualizar solo la fecha
    /// - [`BookMetadata`] - Estructura completa
    pub fn set_metadata(
        &mut self,
        title: Option<String>,
        language: Option<String>,
        identifier: Option<String>,
    ) {
        if let Some(ref mut md) = self.metadata {
            let mut changed = false;
            if let Some(t) = title {
                md.title = t;
                changed = true;
            }
            if let Some(l) = language {
                md.language = l;
                changed = true;
            }
            if let Some(i) = identifier {
                md.identifier = i;
                changed = true;
            }

            if changed {
                md.modified = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
            }
        }
    }

    /// Actualiza selectivamente los metadatos extendidos del libro
    ///
    /// Permite modificar autor, serie, índice de serie, etiquetas, descripción
    /// y metadatos custom con prefijo (ej: `rubrica:source`).
    /// Solo los campos proporcionados (`Some(...)`) se actualizan;
    /// los campos `None` se ignoran y mantienen su valor actual.
    ///
    /// Para eliminar un campo, pasa `Some("".to_string())` (se convertirá en `None`)
    /// en `author`, `series`, o `Some(vec![])` en `tags`.
    ///
    /// # Argumentos
    ///
    /// * `author` - Nombre del autor (`None` = no cambiar)
    /// * `series` - Nombre de la serie (`None` = no cambiar)
    /// * `series_index` - Índice en la serie (`None` = no cambiar)
    /// * `tags` - Lista de etiquetas/temas (`None` = no cambiar)
    /// * `description` - Descripción del libro (`None` = no cambiar)
    /// * `custom_meta` - Metadatos custom (ej: `rubrica:source`) (`None` = no cambiar)
    pub fn set_extended_metadata(
        &mut self,
        author: Option<String>,
        series: Option<String>,
        series_index: Option<f32>,
        tags: Option<Vec<String>>,
        description: Option<String>,
        custom_meta: Option<std::collections::HashMap<String, String>>,
    ) {
        if let Some(ref mut md) = self.metadata {
            let mut changed = false;

            if let Some(a) = author {
                md.author = if a.is_empty() { None } else { Some(a) };
                changed = true;
            }
            if let Some(s) = series {
                md.series = if s.is_empty() { None } else { Some(s) };
                changed = true;
            }
            if let Some(i) = series_index {
                md.series_index = Some(i);
                changed = true;
            }
            if let Some(t) = tags {
                md.tags = t;
                changed = true;
            }
            if let Some(d) = description {
                md.description = if d.is_empty() { None } else { Some(d) };
                changed = true;
            }
            if let Some(c) = custom_meta {
                md.custom_meta = c;
                changed = true;
            }

            if changed {
                md.modified = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
            }
        }
    }
    /// Actualiza la fecha de modificación de los metadatos a la hora actual
    ///
    /// Este método interno establece el campo `modified` de los metadatos
    /// a la fecha y hora actual en formato RFC 3339 (precisión de segundos).
    ///
    /// # Cuándo se usa
    ///
    /// Este método es llamado automáticamente en:
    /// - [`save`](Self::save) - Antes de guardar el OPF
    /// - [`set_metadata`](Self::set_metadata) - Cuando cambia algún campo
    ///
    /// # Comportamiento
    ///
    /// - Si `self.metadata` es `Some`, actualiza su campo `modified`
    /// - Si `self.metadata` es `None`, no hace nada (silenciosamente)
    ///
    /// # Ejemplo de uso manual
    ///
    /// ```ignore
    /// # use gutencore::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    ///
    /// // Realizar cambios que no son capturados por set_metadata
    /// // (ej: modificar directamente core.metadata)
    /// if let Some(meta) = &mut core.metadata {
    ///     meta.title = "Título Manual".to_string();
    /// }
    ///
    /// // Forzar actualización de la fecha de modificación
    /// core.update_modified_date();
    ///
    /// // Guardar los cambios
    /// core.save()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Notas de implementación
    ///
    /// - **Método privado**: Aunque está marcado como `pub(crate)`, solo es
    ///   accesible dentro del crate. No forma parte de la API pública.
    /// - **Formato de fecha**: Usa `SecondsFormat::Secs` para omitir fracciones
    ///   de segundo, que es suficiente para el estándar EPUB.
    /// - **TimeZone**: Usa UTC (Zulu time) como requiere el estándar EPUB.
    ///
    /// # Ver también
    ///
    /// - [`set_metadata`](Self::set_metadata) - Actualiza metadatos + fecha
    /// - [`save`](Self::save) - Guarda cambios (llama a este método)
    /// - [RFC 3339](https://datatracker.ietf.org/doc/html/rfc3339) - Formato de fecha
    pub(crate) fn update_modified_date(&mut self) {
        if let Some(ref mut md) = self.metadata {
            md.modified = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
        }
    }
}
