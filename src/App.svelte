<script lang="ts">
  const categories = ["All notes", "Project", "Research", "Archive"];
  const notes = [
    {
      title: "Local-first note app",
      category: "Project",
      updatedAt: "2026-06-08",
      tags: ["markdown", "sqlite", "fts"],
    },
    {
      title: "Vector search",
      category: "Research",
      updatedAt: "2026-06-08",
      tags: ["embedding", "local"],
    },
  ];
</script>

<main class="shell">
  <aside class="sidebar" aria-label="Navigation">
    <div class="brand">
      <strong>Moa</strong>
      <span>Local Markdown KB</span>
    </div>

    <nav>
      {#each categories as category}
        <button class:active={category === "All notes"}>{category}</button>
      {/each}
    </nav>
  </aside>

  <section class="list-pane" aria-label="Documents">
    <div class="toolbar">
      <input type="search" placeholder="Search notes" />
      <button>New</button>
    </div>

    <div class="notes">
      {#each notes as note}
        <article>
          <div>
            <h2>{note.title}</h2>
            <p>{note.category} - {note.updatedAt}</p>
          </div>
          <div class="tags">
            {#each note.tags as tag}
              <span>{tag}</span>
            {/each}
          </div>
        </article>
      {/each}
    </div>
  </section>

  <section class="editor-pane" aria-label="Document">
    <header>
      <div>
        <input class="title" value="Local-first note app" aria-label="Title" />
        <div class="meta">
          <input value="Project" aria-label="Category" />
          <input value="markdown, sqlite, fts" aria-label="Tags" />
        </div>
      </div>
      <button>Edit</button>
    </header>

    <article class="preview">
      <h1>Local-first note app</h1>
      <p>
        Markdown files are the source of truth. SQLite stores metadata, FTS
        rows, and derived vector indexes that can be rebuilt from the vault.
      </p>
      <h2>Storage</h2>
      <p>
        Notes live under <code>moa-vault/notes</code>. The app should recover
        its index even when <code>.moa/moa.sqlite</code> is deleted.
      </p>
    </article>
  </section>
</main>
