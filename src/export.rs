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
    /// Export the workspace to a valid EPUB file
    pub fn export_epub(&mut self, output_path: impl AsRef<Path>) -> Result<()> {
        // 1. Validate integrity before export
        let errors = self.validate_integrity();
        if !errors.is_empty() {
            return Err(crate::error::GutenError::Other(format!("Integrity check failed: {:?}", errors)));
        }

        // 2. Save current state to OPF
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

                if name_str == "mimetype" {
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
}
