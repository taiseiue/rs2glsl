#[derive(Clone, Debug, PartialEq)]
pub enum GlslType {
    Float,
    Vec2,
    Vec3,
    Vec4,
}

impl GlslType {
    pub fn to_glsl(&self) -> &'static str {
        match self {
            GlslType::Float => "float",
            GlslType::Vec2 => "vec2",
            GlslType::Vec3 => "vec3",
            GlslType::Vec4 => "vec4",
        }
    }
}
