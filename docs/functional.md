# Functional Capabilities - GutenCore

GutenCore acts as the engine that powers the creation and management of electronic books in EPUB 3 format. It abstracts the complexity of the EPUB standard into a set of functional operations.

## Main Capabilities

### 1. Project Management
- **Instant Scaffolding:** Create a compliant EPUB 3 project structure from scratch with a single command.
- **Project Recovery:** Load and analyze existing EPUB project folders, rebuilding the in-memory manifest and spine.
- **Sync Saving:** Ensure all changes (metadata, items, order) are persisted correctly to the standard Open Packaging Format (OPF) file.

### 2. Content Management
- **Smart Imports:** Import external files into the project, automatically placing them in the correct directory (Text, Styles, Images).
- **Atomic Deletion:** Delete items safely. When an item is deleted, GutenCore removes its reference from the manifest, removes it from the reading order (spine), and deletes the physical file from the disk.
- **Filename Sanitization:** Automatically cleans filenames to ensure maximum compatibility across different operating systems and ebook readers.

### 3. Automated Formatting (The Guardian)
- **HTML to XHTML conversion:** Automatically transforms standard HTML fragments into strictly valid XHTML documents, as required by the EPUB 3 spec.
- **Automatic Styling:** Maintains global CSS consistency by injecting stylesheet links into every document added to the project.
- **Security:** Strips dangerous tags (like `<script>`) to ensure the produced books are safe for any reading application.

### 4. Navigation and Metadata
- **Automatic TOC:** Rebuilds the Table of Contents (`nav.xhtml`) automatically by scanning headings (`h1`, `h2`) within the documents listed in the spine.
- **Metadata Sync:** Keeps identifiers (UUID), titles, languages, and "last modified" dates updated according to the spec.
- **Integrity Checks:** Includes a "deep validation" system that detects orphans (files on disk not in manifest) or broken references (items in manifest with missing files).
