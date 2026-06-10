pub mod storage;

use storage::{
    active_vault_path, create_document_in_vault, delete_document_in_vault,
    get_storage_status_in_vault, initialize_vault_at, list_documents_in_vault,
    read_document_in_vault, read_global_settings, rebuild_index_at, save_document_in_vault,
    save_global_settings, search_documents_in_vault, AppState, CreateDocumentInput, DeleteResult,
    DocumentFilter, DocumentListItem, DocumentPayload, IndexStatus, SaveDocumentInput, SaveResult,
    SearchInput, SearchResult, StorageStatus, VaultSummary,
};
use tauri::{AppHandle, Manager, State};

#[tauri::command]
fn initialize_vault(
    app: AppHandle,
    state: State<'_, AppState>,
    path: Option<String>,
) -> Result<VaultSummary, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|err| format!("failed to resolve app config directory: {err}"))?;
    let vault_path = match path {
        Some(path) => std::path::PathBuf::from(path),
        None => read_global_settings(&config_dir)?
            .map(|settings| std::path::PathBuf::from(settings.active_vault_path))
            .unwrap_or(
                app.path()
                    .document_dir()
                    .map_err(|err| format!("failed to resolve documents directory: {err}"))?
                    .join("Moa Vault"),
            ),
    };

    let summary = initialize_vault_at(vault_path.clone())?;
    save_global_settings(&config_dir, &vault_path)?;
    state.set_active_vault(vault_path)?;
    Ok(summary)
}

#[tauri::command]
fn list_documents(
    state: State<'_, AppState>,
    filter: Option<DocumentFilter>,
) -> Result<Vec<DocumentListItem>, String> {
    let vault_path = active_vault_path(&state)?;
    list_documents_in_vault(&vault_path, filter)
}

#[tauri::command]
fn read_document(state: State<'_, AppState>, id: String) -> Result<DocumentPayload, String> {
    let vault_path = active_vault_path(&state)?;
    read_document_in_vault(&vault_path, &id)
}

#[tauri::command]
fn create_document(
    state: State<'_, AppState>,
    input: CreateDocumentInput,
) -> Result<DocumentPayload, String> {
    let vault_path = active_vault_path(&state)?;
    create_document_in_vault(&vault_path, input)
}

#[tauri::command]
fn save_document(
    state: State<'_, AppState>,
    input: SaveDocumentInput,
) -> Result<SaveResult, String> {
    let vault_path = active_vault_path(&state)?;
    save_document_in_vault(&vault_path, input)
}

#[tauri::command]
fn delete_document(state: State<'_, AppState>, id: String) -> Result<DeleteResult, String> {
    let vault_path = active_vault_path(&state)?;
    delete_document_in_vault(&vault_path, &id)
}

#[tauri::command]
fn search_documents(
    state: State<'_, AppState>,
    input: SearchInput,
) -> Result<Vec<SearchResult>, String> {
    let vault_path = active_vault_path(&state)?;
    search_documents_in_vault(&vault_path, input)
}

#[tauri::command]
fn rebuild_index(state: State<'_, AppState>) -> Result<IndexStatus, String> {
    let vault_path = active_vault_path(&state)?;
    rebuild_index_at(&vault_path)
}

#[tauri::command]
fn get_storage_status(state: State<'_, AppState>) -> Result<StorageStatus, String> {
    let vault_path = active_vault_path(&state)?;
    get_storage_status_in_vault(&vault_path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            initialize_vault,
            list_documents,
            read_document,
            create_document,
            save_document,
            delete_document,
            search_documents,
            rebuild_index,
            get_storage_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
