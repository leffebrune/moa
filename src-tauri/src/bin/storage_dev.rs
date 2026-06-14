use moa_lib::storage::{
    create_document_in_vault, delete_document_in_vault, get_storage_status_in_vault,
    initialize_vault_at, list_documents_in_vault, read_document_in_vault, rebuild_index_at,
    save_document_in_vault, search_documents_in_vault, CreateDocumentInput, DocumentFilter,
    SaveDocumentInput, SearchInput,
};
use serde::Serialize;
use std::{collections::HashMap, env, path::PathBuf, process};

const DEFAULT_VAULT_NAME: &str = "moa-manual-vault";
const VAULT_ENV: &str = "MOA_STORAGE_DEV_VAULT";

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        print_usage();
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or_else(|| "missing command".to_string())?;
    let flags = parse_flags(args.collect())?;

    match command.as_str() {
        "init" => {
            let vault = vault_path(&flags)?;
            print_json(&initialize_vault_at(vault)?)?;
        }
        "create" => {
            let vault = vault_path(&flags)?;
            let input = CreateDocumentInput {
                title: optional_flag(&flags, "title"),
                tags: optional_csv_flag(&flags, "tags"),
                body: optional_flag(&flags, "body"),
            };
            print_json(&create_document_in_vault(&vault, input)?)?;
        }
        "list" => {
            let vault = vault_path(&flags)?;
            let filter = DocumentFilter {
                tag: optional_flag(&flags, "tag"),
                query: optional_flag(&flags, "query"),
            };
            print_json(&list_documents_in_vault(&vault, Some(filter))?)?;
        }
        "read" => {
            let vault = vault_path(&flags)?;
            let id = required_flag(&flags, "id")?;
            print_json(&read_document_in_vault(&vault, &id)?)?;
        }
        "save" => {
            let vault = vault_path(&flags)?;
            let input = SaveDocumentInput {
                id: required_flag(&flags, "id")?,
                title: required_flag(&flags, "title")?,
                tags: optional_csv_flag(&flags, "tags").unwrap_or_default(),
                body: required_flag(&flags, "body")?,
            };
            print_json(&save_document_in_vault(&vault, input)?)?;
        }
        "search" => {
            let vault = vault_path(&flags)?;
            let input = SearchInput {
                query: required_flag(&flags, "query")?,
                tags: optional_csv_flag(&flags, "tags"),
                sort: optional_flag(&flags, "sort"),
            };
            print_json(&search_documents_in_vault(&vault, input)?)?;
        }
        "delete" => {
            let vault = vault_path(&flags)?;
            let id = required_flag(&flags, "id")?;
            print_json(&delete_document_in_vault(&vault, &id)?)?;
        }
        "rebuild" => {
            let vault = vault_path(&flags)?;
            print_json(&rebuild_index_at(&vault)?)?;
        }
        "status" => {
            let vault = vault_path(&flags)?;
            print_json(&get_storage_status_in_vault(&vault)?)?;
        }
        "help" | "--help" | "-h" => print_usage(),
        _ => return Err(format!("unknown command: {command}")),
    }

    Ok(())
}

fn parse_flags(args: Vec<String>) -> Result<HashMap<String, String>, String> {
    let mut flags = HashMap::new();
    let mut index = 0;

    while index < args.len() {
        let key = &args[index];
        if !key.starts_with("--") {
            return Err(format!("unexpected argument: {key}"));
        }

        let name = key.trim_start_matches("--").to_string();
        if name.is_empty() {
            return Err("empty flag name".to_string());
        }

        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("missing value for --{name}"))?;
        if value.starts_with("--") {
            return Err(format!("missing value for --{name}"));
        }

        flags.insert(name, value.clone());
        index += 2;
    }

    Ok(flags)
}

fn vault_path(flags: &HashMap<String, String>) -> Result<PathBuf, String> {
    if let Some(path) = optional_flag(flags, "vault") {
        return Ok(PathBuf::from(path));
    }

    if let Ok(path) = env::var(VAULT_ENV) {
        let path = path.trim();
        if !path.is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    default_vault_path()
}

fn default_vault_path() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .ok_or_else(|| "failed to resolve repository root".to_string())?;
    let repo_parent = repo_root.parent().unwrap_or(repo_root);
    Ok(repo_parent.join(DEFAULT_VAULT_NAME))
}

fn required_flag(flags: &HashMap<String, String>, name: &str) -> Result<String, String> {
    optional_flag(flags, name).ok_or_else(|| format!("missing --{name}"))
}

fn optional_flag(flags: &HashMap<String, String>, name: &str) -> Option<String> {
    flags
        .get(name)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn optional_csv_flag(flags: &HashMap<String, String>, name: &str) -> Option<Vec<String>> {
    optional_flag(flags, name).map(|value| {
        value
            .split(',')
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect()
    })
}

fn print_json<T: Serialize>(value: &T) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|err| format!("failed to serialize output: {err}"))?;
    println!("{json}");
    Ok(())
}

fn print_usage() {
    eprintln!(
        r#"Usage:
  cargo run --bin storage_dev -- init
  cargo run --bin storage_dev -- create --title <title> [--tags <a,b>] [--body <body>]
  cargo run --bin storage_dev -- list [--tag <tag>] [--query <query>]
  cargo run --bin storage_dev -- read --id <document-id>
  cargo run --bin storage_dev -- save --id <document-id> --title <title> [--tags <a,b>] --body <body>
  cargo run --bin storage_dev -- search --query <query> [--tags <a,b>] [--sort updatedAt]
  cargo run --bin storage_dev -- delete --id <document-id>
  cargo run --bin storage_dev -- rebuild
  cargo run --bin storage_dev -- status

Vault path:
  default: repository sibling directory named moa-manual-vault
  override: --vault <path> or MOA_STORAGE_DEV_VAULT"#
    );
}
