use crate::spec::*;

mod spec;

const MAX_LINE_LENGTH: usize = 100;

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

        println!("#[derive(Debug, Clone, Serialize, Deserialize)]");
        println!("pub struct {} {{", name);
        println!("}}");

        if ind != specs.components.schemas.len() - 1 {
            println!();
        }
    }
}

fn print_doc(doc: &str, indent_spaces: usize) {
    let prefix = format!("{}/// ", " ".repeat(indent_spaces));
    for line in to_doc_lines(doc, prefix.len()) {
        println!("{}{}", prefix, line);
    }
}

fn to_doc_lines(doc: &str, prefix_length: usize) -> Vec<String> {
    let mut doc = to_starknet_rs_doc(doc.trim());
    if !doc.ends_with('.') {
        doc.push('.');
    }

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

fn to_starknet_rs_doc(doc: &str) -> String {
    to_sentence_case(doc)
        .replace("starknet", "StarkNet")
        .replace("Starknet", "StarkNet")
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
    for (ind, character) in name.chars().enumerate() {
        if character == '.' {
            last_period = Some(ind);
        }

        let uppercase = match last_period {
            Some(last_period) => ind == last_period + 2,
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
