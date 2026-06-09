# Moa

Local-first Markdown knowledge base app.

## Development

```powershell
npm.cmd install
npm.cmd run check
npm.cmd run tauri dev
```

Use `npm.cmd` on Windows PowerShell if script execution policy blocks `npm`.

## Stack

- Tauri v2
- Rust
- Svelte + TypeScript + Vite
- CodeMirror 6
- markdown-it
- SQLite / FTS5 via rusqlite
