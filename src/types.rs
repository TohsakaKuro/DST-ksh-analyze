use clap::error::ErrorKind;
use clap::Error;
use glsl_lang::ast::TypeSpecifierNonArrayData;

#[derive(Debug)]
pub enum VariableScope {
    UNIFORM,
}

impl VariableScope {
    pub fn from_u32(value: u32) -> Result<Self, String> {
        match value {
            0 => Ok(VariableScope::UNIFORM),
            _ => Err(format!("无效的作用域: {}", value)),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum VariableType {
    Float,
    Vec2,
    Vec3,
    Vec4,
    Mat4,
    Sampler2D,
}

impl VariableType {
    const ID_MAP: &'static [(VariableType, u32)] = &[
        (VariableType::Float, 0),
        (VariableType::Vec2, 2),
        (VariableType::Vec3, 3),
        (VariableType::Vec4, 4),
        (VariableType::Mat4, 20),
        (VariableType::Sampler2D, 43),
    ];

    const NAME_MAP: &'static [(VariableType, &'static str)] = &[
        (VariableType::Float, "float"),
        (VariableType::Vec2, "vec2"),
        (VariableType::Vec3, "vec3"),
        (VariableType::Vec4, "vec4"),
        (VariableType::Mat4, "mat4"),
        (VariableType::Sampler2D, "sampler2D"),
    ];

    pub fn default_data_length(&self) -> usize {
        match self {
            VariableType::Float => 1,
            VariableType::Vec2 => 2,
            VariableType::Vec3 => 3,
            VariableType::Vec4 => 4,
            VariableType::Mat4 => 16,
            VariableType::Sampler2D => 0,
        }
    }

    pub fn id(&self) -> u32 {
        Self::ID_MAP
            .iter()
            .find(|(t, _)| t == self)
            .map(|(_, id)| *id)
            .unwrap()
    }

    pub fn name(&self) -> &'static str {
        Self::NAME_MAP
            .iter()
            .find(|(t, _)| t == self)
            .map(|(_, name)| *name)
            .unwrap()
    }
}

impl TryFrom<u32> for VariableType {
    type Error = String;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        VariableType::ID_MAP
            .iter()
            .find(|(_, id)| *id == value)
            .map(|(t, _)| t.clone())
            .ok_or_else(|| format!("无效的类型ID: {}", value))
    }
}

impl TryFrom<&str> for VariableType {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        VariableType::NAME_MAP
            .iter()
            .find(|(_, name)| *name == value)
            .map(|(t, _)| t.clone())
            .ok_or_else(|| format!("无效的类型名称: {}", value))
    }
}

impl From<&VariableType> for u32 {
    fn from(var_type: &VariableType) -> Self {
        var_type.id()
    }
}

impl From<&VariableType> for String {
    fn from(var_type: &VariableType) -> Self {
        var_type.name().to_string()
    }
}

#[derive(Debug)]
pub struct Variable {
    pub name: String,
    pub r#type: TypeSpecifierNonArrayData,
    pub default_data: Vec<u32>,
    pub scope: VariableScope,
    pub array_length: Option<u32>,
}

impl Variable {
    pub fn new() -> Self {
        Variable {
            name: String::new(),
            r#type: TypeSpecifierNonArrayData::Float,
            default_data: vec![],
            scope: VariableScope::UNIFORM,
            array_length: None,
        }
    }

    pub fn set_scope(&mut self, value: u32) -> Result<(), Error> {
        self.scope = match value {
            0 => VariableScope::UNIFORM,
            _ => return Err(Error::new(ErrorKind::InvalidValue)),
        };
        Ok(())
    }

    pub fn set_type(&mut self, value: u32) -> Result<(), Error> {
        self.r#type = match value {
            0 => TypeSpecifierNonArrayData::Float,
            2 => TypeSpecifierNonArrayData::Vec2,
            3 => TypeSpecifierNonArrayData::Vec3,
            4 => TypeSpecifierNonArrayData::Vec4,
            20 => TypeSpecifierNonArrayData::Mat4,
            43 => TypeSpecifierNonArrayData::Sampler2D,
            _ => return Err(Error::new(ErrorKind::InvalidValue)),
        };
        Ok(())
    }

    pub fn get_type_id(&self) -> u32 {
        match self.r#type {
            TypeSpecifierNonArrayData::Float => 0,
            TypeSpecifierNonArrayData::Vec2 => 2,
            TypeSpecifierNonArrayData::Vec3 => 3,
            TypeSpecifierNonArrayData::Vec4 => 4,
            TypeSpecifierNonArrayData::Mat4 => 20,
            TypeSpecifierNonArrayData::Sampler2D => 43,
            _ => panic!("不支持的类型: {:?}", self.r#type),
        }
    }

    pub fn default_data_length(&self) -> usize {
        match self.r#type {
            TypeSpecifierNonArrayData::Float => 1,
            TypeSpecifierNonArrayData::Vec2 => 2,
            TypeSpecifierNonArrayData::Vec3 => 3,
            TypeSpecifierNonArrayData::Vec4 => 4,
            TypeSpecifierNonArrayData::Mat4 => 16,
            TypeSpecifierNonArrayData::Sampler2D => 0,
            _ => panic!("不支持的类型: {:?}", self.r#type),
        }
    }
} 