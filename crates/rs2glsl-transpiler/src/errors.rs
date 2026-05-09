use proc_macro2::Span;
use std::fmt;
use syn::spanned::Spanned;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceLocation {
    pub line: usize,
    pub column: usize,
}

impl SourceLocation {
    fn from_span(span: Span) -> Option<Self> {
        let start = span.start();
        (start.line > 0).then_some(Self {
            line: start.line,
            column: start.column + 1,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TranspileErrorKind {
    DuplicateConst(String),
    UnsupportedType(String),
    UnknownVariable(String),
    UnsupportedSyntax(&'static str),
    MissingReprAttr(String),
    ParseError(String),
    UndefinedFunction(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranspileError {
    kind: TranspileErrorKind,
    location: Option<SourceLocation>,
}

impl TranspileError {
    #[allow(non_snake_case)]
    pub fn DuplicateConst(name: String) -> Self {
        Self::new(TranspileErrorKind::DuplicateConst(name))
    }

    #[allow(non_snake_case)]
    pub fn UnsupportedType(name: String) -> Self {
        Self::new(TranspileErrorKind::UnsupportedType(name))
    }

    #[allow(non_snake_case)]
    pub fn UnknownVariable(name: String) -> Self {
        Self::new(TranspileErrorKind::UnknownVariable(name))
    }

    #[allow(non_snake_case)]
    pub fn UnsupportedSyntax(message: &'static str) -> Self {
        Self::new(TranspileErrorKind::UnsupportedSyntax(message))
    }

    #[allow(non_snake_case)]
    pub fn MissingReprAttr(name: String) -> Self {
        Self::new(TranspileErrorKind::MissingReprAttr(name))
    }

    #[allow(non_snake_case)]
    pub fn ParseError(message: String) -> Self {
        Self::new(TranspileErrorKind::ParseError(message))
    }

    #[allow(non_snake_case)]
    pub fn UndefinedFunction(name: String) -> Self {
        Self::new(TranspileErrorKind::UndefinedFunction(name))
    }

    pub fn from_syn(error: syn::Error) -> Self {
        Self::ParseError(error.to_string()).with_raw_span(error.span())
    }

    pub fn code(&self) -> &'static str {
        match &self.kind {
            TranspileErrorKind::DuplicateConst(_) => "E0002",
            TranspileErrorKind::UnsupportedType(_) => "E0003",
            TranspileErrorKind::UnknownVariable(_) => "E0004",
            TranspileErrorKind::UnsupportedSyntax(_) => "E0005",
            TranspileErrorKind::MissingReprAttr(_) => "E0006",
            TranspileErrorKind::ParseError(_) => "E0007",
            TranspileErrorKind::UndefinedFunction(_) => "E0008",
        }
    }

    pub fn kind(&self) -> &TranspileErrorKind {
        &self.kind
    }

    pub fn location(&self) -> Option<SourceLocation> {
        self.location
    }

    pub fn with_span<T: Spanned>(self, node: &T) -> Self {
        self.with_raw_span(node.span())
    }

    pub fn with_raw_span(mut self, span: Span) -> Self {
        if self.location.is_none() {
            self.location = SourceLocation::from_span(span);
        }
        self
    }

    fn new(kind: TranspileErrorKind) -> Self {
        Self {
            kind,
            location: None,
        }
    }
}

impl fmt::Display for TranspileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            TranspileErrorKind::DuplicateConst(name) => {
                write!(f, "Duplicate const: `{name}`")
            }
            TranspileErrorKind::UnsupportedType(name) => {
                write!(f, "Unsupported type: `{name}`")
            }
            TranspileErrorKind::UnknownVariable(name) => {
                write!(f, "Unknown variable: `{name}`")
            }
            TranspileErrorKind::UnsupportedSyntax(message) => {
                write!(f, "Unsupported syntax: {message}")
            }
            TranspileErrorKind::MissingReprAttr(name) => write!(
                f,
                "struct `{name}` requires a #[structlayout(vec2|vec3|vec4)] attribute"
            ),
            TranspileErrorKind::ParseError(message) => write!(f, "Parse error: {message}"),
            TranspileErrorKind::UndefinedFunction(name) => write!(
                f,
                "Undefined function: `{name}` — declare it with #[builtin(\"glsl_name\")] fn {name}(...)"
            ),
        }
    }
}

impl std::error::Error for TranspileError {}
