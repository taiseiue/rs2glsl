#[derive(Debug, thiserror::Error)]
pub enum TranspileError {
    #[error("Duplicate const: `{0}`")]
    DuplicateConst(String),

    #[error("Unsupported type: `{0}`")]
    UnsupportedType(String),

    #[error("Unknown variable: `{0}`")]
    UnknownVariable(String),

    #[error("Unsupported syntax: {0}")]
    UnsupportedSyntax(&'static str),

    #[error("struct `{0}` requires a #[structlayout(vec2|vec3|vec4)] attribute")]
    MissingReprAttr(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Undefined function: `{0}` — declare it with #[builtin(\"glsl_name\")] fn {0}(...)")]
    UndefinedFunction(String),
}

impl TranspileError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::DuplicateConst(_) => "E0002",
            Self::UnsupportedType(_) => "E0003",
            Self::UnknownVariable(_) => "E0004",
            Self::UnsupportedSyntax(_) => "E0005",
            Self::MissingReprAttr(_) => "E0006",
            Self::ParseError(_) => "E0007",
            Self::UndefinedFunction(_) => "E0008",
        }
    }
}
