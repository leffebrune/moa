<script lang="ts">
  import MarkdownIt from "markdown-it";
  import { onMount, tick } from "svelte";
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
  import { TEXT } from "./text";

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
  let pendingDeleteDocument: DocumentPayload | null = null;
  let searchQuery = "";
  let isSearchDetailsOpen = false;
  let searchSort: SearchSort = "relevance";
  let tagFilter = "";
  let settingVaultPath = "";
  let draftTitle = "";
  let draftTags = "";
  let draftBody = "";
  let editorTextarea: HTMLTextAreaElement | null = null;
  let lastSavedSnapshot = "";
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let saveQueue: Promise<void> = Promise.resolve();
  let draftRevision = 0;
  let requestSerial = 0;
  const deletedDocumentIds = new Set<string>();

  $: renderedHtml = activeDocument ? markdown.render(activeDocument.body) : "";
  $: draftSnapshot = activeDocument
    ? JSON.stringify({
        title: draftTitle.trim(),
        tags: parseTags(draftTags),
        body: draftBody,
      })
    : "";
  $: hasUnsavedChanges =
    Boolean(activeDocument) && draftSnapshot !== lastSavedSnapshot;
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
    const tag = emptyToNull(tagFilter);

    try {
      if (query) {
        const results = await searchDocuments({
          query,
          tags: tag ? [tag] : null,
          sort: searchSort === "updatedAt" ? "updatedAt" : null,
        });
        if (serial !== requestSerial) {
          return;
        }
        documents = results.map(searchResultToListItem);
      } else {
        const items = await listDocuments({
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

    pendingDeleteDocument = null;
    if (!(await leaveEditMode())) {
      return;
    }
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
    pendingDeleteDocument = null;
    if (!(await leaveEditMode())) {
      return;
    }
    error = "";
    message = "";

    try {
      const document = await createDocument({
        title: "",
        tags: tagFilter ? [tagFilter] : [],
        body: "",
      });
      await refreshDocuments(document.id);
      await openDocument(document.id, "edit");
      message = TEXT.messages.documentCreated;
    } catch (err) {
      error = formatError(err);
    }
  }

  async function deleteActiveDocument() {
    if (!activeDocument) {
      return;
    }

    pendingDeleteDocument = activeDocument;
  }

  async function confirmDeleteDocument() {
    if (!pendingDeleteDocument || isDeleting) {
      return;
    }

    const documentId = pendingDeleteDocument.id;
    isDeleting = true;
    error = "";
    message = "";
    deletedDocumentIds.add(documentId);
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }

    try {
      await deleteDocument(documentId);
      pendingDeleteDocument = null;
      activeDocument = null;
      selectedId = null;
      lastSavedSnapshot = "";
      await refreshDocuments(null);
      if (documents[0]) {
        await openDocument(documents[0].id, "view");
      }
      message = TEXT.messages.documentMovedToTrash;
    } catch (err) {
      deletedDocumentIds.delete(documentId);
      error = formatError(err);
    } finally {
      isDeleting = false;
    }
  }

  function cancelDeleteDocument() {
    pendingDeleteDocument = null;
  }

  function loadDraft(document: DocumentPayload) {
    draftTitle = document.title;
    draftTags = document.tags.join(", ");
    draftBody = document.body;
    lastSavedSnapshot = JSON.stringify({
      title: draftTitle.trim(),
      tags: parseTags(draftTags),
      body: draftBody,
    });
    draftRevision = 0;
    saveState = "saved";
  }

  function markDirty() {
    if (!activeDocument) {
      return;
    }
    draftRevision += 1;
    saveState = "dirty";
    message = "";
    scheduleSave();
  }

  function scheduleSave() {
    if (saveTimer) {
      clearTimeout(saveTimer);
    }
    saveTimer = setTimeout(() => {
      void flushSave({ syncFileName: false });
    }, 700);
  }

  async function leaveEditMode() {
    if (viewMode !== "edit") {
      return true;
    }

    const saved = await flushSave({ syncFileName: true, force: true });
    if (saved) {
      viewMode = "view";
    }
    return saved;
  }

  async function flushSave(options: { syncFileName?: boolean; force?: boolean } = {}) {
    if (!activeDocument) {
      return true;
    }

    const syncFileName = options.syncFileName ?? false;
    const force = options.force ?? false;
    if (!force && !hasUnsavedChanges) {
      return true;
    }

    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }

    const documentId = activeDocument.id;
    const revision = draftRevision;
    saveState = "saving";
    error = "";

    const title = draftTitle.trim() || TEXT.defaults.newDocumentTitle;
    const tags = parseTags(draftTags);
    const body = draftBody;
    const saveTask = saveQueue.then(async () => {
      if (deletedDocumentIds.has(documentId)) {
        return;
      }
      if (!syncFileName && activeDocument?.id === documentId && revision !== draftRevision) {
        saveState = "dirty";
        return;
      }
      const result = await saveDocument({
        id: documentId,
        title,
        tags,
        body,
        syncFileName,
      });

      if (deletedDocumentIds.has(documentId)) {
        return;
      }
      if (activeDocument?.id === documentId) {
        activeDocument = {
          ...activeDocument,
          title,
          tags,
          body,
          updatedAt: result.updatedAt,
          relativePath: result.relativePath,
        };
        lastSavedSnapshot = JSON.stringify({ title, tags, body });
        saveState = "saved";
      }
      if (result.fileNameSyncError) {
        message = `내용은 저장됐지만 파일명 변경은 실패했습니다. ${result.fileNameSyncError}`;
      }
      await refreshDocuments(documentId);
      status = await getStorageStatus();
    });
    saveQueue = saveTask.catch(() => {});

    try {
      await saveTask;
      return true;
    } catch (err) {
      if (deletedDocumentIds.has(documentId)) {
        return true;
      }
      saveState = "error";
      error = formatError(err);
      return false;
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
    tagFilter = "";
    await refreshDocuments();
  }

  async function enterViewMode() {
    if (!(await leaveEditMode())) {
      return;
    }
    if (activeDocument) {
      activeDocument = {
        ...activeDocument,
        title: draftTitle.trim() || TEXT.defaults.newDocumentTitle,
        tags: parseTags(draftTags),
        body: draftBody,
      };
    }
    viewMode = "view";
  }

  async function enterEditMode() {
    if (!activeDocument) {
      return;
    }

    pendingDeleteDocument = null;
    viewMode = "edit";
    await tick();
    editorTextarea?.focus();
  }

  function handleKeyboardShortcut(event: KeyboardEvent) {
    if (event.defaultPrevented || event.isComposing || event.altKey || event.ctrlKey || event.metaKey) {
      return;
    }

    if (event.key === "Enter" && viewMode === "view" && activeDocument && !isEditableTarget(event.target)) {
      event.preventDefault();
      void enterEditMode();
      return;
    }

    if (event.key === "Escape" && viewMode === "edit") {
      event.preventDefault();
      void enterViewMode();
    }
  }

  function isEditableTarget(target: EventTarget | null) {
    if (!(target instanceof HTMLElement)) {
      return false;
    }

    const tagName = target.tagName.toLowerCase();
    return (
      tagName === "input" ||
      tagName === "textarea" ||
      tagName === "select" ||
      tagName === "button" ||
      target.isContentEditable
    );
  }

  async function rebuild() {
    if (!(await leaveEditMode())) {
      return;
    }
    isRebuilding = true;
    error = "";
    message = "";

    try {
      await rebuildIndex();
      status = await getStorageStatus();
      await refreshDocuments(selectedId);
      message = TEXT.messages.indexRebuilt;
    } catch (err) {
      error = formatError(err);
    } finally {
      isRebuilding = false;
    }
  }

  async function switchVault() {
    if (!(await leaveEditMode())) {
      return;
    }
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
      message = TEXT.messages.vaultLoaded;
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
        return TEXT.saveState.dirty;
      case "saving":
        return TEXT.saveState.saving;
      case "saved":
        return TEXT.saveState.saved;
      case "error":
        return TEXT.saveState.error;
      default:
        return TEXT.saveState.idle;
    }
  }

  function issueKindLabel(kind: string) {
    return TEXT.issueKinds[kind as keyof typeof TEXT.issueKinds] ?? kind;
  }
</script>

<svelte:window on:keydown={handleKeyboardShortcut} />

<main class="workspace">
  <aside class="library-panel" aria-label={TEXT.aria.library}>
    <header class="app-header">
      <div>
        <p class="eyebrow">{TEXT.app.brand}</p>
        <h1>{TEXT.app.title}</h1>
      </div>
      <button
        class="icon-button primary"
        type="button"
        on:click={createNewDocument}
        title={TEXT.actions.newDocument}
      >
        +
      </button>
    </header>

    <div class="panel-switch" aria-label={TEXT.aria.panel}>
      <button
        type="button"
        class:active={panelMode === "documents"}
        on:click={() => (panelMode = "documents")}
      >
        {TEXT.labels.panelDocuments}
      </button>
      <button
        type="button"
        class:active={panelMode === "settings"}
        on:click={() => (panelMode = "settings")}
      >
        {TEXT.labels.panelSettings}
      </button>
    </div>

    {#if panelMode === "documents"}
      <section class="search-panel" aria-label={TEXT.aria.searchAndFilters}>
        <div class="search-header">
          <div class="search-field">
            <label for="document-search">{TEXT.labels.search}</label>
            <div class="search-control">
              <input
                id="document-search"
                type="search"
                bind:value={searchQuery}
                on:input={queueSearchRefresh}
              />
              <button
                class="search-detail-button"
                type="button"
                aria-label={isSearchDetailsOpen ? TEXT.aria.hideSearchDetails : TEXT.aria.showSearchDetails}
                aria-expanded={isSearchDetailsOpen}
                on:click={() => (isSearchDetailsOpen = !isSearchDetailsOpen)}
              >
                {isSearchDetailsOpen ? TEXT.state.detailsOpen : TEXT.state.detailsClosed}
              </button>
            </div>
          </div>
        </div>

        {#if isSearchDetailsOpen}
          <div class="search-details">
            <div class="filter-grid">
              <label>
                <span>{TEXT.labels.tag}</span>
                <input
                  list="tag-options"
                  bind:value={tagFilter}
                  on:input={queueSearchRefresh}
                />
              </label>
            </div>

            <div class="sort-switch" aria-label={TEXT.aria.searchSort}>
              <button
                type="button"
                class:active={searchSort === "relevance"}
                on:click={() => {
                  searchSort = "relevance";
                  void refreshDocuments();
                }}
              >
                {TEXT.labels.relevance}
              </button>
              <button
                type="button"
                class:active={searchSort === "updatedAt"}
                on:click={() => {
                  searchSort = "updatedAt";
                  void refreshDocuments();
                }}
              >
                {TEXT.labels.modified}
              </button>
            </div>

            <button class="text-button" type="button" on:click={clearFilters}>
              {TEXT.actions.clearFilters}
            </button>
          </div>
        {/if}
      </section>

      <section class="document-list" aria-label={TEXT.labels.documents}>
        {#if isLoading}
          <p class="muted">{TEXT.state.loadingVault}</p>
        {:else if filteredDocuments.length === 0}
          <div class="empty-state">
            <strong>{TEXT.empty.documentListTitle}</strong>
            <span>{TEXT.empty.documentListBody}</span>
          </div>
        {:else}
          {#each filteredDocuments as document}
            <button
              type="button"
              class:active={document.id === selectedId}
              on:click={() => openDocument(document.id)}
            >
              <span class="document-title">{document.title}</span>
              {#if document.tags.length > 0}
                <span class="document-meta">{document.tags.join(", ")}</span>
              {/if}
              <span class="document-date">{formatDate(document.updatedAt)}</span>
              {#if document.snippet}
                <span class="document-snippet">{document.snippet}</span>
              {/if}
            </button>
          {/each}
        {/if}
      </section>
    {:else}
      <section class="settings-panel" aria-label={TEXT.labels.panelSettings}>
        <div>
          <span class="setting-label">{TEXT.labels.vault}</span>
          <label class="path-field">
            <span class="sr-only">{TEXT.aria.vaultPath}</span>
            <input bind:value={settingVaultPath} />
          </label>
        </div>
        <div>
          <span class="setting-label">{TEXT.labels.notes}</span>
          <strong>{vault?.notesPath ?? TEXT.state.notAvailable}</strong>
        </div>
        <div>
          <span class="setting-label">{TEXT.labels.sqliteIndex}</span>
          <strong>{vault?.databasePath ?? TEXT.state.notAvailable}</strong>
        </div>
        <div class="stats-grid">
          <span>{status?.documentCount ?? 0}<small>{TEXT.labels.documents}</small></span>
          <span>{status?.tagCount ?? 0}<small>{TEXT.labels.tags}</small></span>
          <span>{status?.issueCount ?? 0}<small>{TEXT.labels.issues}</small></span>
        </div>
        <div class="settings-actions">
          <button class="wide-button" type="button" disabled={isLoading} on:click={switchVault}>
            {isLoading ? TEXT.state.loading : TEXT.actions.useVault}
          </button>
          <button class="wide-button secondary" type="button" disabled={isRebuilding} on:click={rebuild}>
            {isRebuilding ? TEXT.state.rebuilding : TEXT.actions.rebuildIndex}
          </button>
        </div>

        {#if status?.issues.length}
          <div class="issue-list">
            {#each status.issues as issue}
              <p>
                <strong>{issueKindLabel(issue.kind)}</strong>
                <span>{issue.relativePath ?? ""}</span>
                {issue.message}
              </p>
            {/each}
          </div>
        {/if}
      </section>
    {/if}
  </aside>

  <section class="document-panel" aria-label={TEXT.aria.document}>
    {#if error}
      <div class="alert error">{error}</div>
    {:else if message}
      <div class="alert">{message}</div>
    {/if}

    {#if activeDocument}
      <header class="document-toolbar">
        <div class="mode-switch" aria-label={TEXT.aria.mode}>
          <button
            type="button"
            class:active={viewMode === "view"}
            on:click={enterViewMode}
          >
            {TEXT.labels.viewRead}
          </button>
          <button
            type="button"
            class:active={viewMode === "edit"}
            on:click={enterEditMode}
          >
            {TEXT.labels.viewEdit}
          </button>
        </div>
        <div class="toolbar-actions">
          <span class:warn={saveState === "dirty" || saveState === "error"}>
            {saveLabel(saveState)}
          </span>
          <button class="danger" type="button" disabled={isDeleting} on:click={deleteActiveDocument}>
            {TEXT.actions.delete}
          </button>
        </div>
      </header>

      {#if pendingDeleteDocument?.id === activeDocument.id}
        <div class="confirm-bar" role="alert">
          <span>{TEXT.confirm.moveToTrash(pendingDeleteDocument.title)}</span>
          <div>
            <button type="button" class="secondary" on:click={cancelDeleteDocument}>
              {TEXT.actions.cancel}
            </button>
            <button type="button" class="danger" disabled={isDeleting} on:click={confirmDeleteDocument}>
              {isDeleting ? TEXT.state.deleting : TEXT.actions.moveToTrash}
            </button>
          </div>
        </div>
      {/if}

      {#if viewMode === "edit"}
        <section class="editor-layout" aria-label={TEXT.aria.editor}>
          <div class="metadata-grid">
            <label>
              <span>{TEXT.labels.title}</span>
              <input bind:value={draftTitle} on:input={markDirty} />
            </label>
            <label class="wide-field">
              <span>{TEXT.labels.tags}</span>
              <input
                list="tag-options"
                bind:value={draftTags}
                on:input={markDirty}
              />
            </label>
          </div>

          <textarea
            bind:this={editorTextarea}
            aria-label={TEXT.aria.markdownSource}
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
        <h2>{TEXT.empty.documentPanelTitle}</h2>
        <p>{TEXT.empty.documentPanelBody}</p>
        <button type="button" on:click={createNewDocument}>{TEXT.actions.newDocument}</button>
      </section>
    {/if}
  </section>
</main>

<datalist id="tag-options">
  {#each tags as tag}
    <option value={tag}></option>
  {/each}
</datalist>
