# Moa

Local-first Markdown knowledge base app.

## Development

```powershell
npm.cmd install
npm.cmd run check
npm.cmd run tauri dev
```

Use `npm.cmd` on Windows PowerShell if script execution policy blocks `npm`.

## Storage CLI

Run storage commands from the repository root. The default manual vault is a
repository sibling named `moa-manual-vault`.

```powershell
npm.cmd run storage -- init
npm.cmd run storage -- create --title "Test note" --tags "markdown,local" --body "Searchable body."
npm.cmd run storage -- list
npm.cmd run storage -- search --query "body"
```

Override the vault only when needed:

```powershell
$env:MOA_STORAGE_DEV_VAULT="G:\dev\other-moa-vault"
npm.cmd run storage -- status

npm.cmd run storage -- status --vault G:\dev\other-moa-vault
```

If you want to use `cargo run` directly, run it from `src-tauri`:

```powershell
cd src-tauri
cargo run --bin storage_dev -- status
```

## Stack

- Tauri v2
- Rust
- Svelte + TypeScript + Vite
- CodeMirror 6
- markdown-it
- SQLite / FTS5 via rusqlite
