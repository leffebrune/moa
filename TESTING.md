# Testing Strategy

Moa is a Tauri desktop app with a Svelte/Vite frontend and a Rust storage core.
The fastest feedback loop should avoid packaging the desktop app unless the
change specifically touches Tauri integration.

## Goals

- UI changes should be viewable with Vite hot module reload.
- Storage behavior should be tested in Rust where the filesystem and SQLite
  logic actually live.
- Desktop-only wiring should be checked separately from routine UI iteration.
- Pull requests should have one cheap default command that catches type and
  storage regressions.

## Local Loops

Use this for most frontend and CSS work:

```powershell
npm.cmd run dev:web
```

Open the Vite URL, usually `http://127.0.0.1:1420`. When the app is not running
inside Tauri, the frontend uses an in-memory mock storage adapter with sample
documents. This makes layout, search, editing, filtering, empty states, and save
state changes inspectable without rebuilding or launching the desktop shell.

Use this when validating real desktop integration:

```powershell
npm.cmd run dev:tauri
```

Use this before committing:

```powershell
npm.cmd test
```

That runs Svelte/TypeScript checks and Rust storage tests.

## Test Layers

### 1. Type and Component Compile Checks

Command:

```powershell
npm.cmd run check
```

Coverage:

- Svelte template/type errors
- TypeScript API drift between UI and storage adapter
- Basic build-time regressions

Run this during frontend edits and in CI on every pull request.

### 2. Fast UI Manual Testing

Command:

```powershell
npm.cmd run dev:web
```

Coverage:

- Layout at desktop and narrow widths
- View/edit mode switching
- Search details panel
- Tag filters
- Markdown rendering
- Dirty/saving/saved UI states
- Create/delete flows against disposable mock data

This is the replacement for repeatedly building the Tauri app just to inspect
frontend behavior.

### 3. Rust Storage Tests

Command:

```powershell
npm.cmd run test:rust
```

Coverage already present:

- Vault directory creation
- Markdown document creation and indexing
- Save behavior and metadata preservation
- Index rebuild from Markdown files
- Delete-to-trash behavior

Add new Rust tests whenever behavior changes in `src-tauri/src/storage.rs`.

### 4. Tauri Smoke Test

Command:

```powershell
npm.cmd run dev:tauri
```

Run this only for changes that touch:

- Tauri command names or payload shapes
- Vault path initialization
- File dialogs or OS integration
- Real filesystem persistence
- Packaging or permissions

Manual smoke checklist:

- App launches without the mock adapter.
- A real vault initializes.
- Create, edit, save, reload, search, and delete work against disk.
- Closing and reopening keeps the selected vault available.

## Near-Term Automation

The next useful dependency additions are:

- `vitest` for pure TypeScript utilities and storage adapter contract tests.
- `@testing-library/svelte` for component interaction tests once UI state is
  extracted from `App.svelte`.
- `@playwright/test` for browser screenshots and end-to-end flows against
  `npm run dev:web`.

Recommended order:

1. Extract pure helpers from `App.svelte` (`parseTags`, `formatDate`,
   filtering/search mapping) into testable modules.
2. Add Vitest coverage for those helpers and the mock storage contract.
3. Add one Playwright smoke test: load app, create document, edit title, search,
   switch filters, delete document.
4. Add screenshot checks only for stable screens such as document list, editor,
   settings, and empty state.

## CI Baseline

Start with this minimum gate:

```powershell
npm.cmd test
npm.cmd run build
```

Add Playwright to CI after the browser tests exist and the mock adapter contract
is stable.
