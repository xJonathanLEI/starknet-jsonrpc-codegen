use std::collections::HashSet;

use anyhow::Result;
use clap::Parser;
use indexmap::IndexSet;
use regex::Regex;

use crate::{
    built_info, spec::*, AdditionalDerivesOptions, ArcWrappingOptions, FixedField,
    FixedFieldsOptions, FlattenOption, GenerationProfile, SpecVersion,
};

#[derive(Debug, Parser)]
pub struct Generate {
    #[clap(long, env, help = "Version of the specification")]
    spec: SpecVersion,
}

const MAX_LINE_LENGTH: usize = 100;

#[derive(Debug, Clone)]
struct TypeResolutionResult {
    model_types: Vec<RustType>,
    aliases: Vec<RustAlias>,
    request_response_types: Vec<RustType>,
    not_implemented: Vec<String>,
}

#[derive(Debug, Clone)]
struct RustType {
    title: Option<String>,
    description: Option<String>,
    name: String,
    content: RustTypeKind,
}

#[derive(Debug, Clone)]
struct RustAlias {
    name: String,
    content: RustAliasContent,
}

#[derive(Debug, Clone)]
enum SchemaToRustTypeResult {
    Type(RustTypeKind),
    Alias(RustAliasContent),
}

#[derive(Debug, Clone)]
enum RustTypeKind {
    Struct(RustStruct),
    Enum(RustEnum),
    Wrapper(RustWrapper),
    Unit(RustUnit),
}

#[derive(Debug, Clone)]
struct RustAliasContent {
    src_name: String,
}

#[derive(Debug, Clone)]
struct RustStruct {
    allow_unknown_fields: bool,
    serde_as_obj: bool,
    extra_ref_type: bool,
    fields: Vec<RustField>,
    derives: Vec<String>,
}

#[derive(Debug, Clone)]
struct RustEnum {
    is_error: bool,
    variants: Vec<RustVariant>,
    derives: Vec<String>,
}

#[derive(Debug, Clone)]
struct RustWrapper {
    type_name: String,
}

#[derive(Debug, Clone)]
struct RustUnit {
    serde_as_obj: bool,
}

#[derive(Debug, Clone)]
struct RustField {
    description: Option<String>,
    name: String,
    optional: bool,
    fixed: Option<FixedField>,
    arc_wrap: bool,
    type_name: String,
    serde_rename: Option<String>,
    serde_flatten: bool,
    serializer: Option<SerializerOverride>,
}

#[derive(Debug, Clone)]
struct RustVariant {
    description: Option<String>,
    name: String,
    serde_name: Option<String>,
    error_text: Option<String>,
    error_code: Option<u32>,
    wraps: Option<RustFieldType>,
}

#[derive(Debug, Clone)]
struct RustFieldType {
    type_name: String,
    serializer: Option<SerializerOverride>,
}

#[derive(Debug, Clone)]
enum SerializerOverride {
    Serde(String),
    SerdeAs(String),
}

impl Generate {
    pub(crate) fn run(self, profiles: &[GenerationProfile]) -> Result<()> {
        let profile = profiles
            .iter()
            .find(|profile| profile.version == self.spec)
            .expect("Unable to find profile");

        let specs = profile
            .raw_specs
            .parse_full()
            .expect("Failed to parse specification");

        println!("// AUTO-GENERATED CODE. DO NOT EDIT");
        println!("// To change the code generated, modify the codegen tool instead:");
        println!("//     https://github.com/xJonathanLEI/starknet-jsonrpc-codegen");
        println!();
        println!("// Code generated with version:");
        match built_info::GIT_COMMIT_HASH {
            Some(commit_hash) => println!(
                "//     https://github.com/xJonathanLEI/starknet-jsonrpc-codegen#{commit_hash}"
            ),
            None => println!("    <Unable to determine Git commit hash>"),
        }
        println!();

        if !profile.options.ignore_types.is_empty() {
            println!("// These types are ignored from code generation. Implement them manually:");
            for ignored_type in profile.options.ignore_types.iter() {
                println!("// - `{ignored_type}`");
            }
            println!();
        }

        let result = resolve_types(
            &specs,
            &profile.options.flatten_options,
            &profile.options.ignore_types,
            &profile.options.allow_unknown_field_types,
            &profile.options.fixed_field_types,
            &profile.options.arc_wrapped_types,
            &profile.options.additional_derives_types,
        )
        .expect("Failed to resolve types");

        if !result.not_implemented.is_empty() {
            println!("// Code generation requested but not implemented for these types:");
            for type_name in result.not_implemented.iter() {
                println!("// - `{type_name}`");
            }
            println!();
        }

        println!("#![allow(missing_docs)]");
        println!("#![allow(clippy::doc_markdown)]");
        println!("#![allow(clippy::missing_const_for_fn)]");
        println!();
        println!("use alloc::{{format, string::*, vec::*}};");
        println!();

        println!("use indexmap::IndexMap;");
        println!("use serde::{{Deserialize, Deserializer, Serialize, Serializer}};");
        println!("use serde_with::serde_as;");

        if profile.version == SpecVersion::V0_1_0 {
            println!("use starknet_core::{{");
            println!("    serde::{{byte_array::base64, unsigned_field_element::UfeHex}},");
            println!("    types::Felt,");
            println!("}};");
        } else {
            println!();
            println!("use crate::serde::byte_array::base64;");
        }

        println!();

        // In later versions this type is still defined by never actually used
        if profile.version == SpecVersion::V0_1_0 {
            println!("pub use starknet_core::types::L1Address as EthAddress;");
            println!();
        }

        println!("use super::{{");
        println!("    serde_impls::{{MerkleNodeMap, NumAsHex, OwnedContractExecutionError}},");
        println!("    *,");
        println!("}};");
        println!();

        println!("#[cfg(target_has_atomic = \"ptr\")]");
        println!("pub type OwnedPtr<T> = alloc::sync::Arc<T>;");
        println!("#[cfg(not(target_has_atomic = \"ptr\"))]");
        println!("pub type OwnedPtr<T> = alloc::boxed::Box<T>;");
        println!();
        println!("#[cfg(feature = \"std\")]");
        println!("type RandomState = std::hash::RandomState;");
        println!("#[cfg(not(feature = \"std\"))]");
        println!("type RandomState = foldhash::fast::RandomState;");
        println!();

        println!("const QUERY_VERSION_OFFSET: Felt = Felt::from_raw([");
        println!("    576460752142434320,");
        println!("    18446744073709551584,");
        println!("    17407,");
        println!("    18446744073700081665,");
        println!("]);");
        println!();

        let mut manual_serde_types = vec![];

        if !result.aliases.is_empty() {
            for alias in &result.aliases {
                println!("pub type {} = {};", alias.name, alias.content.src_name);
            }

            println!();
        }

        for rust_type in result
            .model_types
            .iter()
            .chain(result.request_response_types.iter())
        {
            if rust_type.need_custom_serde() {
                manual_serde_types.push(rust_type);
            }

            rust_type.render_stdout();

            println!();
        }

        for (ind, rust_type) in manual_serde_types.iter().enumerate() {
            rust_type.render_serde_stdout();

            if ind != manual_serde_types.len() - 1 {
                println!();
            }
        }

        Ok(())
    }
}

impl RustType {
    pub fn render_stdout(&self) {
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
    }

    pub fn render_serde_stdout(&self) {
        match &self.content {
            RustTypeKind::Struct(content) => content.render_serde_stdout(&self.name),
            RustTypeKind::Unit(content) => content.render_serde_stdout(&self.name),
            _ => todo!("serde blocks only implemented for structs and unit"),
        }
    }

    pub fn need_custom_serde(&self) -> bool {
        match &self.content {
            RustTypeKind::Struct(content) => content.need_custom_serde(),
            RustTypeKind::Enum(content) => content.need_custom_serde(),
            RustTypeKind::Wrapper(content) => content.need_custom_serde(),
            RustTypeKind::Unit(content) => content.need_custom_serde(),
        }
    }
}

impl RustTypeKind {
    pub fn render_stdout(&self, name: &str) {
        match self {
            Self::Struct(value) => value.render_stdout(name),
            Self::Enum(value) => value.render_stdout(name),
            Self::Wrapper(value) => value.render_stdout(name),
            Self::Unit(value) => value.render_stdout(name),
        }
    }
}

impl RustStruct {
    pub fn render_stdout(&self, name: &str) {
        let mut fields = self.fields.clone();
        if fields.iter().any(|field| {
            field
                .fixed
                .as_ref()
                .is_some_and(|fixed| fixed.is_query_version)
        }) {
            fields.push(RustField {
                description: Some(
                    "If set to `true`, \
                    uses a query-only transaction version that's invalid for execution"
                        .into(),
                ),
                name: "is_query".into(),
                optional: false,
                fixed: None,
                arc_wrap: false,
                type_name: "bool".into(),
                serde_rename: None,
                serde_flatten: false,
                serializer: None,
            });
        }

        let derive_serde = !self.need_custom_serde();

        if derive_serde
            && self
                .fields
                .iter()
                .any(|item| matches!(item.serializer, Some(SerializerOverride::SerdeAs(_))))
        {
            println!("#[serde_as]");
        }
        if derive_serde {
            print_rust_derives(&self.with_serde_derives());

            if !self.allow_unknown_fields {
                println!(
                    "#[cfg_attr(feature = \"no_unknown_fields\", serde(deny_unknown_fields))]"
                );
            }
        } else {
            print_rust_derives(&self.with_default_derives());
        }
        println!("pub struct {name} {{");

        for field in fields.iter().filter(|field| field.fixed.is_none()) {
            if let Some(doc) = &field.description {
                print_doc(doc, 4);
            }

            for line in field.def_lines(4, derive_serde, false, false, false) {
                println!("{line}")
            }
        }

        println!("}}");

        if self.extra_ref_type {
            println!();

            print_doc(&format!("Reference version of [{name}]."), 0);
            println!("#[derive(Debug, Clone, PartialEq, Eq)]");
            println!("pub struct {name}Ref<'a> {{");

            for field in fields.iter().filter(|field| field.fixed.is_none()) {
                for line in field.def_lines(4, false, true, false, false) {
                    println!("{line}")
                }
            }

            println!("}}");
        }
    }

    pub fn render_serde_stdout(&self, name: &str) {
        self.render_impl_serialize_stdout(name);
        println!();
        self.render_impl_deserialize_stdout(name);
    }

    pub fn need_custom_serde(&self) -> bool {
        self.serde_as_obj || self.fields.iter().any(|field| field.fixed.is_some())
    }

    fn render_impl_serialize_stdout(&self, name: &str) {
        if self.serde_as_obj {
            self.render_impl_obj_serialize_stdout(name);
        } else {
            self.render_impl_tagged_serialize_stdout(name);
        }
    }

    fn render_impl_deserialize_stdout(&self, name: &str) {
        if self.serde_as_obj {
            self.render_impl_both_deserialize_stdout(name);
        } else {
            self.render_impl_tagged_deserialize_stdout(name);
        }
    }

    fn render_impl_obj_serialize_stdout(&self, name: &str) {
        self.render_impl_array_serialize_stdout_inner(name, false);

        if self.extra_ref_type {
            println!();
            self.render_impl_array_serialize_stdout_inner(name, true);
        }
    }

    fn render_impl_array_serialize_stdout_inner(&self, name: &str, is_ref_type: bool) {
        println!(
            "impl Serialize for {}{} {{",
            name,
            if is_ref_type { "Ref<'_>" } else { "" },
        );
        println!(
            "    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {{"
        );

        println!("        #[derive(Serialize)]");
        println!("        struct AsObject<'a> {{");

        for (ind_field, field) in self.fields.iter().enumerate() {
            if field.optional {
                println!("            #[serde(skip_serializing_if = \"Option::is_none\")]");
                println!(
                    "            {}: Option<Field{}<'a>>,",
                    field.name, ind_field
                );
            } else {
                println!("            {}: Field{}<'a>,", field.name, ind_field);
            }
        }

        println!("        }}");
        println!();

        for (ind_field, field) in self.fields.iter().enumerate() {
            if field.serializer.is_some() {
                println!("        #[serde_as]");
            }

            println!("        #[derive(Serialize)]");
            println!("        #[serde(transparent)]");
            println!("        struct Field{ind_field}<'a> {{");
            for line in field.def_lines(12, true, true, false, true).iter() {
                println!("{line}");
            }
            println!("        }}");
            println!();
        }

        println!("        AsObject::serialize(");
        println!("            &AsObject {{");

        for (ind_field, field) in self.fields.iter().enumerate() {
            if field.optional {
                if field.name.len() > 15 {
                    println!("                {}: self", field.name,);
                    println!("                    .{}", field.name);
                    println!("                    .as_ref()");
                    println!("                    .map(|f| Field{ind_field} {{ value: f }}),");
                } else {
                    println!(
                        "                {}: self.{}.as_ref().map(|f| Field{} {{ value: f }}),",
                        field.name, field.name, ind_field,
                    );
                }
            } else if field.name.len() + if is_ref_type { 0 } else { 1 } > 6 {
                println!("                {}: Field{} {{", field.name, ind_field);
                println!(
                    "                    value: {}self.{},",
                    if is_ref_type { "" } else { "&" },
                    field.name
                );
                println!("                }},");
            } else {
                println!(
                    "                {}: Field{} {{ value: {}self.{} }},",
                    field.name,
                    ind_field,
                    if is_ref_type { "" } else { "&" },
                    field.name
                );
            }
        }

        println!("            }},");
        println!("            serializer,");
        println!("        )");

        println!("    }}");
        println!("}}");
    }

    fn render_impl_tagged_serialize_stdout(&self, name: &str) {
        println!("impl Serialize for {name} {{");
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
            for line in field.def_lines(12, true, true, false, false).iter() {
                println!("{line}");
            }
        }

        println!("        }}");
        println!();

        for field in self.fields.iter().filter_map(|field| field.fixed.as_ref()) {
            if field.is_query_version {
                println!(
                    "        let {} = &(if self.is_query {{",
                    escape_name(&field.name)
                );
                println!(
                    "            {} + QUERY_VERSION_OFFSET",
                    field.value.trim_start_matches('&')
                );
                println!("        }} else {{");
                println!("            {}", field.value.trim_start_matches('&'));
                println!("        }});");
            } else {
                println!(
                    "        let {} = {};",
                    escape_name(&field.name),
                    field.value
                );
            }

            println!();
        }

        println!("        let tagged = Tagged {{");

        for field in self.fields.iter() {
            match &field.fixed {
                Some(_) => {
                    println!("            {},", escape_name(&field.name))
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

    fn render_impl_both_deserialize_stdout(&self, name: &str) {
        println!("impl<'de> Deserialize<'de> for {name} {{");
        println!("    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {{");

        println!("        #[derive(Deserialize)]");
        println!("        struct AsObject {{");

        for (ind_field, field) in self.fields.iter().enumerate() {
            if field.optional {
                println!("            #[serde(skip_serializing_if = \"Option::is_none\")]");
                println!("            {}: Option<Field{}>,", field.name, ind_field);
            } else {
                println!("            {}: Field{},", field.name, ind_field);
            }
        }

        println!("        }}");
        println!();

        for (ind_field, field) in self.fields.iter().enumerate() {
            if field.serializer.is_some() {
                println!("        #[serde_as]");
            }

            println!("        #[derive(Deserialize)]");
            println!("        #[serde(transparent)]");
            println!("        struct Field{ind_field} {{");
            for line in field.def_lines(12, true, false, false, true).iter() {
                println!("{line}");
            }
            println!("        }}");
            println!();
        }

        println!("        let temp = serde_json::Value::deserialize(deserializer)?;");
        println!();
        println!(
            "        if let Ok(mut elements) = Vec::<serde_json::Value>::deserialize(&temp) {{"
        );

        if self.fields.iter().any(|field| field.optional) {
            println!("            let element_count = elements.len();");
            println!();
        }

        for (ind_field, field) in self.fields.iter().enumerate().rev() {
            if field.optional {
                println!("            let field{ind_field} = if element_count > {ind_field} {{");
                println!("                Some(");
                println!("                    serde_json::from_value::<Field{ind_field}>(elements.pop().unwrap()).map_err(|err| {{");
                println!("                        serde::de::Error::custom(format!(\"failed to parse element: {{err}}\"))");
                println!("                    }})?,");
                println!("                )");
                println!("            }} else {{");
                println!("                None");
                println!("            }};");
            } else {
                println!(
                    "            let field{ind_field} = serde_json::from_value::<Field{ind_field}>("
                );
                println!("                elements");
                println!("                    .pop()");
                println!("                    .ok_or_else(|| serde::de::Error::custom(\"invalid sequence length\"))?,");
                println!("            )");
                println!("            .map_err(|err| serde::de::Error::custom(format!(\"failed to parse element: {{err}}\")))?;");
            }
        }

        println!();

        println!("            Ok(Self {{");

        for (ind_field, field) in self.fields.iter().enumerate() {
            if field.optional {
                println!(
                    "                {}: field{}.map(|f| f.value),",
                    field.name, ind_field
                );
            } else {
                println!("                {}: field{}.value,", field.name, ind_field);
            }
        }

        println!("            }})");

        println!("        }} else if let Ok(object) = AsObject::deserialize(&temp) {{");

        println!("            Ok(Self {{");

        for field in self.fields.iter() {
            if field.optional {
                println!(
                    "                {}: object.{}.map(|f| f.value),",
                    field.name, field.name
                );
            } else {
                println!(
                    "                {}: object.{}.value,",
                    field.name, field.name
                );
            }
        }

        println!("            }})");

        println!("        }} else {{");
        println!("            Err(serde::de::Error::custom(\"invalid sequence length\"))");
        println!("        }}");

        println!("    }}");
        println!("}}");
    }

    fn render_impl_tagged_deserialize_stdout(&self, name: &str) {
        println!("impl<'de> Deserialize<'de> for {name} {{");
        println!("    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {{");

        if self
            .fields
            .iter()
            .any(|item| matches!(item.serializer, Some(SerializerOverride::SerdeAs(_))))
        {
            println!("        #[serde_as]");
        }

        println!("        #[derive(Deserialize)]");
        if !self.allow_unknown_fields {
            println!(
                "        #[cfg_attr(feature = \"no_unknown_fields\", serde(deny_unknown_fields))]"
            );
        }

        println!("        struct Tagged {{");

        for field in self.fields.iter() {
            let lines = match &field.fixed {
                Some(fixed) => RustField {
                    description: field.description.clone(),
                    name: field.name.clone(),
                    optional: false,
                    fixed: Some(fixed.to_owned()),
                    arc_wrap: false,
                    type_name: if fixed.must_present_in_deser {
                        field.type_name.to_owned()
                    } else {
                        format!("Option<{}>", field.type_name)
                    },
                    serde_rename: field.serde_rename.clone(),
                    serde_flatten: field.serde_flatten,
                    serializer: field.serializer.as_ref().map(|value| value.to_optional()),
                }
                .def_lines(12, true, false, true, false),
                None => field.def_lines(12, true, false, true, false),
            };

            for line in lines.iter() {
                println!("{line}");
            }
        }

        println!("        }}");
        println!();
        println!("        let tagged = Tagged::deserialize(deserializer)?;");
        println!();

        for fixed_field in self.fields.iter().filter_map(|field| field.fixed.as_ref()) {
            if fixed_field.is_query_version {
                println!(
                    "        let is_query = if tagged.{} == {} {{",
                    fixed_field.name,
                    fixed_field.value.trim_start_matches('&')
                );
                println!("            false");
                println!(
                    "        }} else if tagged.{} == {} + QUERY_VERSION_OFFSET {{",
                    fixed_field.name,
                    fixed_field.value.trim_start_matches('&')
                );
                println!("            true");
                println!("        }} else {{");
                println!(
                    "            return Err(serde::de::Error::custom(\"invalid `{}` value\"));",
                    fixed_field.name
                );
                println!("        }};");
                println!();
            } else if fixed_field.must_present_in_deser {
                let value_is_ref = fixed_field.value.starts_with('&');

                println!(
                    "        if {}tagged.{} != {} {{",
                    if value_is_ref { "" } else { "&" },
                    escape_name(&fixed_field.name),
                    if value_is_ref {
                        &fixed_field.value[1..]
                    } else {
                        &fixed_field.value
                    }
                );
                println!(
                    "            return Err(serde::de::Error::custom(\"invalid `{}` value\"));",
                    fixed_field.name
                );
                println!("        }}");
                println!();
            } else {
                println!(
                    "        if let Some(tag_field) = &tagged.{} {{",
                    escape_name(&fixed_field.name)
                );
                println!("            if tag_field != {} {{", fixed_field.value);
                println!(
                    "                return Err(serde::de::Error::custom(\"invalid `{}` value\"));",
                    fixed_field.name
                );
                println!("            }}");
                println!("        }}");
                println!();
            }
        }

        println!("        Ok(Self {{");

        for field in self.fields.iter().filter(|field| field.fixed.is_none()) {
            println!(
                "            {}: {},",
                escape_name(&field.name),
                if field.arc_wrap {
                    format!("OwnedPtr::new(tagged.{})", escape_name(&field.name))
                } else {
                    format!("tagged.{}", escape_name(&field.name))
                }
            );
        }

        if self.fields.iter().any(|field| {
            field
                .fixed
                .as_ref()
                .is_some_and(|fixed| fixed.is_query_version)
        }) {
            println!("            is_query,",);
        }

        println!("        }})");

        println!("    }}");
        println!("}}");
    }

    fn with_default_derives(&self) -> IndexSet<String> {
        let mut derives: IndexSet<_> = self.derives.iter().cloned().collect();
        derives.insert("Debug".into());
        derives.insert("Clone".into());
        derives.insert("PartialEq".into());
        derives.insert("Eq".into());
        derives
    }

    fn with_serde_derives(&self) -> IndexSet<String> {
        let mut derives = self.with_default_derives();
        derives.insert("Serialize".into());
        derives.insert("Deserialize".into());
        derives
    }
}

impl RustEnum {
    pub fn render_stdout(&self, name: &str) {
        print_rust_derives(&self.with_default_derives());
        println!("pub enum {name} {{");

        for variant in self.variants.iter() {
            if let Some(doc) = &variant.description {
                print_doc(doc, 4);
            }

            if let Some(rename) = &variant.serde_name {
                println!("    #[serde(rename = \"{rename}\")]");
            }
            match &variant.wraps {
                Some(inner) => {
                    println!("    {}({}),", variant.name, inner.type_name);
                }
                None => {
                    println!("    {},", variant.name);
                }
            }
        }

        println!("}}");

        if self.is_error {
            println!();
            println!("#[cfg(feature = \"std\")]");
            println!("impl std::error::Error for {name} {{}}");

            println!();
            println!("impl core::fmt::Display for {name} {{");
            println!("    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {{");
            println!("        match self {{");

            for variant in self.variants.iter() {
                println!(
                    "            Self::{}{} => write!(f, \"{}{}\"),",
                    variant.name,
                    if variant.wraps.is_some() { "(e)" } else { "" },
                    variant.name,
                    if variant.wraps.is_some() {
                        ": {e:?}"
                    } else {
                        ""
                    },
                );
            }

            println!("        }}");
            println!("    }}");
            println!("}}");

            println!();
            println!("impl {name} {{");
            println!("    pub const fn code(&self) -> u32 {{");
            println!("        match self {{");

            for variant in self.variants.iter() {
                let error_code = variant
                    .error_code
                    .as_ref()
                    .expect("error code to be present for errors");

                let variant_handler = format!(
                    "            Self::{}{} => {},",
                    variant.name,
                    if variant.wraps.is_some() { "(_)" } else { "" },
                    error_code
                );

                if variant_handler.len() <= MAX_LINE_LENGTH {
                    println!("{variant_handler}");
                } else {
                    println!(
                        "            Self::{}{} => {{",
                        variant.name,
                        if variant.wraps.is_some() { "(_)" } else { "" }
                    );
                    println!("                {error_code}");
                    println!("            }}");
                }
            }

            println!("        }}");
            println!("    }}");
            println!();

            println!("    pub fn message(&self) -> &'static str {{");
            println!("        match self {{");

            for variant in self.variants.iter() {
                let error_text = variant
                    .error_text
                    .as_ref()
                    .expect("error message to be present for errors");

                let variant_handler = format!(
                    "            Self::{}{} => \"{}\",",
                    variant.name,
                    if variant.wraps.is_some() { "(_)" } else { "" },
                    error_text
                );

                if variant_handler.len() <= MAX_LINE_LENGTH {
                    println!("{variant_handler}");
                } else {
                    println!(
                        "            Self::{}{} => {{",
                        variant.name,
                        if variant.wraps.is_some() { "(_)" } else { "" }
                    );
                    println!("                \"{error_text}\"");
                    println!("            }}");
                }
            }

            println!("        }}");
            println!("    }}");
            println!("}}");
        }
    }

    pub fn need_custom_serde(&self) -> bool {
        false
    }

    fn with_default_derives(&self) -> IndexSet<String> {
        let mut derives: IndexSet<_> = self.derives.iter().cloned().collect();
        derives.insert("Debug".into());
        derives.insert("Clone".into());

        // Implement `Copy` when no variant wraps other types
        if !self.variants.iter().any(|variant| variant.wraps.is_some()) {
            derives.insert("Copy".into());
        }

        derives.insert("PartialEq".into());
        derives.insert("Eq".into());
        derives.insert("Serialize".into());
        derives.insert("Deserialize".into());
        derives
    }
}

impl RustWrapper {
    pub fn render_stdout(&self, name: &str) {
        println!("#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]");
        println!("pub struct {}(pub {});", name, self.type_name);
    }

    pub fn need_custom_serde(&self) -> bool {
        false
    }
}

impl RustUnit {
    pub fn render_stdout(&self, name: &str) {
        if self.need_custom_serde() {
            println!("#[derive(Debug, Clone, PartialEq, Eq)]");
        } else {
            println!("#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]");
        }
        println!("pub struct {name};");
    }

    pub fn render_serde_stdout(&self, name: &str) {
        self.render_impl_serialize_stdout(name);
        println!();
        self.render_impl_deserialize_stdout(name);
    }

    pub fn need_custom_serde(&self) -> bool {
        self.serde_as_obj
    }

    fn render_impl_serialize_stdout(&self, name: &str) {
        println!("impl Serialize for {name} {{");
        println!(
            "    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {{"
        );

        println!("        use serde::ser::SerializeSeq;");
        println!();
        println!("        let seq = serializer.serialize_seq(Some(0))?;");
        println!("        seq.end()");

        println!("    }}");
        println!("}}");
    }

    fn render_impl_deserialize_stdout(&self, name: &str) {
        println!("impl<'de> Deserialize<'de> for {name} {{");
        println!("    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {{");

        println!("        let elements = Vec::<()>::deserialize(deserializer)?;");
        println!("        if !elements.is_empty() {{");
        println!("            return Err(serde::de::Error::custom(\"invalid sequence length\"));");
        println!("        }}");
        println!("        Ok(Self)");

        println!("    }}");
        println!("}}");
    }
}

impl RustField {
    pub fn def_lines(
        &self,
        leading_spaces: usize,
        serde_attrs: bool,
        is_ref: bool,
        no_arc_wrapping: bool,
        is_wrapped_field: bool,
    ) -> Vec<String> {
        let mut lines = vec![];

        let leading_spaces = " ".repeat(leading_spaces);

        if serde_attrs {
            if self.optional && !is_wrapped_field {
                lines.push(format!(
                    "{leading_spaces}#[serde(skip_serializing_if = \"Option::is_none\")]"
                ));
            }
            if let Some(serde_rename) = &self.serde_rename {
                lines.push(format!(
                    "{leading_spaces}#[serde(rename = \"{serde_rename}\")]"
                ));
            }
            if self.serde_flatten {
                lines.push(format!("{leading_spaces}#[serde(flatten)]"));
            }
            if let Some(serde_as) = &self.serializer {
                lines.push(match serde_as {
                    SerializerOverride::Serde(serializer) => {
                        format!("{leading_spaces}#[serde(with = \"{serializer}\")]")
                    }
                    SerializerOverride::SerdeAs(serializer) => {
                        let serializer = if let Some(FixedField {
                            is_query_version: true,
                            ..
                        }) = &self.fixed
                        {
                            if self.optional {
                                "Option<UfeHex>".to_owned()
                            } else {
                                "UfeHex".to_owned()
                            }
                        } else if is_ref && serializer.starts_with("Vec<") {
                            format!("[{}]", &serializer[4..(serializer.len() - 1)])
                        } else {
                            serializer.to_owned()
                        };
                        format!("{leading_spaces}#[serde_as(as = \"{serializer}\")]")
                    }
                });
            } else if self.arc_wrap && !no_arc_wrapping && !is_ref {
                lines.push(format!(
                    "{leading_spaces}#[serde_as(as = \"Owned{}\")]",
                    self.type_name
                ));
            }
        }

        let type_name = if let Some(FixedField {
            is_query_version: true,
            ..
        }) = &self.fixed
        {
            if self.optional {
                "Option<Felt>"
            } else {
                "Felt"
            }
        } else {
            &self.type_name
        };

        lines.push(format!(
            "{}pub {}: {},",
            leading_spaces,
            if is_wrapped_field {
                "value"
            } else {
                escape_name(&self.name)
            },
            if is_ref {
                if type_name == "String" {
                    String::from("&'a str")
                } else if type_name.starts_with("Vec<") {
                    if self.optional && !is_wrapped_field {
                        format!("Option<&'a [{}]>", &type_name[4..(type_name.len() - 1)])
                    } else {
                        format!("&'a [{}]", &type_name[4..(type_name.len() - 1)])
                    }
                } else if self.optional && !is_wrapped_field {
                    format!("&'a Option<{type_name}>")
                } else {
                    format!("&'a {type_name}")
                }
            } else if self.arc_wrap && !no_arc_wrapping {
                format!("OwnedPtr<{type_name}>")
            } else if self.optional && !is_wrapped_field {
                format!("Option<{type_name}>")
            } else {
                type_name.to_owned()
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
            SerializerOverride::SerdeAs(serde_as) => Self::SerdeAs(format!("Option<{serde_as}>")),
        }
    }
}

fn resolve_types(
    specs: &Specification,
    flatten_option: &FlattenOption,
    ignore_types: &[String],
    allow_unknown_field_types: &[String],
    fixed_fields: &FixedFieldsOptions,
    arc_wrapping: &ArcWrappingOptions,
    additional_derives_types: &AdditionalDerivesOptions,
) -> Result<TypeResolutionResult> {
    let mut types = vec![];
    let mut aliases = vec![];
    let mut req_types: Vec<RustType> = vec![];
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

        let derives = additional_derives_types
            .find_additional_derives(&rusty_name)
            .unwrap_or_default();

        let content = match schema_to_rust_type_kind(
            specs,
            entity,
            allow_unknown_field_types.contains(name),
            flatten_option,
            derives,
        )? {
            Some(content) => content,
            None => {
                not_implemented_types.push(name.to_owned());

                eprintln!("OneOf enum generation not implemented. Enum not generated for {name}");
                continue;
            }
        };

        match content {
            SchemaToRustTypeResult::Type(mut content) => {
                if let RustTypeKind::Struct(inner) = &mut content {
                    for field in inner.fields.iter_mut() {
                        field.fixed = fixed_fields.find_fixed_field(&rusty_name, &field.name);
                        field.arc_wrap = arc_wrapping.in_field_wrapped(&rusty_name, &field.name);
                    }
                }

                types.push(RustType {
                    title: title.map(|value| to_starknet_rs_doc(value, true)),
                    description: description.map(|value| to_starknet_rs_doc(value, true)),
                    name: rusty_name,
                    content,
                });
            }
            SchemaToRustTypeResult::Alias(content) => aliases.push(RustAlias {
                name: rusty_name,
                content,
            }),
        }
    }

    types.push(RustType {
        title: Some(String::from("JSON-RPC error codes")),
        description: None,
        name: String::from("StarknetError"),
        content: RustTypeKind::Enum(RustEnum {
            is_error: true,
            variants: specs
                .components
                .errors
                .iter()
                .map(|(name, err)| match err {
                    ErrorType::Error(err) => Ok(RustVariant {
                        description: Some(err.message.clone()),
                        name: to_starknet_rs_name(name),
                        serde_name: None,
                        error_text: Some(err.message.clone()),
                        error_code: Some(err.code),
                        wraps: match &err.data {
                            Some(err_data) => match err_data {
                                Schema::Ref(value) => Some(RustFieldType {
                                    type_name: to_starknet_rs_name(value.name()),
                                    serializer: None,
                                }),
                                Schema::Primitive(_) => Some(get_rust_type_for_field(err_data)?),
                                Schema::OneOf(_) => anyhow::bail!(
                                    "Anonymous oneOf types should not be used for error data"
                                ),
                                Schema::AllOf(_) => anyhow::bail!(
                                    "Anonymous allOf types should not be used for error data"
                                ),
                            },
                            None => None,
                        },
                    }),
                    ErrorType::Reference(_) => todo!("Error redirection not implemented"),
                })
                .collect::<Result<_>>()?,
            derives: additional_derives_types
                .find_additional_derives("StarknetError")
                .unwrap_or_default(),
        }),
    });

    // Request/response types
    for method in specs.methods.iter() {
        let mut request_fields = vec![];

        for param in method.params.iter() {
            let field_type = get_rust_type_for_field(&param.schema)?;

            request_fields.push(RustField {
                description: param.description.clone(),
                name: param.name.clone(),
                optional: !param.required,
                fixed: None,
                arc_wrap: false,
                type_name: field_type.type_name,
                serde_rename: None,
                serde_flatten: false,
                serializer: field_type.serializer,
            });
        }

        let rusty_name = format!(
            "{}Request",
            to_starknet_rs_name(&camel_to_snake_case(
                method.name.trim_start_matches("starknet_")
            ))
        );

        let request_type = RustType {
            title: Some(format!("Request for method {}", method.name)),
            description: None,
            name: rusty_name.clone(),
            content: if request_fields.is_empty() {
                RustTypeKind::Unit(RustUnit { serde_as_obj: true })
            } else {
                RustTypeKind::Struct(RustStruct {
                    allow_unknown_fields: false,
                    serde_as_obj: true,
                    extra_ref_type: true,
                    fields: request_fields,
                    derives: additional_derives_types
                        .find_additional_derives(&rusty_name)
                        .unwrap_or_default(),
                })
            },
        };

        req_types.push(request_type);
    }

    // Sorting the types makes it easier to check diffs in generated code.
    types.sort_by_key(|item| item.name.to_owned());
    req_types.sort_by_key(|item| item.name.to_owned());
    not_implemented_types.sort();

    Ok(TypeResolutionResult {
        model_types: types,
        aliases,
        request_response_types: req_types,
        not_implemented: not_implemented_types,
    })
}

fn schema_to_rust_type_kind(
    specs: &Specification,
    entity: &Schema,
    allow_unknown_fields: bool,
    flatten_option: &FlattenOption,
    derives: Vec<String>,
) -> Result<Option<SchemaToRustTypeResult>> {
    Ok(match entity {
        Schema::Ref(reference) => {
            let ref_type_name = reference.name();

            let should_flatten = match flatten_option {
                FlattenOption::All => true,
                FlattenOption::Selected(items) => items.contains(&ref_type_name.to_owned()),
            };

            if should_flatten {
                let ref_type = specs.components.schemas.get(ref_type_name).ok_or_else(|| {
                    anyhow::anyhow!("Ref target type not found: {}", ref_type_name)
                })?;

                schema_to_rust_type_kind(
                    specs,
                    ref_type,
                    allow_unknown_fields,
                    flatten_option,
                    derives,
                )?
            } else {
                Some(SchemaToRustTypeResult::Alias(RustAliasContent {
                    src_name: to_starknet_rs_name(ref_type_name),
                }))
            }
        }
        Schema::OneOf(one_of) => {
            // Special case: treat as plain enum if all its `oneOf` options are just `string` with
            // enum variants.
            //
            // This exception was made to account for `PRICE_UNIT` changes from v0.8.1 to v0.9.0.
            let string_variants = one_of
                .one_of
                .iter()
                .map(|option| {
                    let Schema::Ref(option_ref) = option else {
                        anyhow::bail!("option not reference");
                    };

                    let ref_type =
                        specs
                            .components
                            .schemas
                            .get(option_ref.name())
                            .ok_or_else(|| {
                                anyhow::anyhow!("Ref target type not found: {}", option_ref.name())
                            })?;

                    let Schema::Primitive(Primitive::String(option_str)) = ref_type else {
                        anyhow::bail!("option not pointing to string");
                    };

                    option_str
                        .r#enum
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("option not pointing to enum"))
                })
                .collect::<Result<Vec<&Vec<String>>>>();

            match string_variants {
                Ok(string_variants) if !string_variants.is_empty() => {
                    Some(SchemaToRustTypeResult::Type(RustTypeKind::Enum(RustEnum {
                        is_error: false,
                        variants: string_variants
                            .into_iter()
                            .flatten()
                            .map(|item| RustVariant {
                                description: None,
                                name: to_starknet_rs_name(item),
                                serde_name: Some(item.to_owned()),
                                error_text: None,
                                error_code: None,
                                wraps: None,
                            })
                            .collect(),
                        derives,
                    })))
                }
                _ => None,
            }
        }
        Schema::AllOf(_) | Schema::Primitive(Primitive::Object(_)) => {
            let mut fields = vec![];
            get_schema_fields(entity, specs, &mut fields, flatten_option)?;
            Some(SchemaToRustTypeResult::Type(RustTypeKind::Struct(
                RustStruct {
                    allow_unknown_fields,
                    serde_as_obj: false,
                    extra_ref_type: false,
                    fields,
                    derives,
                },
            )))
        }
        Schema::Primitive(Primitive::String(value)) => match &value.r#enum {
            Some(variants) => Some(SchemaToRustTypeResult::Type(RustTypeKind::Enum(RustEnum {
                is_error: false,
                variants: variants
                    .iter()
                    .map(|item| RustVariant {
                        description: None,
                        name: to_starknet_rs_name(item),
                        serde_name: Some(item.to_owned()),
                        error_text: None,
                        error_code: None,
                        wraps: None,
                    })
                    .collect(),
                derives,
            }))),
            None => Some(SchemaToRustTypeResult::Type(RustTypeKind::Wrapper(
                RustWrapper {
                    type_name: "String".into(),
                },
            ))),
        },
        _ => {
            anyhow::bail!("Unexpected schema type when generating struct/enum");
        }
    })
}

/// Finds the list of schemas that are used and only used for flattening inside objects
fn get_flatten_only_schemas(specs: &Specification, flatten_option: &FlattenOption) -> Vec<String> {
    // We need this for now since we don't search method calls, so we could get false positives
    const HARD_CODED_NON_FLATTEN_SCHEMAS: [&str; 3] =
        ["FUNCTION_CALL", "PENDING_STATE_UPDATE", "BLOCK_HEADER"];

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
                            let field_name = get_all_of_ref_name_override(reference.name())
                                .unwrap_or_else(|| reference.name().to_lowercase());

                            fields.push(RustField {
                                description: reference.description.to_owned(),
                                name: field_name,
                                optional: false,
                                fixed: None,
                                arc_wrap: false,
                                type_name: to_starknet_rs_name(reference.name()),
                                serde_rename: None,
                                serde_flatten: true,
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
                let doc_string = match prop_value.description() {
                    Some(text) => Some(text),
                    None => match prop_value.title() {
                        Some(text) => Some(text),
                        None => prop_value.summary(),
                    },
                };

                let field_type = get_rust_type_for_field(prop_value)?;

                let field_name = to_rust_field_name(name);
                let rename = if name == &field_name {
                    None
                } else {
                    Some(name.to_owned())
                };

                // Optional field transformation
                let field_optional = !value.required.contains(name);
                let serializer = if field_optional {
                    field_type.serializer.map(|value| value.to_optional())
                } else {
                    field_type.serializer
                };

                fields.push(RustField {
                    description: doc_string.map(|value| to_starknet_rs_doc(value, false)),
                    name: field_name,
                    optional: field_optional,
                    fixed: None,
                    arc_wrap: false,
                    type_name: field_type.type_name,
                    serde_rename: rename,
                    serde_flatten: false,
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

fn get_rust_type_for_field(schema: &Schema) -> Result<RustFieldType> {
    match schema {
        Schema::Ref(value) => {
            let ref_type_name = value.name();

            if let Some(type_override) = get_field_type_override(ref_type_name) {
                // Hard-coded special rules
                Ok(type_override)
            } else {
                // TODO: take non-alias refs into account
                Ok(RustFieldType {
                    type_name: to_starknet_rs_name(ref_type_name),
                    serializer: None,
                })
            }
        }
        Schema::OneOf(_) => {
            anyhow::bail!("Anonymous oneOf types should not be used for properties");
        }
        Schema::AllOf(_) => {
            anyhow::bail!("Anonymous allOf types should not be used for properties");
        }
        Schema::Primitive(value) => match value {
            Primitive::Array(value) => {
                let item_type = get_rust_type_for_field(&value.items)?;
                let serializer = match item_type.serializer {
                    Some(SerializerOverride::Serde(_)) => {
                        todo!("Array wrapper for #[serde(with)] not implemented")
                    }
                    Some(SerializerOverride::SerdeAs(serializer)) => {
                        Some(SerializerOverride::SerdeAs(format!("Vec<{serializer}>")))
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
            type_name: String::from("Felt"),
            serializer: Some(SerializerOverride::SerdeAs(String::from("UfeHex"))),
        },
        "ETH_ADDRESS" => RustFieldType {
            type_name: String::from("EthAddress"),
            serializer: None,
        },
        "EXECUTION_RESULT" => RustFieldType {
            type_name: String::from("ExecutionResult"),
            serializer: None,
        },
        "BLOCK_NUMBER" => RustFieldType {
            type_name: String::from("u64"),
            serializer: None,
        },
        "NUM_AS_HEX" => RustFieldType {
            type_name: String::from("u64"),
            serializer: Some(SerializerOverride::SerdeAs(String::from("NumAsHex"))),
        },
        "SIGNATURE" => RustFieldType {
            type_name: String::from("Vec<Felt>"),
            serializer: Some(SerializerOverride::SerdeAs(String::from("Vec<UfeHex>"))),
        },
        "EVENT_KEYS" => RustFieldType {
            type_name: String::from("Vec<Vec<Felt>>"),
            serializer: Some(SerializerOverride::SerdeAs(String::from(
                "Vec<Vec<UfeHex>>",
            ))),
        },
        "NODE_HASH_TO_NODE_MAPPING" => RustFieldType {
            type_name: String::from("IndexMap<Felt, MerkleNode, RandomState>"),
            serializer: Some(SerializerOverride::SerdeAs(String::from("MerkleNodeMap"))),
        },
        "CONTRACT_ABI" => RustFieldType {
            type_name: String::from("Vec<LegacyContractAbiEntry>"),
            serializer: None,
        },
        "CONTRACT_ENTRY_POINT_LIST" => RustFieldType {
            type_name: String::from("Vec<ContractEntryPoint>"),
            serializer: None,
        },
        "LEGACY_CONTRACT_ENTRY_POINT_LIST" => RustFieldType {
            type_name: String::from("Vec<LegacyContractEntryPoint>"),
            serializer: None,
        },
        "TXN_TYPE" => RustFieldType {
            type_name: String::from("String"),
            serializer: None,
        },
        "NESTED_CALL" => RustFieldType {
            type_name: String::from("FunctionInvocation"),
            serializer: None,
        },
        "HASH_256" | "L1_TXN_HASH" => RustFieldType {
            type_name: String::from("Hash256"),
            serializer: None,
        },
        "u64" => RustFieldType {
            type_name: String::from("u64"),
            serializer: Some(SerializerOverride::SerdeAs(String::from("NumAsHex"))),
        },
        "u128" => RustFieldType {
            type_name: String::from("u128"),
            serializer: Some(SerializerOverride::SerdeAs(String::from("NumAsHex"))),
        },
        "SUBSCRIPTION_BLOCK_ID" => RustFieldType {
            type_name: String::from("ConfirmedBlockId"),
            serializer: None,
        },
        "TXN_STATUS_RESULT" => RustFieldType {
            type_name: String::from("TransactionStatus"),
            serializer: None,
        },
        _ => return None,
    })
}

fn get_all_of_ref_name_override(type_name: &str) -> Option<String> {
    match type_name {
        "TXN_RECEIPT" => Some("receipt".into()),
        "RECEIPT_BLOCK" => Some("block".into()),
        _ => None,
    }
}

fn print_doc(doc: &str, indent_spaces: usize) {
    let prefix = format!("{}/// ", " ".repeat(indent_spaces));
    for line in wrap_lines(doc, prefix.len()) {
        println!("{prefix}{line}");
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
        "SierraContractClass" => String::from("FlattenedSierraClass"),
        "LegacyContractClass" => String::from("CompressedLegacyContractClass"),
        "DeprecatedContractClass" => String::from("CompressedLegacyContractClass"),
        "ContractAbiEntry" => String::from("LegacyContractAbiEntry"),
        "FunctionAbiEntry" => String::from("LegacyFunctionAbiEntry"),
        "EventAbiEntry" => String::from("LegacyEventAbiEntry"),
        "StructAbiEntry" => String::from("LegacyStructAbiEntry"),
        "FunctionAbiType" => String::from("LegacyFunctionAbiType"),
        "EventAbiType" => String::from("LegacyEventAbiType"),
        "StructAbiType" => String::from("LegacyStructAbiType"),
        "StructMember" => String::from("LegacyStructMember"),
        "TypedParameter" => String::from("LegacyTypedParameter"),
        "DeprecatedEntryPointsByType" => String::from("LegacyEntryPointsByType"),
        "DeprecatedCairoEntryPoint" => String::from("LegacyContractEntryPoint"),
        "DaMode" => String::from("DataAvailabilityMode"),
        "L1DaMode" => String::from("L1DataAvailabilityMode"),
        "TransactionStatus" => String::from("SequencerTransactionStatus"),
        _ => name,
    }
}

fn to_rust_field_name(name: &str) -> String {
    let all_upper_letters_regex = Regex::new("^[A-Z]+$").unwrap();

    if all_upper_letters_regex.is_match(name) || name.contains('_') {
        // Already snake case
        name.to_ascii_lowercase()
    } else {
        camel_to_snake_case(name)
    }
}

fn to_starknet_rs_doc(doc: &str, force_period: bool) -> String {
    let mut doc = to_sentence_case(doc);

    for (pattern, target) in [
        (Regex::new(r"(?i)\bethereum\b").unwrap(), "Ethereum"),
        (Regex::new(r"(?i)\bstarknet\b").unwrap(), "Starknet"),
        (Regex::new(r"(?i)\bstarknet\.io\b").unwrap(), "starknet.io"),
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

fn camel_to_snake_case(name: &str) -> String {
    let mut result = String::new();

    for character in name.chars() {
        let is_upper = character.to_ascii_uppercase() == character;
        if is_upper {
            result.push('_');
            result.push(character.to_ascii_lowercase());
        } else {
            result.push(character);
        }
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

fn print_rust_derives(derives: &IndexSet<String>) {
    if !derives.is_empty() {
        println!("#[derive({})]", itertools::join(derives, ", "))
    }
}
