import { invoke } from "@tauri-apps/api/core";
import { mockInvoke } from "./mockStorage";

type TauriWindow = Window & {
  __TAURI_INTERNALS__?: unknown;
};

export type StorageIssue = {
  kind: string;
  message: string;
  relativePath?: string | null;
};

export type VaultSummary = {
  vaultPath: string;
  notesPath: string;
  databasePath: string;
  documentCount: number;
  issueCount: number;
  issues: StorageIssue[];
};

export type DocumentFilter = {
  tag?: string | null;
  query?: string | null;
};

export type DocumentListItem = {
  id: string;
  title: string;
  tags: string[];
  relativePath: string;
  createdAt: string;
  updatedAt: string;
  contentHash: string;
};

export type DocumentPayload = {
  id: string;
  title: string;
  tags: string[];
  body: string;
  relativePath: string;
  createdAt: string;
  updatedAt: string;
};

export type CreateDocumentInput = {
  title?: string | null;
  tags?: string[] | null;
  body?: string | null;
};

export type SaveDocumentInput = {
  id: string;
  title: string;
  tags: string[];
  body: string;
  syncFileName: boolean;
};

export type SaveResult = {
  id: string;
  relativePath: string;
  updatedAt: string;
  fileNameSyncError?: string | null;
};

export type DeleteResult = {
  id: string;
  trashedPath: string;
};

export type SearchInput = {
  query: string;
  tags?: string[] | null;
  sort?: "updatedAt" | null;
};

export type SearchResult = {
  id: string;
  title: string;
  tags: string[];
  relativePath: string;
  updatedAt: string;
  snippet: string;
  score: number;
};

export type IndexStatus = {
  documentCount: number;
  issueCount: number;
  issues: StorageIssue[];
};

export type StorageStatus = {
  vaultPath: string;
  notesPath: string;
  databasePath: string;
  documentCount: number;
  tagCount: number;
  issueCount: number;
  issues: StorageIssue[];
};

export function initializeVault(path?: string | null): Promise<VaultSummary> {
  return desktopInvoke("initialize_vault", { path });
}

export function listDocuments(
  filter?: DocumentFilter | null,
): Promise<DocumentListItem[]> {
  return desktopInvoke("list_documents", { filter });
}

export function readDocument(id: string): Promise<DocumentPayload> {
  return desktopInvoke("read_document", { id });
}

export function createDocument(
  input: CreateDocumentInput,
): Promise<DocumentPayload> {
  return desktopInvoke("create_document", { input });
}

export function saveDocument(input: SaveDocumentInput): Promise<SaveResult> {
  return desktopInvoke("save_document", { input });
}

export function deleteDocument(id: string): Promise<DeleteResult> {
  return desktopInvoke("delete_document", { id });
}

export function searchDocuments(input: SearchInput): Promise<SearchResult[]> {
  return desktopInvoke("search_documents", { input });
}

export function rebuildIndex(): Promise<IndexStatus> {
  return desktopInvoke("rebuild_index");
}

export function getStorageStatus(): Promise<StorageStatus> {
  return desktopInvoke("get_storage_status");
}

function desktopInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!isTauriDesktop()) {
    if (import.meta.env.DEV) {
      return mockInvoke<T>(command, args);
    }

    return Promise.reject(
      new Error(
        "Moa 저장소는 Tauri 데스크톱 앱에서 사용할 수 있습니다. 로컬 보관함을 사용하려면 npm.cmd run tauri dev를 실행하세요.",
      ),
    );
  }

  return invoke<T>(command, args);
}

function isTauriDesktop() {
  return (
    typeof window !== "undefined" &&
    Boolean((window as TauriWindow).__TAURI_INTERNALS__)
  );
}
