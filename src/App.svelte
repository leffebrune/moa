<script lang="ts">
  import MarkdownIt from "markdown-it";
  import { onMount } from "svelte";
  import {
    createDocument,
    deleteDocument,
    getStorageStatus,
    initializeVault,
    listDocuments,
    readDocument,
    rebuildIndex,
    saveDocument,
    searchDocuments,
    type DocumentListItem,
    type DocumentPayload,
    type SearchResult,
    type StorageStatus,
    type VaultSummary,
  } from "./storage";

  type ViewMode = "view" | "edit";
  type PanelMode = "documents" | "settings";
  type SaveState = "idle" | "dirty" | "saving" | "saved" | "error";
  type SearchSort = "relevance" | "updatedAt";
  type VisibleDocument = DocumentListItem & {
    snippet?: string;
    score?: number;
  };

  const markdown = new MarkdownIt({
    breaks: true,
    html: false,
    linkify: true,
  });

  let vault: VaultSummary | null = null;
  let status: StorageStatus | null = null;
  let documents: VisibleDocument[] = [];
  let activeDocument: DocumentPayload | null = null;
  let selectedId: string | null = null;
  let panelMode: PanelMode = "documents";
  let viewMode: ViewMode = "view";
  let saveState: SaveState = "idle";
  let message = "";
  let error = "";
  let isLoading = true;
  let isDeleting = false;
  let isRebuilding = false;
  let searchQuery = "";
  let searchSort: SearchSort = "relevance";
  let categoryFilter = "";
  let tagFilter = "";
  let settingVaultPath = "";
  let draftTitle = "";
  let draftCategory = "";
  let draftTags = "";
  let draftBody = "";
  let lastSavedSnapshot = "";
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let requestSerial = 0;

  $: renderedHtml = activeDocument ? markdown.render(activeDocument.body) : "";
  $: draftSnapshot = activeDocument
    ? JSON.stringify({
        title: draftTitle.trim(),
        category: draftCategory.trim(),
        tags: parseTags(draftTags),
        body: draftBody,
      })
    : "";
  $: hasUnsavedChanges =
    Boolean(activeDocument) && draftSnapshot !== lastSavedSnapshot;
  $: categories = Array.from(
    new Set(
      documents
        .map((document) => document.category?.trim())
        .filter((category): category is string => Boolean(category)),
    ),
  ).sort((a, b) => a.localeCompare(b));
  $: tags = Array.from(new Set(documents.flatMap((document) => document.tags))).sort(
    (a, b) => a.localeCompare(b),
  );
  $: filteredDocuments = documents;

  onMount(() => {
    void boot();

    return () => {
      if (saveTimer) {
        clearTimeout(saveTimer);
      }
      if (searchTimer) {
        clearTimeout(searchTimer);
      }
      void flushSave();
    };
  });

  async function boot() {
    isLoading = true;
    error = "";

    try {
      vault = await initializeVault();
      settingVaultPath = vault.vaultPath;
      status = await getStorageStatus();
      await refreshDocuments();
      if (documents.length > 0) {
        await openDocument(documents[0].id, "view");
      }
    } catch (err) {
      error = formatError(err);
    } finally {
      isLoading = false;
    }
  }

  async function refreshDocuments(nextSelectedId = selectedId) {
    const serial = ++requestSerial;
    const query = searchQuery.trim();
    const category = emptyToNull(categoryFilter);
    const tag = emptyToNull(tagFilter);

    try {
      if (query) {
        const results = await searchDocuments({
          query,
          category,
          tags: tag ? [tag] : null,
          sort: searchSort === "updatedAt" ? "updatedAt" : null,
        });
        if (serial !== requestSerial) {
          return;
        }
        documents = results.map(searchResultToListItem);
      } else {
        const items = await listDocuments({
          category,
          tag,
          query: null,
        });
        if (serial !== requestSerial) {
          return;
        }
        documents = items;
      }

      if (
        nextSelectedId &&
        documents.some((document) => document.id === nextSelectedId)
      ) {
        selectedId = nextSelectedId;
      } else {
        selectedId = documents[0]?.id ?? null;
      }
    } catch (err) {
      error = formatError(err);
    }
  }

  async function openDocument(id: string, mode: ViewMode = viewMode) {
    if (activeDocument?.id === id && viewMode === mode) {
      return;
    }

    await flushSave();
    error = "";
    selectedId = id;

    try {
      const document = await readDocument(id);
      activeDocument = document;
      loadDraft(document);
      viewMode = mode;
    } catch (err) {
      error = formatError(err);
    }
  }

  async function createNewDocument() {
    await flushSave();
    error = "";
    message = "";

    try {
      const document = await createDocument({
        title: "Untitled",
        category: emptyToNull(categoryFilter),
        tags: tagFilter ? [tagFilter] : [],
        body: "# Untitled\n\nStart writing here.",
      });
      await refreshDocuments(document.id);
      await openDocument(document.id, "edit");
      message = "Document created.";
    } catch (err) {
      error = formatError(err);
    }
  }

  async function deleteActiveDocument() {
    if (!activeDocument || isDeleting) {
      return;
    }

    const title = activeDocument.title;
    const confirmed = window.confirm(`Move "${title}" to the local trash?`);
    if (!confirmed) {
      return;
    }

    isDeleting = true;
    error = "";
    message = "";

    try {
      await deleteDocument(activeDocument.id);
      activeDocument = null;
      selectedId = null;
      lastSavedSnapshot = "";
      await refreshDocuments(null);
      if (documents[0]) {
        await openDocument(documents[0].id, "view");
      }
      message = "Document moved to trash.";
    } catch (err) {
      error = formatError(err);
    } finally {
      isDeleting = false;
    }
  }

  function loadDraft(document: DocumentPayload) {
    draftTitle = document.title;
    draftCategory = document.category ?? "";
    draftTags = document.tags.join(", ");
    draftBody = document.body;
    lastSavedSnapshot = JSON.stringify({
      title: draftTitle.trim(),
      category: draftCategory.trim(),
      tags: parseTags(draftTags),
      body: draftBody,
    });
    saveState = "saved";
  }

  function markDirty() {
    if (!activeDocument) {
      return;
    }
    saveState = "dirty";
    message = "";
    scheduleSave();
  }

  function scheduleSave() {
    if (saveTimer) {
      clearTimeout(saveTimer);
    }
    saveTimer = setTimeout(() => {
      void flushSave();
    }, 700);
  }

  async function flushSave() {
    if (!activeDocument || !hasUnsavedChanges || saveState === "saving") {
      return;
    }

    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }

    saveState = "saving";
    error = "";

    const title = draftTitle.trim() || "Untitled";
    const category = emptyToNull(draftCategory);
    const tags = parseTags(draftTags);
    const body = draftBody;

    try {
      const result = await saveDocument({
        id: activeDocument.id,
        title,
        category,
        tags,
        body,
      });
      activeDocument = {
        ...activeDocument,
        title,
        category,
        tags,
        body,
        updatedAt: result.updatedAt,
        relativePath: result.relativePath,
      };
      lastSavedSnapshot = JSON.stringify({ title, category: category ?? "", tags, body });
      saveState = "saved";
      await refreshDocuments(activeDocument.id);
      status = await getStorageStatus();
    } catch (err) {
      saveState = "error";
      error = formatError(err);
    }
  }

  function queueSearchRefresh() {
    if (searchTimer) {
      clearTimeout(searchTimer);
    }
    searchTimer = setTimeout(() => {
      void refreshDocuments();
    }, 220);
  }

  async function clearFilters() {
    searchQuery = "";
    categoryFilter = "";
    tagFilter = "";
    await refreshDocuments();
  }

  async function enterViewMode() {
    await flushSave();
    if (activeDocument) {
      activeDocument = {
        ...activeDocument,
        title: draftTitle.trim() || "Untitled",
        category: emptyToNull(draftCategory),
        tags: parseTags(draftTags),
        body: draftBody,
      };
    }
    viewMode = "view";
  }

  async function rebuild() {
    await flushSave();
    isRebuilding = true;
    error = "";
    message = "";

    try {
      await rebuildIndex();
      status = await getStorageStatus();
      await refreshDocuments(selectedId);
      message = "Index rebuilt from Markdown files.";
    } catch (err) {
      error = formatError(err);
    } finally {
      isRebuilding = false;
    }
  }

  async function switchVault() {
    await flushSave();
    isLoading = true;
    error = "";
    message = "";
    activeDocument = null;
    selectedId = null;
    lastSavedSnapshot = "";

    try {
      vault = await initializeVault(emptyToNull(settingVaultPath));
      settingVaultPath = vault.vaultPath;
      status = await getStorageStatus();
      await refreshDocuments(null);
      if (documents[0]) {
        await openDocument(documents[0].id, "view");
      }
      message = "Vault loaded.";
    } catch (err) {
      error = formatError(err);
    } finally {
      isLoading = false;
    }
  }

  function searchResultToListItem(result: SearchResult): VisibleDocument {
    return {
      id: result.id,
      title: result.title,
      category: result.category,
      tags: result.tags,
      relativePath: result.relativePath,
      createdAt: "",
      updatedAt: result.updatedAt,
      contentHash: "",
      snippet: result.snippet,
      score: result.score,
    };
  }

  function parseTags(value: string) {
    return Array.from(
      new Set(
        value
          .split(",")
          .map((tag) => tag.trim().toLowerCase())
          .filter(Boolean),
      ),
    ).sort((a, b) => a.localeCompare(b));
  }

  function emptyToNull(value: string | null | undefined) {
    const trimmed = value?.trim();
    return trimmed ? trimmed : null;
  }

  function formatDate(value: string) {
    if (!value) {
      return "";
    }
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) {
      return value;
    }
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: "medium",
      timeStyle: "short",
    }).format(date);
  }

  function formatError(err: unknown) {
    if (err instanceof Error) {
      return err.message;
    }
    return String(err);
  }

  function saveLabel(state: SaveState) {
    switch (state) {
      case "dirty":
        return "Unsaved";
      case "saving":
        return "Saving";
      case "saved":
        return "Saved";
      case "error":
        return "Save failed";
      default:
        return "Ready";
    }
  }
</script>

<svelte:window
  on:beforeunload={() => {
    void flushSave();
  }}
/>

<main class="workspace">
  <aside class="library-panel" aria-label="Library">
    <header class="app-header">
      <div>
        <p class="eyebrow">Moa</p>
        <h1>Local notes</h1>
      </div>
      <button class="icon-button primary" type="button" on:click={createNewDocument} title="New document">
        +
      </button>
    </header>

    <div class="panel-switch" aria-label="Panel">
      <button
        type="button"
        class:active={panelMode === "documents"}
        on:click={() => (panelMode = "documents")}
      >
        Documents
      </button>
      <button
        type="button"
        class:active={panelMode === "settings"}
        on:click={() => (panelMode = "settings")}
      >
        Settings
      </button>
    </div>

    {#if panelMode === "documents"}
      <section class="search-panel" aria-label="Search and filters">
        <label>
          <span>Search</span>
          <input
            type="search"
            placeholder="Title, body, category, tags"
            bind:value={searchQuery}
            on:input={queueSearchRefresh}
          />
        </label>

        <div class="filter-grid">
          <label>
            <span>Category</span>
            <input
              list="category-options"
              placeholder="All"
              bind:value={categoryFilter}
              on:input={queueSearchRefresh}
            />
          </label>
          <label>
            <span>Tag</span>
            <input
              list="tag-options"
              placeholder="All"
              bind:value={tagFilter}
              on:input={queueSearchRefresh}
            />
          </label>
        </div>

        <div class="sort-switch" aria-label="Search sort">
          <button
            type="button"
            class:active={searchSort === "relevance"}
            on:click={() => {
              searchSort = "relevance";
              void refreshDocuments();
            }}
          >
            Relevance
          </button>
          <button
            type="button"
            class:active={searchSort === "updatedAt"}
            on:click={() => {
              searchSort = "updatedAt";
              void refreshDocuments();
            }}
          >
            Modified
          </button>
        </div>

        <button class="text-button" type="button" on:click={clearFilters}>Clear filters</button>
      </section>

      <section class="document-list" aria-label="Documents">
        {#if isLoading}
          <p class="muted">Loading vault...</p>
        {:else if filteredDocuments.length === 0}
          <div class="empty-state">
            <strong>No documents</strong>
            <span>Create a note or clear the current filters.</span>
          </div>
        {:else}
          {#each filteredDocuments as document}
            <button
              type="button"
              class:active={document.id === selectedId}
              on:click={() => openDocument(document.id)}
            >
              <span class="document-title">{document.title}</span>
              <span class="document-meta">
                {document.category ?? "Uncategorized"}
                {#if document.tags.length > 0}
                  / {document.tags.join(", ")}
                {/if}
              </span>
              <span class="document-date">{formatDate(document.updatedAt)}</span>
              {#if document.snippet}
                <span class="document-snippet">{document.snippet}</span>
              {/if}
            </button>
          {/each}
        {/if}
      </section>
    {:else}
      <section class="settings-panel" aria-label="Settings">
        <div>
          <span class="setting-label">Vault</span>
          <label class="path-field">
            <span class="sr-only">Vault path</span>
            <input bind:value={settingVaultPath} placeholder="Vault path" />
          </label>
        </div>
        <div>
          <span class="setting-label">Notes</span>
          <strong>{vault?.notesPath ?? "-"}</strong>
        </div>
        <div>
          <span class="setting-label">SQLite index</span>
          <strong>{vault?.databasePath ?? "-"}</strong>
        </div>
        <div class="stats-grid">
          <span>{status?.documentCount ?? 0}<small>Documents</small></span>
          <span>{status?.tagCount ?? 0}<small>Tags</small></span>
          <span>{status?.issueCount ?? 0}<small>Issues</small></span>
        </div>
        <div class="settings-actions">
          <button class="wide-button" type="button" disabled={isLoading} on:click={switchVault}>
            {isLoading ? "Loading..." : "Use vault"}
          </button>
          <button class="wide-button secondary" type="button" disabled={isRebuilding} on:click={rebuild}>
            {isRebuilding ? "Rebuilding..." : "Rebuild index"}
          </button>
        </div>

        {#if status?.issues.length}
          <div class="issue-list">
            {#each status.issues as issue}
              <p>
                <strong>{issue.kind}</strong>
                <span>{issue.relativePath ?? ""}</span>
                {issue.message}
              </p>
            {/each}
          </div>
        {/if}
      </section>
    {/if}
  </aside>

  <section class="document-panel" aria-label="Document">
    {#if error}
      <div class="alert error">{error}</div>
    {:else if message}
      <div class="alert">{message}</div>
    {/if}

    {#if activeDocument}
      <header class="document-toolbar">
        <div class="mode-switch" aria-label="Mode">
          <button
            type="button"
            class:active={viewMode === "view"}
            on:click={enterViewMode}
          >
            View
          </button>
          <button
            type="button"
            class:active={viewMode === "edit"}
            on:click={() => (viewMode = "edit")}
          >
            Edit
          </button>
        </div>
        <div class="toolbar-actions">
          <span class:warn={saveState === "dirty" || saveState === "error"}>
            {saveLabel(saveState)}
          </span>
          <button type="button" on:click={flushSave} disabled={!hasUnsavedChanges || saveState === "saving"}>
            Save now
          </button>
          <button class="danger" type="button" disabled={isDeleting} on:click={deleteActiveDocument}>
            Delete
          </button>
        </div>
      </header>

      {#if viewMode === "edit"}
        <section class="editor-layout" aria-label="Editor">
          <div class="metadata-grid">
            <label>
              <span>Title</span>
              <input bind:value={draftTitle} on:input={markDirty} />
            </label>
            <label>
              <span>Category</span>
              <input list="category-options" bind:value={draftCategory} on:input={markDirty} />
            </label>
            <label class="wide-field">
              <span>Tags</span>
              <input
                list="tag-options"
                placeholder="comma, separated, tags"
                bind:value={draftTags}
                on:input={markDirty}
              />
            </label>
          </div>

          <textarea
            aria-label="Markdown source"
            spellcheck="true"
            bind:value={draftBody}
            on:input={markDirty}
          ></textarea>
        </section>
      {:else}
        <article class="reader">
          <header>
            <h2>{activeDocument.title}</h2>
            <div class="reader-meta">
              <span>{activeDocument.category ?? "Uncategorized"}</span>
              <span>{formatDate(activeDocument.updatedAt)}</span>
              <span>{activeDocument.relativePath}</span>
            </div>
            {#if activeDocument.tags.length > 0}
              <div class="tag-row">
                {#each activeDocument.tags as tag}
                  <button
                    type="button"
                    on:click={() => {
                      tagFilter = tag;
                      panelMode = "documents";
                      void refreshDocuments(activeDocument?.id ?? null);
                    }}
                  >
                    {tag}
                  </button>
                {/each}
              </div>
            {/if}
          </header>
          <div class="markdown-body">{@html renderedHtml}</div>
        </article>
      {/if}
    {:else}
      <section class="empty-document">
        <h2>No document selected</h2>
        <p>Create a new Markdown document or choose one from the list.</p>
        <button type="button" on:click={createNewDocument}>New document</button>
      </section>
    {/if}
  </section>
</main>

<datalist id="category-options">
  {#each categories as category}
    <option value={category}></option>
  {/each}
</datalist>

<datalist id="tag-options">
  {#each tags as tag}
    <option value={tag}></option>
  {/each}
</datalist>
