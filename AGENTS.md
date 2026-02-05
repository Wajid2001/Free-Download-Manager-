# AGENTS.md - AI Coding Agent Guidelines

This document provides guidelines for AI coding agents working on the Free Download Manager project.

## Project Overview

A cross-platform desktop download manager built with:

- **Frontend**: React 19, TypeScript, Vite 7, Tailwind CSS v4, shadcn/ui (radix-nova style)
- **Backend**: Rust with Tauri v2
- **Package Manager**: Bun

## Build & Development Commands

### Frontend

```bash
bun install           # Install dependencies
bun run dev           # Start development server (frontend only)
bun run build         # Build frontend for production
bun run preview       # Preview production build
bun run format        # Format code with Prettier
```

### Tauri (Full Application)

```bash
bun run tauri dev     # Start full Tauri development (frontend + backend)
bun run tauri build   # Build production application
```

### Rust Backend

```bash
cargo build --manifest-path src-tauri/Cargo.toml           # Build Rust backend
cargo build --manifest-path src-tauri/Cargo.toml --release # Build release
cargo check --manifest-path src-tauri/Cargo.toml           # Check Rust code
cargo clippy --manifest-path src-tauri/Cargo.toml          # Run clippy lints
```

### Testing

No test framework is currently configured. When adding tests:

- Frontend: Consider Vitest for React component and unit tests
- Backend: Use Rust's built-in `cargo test`

## Code Style Guidelines

### TypeScript/React

#### Formatting (Prettier)

- No semicolons, double quotes, trailing commas in ES5 contexts
- 100 character line width, 2-space indentation, LF line endings

#### Imports

1. React imports first
2. External library imports (alphabetized)
3. Internal imports using `@/` path alias
4. Separate groups with blank lines

```typescript
import { useCallback, useEffect, useState } from "react"
import { openPath, openUrl } from "@tauri-apps/plugin-opener"

import { Button } from "@/components/ui/button"
import { listDownloads, startDownload } from "@/features/downloads/api"
import type { DownloadInfo } from "@/features/downloads/types"
```

#### Component Structure

- Use function declarations for components
- Export named functions (not default exports)
- Hooks at the top of the component
- Event handlers prefixed with `handle`

```typescript
export function DownloadManager() {
  const [downloads, setDownloads] = useState<DownloadInfo[]>([])
  const handleStartDownload = async () => { /* ... */ }
  return <div>...</div>
}
```

#### Naming Conventions

- Components: `PascalCase` (e.g., `DownloadManager`)
- Functions/variables: `camelCase` (e.g., `formatBytes`)
- Types/interfaces: `PascalCase` (e.g., `DownloadInfo`)
- Files: `kebab-case` for UI components, `PascalCase.tsx` for feature components

#### Error Handling

```typescript
try {
  await someAsyncOperation()
} catch (error) {
  setErrorMessage(error instanceof Error ? error.message : "Operation failed")
}
```

### Rust Backend

#### Imports

Group and order: 1) Standard library (`std::`), 2) External crates, 3) Internal modules

```rust
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
```

#### Serde Attributes

Use `camelCase` for JSON serialization to match TypeScript conventions:

```rust
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadInfo {
    pub file_name: String,  // Serializes as "fileName"
}
```

#### Tauri Commands

```rust
#[tauri::command]
pub async fn command_name(
    state: State<'_, DownloadManager>,
    param: String,
) -> Result<ReturnType, String> {
    // Implementation
}
```

#### Error Handling

Return `Result<T, String>` for Tauri commands with descriptive error messages:

```rust
.map_err(|error| format!("Failed to create directory: {error}"))?
```

## Project Structure

```
src/                          # Frontend source
├── components/ui/            # shadcn/ui components
├── features/<feature>/       # Feature modules
│   ├── api.ts               # Tauri invoke calls
│   ├── types.ts             # TypeScript types
│   ├── utils.ts             # Utility functions
│   └── Component.tsx        # Main component
└── lib/utils.ts             # Shared utilities (cn function)

src-tauri/                    # Rust backend
└── src/
    ├── main.rs              # Entry point
    ├── lib.rs               # Tauri setup and command registration
    └── <module>.rs          # Feature modules
```

## Path Aliases

Use `@/` for imports from `src/`:

```typescript
import { Button } from "@/components/ui/button"
```

## UI Components

Using shadcn/ui with radix-nova style. Add components via:

```bash
bunx shadcn@latest add <component>
```

Components are in `src/components/ui/`. Use the `cn()` utility for conditional classes:

```typescript
import { cn } from "@/lib/utils"
className={cn("base-class", condition && "conditional-class")}
```

## Pre-commit Hooks

Husky runs `lint-staged` on commit, which formats staged files with Prettier.
Ensure code is formatted before committing or run `bun run format`.

## TypeScript Configuration

- Strict mode enabled
- No unused locals/parameters allowed
- Path alias: `@/*` maps to `./src/*`
- Target: ES2020
