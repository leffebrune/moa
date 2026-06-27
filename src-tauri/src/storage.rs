use chrono::{DateTime, Local, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
use uuid::Uuid;

const NOTES_DIR: &str = "notes";
const MOA_DIR: &str = ".moa";
const TRASH_DIR: &str = "trash";
const DB_FILE: &str = "moa.sqlite";
const VAULT_SETTINGS_FILE: &str = "settings.json";

#[derive(Default)]
pub struct AppState {
    active_vault: Mutex<Option<PathBuf>>,
}

impl AppState {
    pub fn set_active_vault(&self, path: PathBuf) -> Result<(), String> {
        let mut active = self
            .active_vault
            .lock()
            .map_err(|_| "저장소 상태 잠금이 손상되었습니다".to_string())?;
        *active = Some(path);
        Ok(())
    }

    fn active_vault(&self) -> Result<PathBuf, String> {
        self.active_vault
            .lock()
            .map_err(|_| "저장소 상태 잠금이 손상되었습니다".to_string())?
            .clone()
            .ok_or_else(|| "보관함이 초기화되지 않았습니다".to_string())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalSettings {
    pub active_vault_path: String,
    pub recent_vault_paths: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VaultSettings {
    schema_version: u32,
    created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StorageIssue {
    pub kind: String,
    pub message: String,
    pub relative_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultSummary {
    pub vault_path: String,
    pub notes_path: String,
    pub database_path: String,
    pub document_count: usize,
    pub issue_count: usize,
    pub issues: Vec<StorageIssue>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentFilter {
    pub tag: Option<String>,
    pub query: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DocumentListItem {
    pub id: String,
    pub title: String,
    pub tags: Vec<String>,
    pub relative_path: String,
    pub created_at: String,
    pub updated_at: String,
    pub content_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentPayload {
    pub id: String,
    pub title: String,
    pub tags: Vec<String>,
    pub body: String,
    pub relative_path: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDocumentInput {
    pub title: Option<String>,
    pub tags: Option<Vec<String>>,
    pub body: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDocumentInput {
    pub id: String,
    pub title: String,
    pub tags: Vec<String>,
    pub body: String,
    pub sync_file_name: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub id: String,
    pub relative_path: String,
    pub updated_at: String,
    pub file_name_sync_error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResult {
    pub id: String,
    pub trashed_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchInput {
    pub query: String,
    pub tags: Option<Vec<String>>,
    pub sort: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub tags: Vec<String>,
    pub relative_path: String,
    pub updated_at: String,
    pub snippet: String,
    pub score: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatus {
    pub document_count: usize,
    pub issue_count: usize,
    pub issues: Vec<StorageIssue>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStatus {
    pub vault_path: String,
    pub notes_path: String,
    pub database_path: String,
    pub document_count: usize,
    pub tag_count: usize,
    pub issue_count: usize,
    pub issues: Vec<StorageIssue>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Frontmatter {
    id: String,
    title: String,
    #[serde(default)]
    tags: Vec<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug)]
struct ParsedDocument {
    meta: Frontmatter,
    body: String,
}

struct IndexedDocument {
    item: DocumentListItem,
    body: String,
    file_mtime: i64,
}

pub fn read_global_settings(config_dir: &Path) -> Result<Option<GlobalSettings>, String> {
    let path = config_dir.join(VAULT_SETTINGS_FILE);
    if !path.exists() {
        return Ok(None);
    }

    let content =
        fs::read_to_string(&path).map_err(|err| format!("전역 설정을 읽지 못했습니다: {err}"))?;
    serde_json::from_str(&content)
        .map(Some)
        .map_err(|err| format!("전역 설정을 해석하지 못했습니다: {err}"))
}

pub fn save_global_settings(config_dir: &Path, vault_path: &Path) -> Result<(), String> {
    fs::create_dir_all(config_dir)
        .map_err(|err| format!("앱 설정 디렉터리를 만들지 못했습니다: {err}"))?;

    let vault_path_string = path_to_string(vault_path);
    let mut recent = read_global_settings(config_dir)?
        .map(|settings| settings.recent_vault_paths)
        .unwrap_or_default();
    recent.retain(|path| path != &vault_path_string);
    recent.insert(0, vault_path_string.clone());
    recent.truncate(10);

    let settings = GlobalSettings {
        active_vault_path: vault_path_string,
        recent_vault_paths: recent,
    };
    let content = serde_json::to_string_pretty(&settings)
        .map_err(|err| format!("전역 설정을 직렬화하지 못했습니다: {err}"))?;
    fs::write(config_dir.join(VAULT_SETTINGS_FILE), content)
        .map_err(|err| format!("전역 설정을 쓰지 못했습니다: {err}"))
}

pub fn initialize_vault_at(vault_path: PathBuf) -> Result<VaultSummary, String> {
    ensure_vault_dirs(&vault_path)?;
    ensure_vault_settings(&vault_path)?;
    let conn = open_database(&vault_path)?;
    initialize_schema(&conn)?;
    let status = rebuild_index_at(&vault_path)?;

    Ok(VaultSummary {
        vault_path: path_to_string(&vault_path),
        notes_path: path_to_string(&notes_path(&vault_path)),
        database_path: path_to_string(&database_path(&vault_path)),
        document_count: status.document_count,
        issue_count: status.issue_count,
        issues: status.issues,
    })
}

pub fn list_documents_in_vault(
    vault_path: &Path,
    filter: Option<DocumentFilter>,
) -> Result<Vec<DocumentListItem>, String> {
    let conn = open_ready_database(vault_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, title, relative_path, created_at, updated_at, content_hash
             FROM documents
             ORDER BY updated_at DESC, title ASC",
        )
        .map_err(|err| format!("문서 목록을 준비하지 못했습니다: {err}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(DocumentListItem {
                id: row.get(0)?,
                title: row.get(1)?,
                relative_path: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                content_hash: row.get(5)?,
                tags: Vec::new(),
            })
        })
        .map_err(|err| format!("문서를 조회하지 못했습니다: {err}"))?;

    let filter = filter.unwrap_or(DocumentFilter {
        tag: None,
        query: None,
    });
    let tag_filter = normalize_optional_string(filter.tag).map(normalize_tag);
    let query_filter = normalize_optional_string(filter.query).map(|query| query.to_lowercase());

    let mut documents = Vec::new();
    for row in rows {
        let mut item = row.map_err(|err| format!("문서 행을 읽지 못했습니다: {err}"))?;
        item.tags = load_tags(&conn, &item.id)?;

        if let Some(tag) = &tag_filter {
            if !item.tags.iter().any(|candidate| candidate == tag) {
                continue;
            }
        }
        if let Some(query) = &query_filter {
            let haystack = format!("{} {}", item.title, item.tags.join(" ")).to_lowercase();
            if !haystack.contains(query) {
                continue;
            }
        }

        documents.push(item);
    }

    Ok(documents)
}

pub fn read_document_in_vault(vault_path: &Path, id: &str) -> Result<DocumentPayload, String> {
    let conn = open_ready_database(vault_path)?;
    let relative_path: String = conn
        .query_row(
            "SELECT relative_path FROM documents WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|err| format!("문서 경로를 조회하지 못했습니다: {err}"))?
        .ok_or_else(|| format!("문서를 찾을 수 없습니다: {id}"))?;

    let path = resolve_vault_relative_path(vault_path, &relative_path)?;
    let content =
        fs::read_to_string(&path).map_err(|err| format!("문서를 읽지 못했습니다: {err}"))?;
    let parsed = parse_markdown_document(&content)
        .map_err(|err| format!("{relative_path} 파일을 해석하지 못했습니다: {err}"))?;

    Ok(DocumentPayload {
        id: parsed.meta.id,
        title: parsed.meta.title,
        tags: parsed.meta.tags,
        body: parsed.body,
        relative_path,
        created_at: parsed.meta.created_at,
        updated_at: parsed.meta.updated_at,
    })
}

pub fn create_document_in_vault(
    vault_path: &Path,
    input: CreateDocumentInput,
) -> Result<DocumentPayload, String> {
    ensure_vault_dirs(vault_path)?;
    let now = Local::now().to_rfc3339();
    let title = normalize_title(input.title);
    let tags = normalize_tags(input.tags.unwrap_or_default());
    let body = input.body.unwrap_or_default();
    let id = Uuid::new_v4().to_string();
    let file_name = unique_note_file_name(vault_path, &title)?;
    let relative_path = format!("{NOTES_DIR}/{file_name}");
    let full_path = notes_path(vault_path).join(&file_name);
    let parsed = ParsedDocument {
        meta: Frontmatter {
            id: id.clone(),
            title,
            tags,
            created_at: now.clone(),
            updated_at: now,
        },
        body,
    };

    let content = compose_markdown_document(&parsed)?;
    write_file_replace(&full_path, content.as_bytes())?;
    index_single_document(vault_path, &relative_path)?;

    read_document_in_vault(vault_path, &id)
}

pub fn save_document_in_vault(
    vault_path: &Path,
    input: SaveDocumentInput,
) -> Result<SaveResult, String> {
    let existing = read_document_in_vault(vault_path, &input.id)?;
    let title = normalize_title(Some(input.title));
    let tags = normalize_tags(input.tags);
    let body = input.body;
    let content_changed = title != existing.title || tags != existing.tags || body != existing.body;
    let updated_at = if content_changed {
        Local::now().to_rfc3339()
    } else {
        existing.updated_at.clone()
    };
    let parsed = ParsedDocument {
        meta: Frontmatter {
            id: existing.id.clone(),
            title,
            tags,
            created_at: existing.created_at.clone(),
            updated_at: updated_at.clone(),
        },
        body,
    };
    let full_path = resolve_vault_relative_path(vault_path, &existing.relative_path)?;

    if content_changed {
        let content = compose_markdown_document(&parsed)?;
        write_file_replace(&full_path, content.as_bytes())?;
    }

    let mut relative_path = existing.relative_path.clone();
    let mut file_name_sync_error = None;

    if input.sync_file_name {
        match synced_note_relative_path(
            vault_path,
            &parsed.meta.created_at,
            &parsed.meta.title,
            &relative_path,
        ) {
            Ok(next_relative_path) if next_relative_path != relative_path => {
                let next_full_path = resolve_vault_relative_path(vault_path, &next_relative_path)?;
                match fs::rename(&full_path, &next_full_path) {
                    Ok(()) => {
                        relative_path = next_relative_path;
                    }
                    Err(err) => {
                        file_name_sync_error =
                            Some(format!("Failed to sync file name with title: {err}"));
                    }
                }
            }
            Ok(_) => {}
            Err(err) => {
                file_name_sync_error = Some(err);
            }
        }
    }

    if content_changed || relative_path != existing.relative_path {
        index_single_document(vault_path, &relative_path)?;
    }

    Ok(SaveResult {
        id: existing.id,
        relative_path,
        updated_at,
        file_name_sync_error,
    })
}

pub fn delete_document_in_vault(vault_path: &Path, id: &str) -> Result<DeleteResult, String> {
    let existing = read_document_in_vault(vault_path, id)?;
    let source_path = resolve_vault_relative_path(vault_path, &existing.relative_path)?;
    fs::create_dir_all(trash_path(vault_path))
        .map_err(|err| format!("휴지통 디렉터리를 만들지 못했습니다: {err}"))?;

    let file_name = source_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "문서 경로에 올바른 파일명이 없습니다".to_string())?;
    let stamp = Utc::now().format("%Y-%m-%dT%H%M%SZ");
    let trash_file_name = format!("{stamp}_{id}_{file_name}");
    let trash_path = unique_path(trash_path(vault_path).join(trash_file_name));
    fs::rename(&source_path, &trash_path)
        .map_err(|err| format!("문서를 휴지통으로 이동하지 못했습니다: {err}"))?;

    let conn = open_ready_database(vault_path)?;
    conn.execute(
        "DELETE FROM documents_fts WHERE document_id = ?1",
        params![id],
    )
    .map_err(|err| format!("FTS 인덱스를 제거하지 못했습니다: {err}"))?;
    conn.execute("DELETE FROM documents WHERE id = ?1", params![id])
        .map_err(|err| format!("문서 메타데이터를 제거하지 못했습니다: {err}"))?;

    Ok(DeleteResult {
        id: id.to_string(),
        trashed_path: path_to_string(&trash_path),
    })
}

pub fn search_documents_in_vault(
    vault_path: &Path,
    input: SearchInput,
) -> Result<Vec<SearchResult>, String> {
    let query = sanitize_fts_query(&input.query);
    if query.is_empty() {
        return Ok(list_documents_in_vault(
            vault_path,
            Some(DocumentFilter {
                tag: input.tags.and_then(|tags| tags.into_iter().next()),
                query: None,
            }),
        )?
        .into_iter()
        .map(|item| SearchResult {
            id: item.id,
            title: item.title,
            tags: item.tags,
            relative_path: item.relative_path,
            updated_at: item.updated_at,
            snippet: String::new(),
            score: 0.0,
        })
        .collect());
    }

    let conn = open_ready_database(vault_path)?;
    let mut stmt = conn
        .prepare(
            "SELECT d.id,
                    d.title,
                    d.relative_path,
                    d.updated_at,
                    snippet(documents_fts, 2, '', '', '...', 16) AS snippet,
                    bm25(documents_fts) AS score
             FROM documents_fts
             JOIN documents d ON d.id = documents_fts.document_id
             WHERE documents_fts MATCH ?1
             ORDER BY score ASC
             LIMIT 100",
        )
        .map_err(|err| format!("검색을 준비하지 못했습니다: {err}"))?;

    let rows = stmt
        .query_map(params![query], |row| {
            Ok(SearchResult {
                id: row.get(0)?,
                title: row.get(1)?,
                relative_path: row.get(2)?,
                updated_at: row.get(3)?,
                snippet: row.get(4)?,
                score: row.get(5)?,
                tags: Vec::new(),
            })
        })
        .map_err(|err| format!("문서를 검색하지 못했습니다: {err}"))?;

    let tag_filters = normalize_tags(input.tags.unwrap_or_default());
    let mut results = Vec::new();

    for row in rows {
        let mut result = row.map_err(|err| format!("검색 결과 행을 읽지 못했습니다: {err}"))?;
        result.tags = load_tags(&conn, &result.id)?;
        if !tag_filters.is_empty()
            && !tag_filters
                .iter()
                .all(|tag| result.tags.iter().any(|candidate| candidate == tag))
        {
            continue;
        }
        results.push(result);
    }

    if input.sort.as_deref() == Some("updatedAt") {
        results.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    }

    Ok(results)
}

pub fn rebuild_index_at(vault_path: &Path) -> Result<IndexStatus, String> {
    ensure_vault_dirs(vault_path)?;
    let conn = open_ready_database(vault_path)?;
    let mut seen_ids = HashSet::new();
    let mut indexed_ids = HashSet::new();
    let mut issues = Vec::new();

    let entries = fs::read_dir(notes_path(vault_path))
        .map_err(|err| format!("노트 디렉터리를 읽지 못했습니다: {err}"))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("노트 항목을 읽지 못했습니다: {err}"))?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }

        let relative_path = format!(
            "{NOTES_DIR}/{}",
            path.file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| "노트 파일명이 올바른 UTF-8이 아닙니다".to_string())?
        );

        match read_indexed_document(vault_path, &relative_path) {
            Ok(document) => {
                if !seen_ids.insert(document.item.id.clone()) {
                    issues.push(StorageIssue {
                        kind: "duplicateDocumentId".to_string(),
                        message: format!("중복된 문서 ID: {}", document.item.id),
                        relative_path: Some(relative_path),
                    });
                    continue;
                }

                upsert_document(&conn, &document)?;
                indexed_ids.insert(document.item.id);
            }
            Err(err) => issues.push(StorageIssue {
                kind: "invalidDocument".to_string(),
                message: err,
                relative_path: Some(relative_path),
            }),
        }
    }

    remove_missing_documents(&conn, &indexed_ids)?;

    Ok(IndexStatus {
        document_count: indexed_ids.len(),
        issue_count: issues.len(),
        issues,
    })
}

pub fn get_storage_status_in_vault(vault_path: &Path) -> Result<StorageStatus, String> {
    let conn = open_ready_database(vault_path)?;
    let document_count = count_rows(&conn, "documents")?;
    let tag_count = count_rows(&conn, "tags")?;

    Ok(StorageStatus {
        vault_path: path_to_string(vault_path),
        notes_path: path_to_string(&notes_path(vault_path)),
        database_path: path_to_string(&database_path(vault_path)),
        document_count,
        tag_count,
        issue_count: 0,
        issues: Vec::new(),
    })
}

pub fn active_vault_path(state: &AppState) -> Result<PathBuf, String> {
    state.active_vault()
}

fn ensure_vault_dirs(vault_path: &Path) -> Result<(), String> {
    fs::create_dir_all(notes_path(vault_path))
        .map_err(|err| format!("노트 디렉터리를 만들지 못했습니다: {err}"))?;
    fs::create_dir_all(moa_path(vault_path))
        .map_err(|err| format!(".moa 디렉터리를 만들지 못했습니다: {err}"))?;
    fs::create_dir_all(trash_path(vault_path))
        .map_err(|err| format!("휴지통 디렉터리를 만들지 못했습니다: {err}"))
}

fn ensure_vault_settings(vault_path: &Path) -> Result<(), String> {
    let path = moa_path(vault_path).join(VAULT_SETTINGS_FILE);
    if path.exists() {
        return Ok(());
    }

    let settings = VaultSettings {
        schema_version: 1,
        created_at: Utc::now().to_rfc3339(),
    };
    let content = serde_json::to_string_pretty(&settings)
        .map_err(|err| format!("보관함 설정을 직렬화하지 못했습니다: {err}"))?;
    fs::write(path, content).map_err(|err| format!("보관함 설정을 쓰지 못했습니다: {err}"))
}

fn open_ready_database(vault_path: &Path) -> Result<Connection, String> {
    let conn = open_database(vault_path)?;
    initialize_schema(&conn)?;
    Ok(conn)
}

fn open_database(vault_path: &Path) -> Result<Connection, String> {
    Connection::open(database_path(vault_path))
        .map_err(|err| format!("데이터베이스를 열지 못했습니다: {err}"))
}

fn initialize_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS schema_migrations (
          version INTEGER PRIMARY KEY,
          applied_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS documents (
          id TEXT PRIMARY KEY,
          relative_path TEXT NOT NULL UNIQUE,
          title TEXT NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          file_mtime INTEGER NOT NULL,
          content_hash TEXT NOT NULL,
          last_indexed_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS tags (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          name TEXT NOT NULL UNIQUE
        );

        CREATE TABLE IF NOT EXISTS document_tags (
          document_id TEXT NOT NULL,
          tag_id INTEGER NOT NULL,
          PRIMARY KEY (document_id, tag_id),
          FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE,
          FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS documents_fts USING fts5(
          document_id UNINDEXED,
          title,
          body,
          tags,
          tokenize = 'unicode61'
        );

        INSERT OR IGNORE INTO schema_migrations (version, applied_at)
        VALUES (1, datetime('now'));
        ",
    )
    .map_err(|err| format!("데이터베이스 스키마를 초기화하지 못했습니다: {err}"))
}

fn read_indexed_document(
    vault_path: &Path,
    relative_path: &str,
) -> Result<IndexedDocument, String> {
    let full_path = resolve_vault_relative_path(vault_path, relative_path)?;
    let content = fs::read_to_string(&full_path)
        .map_err(|err| format!("마크다운 파일을 읽지 못했습니다: {err}"))?;
    let parsed = parse_markdown_document(&content)?;
    let metadata = fs::metadata(&full_path)
        .map_err(|err| format!("마크다운 메타데이터를 읽지 못했습니다: {err}"))?;
    let file_mtime = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    let content_hash = sha256_hex(content.as_bytes());

    Ok(IndexedDocument {
        item: DocumentListItem {
            id: parsed.meta.id,
            title: parsed.meta.title,
            tags: parsed.meta.tags,
            relative_path: relative_path.to_string(),
            created_at: parsed.meta.created_at,
            updated_at: parsed.meta.updated_at,
            content_hash,
        },
        body: parsed.body,
        file_mtime,
    })
}

fn index_single_document(vault_path: &Path, relative_path: &str) -> Result<(), String> {
    let conn = open_ready_database(vault_path)?;
    let document = read_indexed_document(vault_path, relative_path)?;
    upsert_document(&conn, &document)
}

fn upsert_document(conn: &Connection, document: &IndexedDocument) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO documents (
            id, relative_path, title, created_at, updated_at,
            file_mtime, content_hash, last_indexed_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(id) DO UPDATE SET
            relative_path = excluded.relative_path,
            title = excluded.title,
            created_at = excluded.created_at,
            updated_at = excluded.updated_at,
            file_mtime = excluded.file_mtime,
            content_hash = excluded.content_hash,
            last_indexed_at = excluded.last_indexed_at",
        params![
            document.item.id,
            document.item.relative_path,
            document.item.title,
            document.item.created_at,
            document.item.updated_at,
            document.file_mtime,
            document.item.content_hash,
            now
        ],
    )
    .map_err(|err| format!("문서를 갱신하지 못했습니다: {err}"))?;

    conn.execute(
        "DELETE FROM document_tags WHERE document_id = ?1",
        params![document.item.id],
    )
    .map_err(|err| format!("문서 태그를 지우지 못했습니다: {err}"))?;

    for tag in &document.item.tags {
        conn.execute(
            "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
            params![tag],
        )
        .map_err(|err| format!("태그를 갱신하지 못했습니다: {err}"))?;
        let tag_id: i64 = conn
            .query_row("SELECT id FROM tags WHERE name = ?1", params![tag], |row| {
                row.get(0)
            })
            .map_err(|err| format!("태그 ID를 불러오지 못했습니다: {err}"))?;
        conn.execute(
            "INSERT OR IGNORE INTO document_tags (document_id, tag_id) VALUES (?1, ?2)",
            params![document.item.id, tag_id],
        )
        .map_err(|err| format!("문서 태그를 연결하지 못했습니다: {err}"))?;
    }

    conn.execute(
        "DELETE FROM documents_fts WHERE document_id = ?1",
        params![document.item.id],
    )
    .map_err(|err| format!("FTS 행을 지우지 못했습니다: {err}"))?;
    conn.execute(
        "INSERT INTO documents_fts (document_id, title, body, tags)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            document.item.id,
            document.item.title,
            document.body,
            document.item.tags.join(" ")
        ],
    )
    .map_err(|err| format!("FTS 행을 쓰지 못했습니다: {err}"))?;

    Ok(())
}

fn remove_missing_documents(
    conn: &Connection,
    indexed_ids: &HashSet<String>,
) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT id FROM documents")
        .map_err(|err| format!("문서 정리를 준비하지 못했습니다: {err}"))?;
    let ids = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|err| format!("문서 ID를 조회하지 못했습니다: {err}"))?;

    let mut missing = Vec::new();
    for id in ids {
        let id = id.map_err(|err| format!("문서 ID를 읽지 못했습니다: {err}"))?;
        if !indexed_ids.contains(&id) {
            missing.push(id);
        }
    }

    for id in missing {
        conn.execute(
            "DELETE FROM documents_fts WHERE document_id = ?1",
            params![id],
        )
        .map_err(|err| format!("오래된 FTS 행을 제거하지 못했습니다: {err}"))?;
        conn.execute("DELETE FROM documents WHERE id = ?1", params![id])
            .map_err(|err| format!("오래된 문서 행을 제거하지 못했습니다: {err}"))?;
    }

    Ok(())
}

fn parse_markdown_document(content: &str) -> Result<ParsedDocument, String> {
    let normalized = content.replace("\r\n", "\n");
    let rest = normalized
        .strip_prefix("---\n")
        .ok_or_else(|| "YAML frontmatter가 없습니다".to_string())?;
    let closing_index = rest
        .find("\n---\n")
        .ok_or_else(|| "YAML frontmatter가 닫히지 않았습니다".to_string())?;
    let yaml = &rest[..closing_index];
    let body = rest[closing_index + "\n---\n".len()..]
        .strip_prefix('\n')
        .unwrap_or(&rest[closing_index + "\n---\n".len()..])
        .to_string();
    let mut meta: Frontmatter =
        serde_yaml::from_str(yaml).map_err(|err| format!("잘못된 YAML frontmatter: {err}"))?;

    meta.id = normalize_required(meta.id, "id")?;
    meta.title = normalize_title(Some(meta.title));
    meta.tags = normalize_tags(meta.tags);
    validate_rfc3339(&meta.created_at, "created_at")?;
    validate_rfc3339(&meta.updated_at, "updated_at")?;

    Ok(ParsedDocument { meta, body })
}

fn compose_markdown_document(document: &ParsedDocument) -> Result<String, String> {
    let yaml = serde_yaml::to_string(&document.meta)
        .map_err(|err| format!("frontmatter를 직렬화하지 못했습니다: {err}"))?;
    Ok(format!("---\n{}---\n\n{}", yaml, document.body))
}

fn validate_rfc3339(value: &str, field: &str) -> Result<(), String> {
    DateTime::parse_from_rfc3339(value)
        .map(|_| ())
        .map_err(|_| format!("{field} 값은 RFC3339 형식이어야 합니다"))
}

fn write_file_replace(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "대상 경로에 상위 디렉터리가 없습니다".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("상위 디렉터리를 만들지 못했습니다: {err}"))?;

    let temp_path = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("moa"),
        Uuid::new_v4()
    ));
    fs::write(&temp_path, bytes).map_err(|err| format!("임시 파일을 쓰지 못했습니다: {err}"))?;

    if path.exists() {
        #[cfg(windows)]
        {
            fs::remove_file(path)
                .map_err(|err| format!("기존 파일을 교체하지 못했습니다: {err}"))?;
        }
    }

    fs::rename(&temp_path, path).map_err(|err| {
        let _ = fs::remove_file(&temp_path);
        format!("임시 파일을 대상 위치로 이동하지 못했습니다: {err}")
    })
}

fn unique_note_file_name(vault_path: &Path, title: &str) -> Result<String, String> {
    let date = Local::now().format("%Y-%m-%d");
    unique_note_file_name_for_date(vault_path, &date.to_string(), title, None)
}

fn synced_note_relative_path(
    vault_path: &Path,
    created_at: &str,
    title: &str,
    current_relative_path: &str,
) -> Result<String, String> {
    let date = DateTime::parse_from_rfc3339(created_at)
        .map_err(|_| "created_at must be an RFC3339 timestamp".to_string())?
        .format("%Y-%m-%d")
        .to_string();
    let file_name =
        unique_note_file_name_for_date(vault_path, &date, title, Some(current_relative_path))?;
    Ok(format!("{NOTES_DIR}/{file_name}"))
}

fn unique_note_file_name_for_date(
    vault_path: &Path,
    date: &str,
    title: &str,
    current_relative_path: Option<&str>,
) -> Result<String, String> {
    let slug = slugify_title(title);
    let base = format!("{date}-{slug}");
    let mut candidate = format!("{base}.md");
    let mut suffix = 2;

    while note_file_name_exists(vault_path, &candidate, current_relative_path) {
        candidate = format!("{base}-{suffix}.md");
        suffix += 1;
    }

    Ok(candidate)
}

fn note_file_name_exists(
    vault_path: &Path,
    file_name: &str,
    current_relative_path: Option<&str>,
) -> bool {
    let relative_path = format!("{NOTES_DIR}/{file_name}");
    if current_relative_path == Some(relative_path.as_str()) {
        return false;
    }

    notes_path(vault_path).join(file_name).exists()
}

fn slugify_title(title: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in title.trim().chars().flat_map(char::to_lowercase) {
        let is_forbidden = matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        if is_forbidden {
            continue;
        }

        if ch.is_alphanumeric() {
            slug.push(ch);
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "untitled".to_string()
    } else {
        slug
    }
}

fn sanitize_fts_query(query: &str) -> String {
    query
        .split_whitespace()
        .filter_map(|term| {
            let sanitized = term
                .chars()
                .filter(|ch| ch.is_alphanumeric() || *ch == '_' || *ch == '-')
                .collect::<String>();
            if sanitized.is_empty() {
                None
            } else {
                Some(format!("{sanitized}*"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_title(title: Option<String>) -> String {
    normalize_optional_string(title).unwrap_or_else(|| "제목 없음".to_string())
}

fn normalize_required(value: String, field: &str) -> Result<String, String> {
    let normalized = value.trim().to_string();
    if normalized.is_empty() {
        Err(format!("{field} 값은 필수입니다"))
    } else {
        Ok(normalized)
    }
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    tags.into_iter()
        .map(normalize_tag)
        .filter(|tag| !tag.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn normalize_tag(tag: String) -> String {
    tag.trim().to_lowercase()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn load_tags(conn: &Connection, document_id: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT t.name
             FROM tags t
             JOIN document_tags dt ON dt.tag_id = t.id
             WHERE dt.document_id = ?1
             ORDER BY t.name ASC",
        )
        .map_err(|err| format!("태그 불러오기를 준비하지 못했습니다: {err}"))?;
    let rows = stmt
        .query_map(params![document_id], |row| row.get::<_, String>(0))
        .map_err(|err| format!("태그를 조회하지 못했습니다: {err}"))?;

    let mut tags = Vec::new();
    for row in rows {
        tags.push(row.map_err(|err| format!("태그를 읽지 못했습니다: {err}"))?);
    }
    Ok(tags)
}

fn count_rows(conn: &Connection, table: &str) -> Result<usize, String> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    let count: i64 = conn
        .query_row(&sql, [], |row| row.get(0))
        .map_err(|err| format!("{table} 행 수를 세지 못했습니다: {err}"))?;
    Ok(count.try_into().unwrap_or(0))
}

fn unique_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }

    let parent = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("file");
    let extension = path.extension().and_then(|value| value.to_str());

    for suffix in 2.. {
        let file_name = match extension {
            Some(extension) => format!("{stem}-{suffix}.{extension}"),
            None => format!("{stem}-{suffix}"),
        };
        let candidate = parent.join(file_name);
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!()
}

fn resolve_vault_relative_path(vault_path: &Path, relative_path: &str) -> Result<PathBuf, String> {
    if relative_path.contains('\\') || relative_path.contains("..") {
        return Err(format!("안전하지 않은 상대 경로입니다: {relative_path}"));
    }

    let full_path = vault_path.join(relative_path);
    if !full_path.starts_with(vault_path) {
        return Err(format!(
            "보관함 밖으로 벗어나는 경로입니다: {relative_path}"
        ));
    }
    Ok(full_path)
}

fn notes_path(vault_path: &Path) -> PathBuf {
    vault_path.join(NOTES_DIR)
}

fn moa_path(vault_path: &Path) -> PathBuf {
    vault_path.join(MOA_DIR)
}

fn trash_path(vault_path: &Path) -> PathBuf {
    moa_path(vault_path).join(TRASH_DIR)
}

fn database_path(vault_path: &Path) -> PathBuf {
    moa_path(vault_path).join(DB_FILE)
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_vault() -> PathBuf {
        std::env::temp_dir().join(format!("moa-storage-test-{}", Uuid::new_v4()))
    }

    #[test]
    fn creates_vault_with_flat_notes_layout() {
        let vault = temp_vault();
        let summary = initialize_vault_at(vault.clone()).expect("vault initializes");

        assert!(vault.join("notes").is_dir());
        assert!(vault.join(".moa").is_dir());
        assert!(vault.join(".moa").join("trash").is_dir());
        assert!(vault.join(".moa").join("moa.sqlite").is_file());
        let top_level_entries = fs::read_dir(&vault)
            .expect("vault entries")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .to_string()
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            top_level_entries,
            BTreeSet::from([".moa".to_string(), "notes".to_string()])
        );
        assert_eq!(summary.document_count, 0);

        let _ = fs::remove_dir_all(vault);
    }

    #[test]
    fn creates_note_directly_under_notes_and_indexes_it() {
        let vault = temp_vault();
        initialize_vault_at(vault.clone()).expect("vault initializes");

        let document = create_document_in_vault(
            &vault,
            CreateDocumentInput {
                title: Some("Local First Note".to_string()),
                tags: Some(vec!["Markdown".to_string(), " local ".to_string()]),
                body: Some("Body text for search.".to_string()),
            },
        )
        .expect("document is created");

        assert!(document.relative_path.starts_with("notes/"));
        assert_eq!(document.relative_path.matches('/').count(), 1);
        assert_eq!(document.body, "Body text for search.");
        assert!(vault.join(&document.relative_path).is_file());

        let listed = list_documents_in_vault(&vault, None).expect("documents list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].tags, vec!["local", "markdown"]);

        let results = search_documents_in_vault(
            &vault,
            SearchInput {
                query: "search".to_string(),
                tags: None,
                sort: None,
            },
        )
        .expect("search works");
        assert_eq!(results.len(), 1);

        save_document_in_vault(
            &vault,
            SaveDocumentInput {
                id: document.id,
                title: "Korean Search".to_string(),
                tags: vec!["markdown".to_string()],
                body: "검색 가능한 본문입니다.".to_string(),
                sync_file_name: false,
            },
        )
        .expect("document saves");
        let korean_results = search_documents_in_vault(
            &vault,
            SearchInput {
                query: "본문".to_string(),
                tags: None,
                sort: None,
            },
        )
        .expect("korean prefix search works");
        assert_eq!(korean_results.len(), 1);

        let _ = fs::remove_dir_all(vault);
    }

    #[test]
    fn save_preserves_file_name_and_updates_metadata() {
        let vault = temp_vault();
        initialize_vault_at(vault.clone()).expect("vault initializes");
        let document = create_document_in_vault(
            &vault,
            CreateDocumentInput {
                title: Some("Original Title".to_string()),
                tags: None,
                body: None,
            },
        )
        .expect("document is created");
        let original_path = document.relative_path.clone();

        save_document_in_vault(
            &vault,
            SaveDocumentInput {
                id: document.id.clone(),
                title: "Changed Title".to_string(),
                tags: vec!["FTS".to_string()],
                body: "Changed body".to_string(),
                sync_file_name: false,
            },
        )
        .expect("document saves");

        let reloaded = read_document_in_vault(&vault, &document.id).expect("document reloads");
        assert_eq!(reloaded.relative_path, original_path);
        assert_eq!(reloaded.title, "Changed Title");
        assert_eq!(reloaded.tags, vec!["fts"]);
        assert_eq!(reloaded.body, "Changed body");

        let _ = fs::remove_dir_all(vault);
    }

    #[test]
    fn save_skips_unchanged_document_without_touching_file_or_updated_at() {
        let vault = temp_vault();
        initialize_vault_at(vault.clone()).expect("vault initializes");
        let document = create_document_in_vault(
            &vault,
            CreateDocumentInput {
                title: Some("Stable Title".to_string()),
                tags: Some(vec!["note".to_string()]),
                body: Some("Stable body".to_string()),
            },
        )
        .expect("document is created");
        let full_path = vault.join(&document.relative_path);
        let before_content = fs::read_to_string(&full_path).expect("document content reads");
        let before_modified = fs::metadata(&full_path)
            .expect("document metadata reads")
            .modified()
            .expect("document mtime reads");

        let saved = save_document_in_vault(
            &vault,
            SaveDocumentInput {
                id: document.id.clone(),
                title: document.title.clone(),
                tags: document.tags.clone(),
                body: document.body.clone(),
                sync_file_name: false,
            },
        )
        .expect("unchanged document save is accepted");

        let after_content = fs::read_to_string(&full_path).expect("document content reads again");
        let after_modified = fs::metadata(&full_path)
            .expect("document metadata reads again")
            .modified()
            .expect("document mtime reads again");
        let reloaded = read_document_in_vault(&vault, &document.id).expect("document reloads");

        assert_eq!(saved.updated_at, document.updated_at);
        assert_eq!(reloaded.updated_at, document.updated_at);
        assert_eq!(after_content, before_content);
        assert_eq!(after_modified, before_modified);

        let _ = fs::remove_dir_all(vault);
    }

    #[test]
    fn save_syncs_file_name_when_requested() {
        let vault = temp_vault();
        initialize_vault_at(vault.clone()).expect("vault initializes");
        let document = create_document_in_vault(
            &vault,
            CreateDocumentInput {
                title: Some("Original Title".to_string()),
                tags: None,
                body: None,
            },
        )
        .expect("document is created");
        let created_date = DateTime::parse_from_rfc3339(&document.created_at)
            .expect("created_at parses")
            .format("%Y-%m-%d")
            .to_string();

        let saved = save_document_in_vault(
            &vault,
            SaveDocumentInput {
                id: document.id.clone(),
                title: "Changed Title".to_string(),
                tags: Vec::new(),
                body: "Changed body".to_string(),
                sync_file_name: true,
            },
        )
        .expect("document saves");

        assert_eq!(
            saved.relative_path,
            format!("notes/{created_date}-changed-title.md")
        );
        assert!(vault.join(&saved.relative_path).is_file());
        assert!(!vault.join(&document.relative_path).exists());

        let reloaded = read_document_in_vault(&vault, &document.id).expect("document reloads");
        assert_eq!(reloaded.relative_path, saved.relative_path);
        assert_eq!(reloaded.title, "Changed Title");

        let saved_again = save_document_in_vault(
            &vault,
            SaveDocumentInput {
                id: document.id.clone(),
                title: "Changed Title".to_string(),
                tags: Vec::new(),
                body: "Changed body again".to_string(),
                sync_file_name: true,
            },
        )
        .expect("document saves again");

        assert_eq!(saved_again.relative_path, reloaded.relative_path);

        let _ = fs::remove_dir_all(vault);
    }

    #[test]
    fn save_syncs_file_name_without_bumping_updated_at_when_content_is_unchanged() {
        let vault = temp_vault();
        initialize_vault_at(vault.clone()).expect("vault initializes");
        let document = create_document_in_vault(
            &vault,
            CreateDocumentInput {
                title: Some("Original Title".to_string()),
                tags: None,
                body: Some("Body".to_string()),
            },
        )
        .expect("document is created");

        let saved_content = save_document_in_vault(
            &vault,
            SaveDocumentInput {
                id: document.id.clone(),
                title: "Changed Title".to_string(),
                tags: Vec::new(),
                body: "Body".to_string(),
                sync_file_name: false,
            },
        )
        .expect("document content saves");

        let synced_name = save_document_in_vault(
            &vault,
            SaveDocumentInput {
                id: document.id.clone(),
                title: "Changed Title".to_string(),
                tags: Vec::new(),
                body: "Body".to_string(),
                sync_file_name: true,
            },
        )
        .expect("document file name syncs");

        assert_eq!(synced_name.updated_at, saved_content.updated_at);
        assert_ne!(synced_name.relative_path, document.relative_path);

        let reloaded = read_document_in_vault(&vault, &document.id).expect("document reloads");
        assert_eq!(reloaded.updated_at, saved_content.updated_at);
        assert_eq!(reloaded.relative_path, synced_name.relative_path);

        let _ = fs::remove_dir_all(vault);
    }

    #[test]
    fn save_syncs_file_name_with_suffix_on_conflict() {
        let vault = temp_vault();
        initialize_vault_at(vault.clone()).expect("vault initializes");
        let first = create_document_in_vault(
            &vault,
            CreateDocumentInput {
                title: Some("Shared Title".to_string()),
                tags: None,
                body: None,
            },
        )
        .expect("first document is created");
        let second = create_document_in_vault(
            &vault,
            CreateDocumentInput {
                title: Some("Temporary Title".to_string()),
                tags: None,
                body: None,
            },
        )
        .expect("second document is created");

        let saved = save_document_in_vault(
            &vault,
            SaveDocumentInput {
                id: second.id.clone(),
                title: "Shared Title".to_string(),
                tags: Vec::new(),
                body: String::new(),
                sync_file_name: true,
            },
        )
        .expect("document saves");

        assert_ne!(saved.relative_path, first.relative_path);
        assert!(saved.relative_path.ends_with("-shared-title-2.md"));
        assert!(vault.join(&saved.relative_path).is_file());

        let _ = fs::remove_dir_all(vault);
    }

    #[test]
    fn save_sync_uses_untitled_fallback_for_blank_title() {
        let vault = temp_vault();
        initialize_vault_at(vault.clone()).expect("vault initializes");
        let document = create_document_in_vault(
            &vault,
            CreateDocumentInput {
                title: Some("Original Title".to_string()),
                tags: None,
                body: None,
            },
        )
        .expect("document is created");

        let saved = save_document_in_vault(
            &vault,
            SaveDocumentInput {
                id: document.id.clone(),
                title: "   ".to_string(),
                tags: Vec::new(),
                body: "# Body Heading".to_string(),
                sync_file_name: true,
            },
        )
        .expect("document saves");
        let reloaded = read_document_in_vault(&vault, &document.id).expect("document reloads");

        assert_eq!(reloaded.title, "제목 없음");
        assert!(saved.relative_path.ends_with("-제목-없음.md"));
        assert!(!saved.relative_path.ends_with("-body-heading.md"));

        let _ = fs::remove_dir_all(vault);
    }

    #[test]
    fn rebuild_restores_index_from_markdown_files() {
        let vault = temp_vault();
        initialize_vault_at(vault.clone()).expect("vault initializes");
        let document = create_document_in_vault(
            &vault,
            CreateDocumentInput {
                title: Some("Restorable".to_string()),
                tags: None,
                body: Some("Can be indexed again.".to_string()),
            },
        )
        .expect("document is created");

        fs::remove_file(vault.join(".moa").join("moa.sqlite")).expect("db removed");
        let status = rebuild_index_at(&vault).expect("index rebuilds");
        assert_eq!(status.document_count, 1);

        let reloaded = read_document_in_vault(&vault, &document.id).expect("document reloads");
        assert_eq!(reloaded.title, "Restorable");

        let _ = fs::remove_dir_all(vault);
    }

    #[test]
    fn status_does_not_rebuild_index_from_markdown_files() {
        let vault = temp_vault();
        initialize_vault_at(vault.clone()).expect("vault initializes");
        let document = create_document_in_vault(
            &vault,
            CreateDocumentInput {
                title: Some("Indexed Only Once".to_string()),
                tags: None,
                body: Some("Status should not rescan this file.".to_string()),
            },
        )
        .expect("document is created");

        fs::remove_file(vault.join(&document.relative_path)).expect("document file removed");
        let status = get_storage_status_in_vault(&vault).expect("status reads database only");

        assert_eq!(status.document_count, 1);
        assert_eq!(status.issue_count, 0);

        let _ = fs::remove_dir_all(vault);
    }

    #[test]
    fn delete_moves_document_to_trash_and_removes_index() {
        let vault = temp_vault();
        initialize_vault_at(vault.clone()).expect("vault initializes");
        let document = create_document_in_vault(
            &vault,
            CreateDocumentInput {
                title: Some("Delete Me".to_string()),
                tags: None,
                body: None,
            },
        )
        .expect("document is created");

        let deleted = delete_document_in_vault(&vault, &document.id).expect("document deleted");
        assert!(PathBuf::from(&deleted.trashed_path).is_file());
        assert!(list_documents_in_vault(&vault, None)
            .expect("documents list")
            .is_empty());

        let _ = fs::remove_dir_all(vault);
    }
}
