use gutencore::GutenCore;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let project_path = Path::new("test_styles_project");
    let mut core = GutenCore::open_folder(project_path)?;
    
    println!("ID del capítulo: cap1");
    println!("Estilos que DEBERÍAN inyectarse: {:?}", core.get_chapter_styles("cap1"));
    
    let contenido = "<p>Contenido de prueba modificado</p>";
    let xhtml = core.sanitize_to_xhtml("cap1", contenido)?;
    
    println!("\n--- XHTML RESULTANTE ---");
    println!("{}", xhtml);
    println!("------------------------");
    
    Ok(())
}
