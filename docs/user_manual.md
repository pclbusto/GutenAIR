# User Manual - Developer Guide

This manual guide explains how to integrate and use `GutenCore` to build and manage EPUB projects.

## Getting Started

### 1. Creating a New Project
To start a new EPUB project, use the `new_project` method. This creates the folder structure and basic files (mimetype, container, initial content).

```rust
use gutencore::GutenCore;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workdir = "./my_new_book";
    let mut core = GutenCore::new_project(workdir, "My First Book", "en")?;
    
    core.save()?; // Perspectives the initial state
    Ok(())
}
```

### 2. Adding Content
You can add XHTML documents or other resources (Images, CSS).

#### Adding a Document
```rust
// Sanitizes the raw HTML and places it at OEBPS/Text/chap2.xhtml
core.add_document("chap2", "<h1>Chapter 2</h1><p>Content goes here.</p>")?;
```

#### Importing a File
```rust
core.import_file("path/to/cover.jpg", "cover-img", "Images/cover.jpg", "image/jpeg")?;
```

### 3. Managing the Spine
The spine determines the reading order. Adding a document registers it in the manifest, but you must ensure it's in the spine if you want it to be part of the linear narrative.

```rust
core.spine_insert("chap2".to_string(), None); // Append to the end
```

### 4. Atomic Deletion
If you need to remove an item completely:

```rust
// Removes from manifest, spine, and deletes OEBPS/Text/chap1.xhtml
core.delete_item("chap1")?;
```

### 5. Validation and Integrity
Before exporting or finishing, it's recommended to check for consistency.

```rust
let (errors, orphans) = core.validate_integrity_deep();

for err in errors {
    println!("Integrity Error: {}", err);
}

for orphan in orphans {
    println!("Orphan file found (not in manifest): {:?}", orphan);
}
```

### 6. Saving Changes
Always call `save()` to update the `content.opf` and the automatic Table of Contents (`nav.xhtml`).

```rust
core.save()?;
```

## Troubleshooting
- **Error: ID already exists**: Manifest IDs must be unique.
- **Missing File**: If you manually delete files from the disk without using `delete_item`, the manifest will become inconsistent. Run `validate_integrity_deep()` to detect these cases.
