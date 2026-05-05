#[derive(Clone, Debug, PartialEq)]
pub enum GlslType {
    Bool,
    Int,
    Float,
    Vec2,
    Vec3,
    Vec4,
    Array(Box<GlslType>, usize),
    Struct(String, Box<GlslType>),  // (struct名, 対応するGLSLの型)
    Builtin(String, Box<GlslType>), // (GLSLでの変数名, 実際の型)
}

impl GlslType {
    pub fn to_glsl(&self) -> &str {
        match self {
            GlslType::Bool => "bool",
            GlslType::Int => "int",
            GlslType::Float => "float",
            GlslType::Vec2 => "vec2",
            GlslType::Vec3 => "vec3",
            GlslType::Vec4 => "vec4",
            GlslType::Array(inner, _) => inner.to_glsl(),
            GlslType::Struct(_, underlying) => underlying.to_glsl(),
            GlslType::Builtin(_, underlying) => underlying.to_glsl(),
        }
    }

    pub fn render_decl(&self, name: &str) -> String {
        match self {
            GlslType::Array(inner, len) => inner.render_decl(&format!("{name}[{len}]")),
            GlslType::Struct(_, underlying) => underlying.render_decl(name),
            GlslType::Builtin(_, underlying) => underlying.render_decl(name),
            _ => format!("{} {name}", self.to_glsl()),
        }
    }

    pub fn render_return_type(&self) -> String {
        match self {
            GlslType::Array(inner, len) => format!("{}[{len}]", inner.render_return_type()),
            GlslType::Struct(_, underlying) => underlying.render_return_type(),
            GlslType::Builtin(_, underlying) => underlying.render_return_type(),
            _ => self.to_glsl().to_string(),
        }
    }

    pub fn primitive(&self) -> &GlslType {
        match self {
            GlslType::Struct(_, u) => u.primitive(),
            GlslType::Builtin(_, u) => u.primitive(),
            t => t,
        }
    }

    pub fn array_element(&self) -> Option<&GlslType> {
        match self {
            GlslType::Array(inner, _) => Some(inner.as_ref()),
            GlslType::Struct(_, underlying) => underlying.array_element(),
            GlslType::Builtin(_, underlying) => underlying.array_element(),
            _ => None,
        }
    }
}
