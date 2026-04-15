use thiserror::Error;

#[derive(Error, Debug)]
pub enum GutenError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("XML error: {0}")]
    Xml(#[from] quick_xml::Error),

    #[error("Zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("Invalid project: {0}")]
    InvalidProject(String),

    #[error("Manifest error: {0}")]
    Manifest(String),
    
    #[error("Other error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, GutenError>;
