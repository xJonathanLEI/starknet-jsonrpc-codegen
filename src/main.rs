use std::collections::BTreeMap;

use serde::Deserialize;

const STARKNET_API_OPENRPC: &str = include_str!("./specs/starknet_api_openrpc.json");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Specification {
    pub openrpc: String,
    pub info: Info,
    pub servers: Vec<String>,
    pub methods: Vec<Method>,
    pub components: Components,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Info {
    pub version: String,
    pub title: String,
    pub license: Empty,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Method {
    pub name: String,
    pub summary: String,
    pub description: Option<String>,
    pub param_structure: Option<String>,
    pub params: Vec<Param>,
    pub result: MethodResult,
    pub errors: Option<Vec<Reference>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Components {
    pub content_descriptors: Empty,
    pub schemas: BTreeMap<String, Schema>,
    pub errors: BTreeMap<String, ErrorType>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Empty {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Param {
    pub name: String,
    pub description: Option<String>,
    pub summary: Option<String>,
    pub required: bool,
    pub schema: Schema,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MethodResult {
    pub name: String,
    pub description: Option<String>,
    pub schema: Schema,
    pub summary: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Schema {
    Ref(Reference),
    OneOf(OneOf),
    AllOf(AllOf),
    Primitive(Primitive),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Reference {
    pub title: Option<String>,
    #[serde(rename = "$comment")]
    pub comment: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "$ref")]
    pub ref_field: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OneOf {
    pub title: Option<String>,
    pub description: Option<String>,
    pub one_of: Vec<Schema>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AllOf {
    pub title: Option<String>,
    pub description: Option<String>,
    pub all_of: Vec<Schema>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Primitive {
    Array(ArrayPrimitive),
    Boolean(BooleanPrimitive),
    Integer(IntegerPrimitive),
    Object(ObjectPrimitive),
    String(StringPrimitive),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ArrayPrimitive {
    pub title: Option<String>,
    pub description: Option<String>,
    pub items: Box<Schema>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BooleanPrimitive {
    pub description: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct IntegerPrimitive {
    pub description: Option<String>,
    pub minimum: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ObjectPrimitive {
    pub title: Option<String>,
    pub description: Option<String>,
    pub summary: Option<String>,
    pub properties: BTreeMap<String, Schema>,
    pub required: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StringPrimitive {
    pub title: Option<String>,
    #[serde(rename = "$comment")]
    pub comment: Option<String>,
    pub description: Option<String>,
    pub r#enum: Option<Vec<String>>,
    pub pattern: Option<String>,
}
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ErrorType {
    Error(Error),
    Reference(Reference),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Error {
    pub code: i64,
    pub message: String,
    pub data: Option<Schema>,
}

fn main() {
    let api_specs: Specification =
        serde_json::from_str(STARKNET_API_OPENRPC).expect("Failed to parse specification");
    dbg!(api_specs);
}
