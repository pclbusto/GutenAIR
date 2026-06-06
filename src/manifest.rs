use crate::core::GutenCore;
use crate::error::{GutenError, Result};
use crate::types::{EpubHealthReport, ManifestItem};
use path_slash::{PathBufExt, PathExt};
use std::fs;
use std::path::{Path, PathBuf};

impl GutenCore {
    // -------------------------
    // Spine Operations
    // -------------------------

    /// Obtiene una referencia al orden de lectura (spine) actual
    ///
    /// El spine define el orden secuencial en que un lector de EPUB debe
    /// presentar los documentos al usuario.
    ///
    /// # Retorna
    /// * `&Vec<String>` - Lista de IDs en orden de lectura
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let core = GutenCore::open_folder("./mi_epub")?;
    /// let spine = core.get_spine();
    /// for (i, id) in spine.iter().enumerate() {
    ///     println!("{}. {}", i + 1, id);
    /// }
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_spine(&self) -> &Vec<String> {
        &self.spine
    }

    /// Establece un nuevo orden de lectura (spine)
    ///
    /// Reemplaza completamente el spine actual con la lista de IDs proporcionada.
    /// Los IDs deben corresponder a recursos válidos en el manifiesto.
    ///
    /// # Argumentos
    /// * `idrefs` - `Vec<String>` con los IDs en el nuevo orden deseado
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    /// let new_order = vec!["id_portada".to_string(), "id_cap1".to_string(), "id_cap2".to_string(), "id_indice".to_string()];
    /// core.set_spine(new_order);
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn set_spine(&mut self, idrefs: Vec<String>) {
        self.spine = idrefs;
    }

    /// Inserta un ID en el spine en una posición específica
    ///
    /// Si el ID ya existe en el spine, no hace nada (evita duplicados).
    /// Si no se especifica índice, lo agrega al final.
    ///
    /// # Argumentos
    /// * `idref` - ID del item a insertar
    /// * `index` - Posición donde insertar (None = al final)
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    /// // Insertar al principio
    /// core.spine_insert("cover".to_string(), Some(0));
    /// // Insertar al final
    /// core.spine_insert("appendix".to_string(), None);
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn spine_insert(&mut self, idref: String, index: Option<usize>) {
        if self.spine.contains(&idref) {
            return;
        }
        match index {
            Some(i) => {
                let pos = i.min(self.spine.len());
                self.spine.insert(pos, idref);
            }
            None => self.spine.push(idref),
        }
    }
    /// Mueve un ID existente a una nueva posición en el spine
    ///
    /// # Argumentos
    /// * `idref` - ID del item a mover
    /// * `new_index` - Nueva posición (0 = primero)
    ///
    /// # Errores
    /// * `GutenError::Manifest` - Si el ID no existe en el spine
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    /// // Mover el capítulo 3 al principio
    /// core.spine_move("chap3", 0)?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn spine_move(&mut self, idref: &str, new_index: usize) -> Result<()> {
        let pos = self
            .spine
            .iter()
            .position(|r| r == idref)
            .ok_or_else(|| GutenError::Manifest(format!("{} not in spine", idref)))?;

        let idref_owned = self.spine.remove(pos);
        let target_pos = new_index.min(self.spine.len());
        self.spine.insert(target_pos, idref_owned);
        Ok(())
    }
    /// Elimina un ID del spine
    ///
    /// Si el ID no existe, no hace nada (comportamiento silencioso).
    ///
    /// # Argumentos
    /// * `idref` - ID del item a eliminar del orden de lectura
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    /// core.spine_remove("appendix");
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn spine_remove(&mut self, idref: &str) {
        if let Some(pos) = self.spine.iter().position(|r| r == idref) {
            self.spine.remove(pos);
        }
    }

    /// Devuelve `true` si el EPUB tiene una imagen de portada registrada.
    ///
    /// Soporta tanto EPUB 3 (`properties="cover-image"` en el manifiesto)
    /// como EPUB 2 (`<meta name="cover" content="id"/>` en el OPF).
    pub fn has_cover(&self) -> bool {
        // EPUB 3 convention
        if self
            .manifest
            .values()
            .any(|it| it.properties == "cover-image")
        {
            return true;
        }
        // EPUB 2 convention
        if let Some(ref cover_id) = self.cover_image_id {
            return self.manifest.contains_key(cover_id);
        }
        false
    }

    /// Devuelve el `ManifestItem` de la imagen de portada, si existe.
    ///
    /// Soporta tanto EPUB 3 como EPUB 2.
    pub fn get_cover_image(&self) -> Option<&ManifestItem> {
        // EPUB 3 convention
        if let Some(item) = self
            .manifest
            .values()
            .find(|it| it.properties == "cover-image")
        {
            return Some(item);
        }
        // EPUB 2 convention
        if let Some(ref cover_id) = self.cover_image_id {
            return self.manifest.get(cover_id);
        }
        None
    }

    // -------------------------
// Manifest Operations
// -------------------------

    /// Agrega un nuevo recurso al manifiesto
    ///
    /// # Argumentos
    /// * `id` - ID único del recurso
    /// * `href` - Ruta relativa del recurso
    /// * `media_type` - Tipo MIME (ej: "application/xhtml+xml")
    /// * `properties` - Propiedades del recurso (ej: "nav")
    ///
    /// # Errores
    /// * `GutenError::Manifest` - Si el ID ya existe
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    /// core.add_to_manifest("new_chapter".to_string(), "Text/new.xhtml".to_string(), "application/xhtml+xml".to_string(), "".to_string());
    /// # Ok::<_, Box<dyn std::error::Error>>(())  
    /// ```
    pub fn add_to_manifest(
        &mut self,
        id: String,
        href: String,
        media_type: String,
        properties: String,
    ) -> Result<()> {
        if self.manifest.contains_key(&id) {
            return Err(GutenError::Manifest(format!("ID {} already exists", id)));
        }

        // Normalize href to Unix style for safety (handle both separators)
        let clean_href = href.replace('\\', "/");

        let item = ManifestItem {
            id: id.clone(),
            href: clean_href,
            media_type,
            properties,
        };

        self.manifest.insert(id, item);
        Ok(())
    }
    /// Elimina un recurso del manifiesto
    ///
    /// # Argumentos
    /// * `id` - ID del recurso a eliminar
    ///
    /// # Errores
    /// * `GutenError::Manifest` - Si el ID no existe
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    /// core.remove_from_manifest("new_chapter");
    /// # Ok::<_, Box<dyn std::error::Error>>(())  
    /// ```
    pub fn remove_from_manifest(&mut self, id: &str) -> Result<()> {
        self.spine_remove(id);
        self.manifest
            .remove(id)
            .ok_or_else(|| GutenError::Manifest(format!("ID {} not found in manifest", id)))?;

        Ok(())
    }
    /// Elimina físicamente un recurso del spine, el manifiesto y el disco.
    ///
    /// # Argumentos
    /// * `id` - ID del recurso a eliminar
    ///
    /// # Errores
    /// * `GutenError::Manifest` - Si el ID no existe
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    /// core.delete_item("new_chapter");
    /// # Ok::<_, Box<dyn std::error::Error>>(())  
    /// ```
    pub fn delete_item(&mut self, id: &str) -> Result<()> {
        let path = self.get_resource_path(id)?;

        if let Some(db) = &self.index_db {
            let _ = db.clear_chapter(id);
        }

        self.remove_from_manifest(id)?;

        if path.exists() {
            fs::remove_file(path)?;
        }

        Ok(())
    }

    /// Elimina varios recursos y persiste el OPF antes de borrar los archivos físicos.
    ///
    /// A diferencia de [`delete_item`](Self::delete_item), este método encapsula el ciclo
    /// completo pensado para UIs: primero valida las rutas, luego quita los items del
    /// manifiesto/spine en memoria, guarda el OPF actualizado y recién después elimina
    /// los archivos del disco. Así se evita dejar el OPF apuntando a archivos ya borrados
    /// si el guardado falla.
    pub fn delete_items_and_save<I, S>(&mut self, ids: I) -> Result<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let ids: Vec<String> = ids.into_iter().map(|id| id.as_ref().to_string()).collect();
        let mut paths = Vec::new();

        for id in &ids {
            paths.push((id.clone(), self.get_resource_path(id)?));
        }

        for (id, _) in &paths {
            if let Some(db) = &self.index_db {
                let _ = db.clear_chapter(id);
            }
            self.remove_from_manifest(id)?;
        }

        self.save()?;

        for (_, path) in &paths {
            if path.exists() {
                fs::remove_file(path)?;
            }
        }

        self.build_index()?;

        Ok(())
    }
    /// Obtiene un item del manifiesto por ID o ruta (href)
    ///
    /// # Argumentos
    /// * `id_or_href` - Puede ser el ID del item o su ruta (href)
    ///
    /// # Retorna
    /// * `Result<&ManifestItem>` - Referencia al item encontrado
    ///
    /// # Errores
    /// * `GutenError::Manifest` - Si no se encuentra ningún item con ese ID o ruta
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let core = GutenCore::open_folder("./mi_epub")?;
    /// // Buscar por ID
    /// let item = core.get_item("chap1")?;
    /// // Buscar por ruta
    /// let item2 = core.get_item("Text/chap1.xhtml")?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_item(&self, id_or_href: &str) -> Result<&ManifestItem> {
        self.manifest
            .get(id_or_href)
            .or_else(|| self.manifest.values().find(|it| it.href == id_or_href))
            .ok_or_else(|| GutenError::Manifest(format!("Resource not found: {}", id_or_href)))
    }

    /// Obtiene la ruta absoluta de un recurso en el disco
    ///
    /// # Argumentos
    /// * `id_or_href` - ID o ruta del item
    ///
    /// # Retorna
    /// * `Result<PathBuf>` - Ruta absoluta al archivo en el sistema de archivos
    ///
    /// # Errores
    /// * `GutenError::InvalidProject` - Si `opf_dir` no está cargado
    /// * `GutenError::Manifest` - Si el item no existe
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let core = GutenCore::open_folder("./mi_epub")?;
    /// let path = core.get_resource_path("chap1")?;
    /// println!("El archivo está en: {}", path.display());
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_resource_path(&self, id_or_href: &str) -> Result<PathBuf> {
        let item = self.get_item(id_or_href)?;
        let opf_dir = self
            .opf_dir
            .as_ref()
            .ok_or_else(|| GutenError::InvalidProject("OPF dir not loaded".into()))?;

        Ok(opf_dir.join(&item.href))
    }

    /// Valida que todos los archivos referenciados en el manifiesto existan en disco
    ///
    /// # Retorna
    /// * `Vec<String>` - Lista de errores encontrados (vacío si todo está bien)
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let core = GutenCore::open_folder("./mi_epub")?;
    /// let errores = core.validate_integrity();
    /// if !errores.is_empty() {
    ///     for err in errores {
    ///         eprintln!("{}", err);
    ///     }
    /// }
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn validate_integrity(&self) -> Vec<String> {
        let mut errors = Vec::new();
        for item in self.manifest.values() {
            match self.get_resource_path(&item.id) {
                Ok(path) => {
                    if !path.exists() {
                        errors.push(format!("Archivo faltante: {}", item.href));
                    }
                }
                Err(e) => errors.push(format!("Error resolviendo {}: {}", item.href, e)),
            }
        }
        errors
    }

    /// Renombra un archivo del proyecto y actualiza su referencia en el manifiesto
    ///
    /// # Argumentos
    /// * `id` - ID del item a renombrar
    /// * `new_href` - Nueva ruta relativa (desde el directorio OPF)
    ///
    /// # Errores
    /// * `GutenError::Manifest` - Si el ID no existe
    /// * `std::io::Error` - Si falla el renombrado físico del archivo
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    /// core.rename_item("chap1", "Text/chapter_one.xhtml".to_string())?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn rename_item(&mut self, id: &str, new_href: String) -> Result<()> {
        let item = self
            .manifest
            .get_mut(id)
            .ok_or_else(|| GutenError::Manifest(format!("ID {} not found", id)))?;

        // Normalize the new href (handle both separators)
        let clean_href = new_href.replace('\\', "/");

        // Physical rename
        if let (Some(opf_dir), old_href) = (&self.opf_dir, &item.href) {
            let old_path = opf_dir.join(old_href);
            // Use native path for FS operations
            let new_path_native = if cfg!(windows) {
                opf_dir.join(&clean_href.replace('/', "\\"))
            } else {
                opf_dir.join(&clean_href)
            };

            if old_path.exists() {
                if let Some(parent) = new_path_native.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::rename(old_path, new_path_native)?;
            }
        }

        item.href = clean_href;
        Ok(())
    }

    /// Sanitiza un nombre de archivo eliminando caracteres no válidos
    ///
    /// Solo permite:
    /// - Letras y números (a-z, A-Z, 0-9)
    /// - Guión bajo `_`
    /// - Guión medio `-`
    /// - Punto `.`
    ///
    /// # Argumentos
    /// * `name` - Nombre potencialmente con caracteres inválidos
    ///
    /// # Retorna
    /// * `String` - Nombre sanitizado, seguro para usar como nombre de archivo
    ///
    /// # Ejemplo
    /// ```rust
    /// # use gutencore::GutenCore;
    /// let core = GutenCore::new("./proyecto");
    /// let sanitized = core.sanitize_filename("Mi: Capítulo/1!");
    /// assert_eq!(sanitized, "MiCapítulo1");
    /// ```
    pub fn sanitize_filename(&self, name: &str) -> String {
        name.chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
            .collect::<String>()
    }

    // -------------------------
    // High-Level Orchestration
    // -------------------------

    /// Agrega un nuevo recurso desde memoria (escribe a disco y actualiza manifiesto)
    ///
    /// # Argumentos
    /// * `id` - ID único para el manifiesto
    /// * `bytes` - Contenido del archivo en bytes
    /// * `mime_type` - Tipo MIME del recurso
    /// * `target_href` - Ruta relativa destino (desde el directorio OPF)
    ///
    /// # Errores
    /// * `GutenError::Manifest` - Si el archivo ya existe o el ID ya está en uso
    /// * `std::io::Error` - Si falla la escritura del archivo
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    /// let css = "body { font-family: serif; }".as_bytes();
    /// core.add_resource("custom_css".to_string(), css, "text/css", "Styles/custom.css")?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn add_resource(
        &mut self,
        id: String,
        bytes: &[u8],
        mime_type: &str,
        target_href: &str,
    ) -> Result<()> {
        let clean_href = target_href.replace('\\', "/");
        let opf_dir = self
            .opf_dir
            .as_ref()
            .ok_or_else(|| GutenError::InvalidProject("OPF dir not loaded".into()))?;
        let full_path = opf_dir.join(&clean_href);

        if full_path.exists() {
            return Err(GutenError::Manifest(format!(
                "File already exists: {}",
                clean_href
            )));
        }

        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(full_path, bytes)?;

        self.add_to_manifest(id, clean_href, mime_type.to_string(), "".to_string())
    }

    /// Importa un archivo del sistema local al proyecto
    ///
    /// # Argumentos
    /// * `source_path` - Ruta del archivo origen en el sistema
    /// * `id` - ID único para el manifiesto
    /// * `target_href` - Ruta relativa destino en el proyecto
    /// * `mime_type` - Tipo MIME del recurso
    ///
    /// # Errores
    /// * `std::io::Error` - Si falla la lectura del archivo origen
    /// * `GutenError::Manifest` - Si el destino ya existe o el ID está en uso
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    /// core.import_file("./logo.png", "logo".to_string(), "Images/logo.png", "image/png")?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn import_file(
        &mut self,
        source_path: impl AsRef<Path>,
        id: String,
        target_href: &str,
        mime_type: &str,
    ) -> Result<()> {
        let bytes = fs::read(source_path)?;
        self.add_resource(id, &bytes, mime_type, target_href)
    }

    /// Calcula la ruta relativa entre dos recursos (para enlaces internos)
    ///
    /// # Argumentos
    /// * `from_id` - ID del documento que contiene el enlace
    /// * `to_id` - ID del documento destino
    ///
    /// # Retorna
    /// * `Result<String>` - Ruta relativa con separadores `/`
    ///
    /// # Errores
    /// * `GutenError::Manifest` - Si alguno de los IDs no existe
    /// * `GutenError::Other` - Si no se puede calcular la ruta relativa
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let core = GutenCore::open_folder("./mi_epub")?;
    /// let rel_path = core.get_relative_path("chap1", "chap2")?;
    /// // Si chap1 está en Text/ y chap2 en Text/, resultado: "chap2.xhtml"
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_relative_path(&self, from_id: &str, to_id: &str) -> Result<String> {
        let from_item = self.get_item(from_id)?;
        let to_item = self.get_item(to_id)?;

        let from_path = Path::new(&from_item.href);
        let to_path = Path::new(&to_item.href);

        let from_parent = from_path.parent().unwrap_or(Path::new(""));

        let rel = pathdiff::diff_paths(to_path, from_parent).ok_or_else(|| {
            GutenError::Other(format!(
                "Could not calculate relative path from {} to {}",
                from_item.href, to_item.href
            ))
        })?;

        Ok(rel.to_slash_lossy().to_string())
    }

    /// Validación profunda: verifica archivos en manifiesto Y archivos huérfanos en disco
    ///
    /// # Retorna
    /// * `(Vec<String>, Vec<PathBuf>)` - Tupla con:
    ///   - Lista de errores del manifiesto (archivos faltantes)
    ///   - Lista de archivos huérfanos (en disco pero no en manifiesto)
    ///
    /// # Archivos ignorados
    /// - `content.opf` (el propio OPF)
    /// - `mimetype` (archivo raíz del EPUB)
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let core = GutenCore::open_folder("./mi_epub")?;
    /// let (manifest_errors, orphans) = core.validate_integrity_deep();
    ///
    /// if !manifest_errors.is_empty() {
    ///     println!("Archivos faltantes: {:?}", manifest_errors);
    /// }
    /// if !orphans.is_empty() {
    ///     println!("Archivos no referenciados: {:?}", orphans);
    /// }
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn validate_integrity_deep(&self) -> (Vec<String>, Vec<PathBuf>) {
        let manifest_errors = self.validate_integrity();
        let mut orphans = Vec::new();

        if let Some(opf_dir) = &self.opf_dir {
            let known_hrefs: std::collections::HashSet<&str> =
                self.manifest.values().map(|it| it.href.as_str()).collect();

            for entry in walkdir::WalkDir::new(opf_dir).min_depth(1) {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_file() {
                        let rel_path = path.strip_prefix(opf_dir).unwrap();
                        let href = rel_path.to_slash_lossy();

                        if href == "content.opf" || href == "../mimetype" {
                            continue;
                        }

                        if !known_hrefs.contains(href.as_ref()) {
                            orphans.push(path.to_path_buf());
                        }
                    }
                }
            }
        }

        (manifest_errors, orphans)
    }

    /// Validación completa de salud estructural del EPUB.
    ///
    /// Combina y extiende las validaciones existentes en un solo reporte:
    /// - `broken_links`: enlaces internos que apuntan a destinos inexistentes
    /// - `orphan_anchors`: anclajes (`id`) que no son referenciados por ningún enlace
    /// - `missing_manifest_entries`: items en el manifiesto cuyos archivos no existen en disco
    /// - `opf_well_formed`: true si el OPF pudo ser parseado correctamente
    pub fn validate(&self) -> Result<EpubHealthReport> {
        let opf_well_formed = self.metadata.is_some() && self.opf_path.is_some();

        let missing_manifest_entries = self
            .validate_integrity()
            .into_iter()
            .map(|e| {
                if let Some(stripped) = e.strip_prefix("Archivo faltante: ") {
                    stripped.to_string()
                } else {
                    e
                }
            })
            .collect();

        let broken_links = self.validate_links().unwrap_or_default();

        // Find orphan anchors: hooks that exist but aren't referenced
        let mut orphan_anchors: Vec<String> = Vec::new();
        if let Some(db) = &self.index_db {
            let all_links = db.get_all_links().unwrap_or_default();
            let mut referenced_hooks: std::collections::HashSet<String> =
                std::collections::HashSet::new();

            for (_chapter, href) in &all_links {
                if let Some(hook) = href.strip_prefix('#') {
                    referenced_hooks.insert(hook.to_string());
                } else if let Some(pos) = href.find('#') {
                    referenced_hooks.insert(href[pos + 1..].to_string());
                }
            }

            // Build hook index from all manifest XHTML items to find declared anchors
            let hook_index = self.build_hook_index().unwrap_or_default();

            for hooks in hook_index.values() {
                for hook in hooks {
                    if !referenced_hooks.contains(&hook.hook_id) {
                        let full = format!("{}#{}", hook.file_href, hook.hook_id);
                        orphan_anchors.push(full);
                    }
                }
            }
        }

        Ok(EpubHealthReport {
            broken_links,
            orphan_anchors,
            missing_manifest_entries,
            opf_well_formed,
        })
    }

    /// Alias de [`validate`](Self::validate) — reporte completo de salud estructural del EPUB.
    ///
    /// Devuelve un `EpubHealthReport` con enlaces rotos, anclajes huérfanos,
    /// entradas faltantes en el manifiesto y estado del OPF.
    pub fn get_health_report(&self) -> Result<EpubHealthReport> {
        self.validate()
    }

    /// Agrega un documento nuevo al proyecto (sanitiza, escribe y registra en manifiesto)
    ///
    /// Este es el método principal para crear nuevos capítulos. Automáticamente:
    /// 1. Sanitiza el contenido con [`sanitize_to_xhtml`](Self::sanitize_to_xhtml)
    ///    - Inyecta automáticamente los estilos (globales o por excepción).
    /// 2. Escribe el archivo en `OEBPS/Text/{id}.xhtml`
    /// 3. Registra el item en el manifiesto
    ///
    /// # Argumentos
    /// * `id` - ID único para el manifiesto (se usa como nombre base del archivo)
    /// * `raw_content` - Contenido HTML/texto (se sanitiza automáticamente)
    ///
    /// # Errores
    /// * `GutenError::Manifest` - Si el ID ya existe en el manifiesto
    /// * `GutenError::InvalidProject` - Si `opf_dir` no está cargado
    /// * `std::io::Error` - Si falla la escritura del archivo
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    ///
    /// let contenido = r#"
    ///     <h1>Capítulo 2</h1>
    ///     <p>Este es el segundo capítulo.</p>
    ///     <script>alert('Esto se elimina');</script>
    /// "#;
    ///
    /// core.add_document("chap2", contenido)?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Nota
    /// Este método **no** agrega automáticamente el documento al spine.
    /// Después de crear el documento, usa [`spine_insert`](Self::spine_insert)
    /// para incluirlo en el orden de lectura.
    ///
    /// # Ver también
    /// - [`save_chapter`](Self::save_chapter) - Para actualizar un capítulo existente
    /// - [`spine_insert`](Self::spine_insert) - Para agregar al orden de lectura
    pub fn add_document(&mut self, id: &str, raw_content: &str) -> Result<()> {
        let href = format!("Text/{}.xhtml", id);

        if self.manifest.contains_key(id) {
            return Err(GutenError::Manifest(format!(
                "ID '{}' already exists in manifest",
                id
            )));
        }

        let clean_xhtml = self.sanitize_to_xhtml(id, raw_content)?;

        let opf_dir = self
            .opf_dir
            .as_ref()
            .ok_or_else(|| GutenError::InvalidProject("OPF dir not loaded".into()))?;
        let full_path = opf_dir.join(&href);

        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full_path, &clean_xhtml)?;

        if let Some(db) = &self.index_db {
            if db.index_xhtml(id, &clean_xhtml).is_err() {
                self.index_dirty = true;
            }
        }

        self.add_to_manifest(
            id.to_string(),
            href,
            "application/xhtml+xml".to_string(),
            "".to_string(),
        )
    }

    /// Agrega una nueva hoja de estilo CSS al proyecto
    ///
    /// Este método automatiza la creación de archivos CSS:
    /// 1. Escribe el contenido en `OEBPS/Styles/{id}.css`
    /// 2. Registra el recurso en el manifiesto con el tipo MIME `text/css`
    ///
    /// # Argumentos
    /// * `id` - ID único para el manifiesto (se usa como nombre base del archivo)
    /// * `css_content` - Contenido CSS en texto plano
    ///
    /// # Errores
    /// * `GutenError::Manifest` - Si el ID ya existe en el manifiesto
    /// * `GutenError::InvalidProject` - Si `opf_dir` no está cargado
    /// * `std::io::Error` - Si falla la escritura del archivo
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let mut core = GutenCore::open_folder("./mi_epub")?;
    ///
    /// let css = "body { background: #f0f0f0; }";
    /// core.add_style("tema-oscuro", css)?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Nota
    /// Para que este estilo se inyecte automáticamente en los capítulos,
    /// debes agregarlo a la configuración usando [`set_style_as_default`](Self::set_style_as_default).
    ///
    /// # Ver también
    /// - [`add_resource`](Self::add_resource) - Versión de bajo nivel para cualquier tipo de archivo
    /// - [`set_style_as_default`](Self::set_style_as_default) - Para activar el estilo en el proyecto
    pub fn add_style(&mut self, id: &str, css_content: &str) -> Result<()> {
        let href = format!("Styles/{}.css", id);

        if self.manifest.contains_key(id) {
            return Err(GutenError::Manifest(format!(
                "ID '{}' already exists in manifest",
                id
            )));
        }

        let opf_dir = self
            .opf_dir
            .as_ref()
            .ok_or_else(|| GutenError::InvalidProject("OPF dir not loaded".into()))?;
        let full_path = opf_dir.join(&href);

        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full_path, css_content)?;

        self.add_to_manifest(
            id.to_string(),
            href,
            "text/css".to_string(),
            "".to_string(),
        )
    }

    /// Establece la carátula del libro a partir de una imagen externa.
    ///
    /// Este método realiza las siguientes acciones:
    /// 1. Importa la imagen al directorio `OEBPS/Images/` con el nombre `cover.[ext]`.
    /// 2. Si ya existía una imagen de portada con otra extensión, elimina la anterior.
    /// 3. Crea o actualiza un documento XHTML `OEBPS/Text/cover.xhtml`.
    /// 4. Asegura que el documento de portada sea el primero en el orden de lectura (`spine`).
    /// 5. Marca los recursos con las propiedades EPUB 3 necesarias (`cover-image` y `cover`).
    ///
    /// # Argumentos
    /// * `source_image_path` - Ruta local a la imagen que se usará como portada.
    ///
    /// # Errores
    /// * `GutenError::Io` - Si la imagen no existe o no se puede leer.
    /// * `GutenError::Manifest` - Si hay errores al manipular el manifiesto.
    ///
    /// # Ejemplo
    /// ```no_run
    /// # use gutencore::GutenCore;
    /// let mut core = GutenCore::open_folder("./mi_proyecto")?;
    /// core.set_cover("./portada_final.jpg")?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn set_cover(&mut self, source_image_path: impl AsRef<Path>) -> Result<()> {
        let src_path = source_image_path.as_ref();
        if !src_path.exists() {
            return Err(GutenError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Cover image not found: {}", src_path.display()),
            )));
        }

        let extension = src_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("jpg")
            .to_lowercase();

        let mime_type = match extension.as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "svg" => "image/svg+xml",
            "webp" => "image/webp",
            _ => "image/jpeg", // fallback razonable
        };

        let img_id = "cover-image";
        let img_href = format!("Images/cover.{}", extension);

        // 1. Manejar la imagen vieja si existe (borrar archivo físico para evitar basura)
        if self.manifest.contains_key(img_id) {
            self.delete_item(img_id)?;
        }

        // 2. Importar la nueva imagen
        let bytes = fs::read(src_path)?;
        self.add_resource(img_id.to_string(), &bytes, mime_type, &img_href)?;

        // 3. Marcar como cover-image en el manifiesto (Requisito EPUB 3)
        if let Some(item) = self.manifest.get_mut(img_id) {
            item.properties = "cover-image".to_string();
        }

        // 4. Crear el XHTML de la portada
        // Usamos una ruta relativa a Text/ que es "../Images/cover.ext"
        let xhtml_content = format!(
            r#"<div style="text-align: center; padding: 0; margin: 0;">
  <img src="../{}" alt="Cover" style="max-width: 100%; height: auto;" />
</div>"#,
            img_href
        );

        let doc_id = "cover";
        if self.manifest.contains_key(doc_id) {
            // Si ya existe, lo actualizamos (esto lo pasará por sanitize_to_xhtml)
            self.save_chapter(doc_id, &xhtml_content)?;
        } else {
            // Si no existe, lo creamos
            self.add_document(doc_id, &xhtml_content)?;
            // Y lo ponemos al principio del spine
            self.spine_insert(doc_id.to_string(), Some(0));
            // Marcar el XHTML con propiedad cover
            if let Some(item) = self.manifest.get_mut(doc_id) {
                item.properties = "cover".to_string();
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_delete_item() -> Result<()> {
        let dir = tempdir()?;
        let mut core = GutenCore::new_project(dir.path(), "Test Book", "en")?;

        // Ensure chap1 exists (auto-created by new_project)
        assert!(core.manifest.contains_key("chap1"));
        let chap1_path = core.get_resource_path("chap1")?;
        assert!(chap1_path.exists());
        assert!(core.spine.contains(&"chap1".to_string()));

        // Delete chap1
        core.delete_item("chap1")?;

        // Verify deletion
        assert!(!core.manifest.contains_key("chap1"));
        assert!(!chap1_path.exists());
        assert!(!core.spine.contains(&"chap1".to_string()));

        Ok(())
    }

    #[test]
    fn test_delete_items_and_save_persists_manifest_before_file_removal() -> Result<()> {
        let dir = tempdir()?;
        let mut core = GutenCore::new_project(dir.path(), "Delete Batch Test", "en")?;
        core.add_document("extra", "<h1>Extra</h1><p>Content</p>")?;

        let extra_path = core.get_resource_path("extra")?;
        assert!(extra_path.exists());
        assert!(core.manifest.contains_key("extra"));

        core.delete_items_and_save(["extra"])?;

        assert!(!extra_path.exists());
        let reloaded = GutenCore::open_folder(dir.path())?;
        assert!(!reloaded.manifest.contains_key("extra"));
        assert!(!reloaded.spine.contains(&"extra".to_string()));

        Ok(())
    }

    #[test]
    fn test_new_project_folders() -> Result<()> {
        let dir = tempdir()?;
        let project_path = dir.path().join("my_book");
        GutenCore::new_project(&project_path, "Test Book", "en")?;

        let folders = vec![
            "OEBPS/Text",
            "OEBPS/Styles",
            "OEBPS/Images",
            "OEBPS/Fonts",
            "OEBPS/Audio",
            "OEBPS/Video",
            "OEBPS/Misc",
            "META-INF",
        ];

        for folder in folders {
            let path = project_path.join(folder);
            assert!(path.exists(), "Folder {} should exist", folder);
            assert!(path.is_dir(), "Path {} should be a directory", folder);
        }

        Ok(())
    }

    #[test]
    fn test_set_cover() -> Result<()> {
        let dir = tempdir()?;
        let mut core = GutenCore::new_project(dir.path(), "Cover Test", "es")?;

        // Crear una imagen falsa
        let img_path = dir.path().join("test_cover.jpg");
        fs::write(&img_path, b"fake image data")?;

        // Establecer portada
        core.set_cover(&img_path)?;

        // Verificar manifiesto
        assert!(core.manifest.contains_key("cover-image"));
        assert!(core.manifest.contains_key("cover"));

        // Verificar propiedades
        assert_eq!(
            core.manifest.get("cover-image").unwrap().properties,
            "cover-image"
        );
        assert_eq!(core.manifest.get("cover").unwrap().properties, "cover");

        // Verificar spine (debe ser el primero)
        assert_eq!(core.spine[0], "cover");

        // Verificar contenido del archivo
        let cover_path = core.get_resource_path("cover")?;
        let content = fs::read_to_string(&cover_path)?;
        assert!(content.contains("Images/cover.jpg"));

        // Probar cambio de imagen con otra extensión
        let png_path = dir.path().join("other.png");
        fs::write(&png_path, b"fake png data")?;
        core.set_cover(&png_path)?;

        assert!(core.manifest.get("cover-image").unwrap().href.ends_with(".png"));
        let content2 = fs::read_to_string(&cover_path)?;
        assert!(content2.contains("Images/cover.png"));
        assert!(!content2.contains("Images/cover.jpg"));

        Ok(())
    }

    #[test]
    fn test_index_no_ghost_after_delete() -> Result<()> {
        let dir = tempdir()?;
        let mut core = GutenCore::new_project(dir.path(), "Ghost Test", "en")?;

        core.add_document("extra", "<h1>Extra</h1><p id=\"p1\">Unique word: hyoga</p>")?;

        let hits = core.search("hyoga")?;
        assert!(!hits.is_empty(), "should find 'hyoga' before delete");

        core.delete_item("extra")?;
        core.build_index()?;

        let hits = core.search("hyoga")?;
        assert!(hits.is_empty(), "ghost entry: 'hyoga' still found after chapter deleted");

        Ok(())
    }

    #[test]
    fn test_validate_links_local_scoped_to_chapter() -> Result<()> {
        let dir = tempdir()?;
        let mut core = GutenCore::new_project(dir.path(), "Link Test", "en")?;

        // sect tiene el hook "sect-a"
        core.add_document(
            "sect",
            r#"<h1 id="sect-a">Section A</h1><p>Content</p>"#,
        )?;
        // linker tiene un link a #sect-a que NO existe en linker (solo existe en sect)
        core.add_document(
            "linker",
            r##"<p>Ver <a href="#sect-a">sección</a></p>"##,
        )?;

        let orphans = core.validate_links()?;
        assert!(
            orphans.iter().any(|(ch, href)| ch == "linker" && href == "#sect-a"),
            "expected #sect-a in 'linker' to be orphan; got: {:?}",
            orphans
        );

        Ok(())
    }

    #[test]
    fn test_validate_links_cross_file() -> Result<()> {
        let dir = tempdir()?;
        let mut core = GutenCore::new_project(dir.path(), "CrossLink Test", "en")?;

        core.add_document("target", r#"<h1 id="real-hook">Title</h1><p>Content</p>"#)?;
        // source tiene un link válido y uno roto al mismo archivo
        core.add_document(
            "source",
            r#"<p>
                <a href="target.xhtml#real-hook">ok</a>
                <a href="target.xhtml#ghost-hook">broken</a>
            </p>"#,
        )?;

        let orphans = core.validate_links()?;

        assert!(
            !orphans.iter().any(|(_, href)| href == "target.xhtml#real-hook"),
            "valid cross-file link should not be orphan"
        );
        assert!(
            orphans.iter().any(|(ch, href)| ch == "source" && href == "target.xhtml#ghost-hook"),
            "broken cross-file link should be reported; got: {:?}",
            orphans
        );

        Ok(())
    }

    #[test]
    fn test_export_excludes_wal_sidecars() -> Result<()> {
        let dir = tempdir()?;
        let mut core = GutenCore::new_project(dir.path(), "WAL Test", "en")?;

        // Crear sidecars WAL artificialmente
        fs::write(dir.path().join(".gutenair.db-wal"), b"fake wal")?;
        fs::write(dir.path().join(".gutenair.db-shm"), b"fake shm")?;

        let out = dir.path().join("out.epub");
        core.export_epub(&out)?;

        let file = fs::File::open(&out)?;
        let mut zip = zip::ZipArchive::new(file).unwrap();
        for i in 0..zip.len() {
            let name = zip.by_index(i).unwrap().name().to_string();
            assert!(
                !name.starts_with(".gutenair.db"),
                "EPUB contains DB sidecar: {}",
                name
            );
        }

        Ok(())
    }
}
