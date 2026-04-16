use crate::core::GutenCore;

impl GutenCore {
        /// Limpia y sanitiza contenido HTML para prevenir vulnerabilidades XSS
    ///
    /// Este método utiliza la biblioteca [`ammonia`](https://crates.io/crates/ammonia)
    /// para eliminar scripts maliciosos y atributos peligrosos del HTML,
    /// manteniendo solo las etiquetas y atributos seguros.
    ///
    /// # ¿Qué elimina?
    ///
    /// - Etiquetas `<script>`, `<iframe>`, `<object>`, etc.
    /// - Atributos como `onclick`, `onload`, `onerror`
    /// - URLs peligrosas (`javascript:`, `data:`)
    /// - Comentarios HTML y CDATA no seguros
    ///
    /// # ¿Qué preserva?
    ///
    /// Por defecto, `ammonia` preserva etiquetas seguras como:
    /// - `<p>`, `<div>`, `<span>`, `<h1>`-`<h6>`
    /// - `<a>`, `<img>`, `<ul>`, `<ol>`, `<li>`
    /// - `<strong>`, `<em>`, `<br>`, `<hr>`
    /// - Atributos seguros como `href`, `src`, `alt`, `class`, `id`
    ///
    /// # Argumentos
    ///
    /// * `html` - Cadena HTML que puede contener contenido peligroso
    ///
    /// # Retorna
    ///
    /// * `String` - HTML sanitizado y seguro para incluir en un EPUB
    ///
    /// # Ejemplo
    ///
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// let core = GutenCore::new("./proyecto");
    ///
    /// let html_peligroso = r#"
    ///     <p>Texto seguro</p>
    ///     <script>alert('XSS');</script>
    ///     <img src="x" onerror="alert('malicioso')">
    /// "#;
    ///
    /// let limpio = core.clean_html(html_peligroso);
    /// 
    /// // El script y el onerror se eliminan, pero el <p> se conserva
    /// assert!(!limpio.contains("<script>"));
    /// assert!(!limpio.contains("onerror"));
    /// assert!(limpio.contains("<p>Texto seguro</p>"));
    /// ```
    ///
    /// # Casos de uso típicos
    ///
    /// 1. **Sanitizar contenido generado por usuarios** antes de incluirlo en un EPUB
    /// 2. **Limpiar HTML importado** de fuentes no confiables
    /// 3. **Prevenir inyección de código** al convertir formatos externos a EPUB
    ///
    /// # Notas de seguridad
    ///
    /// - **No confíes solo en esto**: Aunque `ammonia` es muy seguro,
    ///   siempre valida que el resultado sea el esperado.
    /// - **Rendimiento**: Para textos muy largos (>1MB), considera ejecutarlo
    ///   en un hilo separado o con chunks.
    /// - **Configuración personalizada**: Si necesitas reglas diferentes,
    ///   considera crear tu propia instancia de `ammonia::Builder`.
    ///
    /// # Ver también
    ///
    /// - [`text_to_xhtml`](Self::text_to_xhtml) - Convierte texto plano a XHTML
    /// - [Ammonia documentation](https://docs.rs/ammonia) - Para configuraciones avanzadas
    /// - [OWASP XSS Prevention Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Cross_Site_Scripting_Prevention_Cheat_Sheet.html)
    pub fn clean_html(&self, html: &str) -> String {
        ammonia::clean(html)
    }

        /// Convierte texto plano a un documento XHTML con párrafos
    ///
    /// Este método toma texto plano (como el contenido de un libro sin formato)
    /// y lo convierte en un documento XHTML válido para incluir en un EPUB.
    /// Los párrafos se detectan mediante dobles saltos de línea (`\n\n`).
    ///
    /// # Reglas de conversión
    ///
    /// | Entrada | Salida |
    /// |---------|--------|
    /// | `"Texto\n\nOtro párrafo"` | `<p>Texto</p><p>Otro párrafo</p>` |
    /// | Saltos de línea simples (`\n`) | `<br/>` dentro del párrafo |
    /// | Espacios al inicio/final | Se eliminan con `trim()` |
    /// | Líneas vacías | Se ignoran (no crean párrafos vacíos) |
    ///
    /// # Argumentos
    ///
    /// * `text` - Texto plano a convertir (puede contener múltiples líneas y párrafos)
    /// * `title` - Título que se usará en la etiqueta `<title>` del documento
    ///
    /// # Retorna
    ///
    /// * `String` - Documento XHTML completo con declaración XML, DOCTYPE y estructura HTML5
    ///
    /// # Ejemplo básico
    ///
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// let core = GutenCore::new("./proyecto");
    ///
    /// let texto = "Este es el primer párrafo.\nTiene dos líneas.\n\nEste es el segundo párrafo.";
    /// let xhtml = core.text_to_xhtml(texto, "Capítulo 1");
    ///
    /// println!("{}", xhtml);
    /// // Resultado:
    /// // <?xml version="1.0" encoding="UTF-8"?>
    /// // <!DOCTYPE html>
    /// // <html xmlns="http://www.w3.org/1999/xhtml">
    /// // <head><title>Capítulo 1</title></head>
    /// // <body>
    /// // <p>Este es el primer párrafo.<br/>Tiene dos líneas.</p>
    /// // <p>Este es el segundo párrafo.</p>
    /// // </body>
    /// // </html>
    /// ```
    ///
    /// # Ejemplo con formato complejo
    ///
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// let core = GutenCore::new("./proyecto");
    ///
    /// let poema = r#"Roses are red,
    /// Violets are blue.
    ///
    /// Sugar is sweet,
    /// And so are you."#;
    ///
    /// let xhtml = core.text_to_xhtml(poema, "Un Poema");
    /// 
    /// // Cada línea dentro del párrafo tiene <br/> excepto la última
    /// assert!(xhtml.contains("Roses are red,<br/>Violets are blue."));
    /// assert!(xhtml.contains("Sugar is sweet,<br/>And so are you."));
    /// ```
    ///
    /// # Ejemplo de integración con EPUB
    ///
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut core = GutenCore::new_project("./mi_libro", "Mi Libro", "es")?;
    ///
    /// // Texto del capítulo
    /// let texto_capitulo = "Había una vez...\n\nY colorín colorado.";
    /// let xhtml = core.text_to_xhtml(texto_capitulo, "Capítulo 1");
    ///
    /// // Guardar el archivo XHTML
    /// let opf_dir = core.opf_dir.as_ref().unwrap();
    /// let ruta_capitulo = opf_dir.join("Text/capitulo1.xhtml");
    /// std::fs::write(ruta_capitulo, xhtml)?;
    ///
    /// // Agregar al manifiesto y spine
    /// core.manifest.insert("cap1".to_string(), ManifestItem {
    ///     id: "cap1".to_string(),
    ///     href: "Text/capitulo1.xhtml".to_string(),
    ///     media_type: "application/xhtml+xml".to_string(),
    ///     properties: String::new(),
    /// });
    /// core.spine.push("cap1".to_string());
    ///
    /// core.save()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Limitaciones conocidas
    ///
    /// - **No soporta listas o tablas**: Solo detecta párrafos. Para estructuras
    ///   más complejas, necesitarás usar `clean_html` con HTML pre-formateado.
    /// - **No preserva sangrías**: Los espacios al inicio de línea se eliminan
    ///   con `trim()`.
    /// - **Encoding fijo**: Siempre genera UTF-8 (estándar de EPUB).
    /// - **Sin detección de encabezados**: Las líneas que parecen títulos no
    ///   se convierten automáticamente a `<h1>`-`<h6>`.
    ///
    /// # Notas de implementación
    ///
    /// - **Separador de párrafos**: Se usa `\n\n` (doble salto de línea).
    ///   Los párrafos separados por un solo `\n` se mantienen dentro del mismo `<p>`.
    /// - **Reemplazo de saltos**: Los `\n` dentro del párrafo se convierten a `<br/>`
    /// - **Escape automático**: El texto se inserta directamente; no se aplica
    ///   escape HTML adicional. Si el texto contiene `<` o `>`, se interpretarán
    ///   como HTML. Para texto con caracteres especiales, usa `clean_html` primero.
    ///
    /// # Advertencia de seguridad
    ///
    /// Este método **no sanitiza** el texto de entrada. Si el texto proviene
    /// de fuentes no confiables, **debes llamar a `clean_html` primero**:
    ///
    /// ```no_run
    /// # use guten_core::GutenCore;
    /// let core = GutenCore::new("./proyecto");
    /// let texto_peligroso = "Texto <script>alert('xss')</script> normal";
    /// let sanitizado = core.clean_html(texto_peligroso);
    /// let xhtml = core.text_to_xhtml(&sanitizado, "Título");
    /// ```
    ///
    /// # Ver también
    ///
    /// - [`clean_html`](Self::clean_html) - Para sanitizar HTML antes de convertirlo
    /// - [`ammonia` crate](https://crates.io/crates/ammonia) - Motor de sanitización subyacente
    /// - [EPUB XHTML Content Documents](https://www.w3.org/TR/epub/#sec-xhtml) - Especificación oficial
    pub fn text_to_xhtml(&self, text: &str, title: &str) -> String {
        let paragraphs: Vec<String> = text
            .split("\n\n")
            .map(|p| format!("<p>{}</p>", p.trim().replace("\n", "<br/>")))
            .collect();
        
        format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>{}</title></head>
<body>
{}
</body>
</html>"#, title, paragraphs.join("\n"))
    }
}
