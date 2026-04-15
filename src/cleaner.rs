use crate::core::GutenCore;

impl GutenCore {
    /// Cleans HTML content using the ammonia crate
    pub fn clean_html(&self, html: &str) -> String {
        ammonia::clean(html)
    }

    /// Converts plain text to basic XHTML paragraphs
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
