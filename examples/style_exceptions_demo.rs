use gutencore::{GutenCore, error::Result};
use std::fs;
use std::path::Path;

fn main() -> Result<()> {
    let project_path = Path::new("test_styles_project");
    if project_path.exists() {
        fs::remove_dir_all(project_path).ok();
    }

    // 1. Crear proyecto
    let mut core = GutenCore::new_project(project_path, "Test de Estilos", "es")?;

    // 2. Agregar estilos
    core.add_style("global", "body { color: black; }")?;
    core.add_style("special", "body { color: red; }")?;

    // 3. Establecer global como default
    core.set_style_as_default("global")?;

    // 4. Agregar capítulos y al spine
    core.add_document("intro", "<h1>Introducción</h1><p>Texto normal</p>")?;
    core.spine_insert("intro".to_string(), None);

    core.add_document("cap1", "<h1>Capítulo 1</h1><p>Texto especial</p>")?;
    core.spine_insert("cap1".to_string(), None);

    // 5. Crear excepción para cap1: remover global, agregar special
    // remove_style_from_chapter crea la excepción basándose en los defaults actuales
    core.remove_style_from_chapter("cap1", "global")?;
    
    // Ahora cap1 tiene su propia lista de estilos. Vamos a agregarle 'special'.
    // Debemos hacerlo manualmente en config.exceptions o implementar un add_style_to_chapter.
    // El spec no pedía add_style_to_chapter, pero decía:
    // "El Guardian inyecta automáticamente los enlaces <link> correspondientes a todos los default_styles... 
    // Si un capítulo está marcado como excepción, el sistema permite aplicar solo una lista específica de estilos"
    
    // Vamos a agregar 'special' a la excepción de cap1
    if let Some(styles) = core.config.exceptions.get_mut("cap1") {
        styles.push("special".to_string());
    }

    // 6. Exportar (esto disparará la Alineación Total)
    core.export_epub("test_styles.epub")?;

    println!("Proyecto exportado. Verificando contenidos...");

    // 7. Verificar archivos físicos en el workdir (fueron actualizados por la alineación)
    let intro_path = project_path.join("OEBPS/Text/intro.xhtml");
    let cap1_path = project_path.join("OEBPS/Text/cap1.xhtml");

    let intro_html = fs::read_to_string(intro_path)?;
    let cap1_html = fs::read_to_string(cap1_path)?;

    println!("--- Contenido Intro ---");
    println!("{}", intro_html);
    assert!(intro_html.contains("global.css"));
    assert!(!intro_html.contains("special.css"));

    println!("--- Contenido Cap1 ---");
    println!("{}", cap1_html);
    assert!(!cap1_html.contains("global.css"));
    assert!(cap1_html.contains("special.css"));

    println!("¡Verificación exitosa!");

    Ok(())
}
