use std::collections::HashSet;

use anyhow::Result;
use regex::Regex;

use crate::spec::*;

mod spec;

const MAX_LINE_LENGTH: usize = 100;

const STARKNET_API_OPENRPC: &str = include_str!("./specs/0.2.1/starknet_api_openrpc.json");

struct RustType {
    title: Option<String>,
    description: Option<String>,
    name: String,
    content: RustTypeKind,
}

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

#[allow(unused)]
enum FlattenOption {
    All,
    Selected(Vec<String>),
}

impl RustType {
    pub fn render_stdout(&self, trailing_line: bool) {
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

        self.content.render_stdout(&self.name);

        if trailing_line {
            println!();
        }
    }
}

impl RustTypeKind {
    pub fn render_stdout(&self, name: &str) {
        match self {
            Self::Struct(value) => value.render_stdout(name),
            Self::Enum(value) => value.render_stdout(name),
            Self::Wrapper(value) => value.render_stdout(name),
        }
    }
}

impl RustStruct {
    pub fn render_stdout(&self, name: &str) {
        if self
            .fields
            .iter()
            .any(|item| matches!(item.serializer, Some(SerializerOverride::SerdeAs(_))))
        {
            println!("#[serde_as]");
        }
        println!("#[derive(Debug, Clone, Serialize, Deserialize)]");
        println!("pub struct {} {{", name);

        for field in self.fields.iter() {
            if let Some(doc) = &field.description {
                print_doc(doc, 4);
            }
            if field.optional {
                println!("    #[serde(default, skip_serializing_if = \"Option::is_none\")]");
            }
            if let Some(serde_rename) = &field.serde_rename {
                println!("    #[serde(rename = \"{}\")]", serde_rename);
            }
            if field.serde_faltten {
                println!("    #[serde(flatten)]");
            }
            if let Some(serde_as) = &field.serializer {
                match serde_as {
                    SerializerOverride::Serde(serializer) => {
                        println!("    #[serde(with = \"{}\")]", serializer);
                    }
                    SerializerOverride::SerdeAs(serializer) => {
                        println!("    #[serde_as(as = \"{}\")]", serializer);
                    }
                }
            }

            let escaped_name = if field.name == "type" {
                "r#type"
            } else {
                &field.name
            };
            println!("    pub {}: {},", escaped_name, field.type_name);
        }

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

fn main() {
    let specs: Specification =
        serde_json::from_str(STARKNET_API_OPENRPC).expect("Failed to parse specification");

    println!("use serde::{{Deserialize, Serialize}};");
    println!("use serde_with::serde_as;");
    println!("use starknet_core::{{");
    println!("    serde::{{byte_array::base64, unsigned_field_element::UfeHex}},");
    println!("    types::FieldElement,");
    println!("}};");
    println!();
    println!("pub use starknet_core::types::L1Address as EthAddress;");
    println!();
    println!("use super::serde_impls::NumAsHex;");
    println!();

    let types = resolve_types(&specs, &FlattenOption::All).expect("Failed to resolve types");
    for (ind, rust_type) in types.iter().enumerate() {
        rust_type.render_stdout(ind != types.len() - 1);
    }
}

fn resolve_types(specs: &Specification, flatten_option: &FlattenOption) -> Result<Vec<RustType>> {
    let mut types = vec![];

    let flatten_only_types = get_flatten_only_schemas(specs, flatten_option);

    for (name, entity) in specs.components.schemas.iter() {
        let rusty_name = to_starknet_rs_name(name);

        let title = entity.title();
        let description = match entity.description() {
            Some(description) => Some(description),
            None => entity.summary(),
        };

        eprintln!("Processing schema: {}", name);

        if flatten_only_types.contains(name) {
            continue;
        }

        // Manual override exists
        if get_field_type_override(name).is_some() {
            continue;
        }

        let content = {
            match entity {
                Schema::Ref(_) => RustTypeKind::Wrapper(RustWrapper {
                    type_name: get_rust_type_for_field(entity, specs)?.type_name,
                }),
                Schema::OneOf(_) => {
                    // TODO: implement
                    eprintln!("WARNING: enum generation with oneOf not implemented");
                    continue;
                }
                Schema::AllOf(_) | Schema::Primitive(Primitive::Object(_)) => {
                    let mut fields = vec![];
                    if get_schema_fields(entity, specs, &mut fields, flatten_option).is_err() {
                        eprintln!("WARNING: unable to generate struct for {name}");
                        continue;
                    }
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

    Ok(types)
}

/// Finds the list of schemas that are used and only used for flattening inside objects
fn get_flatten_only_schemas(specs: &Specification, flatten_option: &FlattenOption) -> Vec<String> {
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
            if non_flatten_fields.contains(&item) {
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
                    match field_type.serializer {
                        Some(SerializerOverride::Serde(_)) => {
                            todo!("Optional transformation of #[serde(with)] not implemented")
                        }
                        Some(SerializerOverride::SerdeAs(serde_as)) => {
                            Some(SerializerOverride::SerdeAs(format!("Option<{}>", serde_as)))
                        }
                        None => None,
                    }
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
    to_pascal_case(name).replace("Txn", "Transaction")
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
