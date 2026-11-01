# Architecture

NEXUS has two halves. A React interface renders state, and a Rust core owns everything that touches the system. They talk over Tauri IPC: commands for requests, events for streaming.

```text
React screens ── stores (Zustand) ── lib/ipc.ts ══ Tauri IPC ══ commands/ ─┬─ system/   IOKit, Mach, sysctl
                                                                          ├─ storage/  scanner, explorer
             events: telemetry, scan progress, cleanup progress           ├─ cleaner/  catalog, Docker
                                                                          ├─ duplicates, apps, advisor
                                                                          ├─ plan.rs ── safety.rs
                                                                          └─ db.rs     SQLite
```

## Threading

- Short commands use `#[tauri::command(async)]`, which keeps them off the main thread.
- Long jobs return immediately and report through events. `jobs.rs` allows one job per kind and gives each a cancel token.
- Filesystem work runs on a bounded rayon pool of two to eight threads at utility QoS, so foreground apps stay responsive.
- Telemetry is a single utility-QoS thread on a two-second tick. Slow probes run less often: battery every 20 seconds, temperature every 10, Apple's battery profile hourly.

## Storage scanner

`storage/scanner.rs` walks each approved root in parallel.

- **Size** is allocated blocks, not logical length, so sparse files such as `Docker.raw` report what they really use.
- **Links:** symlinks are measured as themselves and never followed. Hard links count once. A visited set of `(device, inode)` breaks any directory cycle.
- **Mounts:** the walk stays on one device.
- **Memory is bounded per directory.** Each keeps its 96 largest children and folds the rest into a "Smaller items" node. A scan of 5.7 million files persists as a tree of under 1 MB.
- **Cache:** directories with many small files are remembered by mtime in `dir_cache`. A rescan reuses them but still visits their subfolders, so deeper changes are seen. Reused entries are carried forward, so the cache survives each save. Deep rescan ignores the cache.
- **Failure:** an unreadable entry is counted, not fatal, and appears in the summary.

## Destructive operations

Every removal in the app follows the same path.

```text
UI selection ─▶ prepare_* command ─▶ plan::build ─▶ safety::validate ─▶ Plan held in Rust
                                                                            │
UI shows preview ◀──────────────────────────────────────────────────────────┘
       │ hold to confirm
       ▼
execute_plan(id, confirmed) ─▶ safety::validate again ─▶ Trash / delete / Docker prune ─▶ audit log
```

`safety.rs` applies a policy according to how a path became a candidate.

| Policy | Must satisfy | Allowed action |
| --- | --- | --- |
| `CleanupTarget` | Inside a location from `cleaner/catalog.rs` | Trash, or delete if low risk and enabled |
| `Leftover` | Directly in a Library folder, third-party reverse-DNS name | Trash |
| `UserSelected` | Inside an approved folder | Trash |
| `Application` | App bundle, or a direct Library entry NEXUS associated with it | Trash |

Parents are resolved and the item itself is not, which keeps a symlink swapped in after planning from redirecting the operation. Absolute protections run before any policy. Browser profile folders are default-deny: only recognised cache folders inside them can be reached.

A running plan can be stopped: the current item finishes, and the rest are logged as skipped.

After a run, `reconcile.rs` subtracts what the cleanup log says was removed from the held scan, cleaner and duplicate results, so screens stay right without a rescan. Items too small to have their own tree node are estimated from the plan, so the summary records `adjusted_at` and the UI labels it until the next scan.

An item belongs to one cleaner target. A possible leftover is removed from preselected targets, so anything needing review stays review-only.

## Health and advice

`system/health.rs` scores storage, memory, CPU, battery and performance. Memory uses the kernel's pressure signal, not usage, because cached memory is not a problem. A Mac without a battery is scored on the remaining signals. The overall score is capped relative to the worst subsystem, so one failing part cannot be hidden by healthy ones.

`advisor.rs` turns local facts into recommendations with deterministic rules. Each one routes to a screen, where the user reviews and confirms. Nothing in the advisor can act on its own.

`AdvisorProvider` is the extension point for an optional LLM. A provider receives recommendations, never file contents, and can reword the summary but cannot add actions.

## Data

SQLite lives in the app data folder, with migrations in `src-tauri/migrations` tracked by `PRAGMA user_version`. It holds settings, scan history, the latest scan tree, the directory cache, the cleanup audit log and five-minute metric rollups. Retention is configurable.

## Frontend

- `lib/ipc.ts` is the only code that calls the backend. `lib/types.ts` mirrors the Rust types.
- Screens read Zustand stores. `bootstrap()` loads initial state and subscribes to events once.
- `usePlan` drives the single deletion preview used by every destructive flow.
- Themes and accents are CSS variables on the root element. Component classes such as `.glass` live in `@layer components`, because unlayered CSS beats every Tailwind utility and would silently cancel classes like `fixed`.
- Ambient motion runs in pulses. Continuous animation kept WebKit at 60 Hz and measured about 18% of a core while idle, against under 1% with it off. `useAmbient` runs motion for 6 seconds in every 24 (every 60 when reduced), and only while the window has focus. Off and the system's reduced-motion preference disable it.
- The Holographic Map is DOM tiles, capped in number so each stays a real element for hover and context menus. Holo mode uses CSS 3D transforms.
- Category colors are validated for color-vision deficiency and bound to the category, never to rank. Charts carry direct labels or legends, so identity is never color alone.

## Testing

| Layer | Covers |
| --- | --- |
| Rust unit tests | Safety rules, symlink swaps, plan execution, scanner edge cases, duplicates, health, advisor, migrations |
| Vitest | Treemap geometry, formatting, scoring |
| `e2e/run.sh` | The real binary and webview on a disposable disk image, with disk assertions and on-screen checks for every drawer. Docker uses a stand-in, and guards stop test runs reaching real Docker or Finder's Empty Trash |
| `examples/probe.rs` | Read-only check of native collectors on real hardware |
