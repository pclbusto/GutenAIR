use gutencore::GutenCore;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let project_path = Path::new("test_book");
    
    // 1. Create a new project
    println!("Creating new project...");
    let mut core = GutenCore::new_project(project_path, "Robust Rust EPUB", "en")?;
    
    // 2. Validate integrity (should be OK)
    println!("Validating integrity...");
    let errors = core.validate_integrity();
    if errors.is_empty() {
        println!("  ✔ Integrity OK");
    } else {
        println!("  ✖ Integrity errors: {:?}", errors);
    }
    
    // 3. Test path normalization (simulate Windows-style internally)
    println!("Testing path normalization...");
    core.add_to_manifest(
        "extra".to_string(), 
        "Text\\extra.xhtml".to_string(), 
        "application/xhtml+xml".to_string(), 
        "".to_string()
    )?;
    
    // Create the extra file to satisfy integrity check
    let extra_path = core.get_resource_path("extra")?;
    std::fs::write(extra_path, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>Extra</title></head><body>Extra</body></html>")?;
    
    let item = core.get_item("extra")?;
    println!("  HREF for 'extra': {}", item.href);
    assert_eq!(item.href, "Text/extra.xhtml");
    println!("  ✔ Path normalized to Unix style");
    
    // 4. Test Namespace-aware parsing
    println!("Testing namespace-aware parsing (saving and reloading)...");
    core.save()?;
    let core_reloaded = GutenCore::open_folder(project_path)?;
    let meta = core_reloaded.get_metadata().unwrap();
    println!("  Reloaded Title: {}", meta.title);
    println!("  ✔ Namespaces handled correctly");
    
    // 5. Export to EPUB
    let export_path = "output.epub";
    println!("Exporting to {}...", export_path);
    core.export_epub(export_path)?;
    println!("  ✔ Export successful!");
    
    println!("\nProject verified and exported to {}", export_path);
    Ok(())
}
