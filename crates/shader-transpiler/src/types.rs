#[derive(Clone, Debug, PartialEq)]
pub enum GlslType {
    Bool,
    Float,
    Vec2,
    Vec3,
    Vec4,
    Struct(String, Box<GlslType>), // (struct名, 対応するGLSLの型)
}

impl GlslType {
    pub fn to_glsl(&self) -> &str {
        match self {
            GlslType::Bool => "bool",
            GlslType::Float => "float",
            GlslType::Vec2 => "vec2",
            GlslType::Vec3 => "vec3",
            GlslType::Vec4 => "vec4",
            GlslType::Struct(_, underlying) => underlying.to_glsl(),
        }
    }

    pub fn primitive(&self) -> &GlslType {
        match self {
            GlslType::Struct(_, u) => u.primitive(),
            t => t,
        }
    }
}
