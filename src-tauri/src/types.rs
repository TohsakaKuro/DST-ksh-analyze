use glsl_lang::ast::TypeSpecifierNonArrayData;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VariableScope {
    Uniform,
    Unknown(u32),
}

impl VariableScope {
    pub const UNIFORM: Self = Self::Uniform;

    pub const fn code(self) -> u32 {
        match self {
            Self::Uniform => 0,
            Self::Unknown(value) => value,
        }
    }

    pub fn from_u32(value: u32) -> Result<Self, String> {
        match value {
            0 => Ok(Self::Uniform),
            _ => Ok(Self::Unknown(value)),
        }
    }
}

/// KSH 中保存的 uniform 类型。
///
/// 当前发布的 65 个官方文件只使用 0、2、3、4、20、43。完整具名表来自
/// 官方 ShaderCompiler 的 ConvertCgType；1 和 22 是该转换表无法生成的保留槽。
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum KshUniformType {
    Float,
    ReservedCode1,
    Vec2,
    Vec3,
    Vec4,
    Float1x1,
    Float1x2,
    Float1x3,
    Float1x4,
    Float2x1,
    Mat2,
    Mat2x3,
    Mat2x4,
    Float3x1,
    Mat3x2,
    Mat3,
    Mat3x4,
    Float4x1,
    Mat4x2,
    Mat4x3,
    Mat4,
    Int,
    ReservedCode22,
    IVec2,
    IVec3,
    IVec4,
    Int1x1,
    Int1x2,
    Int1x3,
    Int1x4,
    Int2x1,
    Int2x2,
    Int2x3,
    Int2x4,
    Int3x1,
    Int3x2,
    Int3x3,
    Int3x4,
    Int4x1,
    Int4x2,
    Int4x3,
    Int4x4,
    Sampler1D,
    Sampler2D,
    Sampler3D,
    SamplerCube,
    Unknown(u32),
}

impl KshUniformType {
    pub const fn from_code(code: u32) -> Self {
        match code {
            0 => Self::Float,
            1 => Self::ReservedCode1,
            2 => Self::Vec2,
            3 => Self::Vec3,
            4 => Self::Vec4,
            5 => Self::Float1x1,
            6 => Self::Float1x2,
            7 => Self::Float1x3,
            8 => Self::Float1x4,
            9 => Self::Float2x1,
            10 => Self::Mat2,
            11 => Self::Mat2x3,
            12 => Self::Mat2x4,
            13 => Self::Float3x1,
            14 => Self::Mat3x2,
            15 => Self::Mat3,
            16 => Self::Mat3x4,
            17 => Self::Float4x1,
            18 => Self::Mat4x2,
            19 => Self::Mat4x3,
            20 => Self::Mat4,
            21 => Self::Int,
            22 => Self::ReservedCode22,
            23 => Self::IVec2,
            24 => Self::IVec3,
            25 => Self::IVec4,
            26 => Self::Int1x1,
            27 => Self::Int1x2,
            28 => Self::Int1x3,
            29 => Self::Int1x4,
            30 => Self::Int2x1,
            31 => Self::Int2x2,
            32 => Self::Int2x3,
            33 => Self::Int2x4,
            34 => Self::Int3x1,
            35 => Self::Int3x2,
            36 => Self::Int3x3,
            37 => Self::Int3x4,
            38 => Self::Int4x1,
            39 => Self::Int4x2,
            40 => Self::Int4x3,
            41 => Self::Int4x4,
            42 => Self::Sampler1D,
            43 => Self::Sampler2D,
            44 => Self::Sampler3D,
            45 => Self::SamplerCube,
            value => Self::Unknown(value),
        }
    }

    pub fn default_data_length(&self) -> Option<usize> {
        match self {
            Self::Float | Self::Float1x1 | Self::Int | Self::Int1x1 => Some(1),
            Self::Vec2
            | Self::Float1x2
            | Self::Float2x1
            | Self::IVec2
            | Self::Int1x2
            | Self::Int2x1 => Some(2),
            Self::Vec3
            | Self::Float1x3
            | Self::Float3x1
            | Self::IVec3
            | Self::Int1x3
            | Self::Int3x1 => Some(3),
            Self::Vec4
            | Self::Float1x4
            | Self::Mat2
            | Self::Float4x1
            | Self::IVec4
            | Self::Int1x4
            | Self::Int2x2
            | Self::Int4x1 => Some(4),
            Self::Mat2x3 | Self::Mat3x2 | Self::Int2x3 | Self::Int3x2 => Some(6),
            Self::Mat2x4 | Self::Mat4x2 | Self::Int2x4 | Self::Int4x2 => Some(8),
            Self::Mat3 | Self::Int3x3 => Some(9),
            Self::Mat3x4 | Self::Mat4x3 | Self::Int3x4 | Self::Int4x3 => Some(12),
            Self::Mat4 | Self::Int4x4 => Some(16),
            Self::Sampler1D | Self::Sampler2D | Self::Sampler3D | Self::SamplerCube => Some(0),
            Self::ReservedCode1 | Self::ReservedCode22 | Self::Unknown(_) => None,
        }
    }

    pub const fn id(self) -> u32 {
        match self {
            Self::Float => 0,
            Self::ReservedCode1 => 1,
            Self::Vec2 => 2,
            Self::Vec3 => 3,
            Self::Vec4 => 4,
            Self::Float1x1 => 5,
            Self::Float1x2 => 6,
            Self::Float1x3 => 7,
            Self::Float1x4 => 8,
            Self::Float2x1 => 9,
            Self::Mat2 => 10,
            Self::Mat2x3 => 11,
            Self::Mat2x4 => 12,
            Self::Float3x1 => 13,
            Self::Mat3x2 => 14,
            Self::Mat3 => 15,
            Self::Mat3x4 => 16,
            Self::Float4x1 => 17,
            Self::Mat4x2 => 18,
            Self::Mat4x3 => 19,
            Self::Mat4 => 20,
            Self::Int => 21,
            Self::ReservedCode22 => 22,
            Self::IVec2 => 23,
            Self::IVec3 => 24,
            Self::IVec4 => 25,
            Self::Int1x1 => 26,
            Self::Int1x2 => 27,
            Self::Int1x3 => 28,
            Self::Int1x4 => 29,
            Self::Int2x1 => 30,
            Self::Int2x2 => 31,
            Self::Int2x3 => 32,
            Self::Int2x4 => 33,
            Self::Int3x1 => 34,
            Self::Int3x2 => 35,
            Self::Int3x3 => 36,
            Self::Int3x4 => 37,
            Self::Int4x1 => 38,
            Self::Int4x2 => 39,
            Self::Int4x3 => 40,
            Self::Int4x4 => 41,
            Self::Sampler1D => 42,
            Self::Sampler2D => 43,
            Self::Sampler3D => 44,
            Self::SamplerCube => 45,
            Self::Unknown(value) => value,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Float => "float",
            Self::ReservedCode1 => "reserved(1)",
            Self::Vec2 => "vec2",
            Self::Vec3 => "vec3",
            Self::Vec4 => "vec4",
            Self::Float1x1 => "float1x1",
            Self::Float1x2 => "float1x2",
            Self::Float1x3 => "float1x3",
            Self::Float1x4 => "float1x4",
            Self::Float2x1 => "float2x1",
            Self::Mat2 => "mat2",
            Self::Mat2x3 => "mat2x3",
            Self::Mat2x4 => "mat2x4",
            Self::Float3x1 => "float3x1",
            Self::Mat3x2 => "mat3x2",
            Self::Mat3 => "mat3",
            Self::Mat3x4 => "mat3x4",
            Self::Float4x1 => "float4x1",
            Self::Mat4x2 => "mat4x2",
            Self::Mat4x3 => "mat4x3",
            Self::Mat4 => "mat4",
            Self::Int => "int",
            Self::ReservedCode22 => "reserved(22)",
            Self::IVec2 => "ivec2",
            Self::IVec3 => "ivec3",
            Self::IVec4 => "ivec4",
            Self::Int1x1 => "int1x1",
            Self::Int1x2 => "int1x2",
            Self::Int1x3 => "int1x3",
            Self::Int1x4 => "int1x4",
            Self::Int2x1 => "int2x1",
            Self::Int2x2 => "int2x2",
            Self::Int2x3 => "int2x3",
            Self::Int2x4 => "int2x4",
            Self::Int3x1 => "int3x1",
            Self::Int3x2 => "int3x2",
            Self::Int3x3 => "int3x3",
            Self::Int3x4 => "int3x4",
            Self::Int4x1 => "int4x1",
            Self::Int4x2 => "int4x2",
            Self::Int4x3 => "int4x3",
            Self::Int4x4 => "int4x4",
            Self::Sampler1D => "sampler1D",
            Self::Sampler2D => "sampler2D",
            Self::Sampler3D => "sampler3D",
            Self::SamplerCube => "samplerCube",
            Self::Unknown(_) => "unknown",
        }
    }

    pub const fn is_sampler(self) -> bool {
        matches!(self.id(), 42..=45)
    }

    pub fn from_glsl(value: &TypeSpecifierNonArrayData) -> Result<Self, String> {
        let result = match value {
            TypeSpecifierNonArrayData::Int => Self::Int,
            TypeSpecifierNonArrayData::Float => Self::Float,
            TypeSpecifierNonArrayData::Vec2 => Self::Vec2,
            TypeSpecifierNonArrayData::Vec3 => Self::Vec3,
            TypeSpecifierNonArrayData::Vec4 => Self::Vec4,
            TypeSpecifierNonArrayData::IVec2 => Self::IVec2,
            TypeSpecifierNonArrayData::IVec3 => Self::IVec3,
            TypeSpecifierNonArrayData::IVec4 => Self::IVec4,
            TypeSpecifierNonArrayData::Mat2 | TypeSpecifierNonArrayData::Mat22 => Self::Mat2,
            TypeSpecifierNonArrayData::Mat23 => Self::Mat2x3,
            TypeSpecifierNonArrayData::Mat24 => Self::Mat2x4,
            TypeSpecifierNonArrayData::Mat32 => Self::Mat3x2,
            TypeSpecifierNonArrayData::Mat3 | TypeSpecifierNonArrayData::Mat33 => Self::Mat3,
            TypeSpecifierNonArrayData::Mat34 => Self::Mat3x4,
            TypeSpecifierNonArrayData::Mat42 => Self::Mat4x2,
            TypeSpecifierNonArrayData::Mat43 => Self::Mat4x3,
            TypeSpecifierNonArrayData::Mat4 | TypeSpecifierNonArrayData::Mat44 => Self::Mat4,
            TypeSpecifierNonArrayData::Sampler1D => Self::Sampler1D,
            TypeSpecifierNonArrayData::Sampler2D => Self::Sampler2D,
            TypeSpecifierNonArrayData::Sampler3D => Self::Sampler3D,
            TypeSpecifierNonArrayData::SamplerCube => Self::SamplerCube,
            unsupported => {
                return Err(format!(
                    "DST KSH 编译器不支持 GLSL uniform 类型: {unsupported:?}"
                ));
            }
        };
        Ok(result)
    }

    pub fn to_glsl(self) -> Option<TypeSpecifierNonArrayData> {
        match self {
            Self::Int => Some(TypeSpecifierNonArrayData::Int),
            Self::Float => Some(TypeSpecifierNonArrayData::Float),
            Self::Vec2 => Some(TypeSpecifierNonArrayData::Vec2),
            Self::Vec3 => Some(TypeSpecifierNonArrayData::Vec3),
            Self::Vec4 => Some(TypeSpecifierNonArrayData::Vec4),
            Self::IVec2 => Some(TypeSpecifierNonArrayData::IVec2),
            Self::IVec3 => Some(TypeSpecifierNonArrayData::IVec3),
            Self::IVec4 => Some(TypeSpecifierNonArrayData::IVec4),
            Self::Mat2 => Some(TypeSpecifierNonArrayData::Mat2),
            Self::Mat2x3 => Some(TypeSpecifierNonArrayData::Mat23),
            Self::Mat2x4 => Some(TypeSpecifierNonArrayData::Mat24),
            Self::Mat3x2 => Some(TypeSpecifierNonArrayData::Mat32),
            Self::Mat3 => Some(TypeSpecifierNonArrayData::Mat3),
            Self::Mat3x4 => Some(TypeSpecifierNonArrayData::Mat34),
            Self::Mat4x2 => Some(TypeSpecifierNonArrayData::Mat42),
            Self::Mat4x3 => Some(TypeSpecifierNonArrayData::Mat43),
            Self::Mat4 => Some(TypeSpecifierNonArrayData::Mat4),
            Self::Sampler1D => Some(TypeSpecifierNonArrayData::Sampler1D),
            Self::Sampler2D => Some(TypeSpecifierNonArrayData::Sampler2D),
            Self::Sampler3D => Some(TypeSpecifierNonArrayData::Sampler3D),
            Self::SamplerCube => Some(TypeSpecifierNonArrayData::SamplerCube),
            _ => None,
        }
    }
}

pub type VariableType = KshUniformType;

impl TryFrom<u32> for KshUniformType {
    type Error = String;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Ok(KshUniformType::from_code(value))
    }
}

impl TryFrom<&str> for KshUniformType {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "int" => Ok(Self::Int),
            "float" => Ok(Self::Float),
            "vec2" => Ok(Self::Vec2),
            "vec3" => Ok(Self::Vec3),
            "vec4" => Ok(Self::Vec4),
            "ivec2" => Ok(Self::IVec2),
            "ivec3" => Ok(Self::IVec3),
            "ivec4" => Ok(Self::IVec4),
            "mat2" | "mat2x2" => Ok(Self::Mat2),
            "mat2x3" => Ok(Self::Mat2x3),
            "mat2x4" => Ok(Self::Mat2x4),
            "mat3x2" => Ok(Self::Mat3x2),
            "mat3" | "mat3x3" => Ok(Self::Mat3),
            "mat3x4" => Ok(Self::Mat3x4),
            "mat4x2" => Ok(Self::Mat4x2),
            "mat4x3" => Ok(Self::Mat4x3),
            "mat4" | "mat4x4" => Ok(Self::Mat4),
            "sampler1D" => Ok(Self::Sampler1D),
            "sampler2D" => Ok(Self::Sampler2D),
            "sampler3D" => Ok(Self::Sampler3D),
            "samplerCube" => Ok(Self::SamplerCube),
            _ => Err(format!("DST KSH 编译器不支持 uniform 类型: {value}")),
        }
    }
}

impl From<&KshUniformType> for u32 {
    fn from(var_type: &KshUniformType) -> Self {
        (*var_type).id()
    }
}

impl From<&KshUniformType> for String {
    fn from(var_type: &KshUniformType) -> Self {
        match var_type {
            KshUniformType::Unknown(code) => format!("unknown({code})"),
            _ => (*var_type).name().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KshUniform {
    pub name: String,
    pub scope: VariableScope,
    pub uniform_type: KshUniformType,
    pub array_count: u32,
    /// IEEE-754 values are kept as raw bits for byte-exact round trips.
    pub default_data: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KshStage {
    pub source_name: String,
    /// The binary format's terminal NUL is not included here.
    pub source: String,
    pub uniform_indices: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KshFile {
    pub effect_name: String,
    pub uniforms: Vec<KshUniform>,
    pub vertex: KshStage,
    pub pixel: KshStage,
}

/// A validated declaration produced from GLSL source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniformDeclaration {
    pub name: String,
    pub uniform_type: KshUniformType,
    pub array_count: u32,
    pub is_array: bool,
    /// Explicit defaults encoded as IEEE-754 float32 bits. `None` means no source initializer.
    pub default_data: Option<Vec<u32>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_type_codes_are_stable() {
        for code in 0..=45 {
            let uniform_type = VariableType::try_from(code).unwrap();
            assert_eq!(uniform_type.id(), code);
            assert!(!matches!(uniform_type, VariableType::Unknown(_)));
        }
    }

    #[test]
    fn known_type_default_widths_are_complete() {
        let expected = [
            Some(1),
            None,
            Some(2),
            Some(3),
            Some(4),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            Some(2),
            Some(4),
            Some(6),
            Some(8),
            Some(3),
            Some(6),
            Some(9),
            Some(12),
            Some(4),
            Some(8),
            Some(12),
            Some(16),
            Some(1),
            None,
            Some(2),
            Some(3),
            Some(4),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            Some(2),
            Some(4),
            Some(6),
            Some(8),
            Some(3),
            Some(6),
            Some(9),
            Some(12),
            Some(4),
            Some(8),
            Some(12),
            Some(16),
            Some(0),
            Some(0),
            Some(0),
            Some(0),
        ];

        for (code, width) in expected.into_iter().enumerate() {
            assert_eq!(
                KshUniformType::from_code(code as u32).default_data_length(),
                width
            );
        }
    }

    #[test]
    fn unknown_type_codes_are_lossless() {
        let unknown = VariableType::try_from(0xfeed).unwrap();
        assert_eq!(unknown, VariableType::Unknown(0xfeed));
        assert_eq!(unknown.id(), 0xfeed);
        assert_eq!(String::from(&unknown), "unknown(65261)");
        assert_eq!(unknown.default_data_length(), None);
        assert_eq!(unknown.to_glsl(), None);
    }

    #[test]
    fn glsl_type_mappings_are_bidirectional() {
        let mappings = [
            (
                "float",
                TypeSpecifierNonArrayData::Float,
                KshUniformType::Float,
            ),
            (
                "vec2",
                TypeSpecifierNonArrayData::Vec2,
                KshUniformType::Vec2,
            ),
            (
                "vec3",
                TypeSpecifierNonArrayData::Vec3,
                KshUniformType::Vec3,
            ),
            (
                "vec4",
                TypeSpecifierNonArrayData::Vec4,
                KshUniformType::Vec4,
            ),
            (
                "mat2",
                TypeSpecifierNonArrayData::Mat2,
                KshUniformType::Mat2,
            ),
            (
                "mat2x3",
                TypeSpecifierNonArrayData::Mat23,
                KshUniformType::Mat2x3,
            ),
            (
                "mat2x4",
                TypeSpecifierNonArrayData::Mat24,
                KshUniformType::Mat2x4,
            ),
            (
                "mat3x2",
                TypeSpecifierNonArrayData::Mat32,
                KshUniformType::Mat3x2,
            ),
            (
                "mat3",
                TypeSpecifierNonArrayData::Mat3,
                KshUniformType::Mat3,
            ),
            (
                "mat3x4",
                TypeSpecifierNonArrayData::Mat34,
                KshUniformType::Mat3x4,
            ),
            (
                "mat4x2",
                TypeSpecifierNonArrayData::Mat42,
                KshUniformType::Mat4x2,
            ),
            (
                "mat4x3",
                TypeSpecifierNonArrayData::Mat43,
                KshUniformType::Mat4x3,
            ),
            (
                "mat4",
                TypeSpecifierNonArrayData::Mat4,
                KshUniformType::Mat4,
            ),
            ("int", TypeSpecifierNonArrayData::Int, KshUniformType::Int),
            (
                "ivec2",
                TypeSpecifierNonArrayData::IVec2,
                KshUniformType::IVec2,
            ),
            (
                "ivec3",
                TypeSpecifierNonArrayData::IVec3,
                KshUniformType::IVec3,
            ),
            (
                "ivec4",
                TypeSpecifierNonArrayData::IVec4,
                KshUniformType::IVec4,
            ),
            (
                "sampler1D",
                TypeSpecifierNonArrayData::Sampler1D,
                KshUniformType::Sampler1D,
            ),
            (
                "sampler2D",
                TypeSpecifierNonArrayData::Sampler2D,
                KshUniformType::Sampler2D,
            ),
            (
                "sampler3D",
                TypeSpecifierNonArrayData::Sampler3D,
                KshUniformType::Sampler3D,
            ),
            (
                "samplerCube",
                TypeSpecifierNonArrayData::SamplerCube,
                KshUniformType::SamplerCube,
            ),
        ];

        for (name, glsl_type, ksh_type) in mappings {
            assert_eq!(KshUniformType::try_from(name).unwrap(), ksh_type);
            assert_eq!(KshUniformType::from_glsl(&glsl_type).unwrap(), ksh_type);
            assert_eq!(ksh_type.to_glsl(), Some(glsl_type));
        }

        assert_eq!(
            KshUniformType::from_glsl(&TypeSpecifierNonArrayData::Mat22).unwrap(),
            KshUniformType::Mat2
        );
        assert_eq!(
            KshUniformType::from_glsl(&TypeSpecifierNonArrayData::Mat33).unwrap(),
            KshUniformType::Mat3
        );
        assert_eq!(
            KshUniformType::from_glsl(&TypeSpecifierNonArrayData::Mat44).unwrap(),
            KshUniformType::Mat4
        );
        assert_eq!(
            KshUniformType::try_from("mat2x2").unwrap(),
            KshUniformType::Mat2
        );
        assert_eq!(
            KshUniformType::try_from("mat3x3").unwrap(),
            KshUniformType::Mat3
        );
        assert_eq!(
            KshUniformType::try_from("mat4x4").unwrap(),
            KshUniformType::Mat4
        );
    }

    #[test]
    fn non_glsl_ksh_types_remain_explicit() {
        let non_glsl_types = [
            KshUniformType::ReservedCode1,
            KshUniformType::Float1x1,
            KshUniformType::Float1x2,
            KshUniformType::Float1x3,
            KshUniformType::Float1x4,
            KshUniformType::Float2x1,
            KshUniformType::Float3x1,
            KshUniformType::Float4x1,
            KshUniformType::ReservedCode22,
            KshUniformType::Int1x1,
            KshUniformType::Int1x2,
            KshUniformType::Int1x3,
            KshUniformType::Int1x4,
            KshUniformType::Int2x1,
            KshUniformType::Int2x2,
            KshUniformType::Int2x3,
            KshUniformType::Int2x4,
            KshUniformType::Int3x1,
            KshUniformType::Int3x2,
            KshUniformType::Int3x3,
            KshUniformType::Int3x4,
            KshUniformType::Int4x1,
            KshUniformType::Int4x2,
            KshUniformType::Int4x3,
            KshUniformType::Int4x4,
        ];

        for uniform_type in non_glsl_types {
            assert_eq!(uniform_type.to_glsl(), None);
            assert!(KshUniformType::try_from(uniform_type.name()).is_err());
        }

        assert!(KshUniformType::from_glsl(&TypeSpecifierNonArrayData::Bool).is_err());
        assert!(KshUniformType::try_from("unknown(99)").is_err());
    }
}
