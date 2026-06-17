import type {
  CreateDocumentInput,
  DeleteResult,
  DocumentFilter,
  DocumentListItem,
  DocumentPayload,
  IndexStatus,
  SaveDocumentInput,
  SaveResult,
  SearchInput,
  SearchResult,
  StorageStatus,
  VaultSummary,
} from "./storage";

type MockDocument = DocumentPayload & {
  contentHash: string;
};

const vaultPath = "mock://moa-dev-vault";
const notesPath = "mock://moa-dev-vault/notes";
const databasePath = "mock://moa-dev-vault/.moa/moa.sqlite";

let documents: MockDocument[] = [
  createSeedDocument({
    id: "mock-weekly-review",
    title: "Weekly Review",
    tags: ["review", "planning"],
    body: [
      "# Weekly Review",
      "",
      "- Check active projects",
      "- Capture follow-up items",
      "- Update the knowledge base",
    ].join("\n"),
    relativePath: "notes/weekly-review.md",
    minutesAgo: 12,
  }),
  createSeedDocument({
    id: "mock-markdown-guide",
    title: "Markdown Guide",
    tags: ["markdown", "writing"],
    body: [
      "# Markdown Guide",
      "",
      "Use headings, lists, links, and fenced code blocks for durable notes.",
      "",
      "```ts",
      "const note = 'local first';",
      "```",
    ].join("\n"),
    relativePath: "notes/markdown-guide.md",
    minutesAgo: 80,
  }),
  createSeedDocument({
    id: "mock-search-notes",
    title: "Search Notes",
    tags: ["search", "sqlite"],
    body: [
      "# Search Notes",
      "",
      "The local index should support title, body, and tag lookup.",
    ].join("\n"),
    relativePath: "notes/search-notes.md",
    minutesAgo: 240,
  }),
];

export async function mockInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  switch (command) {
    case "initialize_vault":
      return initializeVault() as T;
    case "list_documents":
      return listDocuments(args?.filter as DocumentFilter | null | undefined) as T;
    case "read_document":
      return readDocument(String(args?.id)) as T;
    case "create_document":
      return createDocument(args?.input as CreateDocumentInput) as T;
    case "save_document":
      return saveDocument(args?.input as SaveDocumentInput) as T;
    case "delete_document":
      return deleteDocument(String(args?.id)) as T;
    case "search_documents":
      return searchDocuments(args?.input as SearchInput) as T;
    case "rebuild_index":
      return indexStatus() as T;
    case "get_storage_status":
      return storageStatus() as T;
    default:
      throw new Error(`Unsupported mock storage command: ${command}`);
  }
}

function initializeVault(): VaultSummary {
  return {
    vaultPath,
    notesPath,
    databasePath,
    documentCount: documents.length,
    issueCount: 0,
    issues: [],
  };
}

function listDocuments(filter?: DocumentFilter | null): DocumentListItem[] {
  return applyFilter(documents, filter).map(toListItem);
}

function readDocument(id: string): DocumentPayload {
  const document = documents.find((candidate) => candidate.id === id);
  if (!document) {
    throw new Error(`Document not found: ${id}`);
  }

  const { contentHash: _contentHash, ...payload } = document;
  return { ...payload, tags: [...payload.tags] };
}

function createDocument(input: CreateDocumentInput): DocumentPayload {
  const now = new Date().toISOString();
  const title = normalizeTitle(input?.title);
  const document: MockDocument = {
    id: createId(),
    title,
    tags: normalizeTags(input?.tags ?? []),
    body: input?.body ?? "",
    relativePath: uniqueMockRelativePath(title, now),
    createdAt: now,
    updatedAt: now,
    contentHash: createHash(title, input?.body ?? ""),
  };
  documents = [document, ...documents];
  return readDocument(document.id);
}

function saveDocument(input: SaveDocumentInput): SaveResult {
  const index = documents.findIndex((candidate) => candidate.id === input.id);
  if (index === -1) {
    throw new Error(`Document not found: ${input.id}`);
  }

  const updatedAt = new Date().toISOString();
  const title = normalizeTitle(input.title);
  const relativePath = input.syncFileName
    ? uniqueMockRelativePath(title, documents[index].createdAt, documents[index].relativePath)
    : documents[index].relativePath;
  documents[index] = {
    ...documents[index],
    title,
    tags: normalizeTags(input.tags),
    body: input.body,
    relativePath,
    updatedAt,
    contentHash: createHash(title, input.body),
  };

  return {
    id: input.id,
    relativePath: documents[index].relativePath,
    updatedAt,
    fileNameSyncError: null,
  };
}

function deleteDocument(id: string): DeleteResult {
  const document = documents.find((candidate) => candidate.id === id);
  if (!document) {
    throw new Error(`Document not found: ${id}`);
  }

  documents = documents.filter((candidate) => candidate.id !== id);
  return {
    id,
    trashedPath: `mock://moa-dev-vault/.moa/trash/${document.relativePath.split("/").pop()}`,
  };
}

function searchDocuments(input: SearchInput): SearchResult[] {
  const query = input.query.trim().toLowerCase();
  const filtered = applyFilter(documents, {
    tag: input.tags?.[0] ?? null,
    query: null,
  });

  const results = filtered
    .filter((document) => searchableText(document).includes(query))
    .map((document) => ({
      id: document.id,
      title: document.title,
      tags: [...document.tags],
      relativePath: document.relativePath,
      updatedAt: document.updatedAt,
      snippet: createSnippet(document, query),
      score: scoreDocument(document, query),
    }));

  if (input.sort === "updatedAt") {
    return results.sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
  }

  return results.sort((a, b) => b.score - a.score);
}

function storageStatus(): StorageStatus {
  return {
    vaultPath,
    notesPath,
    databasePath,
    documentCount: documents.length,
    tagCount: new Set(documents.flatMap((document) => document.tags)).size,
    issueCount: 0,
    issues: [],
  };
}

function indexStatus(): IndexStatus {
  return {
    documentCount: documents.length,
    issueCount: 0,
    issues: [],
  };
}

function applyFilter(
  candidates: MockDocument[],
  filter?: DocumentFilter | null,
): MockDocument[] {
  const tag = normalizeOptional(filter?.tag)?.toLowerCase();
  const query = normalizeOptional(filter?.query)?.toLowerCase();

  return candidates
    .filter((document) => {
      if (tag && !document.tags.includes(tag)) {
        return false;
      }
      if (query && !searchableText(document).includes(query)) {
        return false;
      }
      return true;
    })
    .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
}

function toListItem(document: MockDocument): DocumentListItem {
  return {
    id: document.id,
    title: document.title,
    tags: [...document.tags],
    relativePath: document.relativePath,
    createdAt: document.createdAt,
    updatedAt: document.updatedAt,
    contentHash: document.contentHash,
  };
}

function createSeedDocument(input: {
  id: string;
  title: string;
  tags: string[];
  body: string;
  relativePath: string;
  minutesAgo: number;
}): MockDocument {
  const now = Date.now() - input.minutesAgo * 60 * 1000;
  const timestamp = new Date(now).toISOString();
  return {
    id: input.id,
    title: input.title,
    tags: input.tags,
    body: input.body,
    relativePath: input.relativePath,
    createdAt: timestamp,
    updatedAt: timestamp,
    contentHash: createHash(input.title, input.body),
  };
}

function searchableText(document: MockDocument): string {
  return [
    document.title,
    document.tags.join(" "),
    document.body,
  ]
    .join(" ")
    .toLowerCase();
}

function createSnippet(document: MockDocument, query: string): string {
  if (!query) {
    return "";
  }

  const body = document.body.replace(/\s+/g, " ");
  const index = body.toLowerCase().indexOf(query);
  if (index === -1) {
    return body.slice(0, 96);
  }

  return body.slice(Math.max(0, index - 32), index + query.length + 64);
}

function scoreDocument(document: MockDocument, query: string): number {
  if (!query) {
    return 0;
  }
  if (document.title.toLowerCase().includes(query)) {
    return 3;
  }
  if (document.tags.some((tag) => tag.includes(query))) {
    return 2;
  }
  return 1;
}

function normalizeTitle(value?: string | null): string {
  return normalizeOptional(value) ?? "제목 없음";
}

function normalizeOptional(value?: string | null): string | null {
  const trimmed = value?.trim();
  return trimmed ? trimmed : null;
}

function normalizeTags(tags: string[]): string[] {
  return Array.from(
    new Set(tags.map((tag) => tag.trim().toLowerCase()).filter(Boolean)),
  ).sort((a, b) => a.localeCompare(b));
}

function createId(): string {
  return globalThis.crypto?.randomUUID?.() ?? `mock-${Date.now()}`;
}

function uniqueMockRelativePath(
  title: string,
  createdAt: string,
  currentRelativePath?: string,
): string {
  const date = createdAt.slice(0, 10);
  const slug = slugify(title);
  const base = `notes/${date}-${slug}`;
  let candidate = `${base}.md`;
  let suffix = 2;

  while (
    candidate !== currentRelativePath &&
    documents.some((document) => document.relativePath === candidate)
  ) {
    candidate = `${base}-${suffix}.md`;
    suffix += 1;
  }

  return candidate;
}

function slugify(value: string): string {
  let slug = "";
  let lastWasDash = false;

  for (const char of value.trim().toLowerCase()) {
    if (`<>:"/\\|?*`.includes(char)) {
      continue;
    }

    if (/[\p{Letter}\p{Number}]/u.test(char)) {
      slug += char;
      lastWasDash = false;
    } else if (!lastWasDash) {
      slug += "-";
      lastWasDash = true;
    }
  }

  return slug.replace(/^-+|-+$/g, "") || "untitled";
}

function createHash(...values: string[]): string {
  let hash = 0;
  for (const value of values.join("\n")) {
    hash = (hash * 31 + value.charCodeAt(0)) >>> 0;
  }
  return hash.toString(16).padStart(8, "0");
}
