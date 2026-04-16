# GutenAIR 📚 ✨

**GutenAIR** (formerly GutenAI) is a high-performance, robust Rust engine for managing and generating **EPUB 3** electronic books. 

It powers the functional core of the Guten ecosystem, providing a clean abstraction over the complex EPUB standard while enforcing strict semantic and structural validity through its specialized sub-engine, **The Guardian**.

---

## 🚀 Key Features

- **⚡ Instant Scaffolding**: Create compliant EPUB 3 structures from scratch with a single command.
- **🛡️ The Guardian (Auto-Formatting)**: 
    - Automatic HTML → XHTML conversion.
    - Strict tag sanitization and security stripping.
    - Automated CSS injection and global styling consistency.
- **📂 Smart Content Management**:
    - **Atomic Operations**: Safe item deletion (removes from disk + manifest + spine).
    - **Filename Sanitization**: Cross-platform compatibility out of the box.
    - **Asset Handling**: Centralized management of Text, Images, and Styles.
- **🔍 Deep Integrity Validation**:
    - Detects broken references and orphaned files.
    - Synchronizes Spine (reading order) and Manifest automatically.
- **🗺️ Reactive Navigation**: Automatic TOC (`nav.xhtml`) generation based on content headers.

---

## 🛠️ Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
gutencore = { git = "https://github.com/pclbusto/GutenAIR.git" }
```

---

## 📖 Quick Start

### 1. Create a New Project
```rust
use gutencore::GutenCore;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let project_path = Path::new("my_awesome_book");
    
    // Initialize a new EPUB 3 project
    let mut core = GutenCore::new_project(project_path, "The Rust Chronicles", "en")?;
    
    // Add an image
    core.add_resource(
        "cover".to_string(), 
        std::fs::read("cover.png")?, 
        "image/png", 
        "Images/cover.png"
    )?;
    
    Ok(())
}
```

### 2. Safeguarding Content with "The Guardian"
GutenAIR doesn't just save your HTML; it ensures it's valid for e-readers.

```rust
let dirty_html = r#"<h1>Chapter 1</h1><p>Unclosed tags <br> and <b>unclosed bold."#;

// The Guardian will close tags, add XML namespaces, and link the global stylesheet automatically
core.save_chapter("chap1", dirty_html)?;

core.save()?;
core.export_epub("book.epub")?;
```

---

## 🏗️ Architecture

- **`GutenCore`**: The main entry point. Handles the OPF (Open Packaging Format), project hierarchy, and persistence.
- **Manifest & Spine**: In-memory models that mirror the EPUB internal structure, allowing for fast manipulations.
- **The Guardian**: A specialized module using `html5ever` and `ammonia` to clean and transform content into valid XHTML.

---

## 📁 Project Structure

```text
GutenAIR/
├── src/            # Core Rust logic
├── docs/           # Documentation and Manuals
├── examples/       # Usage demos
└── Cargo.toml      # Dependencies and Metadata
```

---

## 🤝 Contributing

Contributions are welcome! Please check the `docs/functional.md` for a deep dive into the current capabilities.

## 📄 License

Licensed under the MIT License.
