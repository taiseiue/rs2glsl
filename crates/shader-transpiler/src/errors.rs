#[derive(Debug, thiserror::Error)]
pub enum TranspileError {
    #[error("main_image function cannot be found")]
    MainImageNotFound,

    #[error("Unsupported type: `{0}`")]
    UnsupportedType(String),

    #[error("Unsupported variable: `{0}`")]
    UnknownVariable(String),

    #[error("Duplicate const: `{0}`")]
    DuplicateConst(String),

    #[error("Unsupported syntax: {0}")]
    UnsupportedSyntax(&'static str),
}

impl TranspileError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::MainImageNotFound    => "E0001",
            Self::DuplicateConst(_)    => "E0002",
            Self::UnsupportedType(_)   => "E0003",
            Self::UnknownVariable(_)   => "E0004",
            Self::UnsupportedSyntax(_) => "E0005",
        }
    }
}
