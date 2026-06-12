import { invoke } from "@tauri-apps/api/core";

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
  category?: string | null;
  tag?: string | null;
  query?: string | null;
};

export type DocumentListItem = {
  id: string;
  title: string;
  category?: string | null;
  tags: string[];
  relativePath: string;
  createdAt: string;
  updatedAt: string;
  contentHash: string;
};

export type DocumentPayload = {
  id: string;
  title: string;
  category?: string | null;
  tags: string[];
  body: string;
  relativePath: string;
  createdAt: string;
  updatedAt: string;
};

export type CreateDocumentInput = {
  title?: string | null;
  category?: string | null;
  tags?: string[] | null;
  body?: string | null;
};

export type SaveDocumentInput = {
  id: string;
  title: string;
  category?: string | null;
  tags: string[];
  body: string;
};

export type SaveResult = {
  id: string;
  relativePath: string;
  updatedAt: string;
};

export type DeleteResult = {
  id: string;
  trashedPath: string;
};

export type SearchInput = {
  query: string;
  category?: string | null;
  tags?: string[] | null;
  sort?: "updatedAt" | null;
};

export type SearchResult = {
  id: string;
  title: string;
  category?: string | null;
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
    return Promise.reject(
      new Error(
        "Moa storage is available in the Tauri desktop app. Run npm.cmd run tauri dev to use the local vault.",
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
