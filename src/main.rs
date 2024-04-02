use std::str::FromStr;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

use crate::{
    spec::Specification,
    subcommands::{Generate, Print},
};

mod spec;
mod subcommands;

mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Cli {
    #[clap(subcommand)]
    command: Subcommands,
}

#[derive(Debug, Subcommand)]
enum Subcommands {
    #[clap(about = "Generate Rust code")]
    Generate(Generate),
    #[clap(about = "Print the spec to standard output")]
    Print(Print),
}

#[derive(Debug, Clone)]
struct GenerationProfile {
    version: SpecVersion,
    raw_specs: RawSpecs,
    options: ProfileOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpecVersion {
    V0_1_0,
    V0_2_1,
    V0_3_0,
    V0_4_0,
    V0_5_1,
    V0_6_0,
    V0_7_1,
}

#[derive(Debug, Clone)]
struct RawSpecs {
    main: &'static str,
    write: &'static str,
    trace: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileOptions {
    flatten_options: FlattenOption,
    ignore_types: Vec<String>,
    allow_unknown_field_types: Vec<String>,
    fixed_field_types: FixedFieldsOptions,
    arc_wrapped_types: ArcWrappingOptions,
    additional_derives_types: AdditionalDerivesOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FixedFieldsOptions {
    fixed_field_types: Vec<RustTypeWithFixedFields>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ArcWrappingOptions {
    arc_wrapped_types: Vec<RustTypeWithArcWrappedFields>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AdditionalDerivesOptions {
    additional_derives_types: Vec<RustTypesWithAdditionalDerives>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RustTypeWithFixedFields {
    name: String,
    fields: Vec<FixedField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RustTypeWithArcWrappedFields {
    name: String,
    fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RustTypesWithAdditionalDerives {
    name: String,
    derives: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FixedField {
    name: String,
    value: String,
    is_query_version: bool,
    #[serde(default)]
    must_present_in_deser: bool,
}

#[allow(unused)]
#[derive(Debug, Clone, Serialize, Deserialize)]
enum FlattenOption {
    All,
    Selected(Vec<String>),
}

impl FromStr for SpecVersion {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        Ok(match s {
            "0.1.0" | "v0.1.0" => Self::V0_1_0,
            "0.2.1" | "v0.2.1" => Self::V0_2_1,
            "0.3.0" | "v0.3.0" => Self::V0_3_0,
            "0.4.0" | "v0.4.0" => Self::V0_4_0,
            "0.5.1" | "v0.5.1" => Self::V0_5_1,
            "0.6.0" | "v0.6.0" => Self::V0_6_0,
            "0.7.1" | "v0.7.1" => Self::V0_7_1,
            _ => anyhow::bail!("unknown spec version: {}", s),
        })
    }
}

impl ValueEnum for SpecVersion {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self::V0_1_0,
            Self::V0_2_1,
            Self::V0_3_0,
            Self::V0_4_0,
            Self::V0_5_1,
            Self::V0_6_0,
            Self::V0_7_1,
        ]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        use clap::builder::PossibleValue;

        match self {
            Self::V0_1_0 => Some(PossibleValue::new("0.1.0").alias("v0.1.0")),
            Self::V0_2_1 => Some(PossibleValue::new("0.2.1").alias("v0.2.1")),
            Self::V0_3_0 => Some(PossibleValue::new("0.3.0").alias("v0.3.0")),
            Self::V0_4_0 => Some(PossibleValue::new("0.4.0").alias("v0.4.0")),
            Self::V0_5_1 => Some(PossibleValue::new("0.5.1").alias("v0.5.1")),
            Self::V0_6_0 => Some(PossibleValue::new("0.6.0").alias("v0.6.0")),
            Self::V0_7_1 => Some(PossibleValue::new("0.7.1").alias("v0.7.1")),
        }
    }
}

impl RawSpecs {
    pub fn parse_full(&self) -> Result<Specification> {
        let mut specs: Specification = serde_json::from_str(self.main)?;
        let mut write_specs: Specification = serde_json::from_str(self.write)?;
        let mut trace_specs: Specification = serde_json::from_str(self.trace)?;

        for additional_specs in [&mut write_specs, &mut trace_specs].into_iter() {
            specs.methods.append(&mut additional_specs.methods);

            for (key, value) in additional_specs.components.schemas.iter() {
                match specs.components.schemas.entry(key.to_owned()) {
                    indexmap::map::Entry::Occupied(entry) => match &value {
                        spec::Schema::Ref(_) => {}
                        _ => {
                            if value != entry.get() {
                                anyhow::bail!(
                                    "duplicate entries must be ref or identical: {}",
                                    key
                                );
                            }
                        }
                    },
                    indexmap::map::Entry::Vacant(entry) => {
                        entry.insert(value.to_owned());
                    }
                }
            }

            for (key, value) in additional_specs.components.errors.iter() {
                if let indexmap::map::Entry::Vacant(entry) =
                    specs.components.errors.entry(key.to_owned())
                {
                    entry.insert(value.to_owned());
                }
            }
        }

        Ok(specs)
    }
}

impl FixedFieldsOptions {
    fn find_fixed_field(&self, type_name: &str, field_name: &str) -> Option<FixedField> {
        self.fixed_field_types.iter().find_map(|item| {
            if item.name == type_name {
                item.fields
                    .iter()
                    .find(|field| field.name == field_name)
                    .cloned()
            } else {
                None
            }
        })
    }
}

impl ArcWrappingOptions {
    fn in_field_wrapped(&self, type_name: &str, field_name: &str) -> bool {
        self.arc_wrapped_types.iter().any(|item| {
            if item.name == type_name {
                item.fields.iter().any(|field| field == field_name)
            } else {
                false
            }
        })
    }
}

impl AdditionalDerivesOptions {
    fn find_additional_derives(&self, type_name: &str) -> Option<Vec<String>> {
        self.additional_derives_types.iter().find_map(|item| {
            if item.name == type_name {
                Some(item.derives.clone())
            } else {
                None
            }
        })
    }
}

fn main() {
    let cli = Cli::parse();

    let profiles: [GenerationProfile; 7] = [
        GenerationProfile {
            version: SpecVersion::V0_1_0,
            raw_specs: RawSpecs {
                main: include_str!("./specs/0.1.0/starknet_api_openrpc.json"),
                write: include_str!("./specs/0.1.0/starknet_write_api.json"),
                trace: include_str!("./specs/0.1.0/starknet_trace_api_openrpc.json"),
            },
            options: serde_json::from_str(include_str!("./profiles/0.1.0.json"))
                .expect("Unable to parse profile options"),
        },
        GenerationProfile {
            version: SpecVersion::V0_2_1,
            raw_specs: RawSpecs {
                main: include_str!("./specs/0.2.1/starknet_api_openrpc.json"),
                write: include_str!("./specs/0.2.1/starknet_write_api.json"),
                trace: include_str!("./specs/0.2.1/starknet_trace_api_openrpc.json"),
            },
            options: serde_json::from_str(include_str!("./profiles/0.2.1.json"))
                .expect("Unable to parse profile options"),
        },
        GenerationProfile {
            version: SpecVersion::V0_3_0,
            raw_specs: RawSpecs {
                main: include_str!("./specs/0.3.0/starknet_api_openrpc.json"),
                write: include_str!("./specs/0.3.0/starknet_write_api.json"),
                trace: include_str!("./specs/0.3.0/starknet_trace_api_openrpc.json"),
            },
            options: serde_json::from_str(include_str!("./profiles/0.3.0.json"))
                .expect("Unable to parse profile options"),
        },
        GenerationProfile {
            version: SpecVersion::V0_4_0,
            raw_specs: RawSpecs {
                main: include_str!("./specs/0.4.0/starknet_api_openrpc.json"),
                write: include_str!("./specs/0.4.0/starknet_write_api.json"),
                trace: include_str!("./specs/0.4.0/starknet_trace_api_openrpc.json"),
            },
            options: serde_json::from_str(include_str!("./profiles/0.4.0.json"))
                .expect("Unable to parse profile options"),
        },
        GenerationProfile {
            version: SpecVersion::V0_5_1,
            raw_specs: RawSpecs {
                main: include_str!("./specs/0.5.1/starknet_api_openrpc.json"),
                write: include_str!("./specs/0.5.1/starknet_write_api.json"),
                trace: include_str!("./specs/0.5.1/starknet_trace_api_openrpc.json"),
            },
            options: serde_json::from_str(include_str!("./profiles/0.5.1.json"))
                .expect("Unable to parse profile options"),
        },
        GenerationProfile {
            version: SpecVersion::V0_6_0,
            raw_specs: RawSpecs {
                main: include_str!("./specs/0.6.0/starknet_api_openrpc.json"),
                write: include_str!("./specs/0.6.0/starknet_write_api.json"),
                trace: include_str!("./specs/0.6.0/starknet_trace_api_openrpc.json"),
            },
            options: serde_json::from_str(include_str!("./profiles/0.6.0.json"))
                .expect("Unable to parse profile options"),
        },
        GenerationProfile {
            version: SpecVersion::V0_7_1,
            raw_specs: RawSpecs {
                main: include_str!("./specs/0.7.1/starknet_api_openrpc.json"),
                write: include_str!("./specs/0.7.1/starknet_write_api.json"),
                trace: include_str!("./specs/0.7.1/starknet_trace_api_openrpc.json"),
            },
            options: serde_json::from_str(include_str!("./profiles/0.7.1.json"))
                .expect("Unable to parse profile options"),
        },
    ];

    let result = match cli.command {
        Subcommands::Generate(cmd) => cmd.run(&profiles),
        Subcommands::Print(cmd) => cmd.run(&profiles),
    };

    result.expect("Error running commmand");
}
