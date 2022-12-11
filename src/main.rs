use std::collections::HashSet;

use anyhow::Result;
use regex::Regex;

use crate::spec::*;

mod spec;

const MAX_LINE_LENGTH: usize = 100;

const TARGET_VERSION: SpecVersion = SpecVersion::V0_2_1;

struct GenerationProfile {
    version: SpecVersion,
    raw_specs: &'static str,
    flatten_options: FlattenOption,
    ignore_types: Vec<String>,
    fixed_field_types: Vec<RustTypeWithFixedFields>,
}

#[derive(PartialEq, Eq)]
enum SpecVersion {
    V0_1_0,
    V0_2_1,
}

struct RustTypeWithFixedFields {
    name: &'static str,
    fields: Vec<FixedField>,
}

#[derive(Clone)]
struct FixedField {
    name: &'static str,
    value: &'static str,
}

struct TypeResolutionResult {
    types: Vec<RustType>,
    not_implemented: Vec<String>,
}

struct RustType {
    title: Option<String>,
    description: Option<String>,
    name: String,
    content: RustTypeKind,
}

#[allow(unused)]
enum RustTypeKind {
    Struct(RustStruct),
    Enum(RustEnum),
    Wrapper(RustWrapper),
}

struct RustStruct {
    fields: Vec<RustField>,
}

struct RustEnum {
    variants: Vec<RustVariant>,
}

struct RustWrapper {
    type_name: String,
}

struct RustField {
    description: Option<String>,
    name: String,
    optional: bool,
    type_name: String,
    serde_rename: Option<String>,
    serde_faltten: bool,
    serializer: Option<SerializerOverride>,
}

struct RustVariant {
    description: Option<String>,
    name: String,
    serde_name: String,
}

struct RustFieldType {
    type_name: String,
    serializer: Option<SerializerOverride>,
}

enum SerializerOverride {
    Serde(String),
    SerdeAs(String),
}

enum FlattenOption {
    All,
    Selected(Vec<String>),
}

impl RustType {
    pub fn render_stdout(&self, fixed_fields: &[FixedField]) {
        match (self.title.as_ref(), self.description.as_ref()) {
            (Some(title), Some(description)) => {
                print_doc(title, 0);
                println!("///");
                print_doc(description, 0);
            }
            (Some(title), None) => {
                print_doc(title, 0);
            }
            (None, Some(description)) => {
                print_doc(description, 0);
            }
            (None, None) => {}
        }

        self.content.render_stdout(&self.name, fixed_fields);
    }

    pub fn render_serde_stdout(&self, fixed_fields: &[FixedField]) {
        match &self.content {
            RustTypeKind::Struct(content) => content.render_serde_stdout(&self.name, fixed_fields),
            _ => todo!("serde blocks only implemented for structs"),
        }
    }
}

impl RustTypeKind {
    pub fn render_stdout(&self, name: &str, fixed_fields: &[FixedField]) {
        match self {
            Self::Struct(value) => value.render_stdout(name, fixed_fields),
            Self::Enum(value) => value.render_stdout(name),
            Self::Wrapper(value) => value.render_stdout(name),
        }
    }
}

impl RustStruct {
    pub fn render_stdout(&self, name: &str, fixed_fields: &[FixedField]) {
        let derive_serde = fixed_fields.is_empty();

        if derive_serde
            && self
                .fields
                .iter()
                .any(|item| matches!(item.serializer, Some(SerializerOverride::SerdeAs(_))))
        {
            println!("#[serde_as]");
        }
        if derive_serde {
            println!("#[derive(Debug, Clone, Serialize, Deserialize)]");
        } else {
            println!("#[derive(Debug, Clone)]");
        }
        println!("pub struct {} {{", name);

        for field in self.fields.iter() {
            // Fixed fields only exist in serde impls
            if fixed_fields.iter().any(|item| item.name == field.name) {
                continue;
            }

            if let Some(doc) = &field.description {
                print_doc(doc, 4);
            }

            for line in field.def_lines(4, derive_serde, false) {
                println!("{line}")
            }
        }

        println!("}}");
    }

    pub fn render_serde_stdout(&self, name: &str, fixed_fields: &[FixedField]) {
        self.render_impl_serialize_stdout(name, fixed_fields);
        println!();
        self.render_impl_deserialize_stdout(name, fixed_fields);
    }

    fn render_impl_serialize_stdout(&self, name: &str, fixed_fields: &[FixedField]) {
        println!("impl Serialize for {} {{", name);
        println!(
            "    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {{"
        );

        if self
            .fields
            .iter()
            .any(|item| matches!(item.serializer, Some(SerializerOverride::SerdeAs(_))))
        {
            println!("        #[serde_as]");
        }

        println!("        #[derive(Serialize)]");
        println!("        struct Tagged<'a> {{");

        for field in self.fields.iter() {
            for line in field.def_lines(12, true, true).iter() {
                println!("{line}");
            }
        }

        println!("        }}");
        println!();
        println!("        let tagged = Tagged {{");

        for field in self.fields.iter() {
            match fixed_fields.iter().find(|item| item.name == field.name) {
                Some(fixed_field) => {
                    println!(
                        "            {}: {},",
                        escape_name(&field.name),
                        fixed_field.value
                    )
                }
                None => println!(
                    "            {}: &self.{},",
                    escape_name(&field.name),
                    escape_name(&field.name)
                ),
            }
        }

        println!("        }};");
        println!();
        println!("        Tagged::serialize(&tagged, serializer)");

        println!("    }}");
        println!("}}");
    }

    fn render_impl_deserialize_stdout(&self, name: &str, fixed_fields: &[FixedField]) {
        println!("impl<'de> Deserialize<'de> for {} {{", name);
        println!("    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {{");

        if self
            .fields
            .iter()
            .any(|item| matches!(item.serializer, Some(SerializerOverride::SerdeAs(_))))
        {
            println!("        #[serde_as]");
        }

        println!("        #[derive(Deserialize)]");
        println!("        struct Tagged {{");

        for field in self.fields.iter() {
            let lines = match fixed_fields.iter().find(|item| item.name == field.name) {
                Some(_) => RustField {
                    description: field.description.clone(),
                    name: field.name.clone(),
                    optional: true,
                    type_name: format!("Option<{}>", field.type_name),
                    serde_rename: field.serde_rename.clone(),
                    serde_faltten: field.serde_faltten,
                    serializer: field.serializer.as_ref().map(|value| value.to_optional()),
                }
                .def_lines(12, true, false),
                None => field.def_lines(12, true, false),
            };

            for line in lines.iter() {
                println!("{line}");
            }
        }

        println!("        }}");
        println!();
        println!("        let tagged = Tagged::deserialize(deserializer)?;");
        println!();

        for fixed_field in fixed_fields.iter() {
            println!(
                "        if let Some(tag_field) = &tagged.{} {{",
                escape_name(fixed_field.name)
            );
            println!("            if tag_field != {} {{", fixed_field.value);
            println!(
                "                return Err(serde::de::Error::custom(\"Invalid `{}` value\"));",
                fixed_field.name
            );
            println!("            }}");
            println!("        }}");
            println!();
        }

        println!("        Ok(Self {{");

        for field in self.fields.iter() {
            if !fixed_fields.iter().any(|item| item.name == field.name) {
                println!(
                    "            {}: tagged.{},",
                    escape_name(&field.name),
                    escape_name(&field.name)
                );
            }
        }

        println!("        }})");

        println!("    }}");
        println!("}}");
    }
}

impl RustEnum {
    pub fn render_stdout(&self, name: &str) {
        println!("#[derive(Debug, Clone, Serialize, Deserialize)]");
        println!("pub enum {} {{", name);

        for variant in self.variants.iter() {
            if let Some(doc) = &variant.description {
                print_doc(doc, 4);
            }

            println!("    #[serde(rename = \"{}\")]", variant.serde_name);
            println!("    {},", variant.name);
        }

        println!("}}");
    }
}

impl RustWrapper {
    pub fn render_stdout(&self, name: &str) {
        println!("#[derive(Debug, Clone, Serialize, Deserialize)]");
        println!("pub struct {}(pub {});", name, self.type_name);
    }
}

impl RustField {
    pub fn def_lines(&self, leading_spaces: usize, serde_attrs: bool, is_ref: bool) -> Vec<String> {
        let mut lines = vec![];

        let leading_spaces = " ".repeat(leading_spaces);

        if serde_attrs {
            if self.optional {
                lines.push(format!(
                    "{}#[serde(default, skip_serializing_if = \"Option::is_none\")]",
                    leading_spaces
                ));
            }
            if let Some(serde_rename) = &self.serde_rename {
                lines.push(format!(
                    "{}#[serde(rename = \"{}\")]",
                    leading_spaces, serde_rename
                ));
            }
            if self.serde_faltten {
                lines.push(format!("{}#[serde(flatten)]", leading_spaces));
            }
            if let Some(serde_as) = &self.serializer {
                lines.push(match serde_as {
                    SerializerOverride::Serde(serializer) => {
                        format!("{}#[serde(with = \"{}\")]", leading_spaces, serializer)
                    }
                    SerializerOverride::SerdeAs(serializer) => {
                        format!("{}#[serde_as(as = \"{}\")]", leading_spaces, serializer)
                    }
                });
            }
        }

        lines.push(format!(
            "{}pub {}: {},",
            leading_spaces,
            escape_name(&self.name),
            if is_ref {
                if self.type_name == "String" {
                    String::from("&'a str")
                } else {
                    format!("&'a {}", self.type_name)
                }
            } else {
                self.type_name.clone()
            },
        ));

        lines
    }
}

impl SerializerOverride {
    pub fn to_optional(&self) -> Self {
        match self {
            SerializerOverride::Serde(_) => {
                todo!("Optional transformation of #[serde(with)] not implemented")
            }
            SerializerOverride::SerdeAs(serde_as) => Self::SerdeAs(format!("Option<{}>", serde_as)),
        }
    }
}

fn main() {
    let profiles: [GenerationProfile; 2] = [
        GenerationProfile {
            version: SpecVersion::V0_1_0,
            raw_specs: include_str!("./specs/0.1.0/starknet_api_openrpc.json"),
            flatten_options: FlattenOption::Selected(vec![
                String::from("BLOCK_BODY_WITH_TXS"),
                String::from("BLOCK_BODY_WITH_TX_HASHES"),
            ]),
            ignore_types: vec![],
            fixed_field_types: vec![],
        },
        GenerationProfile {
            version: SpecVersion::V0_2_1,
            raw_specs: include_str!("./specs/0.2.1/starknet_api_openrpc.json"),
            flatten_options: FlattenOption::All,
            ignore_types: vec![],
            fixed_field_types: vec![RustTypeWithFixedFields {
                name: "DeclareTransaction",
                fields: vec![FixedField {
                    name: "type",
                    value: "\"DECLARE\"",
                }],
            }],
        },
    ];

    let profile = profiles
        .into_iter()
        .find(|profile| profile.version == TARGET_VERSION)
        .expect("Unable to find profile");

    let specs: Specification =
        serde_json::from_str(profile.raw_specs).expect("Failed to parse specification");

    println!("// AUTO-GENERATED CODE. DO NOT EDIT");
    println!("// To change the code generated, modify the codegen tool instead:");
    println!("//     https://github.com/xJonathanLEI/starknet-jsonrpc-codegen");
    println!();

    if !profile.ignore_types.is_empty() {
        println!("// These types are ignored from code generation. Implement them manually:");
        for ignored_type in profile.ignore_types.iter() {
            println!("// - `{}`", ignored_type);
        }
        println!();
    }

    let result = resolve_types(&specs, &profile.flatten_options, &profile.ignore_types)
        .expect("Failed to resolve types");

    if !result.not_implemented.is_empty() {
        println!("// Code generation requested but not implemented for these types:");
        for type_name in result.not_implemented.iter() {
            println!("// - `{}`", type_name);
        }
        println!();
    }

    println!("use serde::{{Deserialize, Deserializer, Serialize, Serializer}};");
    println!("use serde_with::serde_as;");
    println!("use starknet_core::{{");
    println!("    serde::{{byte_array::base64, unsigned_field_element::UfeHex}},");
    println!("    types::FieldElement,");
    println!("}};");
    println!();

    // In later versions this type is still defined by never actually used
    if profile.version == SpecVersion::V0_1_0 {
        println!("pub use starknet_core::types::L1Address as EthAddress;");
        println!();
    }

    println!("use super::{{serde_impls::NumAsHex, *}};");
    println!();

    let mut manual_serde_types = vec![];

    for (ind, rust_type) in result.types.iter().enumerate() {
        let fixed_fields_for_type = profile
            .fixed_field_types
            .iter()
            .find_map(|item| {
                if item.name == rust_type.name {
                    Some(item.fields.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        if !fixed_fields_for_type.is_empty() {
            manual_serde_types.push(rust_type);
        }

        rust_type.render_stdout(&fixed_fields_for_type);

        if ind != result.types.len() - 1 || !manual_serde_types.is_empty() {
            println!();
        }
    }

    for (ind, rust_type) in manual_serde_types.iter().enumerate() {
        let fixed_fields_for_type = profile
            .fixed_field_types
            .iter()
            .find_map(|item| {
                if item.name == rust_type.name {
                    Some(item.fields.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        rust_type.render_serde_stdout(&fixed_fields_for_type);

        if ind != manual_serde_types.len() - 1 {
            println!();
        }
    }
}

fn resolve_types(
    specs: &Specification,
    flatten_option: &FlattenOption,
    ignore_types: &[String],
) -> Result<TypeResolutionResult> {
    let mut types = vec![];
    let mut not_implemented_types = vec![];

    let flatten_only_types = get_flatten_only_schemas(specs, flatten_option);

    for (name, entity) in specs.components.schemas.iter() {
        let rusty_name = to_starknet_rs_name(name);

        let title = entity.title();
        let description = match entity.description() {
            Some(description) => Some(description),
            None => entity.summary(),
        };

        // Explicitly ignored types
        if ignore_types.contains(name) {
            continue;
        }

        // Manual override exists
        if get_field_type_override(name).is_some() {
            continue;
        }

        if flatten_only_types.contains(name) {
            continue;
        }

        let content = {
            match entity {
                Schema::Ref(reference) => {
                    let mut fields = vec![];
                    let redirected_schema = specs
                        .components
                        .schemas
                        .get(reference.name())
                        .ok_or_else(|| anyhow::anyhow!(""))?;
                    get_schema_fields(redirected_schema, specs, &mut fields, flatten_option)?;
                    RustTypeKind::Struct(RustStruct { fields })
                }
                Schema::OneOf(_) => {
                    not_implemented_types.push(name.to_owned());

                    eprintln!(
                        "OneOf enum generation not implemented. Enum not generated for {}",
                        name
                    );
                    continue;
                }
                Schema::AllOf(_) | Schema::Primitive(Primitive::Object(_)) => {
                    let mut fields = vec![];
                    get_schema_fields(entity, specs, &mut fields, flatten_option)?;
                    RustTypeKind::Struct(RustStruct { fields })
                }
                Schema::Primitive(Primitive::String(value)) => match &value.r#enum {
                    Some(variants) => RustTypeKind::Enum(RustEnum {
                        variants: variants
                            .iter()
                            .map(|item| RustVariant {
                                description: None,
                                name: to_starknet_rs_name(item),
                                serde_name: item.to_owned(),
                            })
                            .collect(),
                    }),
                    None => {
                        anyhow::bail!(
                            "Unexpected non-enum string type when generating struct/enum"
                        );
                    }
                },
                _ => {
                    anyhow::bail!("Unexpected schema type when generating struct/enum");
                }
            }
        };

        types.push(RustType {
            title: title.map(|value| to_starknet_rs_doc(value, true)),
            description: description.map(|value| to_starknet_rs_doc(value, true)),
            name: rusty_name,
            content,
        });
    }

    Ok(TypeResolutionResult {
        types,
        not_implemented: not_implemented_types,
    })
}

/// Finds the list of schemas that are used and only used for flattening inside objects
fn get_flatten_only_schemas(specs: &Specification, flatten_option: &FlattenOption) -> Vec<String> {
    // We need this for now since we don't search method calls, so we could get false positives
    const HARD_CODED_NON_FLATTEN_SCHEMAS: [&str; 1] = ["FUNCTION_CALL"];

    let mut flatten_fields = HashSet::<String>::new();
    let mut non_flatten_fields = HashSet::<String>::new();

    for (_, schema) in specs.components.schemas.iter() {
        visit_schema_for_flatten_only(
            schema,
            flatten_option,
            &mut flatten_fields,
            &mut non_flatten_fields,
        );
    }

    flatten_fields
        .into_iter()
        .filter_map(|item| {
            if non_flatten_fields.contains(&item)
                || HARD_CODED_NON_FLATTEN_SCHEMAS.contains(&item.as_str())
            {
                None
            } else {
                Some(item)
            }
        })
        .collect()
}

fn visit_schema_for_flatten_only(
    schema: &Schema,
    flatten_option: &FlattenOption,
    flatten_fields: &mut HashSet<String>,
    non_flatten_fields: &mut HashSet<String>,
) {
    match schema {
        Schema::OneOf(one_of) => {
            // Recursion
            for variant in one_of.one_of.iter() {
                match variant {
                    Schema::Ref(reference) => {
                        non_flatten_fields.insert(reference.name().to_owned());
                    }
                    _ => visit_schema_for_flatten_only(
                        variant,
                        flatten_option,
                        flatten_fields,
                        non_flatten_fields,
                    ),
                }
            }
        }
        Schema::AllOf(all_of) => {
            for fragment in all_of.all_of.iter() {
                match fragment {
                    Schema::Ref(reference) => {
                        let should_flatten = match flatten_option {
                            FlattenOption::All => true,
                            FlattenOption::Selected(flatten_types) => {
                                flatten_types.contains(&reference.name().to_owned())
                            }
                        };

                        if should_flatten {
                            flatten_fields.insert(reference.name().to_owned());
                        } else {
                            non_flatten_fields.insert(reference.name().to_owned());
                            visit_schema_for_flatten_only(
                                fragment,
                                flatten_option,
                                flatten_fields,
                                non_flatten_fields,
                            );
                        }
                    }
                    _ => visit_schema_for_flatten_only(
                        fragment,
                        flatten_option,
                        flatten_fields,
                        non_flatten_fields,
                    ),
                }
            }
        }
        Schema::Primitive(Primitive::Object(object)) => {
            for (_, prop_type) in object.properties.iter() {
                match prop_type {
                    Schema::Ref(reference) => {
                        non_flatten_fields.insert(reference.name().to_owned());
                    }
                    _ => visit_schema_for_flatten_only(
                        prop_type,
                        flatten_option,
                        flatten_fields,
                        non_flatten_fields,
                    ),
                }
            }
        }
        Schema::Primitive(Primitive::Array(array)) => match array.items.as_ref() {
            Schema::Ref(reference) => {
                non_flatten_fields.insert(reference.name().to_owned());
            }
            _ => visit_schema_for_flatten_only(
                &array.items,
                flatten_option,
                flatten_fields,
                non_flatten_fields,
            ),
        },
        _ => {}
    }
}

fn get_schema_fields(
    schema: &Schema,
    specs: &Specification,
    fields: &mut Vec<RustField>,
    flatten_option: &FlattenOption,
) -> Result<()> {
    match schema {
        Schema::Ref(value) => {
            let ref_type_name = value.name();
            let ref_type = match specs.components.schemas.get(ref_type_name) {
                Some(ref_type) => ref_type,
                None => anyhow::bail!("Ref target type not found: {}", ref_type_name),
            };

            // Schema redirection
            get_schema_fields(ref_type, specs, fields, flatten_option)?;
        }
        Schema::AllOf(value) => {
            for item in value.all_of.iter() {
                match item {
                    Schema::Ref(reference) => {
                        let should_flatten = match flatten_option {
                            FlattenOption::All => true,
                            FlattenOption::Selected(flatten_types) => {
                                flatten_types.contains(&reference.name().to_owned())
                            }
                        };

                        if should_flatten {
                            get_schema_fields(item, specs, fields, flatten_option)?;
                        } else {
                            fields.push(RustField {
                                description: reference.description.to_owned(),
                                name: reference.name().to_lowercase(),
                                optional: false,
                                type_name: to_starknet_rs_name(reference.name()),
                                serde_rename: None,
                                serde_faltten: true,
                                serializer: None,
                            });
                        }
                    }
                    _ => {
                        // We don't have a choice but to flatten it
                        get_schema_fields(item, specs, fields, flatten_option)?;
                    }
                }
            }
        }
        Schema::Primitive(Primitive::Object(value)) => {
            for (name, prop_value) in value.properties.iter() {
                // For fields we keep things simple and only use one line
                let doc_string = match prop_value.title() {
                    Some(text) => Some(text),
                    None => match prop_value.description() {
                        Some(text) => Some(text),
                        None => prop_value.summary(),
                    },
                };

                let field_type = get_rust_type_for_field(prop_value, specs)?;

                let lower_name = name.to_lowercase();
                let rename = if name == &lower_name {
                    None
                } else {
                    Some(name.to_owned())
                };

                // Optional field transformation
                let field_optional = match &value.required {
                    Some(required) => !required.contains(name),
                    None => false,
                };
                let type_name = if field_optional {
                    format!("Option<{}>", field_type.type_name)
                } else {
                    field_type.type_name
                };
                let serializer = if field_optional {
                    field_type.serializer.map(|value| value.to_optional())
                } else {
                    field_type.serializer
                };

                fields.push(RustField {
                    description: doc_string.map(|value| to_starknet_rs_doc(value, false)),
                    name: lower_name,
                    optional: field_optional,
                    type_name,
                    serde_rename: rename,
                    serde_faltten: false,
                    serializer,
                });
            }
        }
        _ => {
            dbg!(schema);
            anyhow::bail!("Unexpected schema type when getting object fields");
        }
    }

    Ok(())
}

fn get_rust_type_for_field(schema: &Schema, specs: &Specification) -> Result<RustFieldType> {
    match schema {
        Schema::Ref(value) => {
            let ref_type_name = value.name();
            if !specs.components.schemas.contains_key(ref_type_name) {
                anyhow::bail!("Ref target type not found: {}", ref_type_name);
            }

            // Hard-coded special rules
            Ok(
                get_field_type_override(ref_type_name).unwrap_or_else(|| RustFieldType {
                    type_name: to_starknet_rs_name(ref_type_name),
                    serializer: None,
                }),
            )
        }
        Schema::OneOf(_) => {
            anyhow::bail!("Anonymous oneOf types should not be used for properties");
        }
        Schema::AllOf(_) => {
            anyhow::bail!("Anonymous allOf types should not be used for properties");
        }
        Schema::Primitive(value) => match value {
            Primitive::Array(value) => {
                let item_type = get_rust_type_for_field(&value.items, specs)?;
                let serializer = match item_type.serializer {
                    Some(SerializerOverride::Serde(_)) => {
                        todo!("Array wrapper for #[serde(with)] not implemented")
                    }
                    Some(SerializerOverride::SerdeAs(serializer)) => {
                        Some(SerializerOverride::SerdeAs(format!("Vec<{}>", serializer)))
                    }
                    None => None,
                };
                Ok(RustFieldType {
                    type_name: format!("Vec<{}>", item_type.type_name),
                    serializer,
                })
            }
            Primitive::Boolean(_) => Ok(RustFieldType {
                type_name: String::from("bool"),
                serializer: None,
            }),
            Primitive::Integer(_) => Ok(RustFieldType {
                type_name: String::from("u64"),
                serializer: None,
            }),
            Primitive::Object(_) => {
                anyhow::bail!("Anonymous object types should not be used for properties");
            }
            Primitive::String(value) => {
                // Hacky solution but it's the best we can do given the specs
                if let Some(desc) = &value.description {
                    if desc.contains("base64") {
                        return Ok(RustFieldType {
                            type_name: String::from("Vec<u8>"),
                            serializer: Some(SerializerOverride::Serde(String::from("base64"))),
                        });
                    }
                }

                Ok(RustFieldType {
                    type_name: String::from("String"),
                    serializer: None,
                })
            }
        },
    }
}

fn get_field_type_override(type_name: &str) -> Option<RustFieldType> {
    Some(match type_name {
        "ADDRESS" | "STORAGE_KEY" | "TXN_HASH" | "FELT" | "BLOCK_HASH" | "CHAIN_ID"
        | "PROTOCOL_VERSION" => RustFieldType {
            type_name: String::from("FieldElement"),
            serializer: Some(SerializerOverride::SerdeAs(String::from("UfeHex"))),
        },
        "BLOCK_NUMBER" => RustFieldType {
            type_name: String::from("u64"),
            serializer: None,
        },
        "NUM_AS_HEX" => RustFieldType {
            type_name: String::from("u64"),
            serializer: Some(SerializerOverride::SerdeAs(String::from("NumAsHex"))),
        },
        "ETH_ADDRESS" => RustFieldType {
            type_name: String::from("EthAddress"),
            serializer: None,
        },
        "SIGNATURE" => RustFieldType {
            type_name: String::from("Vec<FieldElement>"),
            serializer: Some(SerializerOverride::SerdeAs(String::from("Vec<UfeHex>"))),
        },
        "CONTRACT_ABI" => RustFieldType {
            type_name: String::from("Vec<ContractAbiEntry>"),
            serializer: None,
        },
        "CONTRACT_ENTRY_POINT_LIST" => RustFieldType {
            type_name: String::from("Vec<ContractEntryPoint>"),
            serializer: None,
        },
        "TXN_TYPE" => RustFieldType {
            type_name: String::from("String"),
            serializer: None,
        },
        _ => return None,
    })
}

fn print_doc(doc: &str, indent_spaces: usize) {
    let prefix = format!("{}/// ", " ".repeat(indent_spaces));
    for line in wrap_lines(doc, prefix.len()) {
        println!("{}{}", prefix, line);
    }
}

fn wrap_lines(doc: &str, prefix_length: usize) -> Vec<String> {
    let mut lines = vec![];
    let mut current_line = String::new();

    for part in doc.split(' ') {
        let mut addition = String::new();
        if !current_line.is_empty() {
            addition.push(' ');
        }
        addition.push_str(part);

        if prefix_length + current_line.len() + addition.len() <= MAX_LINE_LENGTH {
            current_line.push_str(&addition);
        } else {
            lines.push(current_line.clone());
            current_line.clear();
            current_line.push_str(part);
        }
    }

    lines.push(current_line);
    lines
}

fn to_starknet_rs_name(name: &str) -> String {
    let name = to_pascal_case(name).replace("Txn", "Transaction");

    // Hard-coded renames
    match name.as_ref() {
        "CommonTransactionProperties" => String::from("TransactionMeta"),
        "CommonReceiptProperties" => String::from("TransactionReceiptMeta"),
        "InvokeTransactionReceiptProperties" => String::from("InvokeTransactionReceiptData"),
        "PendingCommonReceiptProperties" => String::from("PendingTransactionReceiptMeta"),
        _ => name,
    }
}

fn to_starknet_rs_doc(doc: &str, force_period: bool) -> String {
    let mut doc = to_sentence_case(doc);

    for (pattern, target) in [
        (Regex::new(r"(?i)\bethereum\b").unwrap(), "Ethereum"),
        (Regex::new(r"(?i)\bstarknet\b").unwrap(), "StarkNet"),
        (Regex::new(r"(?i)\bstarknet\.io\b").unwrap(), "starknet.io"),
        (Regex::new(r"\bStarknet\b").unwrap(), "L1"),
        (Regex::new(r"\bl1\b").unwrap(), "L1"),
        (Regex::new(r"\bl2\b").unwrap(), "L2"),
        (Regex::new(r"\bunix\b").unwrap(), "Unix"),
    ]
    .into_iter()
    {
        doc = pattern.replace_all(&doc, target).into_owned();
    }

    if force_period && !doc.ends_with('.') {
        doc.push('.');
    }

    doc
}

fn to_pascal_case(name: &str) -> String {
    let mut result = String::new();

    let mut last_underscore = None;
    for (ind, character) in name.chars().enumerate() {
        if character == '_' {
            last_underscore = Some(ind);
            continue;
        }

        let uppercase = match last_underscore {
            Some(last_underscore) => ind == last_underscore + 1,
            None => ind == 0,
        };

        result.push(if uppercase {
            character.to_ascii_uppercase()
        } else {
            character.to_ascii_lowercase()
        });
    }

    result
}

fn to_sentence_case(name: &str) -> String {
    let mut result = String::new();

    let mut last_period = None;
    let mut last_char = None;

    for (ind, character) in name.chars().enumerate() {
        if character == '.' {
            last_period = Some(ind);
        }

        let uppercase = match last_period {
            Some(last_period) => ind == last_period + 2 && matches!(last_char, Some(' ')),
            None => ind == 0,
        };

        result.push(if uppercase {
            character.to_ascii_uppercase()
        } else {
            character.to_ascii_lowercase()
        });

        last_char = Some(character);
    }

    result
}

fn escape_name(name: &str) -> &str {
    if name == "type" {
        "r#type"
    } else {
        name
    }
}
