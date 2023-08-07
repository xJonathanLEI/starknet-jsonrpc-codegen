use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Specification {
    pub openrpc: String,
    pub info: Info,
    pub servers: Vec<String>,
    pub methods: Vec<Method>,
    pub components: Components,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Info {
    pub version: String,
    pub title: String,
    pub license: Empty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Method {
    pub name: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param_structure: Option<String>,
    pub params: Vec<Param>,
    pub result: MethodResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<Reference>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Components {
    pub content_descriptors: Empty,
    pub schemas: IndexMap<String, Schema>,
    pub errors: IndexMap<String, ErrorType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Empty {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Param {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub required: bool,
    pub schema: Schema,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MethodResult {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub schema: Schema,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Schema {
    Ref(Reference),
    OneOf(OneOf),
    AllOf(AllOf),
    Primitive(Primitive),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Reference {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "$comment")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "$ref")]
    pub ref_field: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OneOf {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub one_of: Vec<Schema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AllOf {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub all_of: Vec<Schema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Primitive {
    Array(ArrayPrimitive),
    Boolean(BooleanPrimitive),
    Integer(IntegerPrimitive),
    Object(ObjectPrimitive),
    String(StringPrimitive),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ArrayPrimitive {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub items: Box<Schema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BooleanPrimitive {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct IntegerPrimitive {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ObjectPrimitive {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub properties: IndexMap<String, Schema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StringPrimitive {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "$comment")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#enum: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ErrorType {
    Error(Error),
    Reference(Reference),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Error {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Schema>,
}

impl Schema {
    pub fn title(&self) -> Option<&String> {
        match self {
            Self::Ref(schema) => schema.title.as_ref(),
            Self::OneOf(schema) => schema.title.as_ref(),
            Self::AllOf(schema) => schema.title.as_ref(),
            Self::Primitive(schema) => schema.title(),
        }
    }

    pub fn description(&self) -> Option<&String> {
        match self {
            Self::Ref(schema) => schema.description.as_ref(),
            Self::OneOf(schema) => schema.description.as_ref(),
            Self::AllOf(schema) => schema.description.as_ref(),
            Self::Primitive(schema) => schema.description(),
        }
    }

    pub fn summary(&self) -> Option<&String> {
        match self {
            Self::Ref(_) => None,
            Self::OneOf(_) => None,
            Self::AllOf(_) => None,
            Self::Primitive(schema) => schema.summary(),
        }
    }
}

impl Primitive {
    pub fn title(&self) -> Option<&String> {
        match self {
            Self::Array(schema) => schema.title.as_ref(),
            Self::Boolean(schema) => schema.title.as_ref(),
            Self::Integer(schema) => schema.title.as_ref(),
            Self::Object(schema) => schema.title.as_ref(),
            Self::String(schema) => schema.title.as_ref(),
        }
    }

    pub fn description(&self) -> Option<&String> {
        match self {
            Self::Array(schema) => schema.description.as_ref(),
            Self::Boolean(schema) => schema.description.as_ref(),
            Self::Integer(schema) => schema.description.as_ref(),
            Self::Object(schema) => schema.description.as_ref(),
            Self::String(schema) => schema.description.as_ref(),
        }
    }

    pub fn summary(&self) -> Option<&String> {
        match self {
            Self::Array(_) => None,
            Self::Boolean(_) => None,
            Self::Integer(_) => None,
            Self::Object(schema) => schema.summary.as_ref(),
            Self::String(_) => None,
        }
    }
}

impl Reference {
    pub fn name(&self) -> &str {
        match self.ref_field.rfind('/') {
            Some(ind_slash) => &self.ref_field[ind_slash + 1..],
            None => &self.ref_field,
        }
    }
}
