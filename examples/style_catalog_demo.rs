use gutencore::GutenCore;
use std::fs;
use tempfile::tempdir;

fn main() -> anyhow::Result<()> {
    // 1. Setup de un proyecto temporal
    let dir = tempdir()?;
    let mut core = GutenCore::new_project(dir.path(), "Demo Estilos", "es")?;

    // 2. Crear un archivo CSS con selectores mixtos
    let css_content = r#"
        /* Estilos de bloque */
        p.cita-autor { font-style: italic; margin: 20px; }
        h2.seccion-especial { color: navy; border-bottom: 2px solid; }
        div.recuadro-info { background: #f0f0f0; padding: 10px; }

        /* Estilos de línea */
        span.glosario-termino { font-weight: bold; color: darkgreen; }
        em.enfasis-fuerte { text-decoration: underline; }
        .alerta-general { color: red; } /* Genérico -> Línea */
    "#;

    core.add_style("main-style", css_content)?;
    core.set_style_as_default("main-style")?;

    // 3. Crear un capítulo
    core.add_to_manifest(
        "intro".to_string(),
        "Text/intro.xhtml".to_string(),
        "application/xhtml+xml".to_string(),
        "".to_string(),
    )?;
    core.save_chapter("intro", "<p>Hola mundo</p>")?;

    println!("--- CATÁLOGO DE ESTILOS PARA 'intro' ---");
    let catalogs = core.get_style_catalog("intro")?;
    
    for cat in catalogs {
        println!("Archivo: {}", cat.archivo_origen);
        println!("  BLOQUE:");
        for s in cat.estilos.bloque {
            println!("    - .{} (Sugerido: <{}>)", s.clase, s.tag_sugerido.unwrap_or_default());
        }
        println!("  LÍNEA:");
        for s in cat.estilos.linea {
            println!("    - .{} (Sugerido: <{}>)", s.clase, s.tag_sugerido.unwrap_or_default());
        }
    }

    // 4. VALIDACIÓN: El Guardián en acción
    
    println!("\n--- PRUEBA DE VALIDACIÓN ---");

    // Intento 1: Clase válida (Debe funcionar)
    println!("Guardando con clase válida 'cita-autor'...");
    let html_ok = r#"<p class="cita-autor">"El código es poesía".</p>"#;
    match core.save_chapter("intro", html_ok) {
        Ok(_) => println!("✅ Guardado exitoso."),
        Err(e) => println!("❌ Error inesperado: {}", e),
    }

    // Intento 2: Clase inexistente (Debe fallar)
    println!("\nGuardando con clase inexistente 'hacker-style'...");
    let html_error = r#"<p class="hacker-style">Intentando romper el core.</p>"#;
    match core.save_chapter("intro", html_error) {
        Ok(_) => println!("❌ Error: ¡El Core permitió una clase inexistente!"),
        Err(e) => println!("✅ El Core bloqueó el guardado correctamente:\n   '{}'", e),
    }

    // Intento 3: Clase genérica en etiqueta correcta
    println!("\nGuardando con clase genérica 'alerta-general' en un span...");
    let html_linea = r#"<p>Atención: <span class="alerta-general">Esto es importante</span>.</p>"#;
    match core.save_chapter("intro", html_linea) {
        Ok(_) => println!("✅ Guardado exitoso."),
        Err(e) => println!("❌ Error inesperado: {}", e),
    }

    Ok(())
}
