use crate::spec::*;

mod spec;

const STARKNET_API_OPENRPC: &str = include_str!("./specs/starknet_api_openrpc.json");

fn main() {
    let specs: Specification =
        serde_json::from_str(STARKNET_API_OPENRPC).expect("Failed to parse specification");

    println!("use serde::{{Deserialize, Serialize}};");
    println!();

    for (ind, (name, entity)) in specs.components.schemas.iter().enumerate() {
        let name = to_starknet_rs_name(name);

        let title = entity.title();
        let description = match entity.description() {
            Some(description) => Some(description),
            None => entity.summary(),
        };

        match (title, description) {
            (Some(title), Some(description)) => {
                println!("/// {}", title);
                println!("///");
                println!("/// {}", description);
            }
            (Some(title), None) => {
                println!("/// {}", title);
            }
            (None, Some(description)) => {
                println!("/// {}", description);
            }
            (None, None) => {}
        }

        println!("#[derive(Debug, Clone, Serialize, Deserialize)]");
        println!("pub struct {} {{", name);
        println!("}}");

        if ind != specs.components.schemas.len() - 1 {
            println!();
        }
    }
}

fn to_starknet_rs_name(name: &str) -> String {
    to_pascal_case(name).replace("Txn", "Transaction")
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
