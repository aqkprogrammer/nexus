<div align="center">

# NEXUS

**Understand. Optimize. Evolve.**

A macOS intelligence platform: storage analyzer, safe system cleaner, duplicate finder, performance monitor and battery tool in one native desktop app.

Tauri 2 · Rust · React 19 · TypeScript · SQLite · macOS 13.3+

</div>

![NEXUS Command Center](docs/screenshots/02-command-center.webp)

NEXUS runs entirely on your Mac. There is no account, server or analytics, and file contents never leave the machine. Scanning changes nothing, and nothing is removed without a preview you confirm.

> **About these screenshots:** they use the built-in demo dataset so that no real machine's files or apps are published, which is why each one shows a "mock data" banner. The desktop app itself always shows live readings, and the demo data is excluded from production builds.

## Contents

- [Features](#features)
- [Product flows](#product-flows)
- [Tech stack](#tech-stack)
- [Architecture](#architecture)
- [Data model](#data-model)
- [IPC reference](#ipc-reference)
- [Design system](#design-system)
- [Safety model](#safety-model)
- [Privacy and permissions](#privacy-and-permissions)
- [Performance](#performance)
- [Getting started](#getting-started)
- [Testing](#testing)
- [Data accuracy](#data-accuracy)
- [Project structure](#project-structure)
- [Project status](#project-status)

## Features

| Screen | What you get |
| --- | --- |
| **Command Center** | Health score across five subsystems, ten live metric cards with history, system overview |
| **Storage Intelligence** | Category breakdown, Holographic Map treemap, file explorer, largest files |
| **Space Hogs** | Large videos, installers, archives, VMs, Docker, DerivedData, `node_modules`, build output, backups, unused downloads, AI models |
| **Optimization Center** | One prioritized action list with an optimization score and a preview for each action |
| **Smart Cleaner** | Scan, inspect, select, preview, confirm, recover |
| **Duplicate Finder** | Byte-identical files confirmed by a BLAKE3 hash of the full contents |
| **Performance Monitor** | Total and per-core CPU, memory pressure and composition, disk I/O, process manager |
| **Battery Intelligence** | Condition, maximum capacity, cycles, temperature, power flow, charge history |
| **Applications** | Bundle and data size, version, bundle ID, associated files, uninstall review |
| **NEXUS AI** | Local, deterministic recommendations with cause, impact, estimate and risk |
| **Scan History** | Health, storage, CPU and memory trends, an item-level cleanup log, health reports |
| **Settings** | Five themes, accent colors, animation intensity, exclusions, cleanup, privacy, retention |

Across the app: command palette and global search (`⌘K`), keyboard shortcuts, context menus, native notifications, toasts, dry-run mode, cancellable scans, and skeleton, empty, error and permission states.

| Shortcut | Action |
| --- | --- |
| `⌘K` | Command palette and file search |
| `⌘1` to `⌘9` | Jump to a screen |
| `⌘B` | Collapse the sidebar |
| `⌘,` | Settings |
| `Esc` | Close the current menu or preview |

## Product flows

### 1. First run

Nothing is read until you approve it. The dashboard works with no folder access at all.

| Choose what NEXUS may read | Live Command Center |
| --- | --- |
| ![Folder approval on first launch](docs/screenshots/01-welcome.webp) | ![Command Center](docs/screenshots/02-command-center.webp) |

Every metric card shows its current value, a history chart, a status and when it was last updated. A value the Mac doesn't report reads "Unavailable", as CPU temperature does here, rather than an invented number.

![Live metric cards and NEXUS AI summary](docs/screenshots/03-live-metrics.webp)

### 2. Understand your storage

Start a scan, watch the live discovery feed, then explore the result. Click any block to drill down, and open any item to reveal it in Finder or review moving it to the Trash.

| 1. Ready to scan | 2. Deep scan with live discovery |
| --- | --- |
| ![Storage ready to scan](docs/screenshots/04-storage-ready.webp) | ![Deep scan radar and discovery feed](docs/screenshots/05-deep-scan.webp) |

| 3. Storage overview | 4. Holographic Map |
| --- | --- |
| ![Storage overview by category](docs/screenshots/06-storage-overview.webp) | ![Treemap of storage](docs/screenshots/07-holographic-map.webp) |

| 5. Holographic 3D mode | 6. Drill down |
| --- | --- |
| ![Tilted 3D storage map](docs/screenshots/08-holographic-3d.webp) | ![Drilled into the Downloads folder](docs/screenshots/09-map-drilldown.webp) |

| 7. Item details | 8. Explorer |
| --- | --- |
| ![Item details drawer](docs/screenshots/10-item-details.webp) | ![File and folder explorer](docs/screenshots/11-explorer.webp) |

| 9. Largest files | 10. Space Hogs |
| --- | --- |
| ![Largest files list](docs/screenshots/12-largest-files.webp) | ![Space Hogs categories](docs/screenshots/13-space-hogs.webp) |

### 3. Clean safely

Every destructive action in the app uses this same flow. You always see exactly what will happen, and confirming means holding the button, so a stray click can't run a cleanup.

| 1. Start | 2. Read-only analysis |
| --- | --- |
| ![Smart Cleaner start](docs/screenshots/15-cleaner-start.webp) | ![Cleaner scan in progress](docs/screenshots/16-cleaner-scanning.webp) |

| 3. Categorized results | 4. Inspect individual items |
| --- | --- |
| ![Cleaner results by category](docs/screenshots/17-cleaner-results.webp) | ![Expanded category with item checkboxes](docs/screenshots/18-cleaner-inspect.webp) |

| 5. Deletion preview | 6. Hold to confirm |
| --- | --- |
| ![Deletion preview dialog](docs/screenshots/19-deletion-preview.webp) | ![Hold-to-confirm button in progress](docs/screenshots/20-hold-to-confirm.webp) |

| 7. Space recovered | 8. Optimization Center |
| --- | --- |
| ![Cleanup result](docs/screenshots/21-cleanup-complete.webp) | ![Prioritized actions and optimization score](docs/screenshots/14-optimization-center.webp) |

### 4. Remove duplicates

Files are compared by content, not name. Nothing is preselected, and a selection that would remove every copy of a file is refused.

| 1. Choose folders | 2. Size, sample, then full hash |
| --- | --- |
| ![Duplicate scan setup](docs/screenshots/22-duplicates-setup.webp) | ![Duplicate scan in progress](docs/screenshots/23-duplicates-scanning.webp) |

| 3. Groups and recoverable space | 4. Compare copies |
| --- | --- |
| ![Duplicate groups](docs/screenshots/24-duplicates-results.webp) | ![Compare drawer with copies](docs/screenshots/25-duplicates-compare.webp) |

### 5. Monitor the system

Telemetry updates every two seconds. Memory is judged by the kernel's pressure signal, because cached memory isn't wasted.

| Performance Monitor | Process manager |
| --- | --- |
| ![CPU, memory and disk telemetry](docs/screenshots/26-performance.webp) | ![Process table](docs/screenshots/27-process-manager.webp) |

| Process details | Battery Intelligence |
| --- | --- |
| ![Process details with quit actions](docs/screenshots/28-process-details.webp) | ![Battery health and history](docs/screenshots/29-battery.webp) |

### 6. Manage applications

| Applications | Associated files and uninstall review |
| --- | --- |
| ![Installed applications](docs/screenshots/30-applications.webp) | ![Application details drawer](docs/screenshots/31-app-details.webp) |

### 7. Get advice and track change

NEXUS AI only proposes. Each recommendation opens the screen where you review and confirm, and it can't delete, stop or change anything itself.

| NEXUS AI | Scan History |
| --- | --- |
| ![Recommendations](docs/screenshots/32-nexus-ai.webp) | ![Trends and cleanup log](docs/screenshots/33-scan-history.webp) |

### 8. Make it yours

| Appearance | Privacy |
| --- | --- |
| ![Theme and accent settings](docs/screenshots/34-settings.webp) | ![Privacy settings](docs/screenshots/35-settings-privacy.webp) |

| Cleanup preferences | Command palette |
| --- | --- |
| ![Cleanup settings](docs/screenshots/36-settings-cleanup.webp) | ![Command palette](docs/screenshots/37-command-palette.webp) |

| Arctic Blue | Cyber Violet |
| --- | --- |
| ![Arctic Blue theme](docs/screenshots/theme-arctic.webp) | ![Cyber Violet theme](docs/screenshots/theme-violet.webp) |

| Obsidian | Holographic |
| --- | --- |
| ![Obsidian theme](docs/screenshots/theme-obsidian.webp) | ![Holographic theme](docs/screenshots/theme-holographic.webp) |

## Tech stack

### Frontend

| Technology | Version | Used for |
| --- | --- | --- |
| [React](https://react.dev) | 19.3 | Interface |
| [TypeScript](https://www.typescriptlang.org) | 7.0 | Strict typing across the IPC boundary |
| [Vite](https://vite.dev) | 8.3 | Dev server and bundling |
| [Tailwind CSS](https://tailwindcss.com) | 4.3 | Utilities over a CSS-variable design system |
| [Framer Motion](https://motion.dev) | 14.8 | Micro-interactions and transitions |
| [Zustand](https://zustand.docs.pmnd.rs) | 5.0 | State stores |
| [Recharts](https://recharts.org) | 3.10 | Time-series charts |
| [Lucide](https://lucide.dev) | 1.65 | Icons |
| Inter, Space Grotesk, JetBrains Mono | Variable | Bundled locally, so the app makes no font requests |
| [Vitest](https://vitest.dev) | 5.0 | Unit tests |

### Backend

| Technology | Version | Used for |
| --- | --- | --- |
| [Tauri](https://tauri.app) | 2.12 | Native shell, IPC, bundling |
| [Rust](https://www.rust-lang.org) | stable | All system access |
| [rusqlite](https://github.com/rusqlite/rusqlite) | 0.41 (bundled SQLite) | Local database |
| [sysinfo](https://github.com/GuillaumeGomez/sysinfo) | 0.40 | CPU, processes, sensors, volumes |
| [rayon](https://github.com/rayon-rs/rayon) | 1.12 | Bounded parallel filesystem work |
| [blake3](https://github.com/BLAKE3-team/BLAKE3) | 1.8 | Cryptographic file hashing |
| [trash](https://github.com/Byron/trash-rs) | 5.2 | Moves items to the Trash through `NSFileManager` |
| [plist](https://github.com/ebarnard/rust-plist) | 1.10 | Reads `Info.plist` and `diskutil` output |
| [objc2](https://github.com/madsmtm/objc2) | 0.6 | `NSWorkspace` and `NSProcessInfo` |
| [io-kit-sys](https://github.com/jtakakura/io-kit-rs) and [core-foundation](https://github.com/servo/core-foundation-rs) | 0.5, 0.10 | IOKit registry access |
| Tauri plugins | 2.x | Native dialogs, notifications, launch at login |

### macOS integrations

| Interface | Provides |
| --- | --- |
| IOKit `AppleSmartBattery` and `AppleSmartBatteryPack` | Charge, capacity, cycles, temperature, power |
| IOKit `IOBlockStorageDriver` | Disk read and write throughput |
| Mach `host_statistics64` | App, wired, compressed and cached memory |
| `sysctl` | Memory pressure, core counts, CPU brand, OS build |
| `proc_pid_rusage` | Activity Monitor's physical footprint for each process |
| `NSProcessInfo` | Thermal state |
| `NSWorkspace` | Application icons |
| `system_profiler` | Model, chip, Apple's battery condition and maximum capacity |
| `diskutil` | Drive name, SMART status, file system, encryption |
| Spotlight (`mdfind`) | Fast file search |
| Finder (`open -R`, Apple Events) | Reveal in Finder, Empty Trash fallback |
| `sips` | App icon and image preview conversion |
| Docker CLI | Reclaimable space and prune, through Docker itself |

## Architecture

NEXUS has two halves. A React interface renders state, and a Rust core owns everything that touches the system. The webview has no filesystem access of its own: its [capabilities](src-tauri/capabilities/default.json) are limited to window control, native dialogs, notifications and login items, and all file work goes through validated Rust commands.

```mermaid
flowchart TB
    subgraph UI["WebView: React 19"]
        Screens["Feature screens"]
        Stores["Zustand stores"]
        IPC["lib/ipc.ts: typed client"]
        Screens <--> Stores
        Stores <--> IPC
    end

    subgraph Core["Rust core"]
        Commands["commands/: 43 IPC commands"]
        Jobs["jobs.rs: cancellation and bounded worker pool"]
        Telemetry["system/telemetry: 2 second sampler"]
        Scanner["storage/: scanner and explorer"]
        Cleaner["cleaner/: target catalog and Docker"]
        Dupes["duplicates.rs: BLAKE3 pipeline"]
        Apps["apps/: bundles and leftovers"]
        Advisor["advisor.rs: rules engine"]
        Plan["plan.rs: preview and execution"]
        Safety["safety.rs: path validation"]
        DB["db.rs: SQLite"]
    end

    subgraph Mac["macOS"]
        Native["IOKit, Mach, sysctl, AppKit"]
        FS["Filesystem and Trash"]
        Tools["system_profiler, diskutil, mdfind, Docker"]
    end

    IPC <-->|"commands"| Commands
    Telemetry -->|"events"| IPC
    Scanner -->|"progress events"| IPC
    Commands --> Scanner & Cleaner & Dupes & Apps & Advisor & Plan & DB
    Commands --> Jobs
    Plan --> Safety
    Plan --> DB
    Plan --> FS
    Telemetry --> Native
    Scanner --> FS
    Cleaner --> Tools
    Apps --> Native
```

### Threading

- Short commands run off the main thread through `#[tauri::command(async)]`.
- Long jobs return immediately and report through events. `jobs.rs` allows one job per kind and gives each a cancel token.
- Filesystem work uses a bounded rayon pool of two to eight threads at utility QoS, which keeps foreground apps responsive during a whole-disk scan.
- Telemetry is one utility-QoS thread on a two-second tick. Slower probes run less often: temperature every 10 seconds, battery every 20, volume every 15, Apple's battery profile hourly.

### Storage scan

```mermaid
flowchart LR
    A["Approved folders"] --> B["Parallel walk with lstat"]
    B --> C{"Entry type"}
    C -->|"Symlink"| D["Measure the link, never follow"]
    C -->|"File"| E["Allocated blocks, hard links once"]
    C -->|"Directory"| F{"Same device, not excluded, not visited?"}
    F -->|"Yes"| G{"Unchanged since last scan?"}
    G -->|"Yes"| H["Reuse cached listing"]
    G -->|"No"| B
    H --> B
    E --> I["Classify category and hogs"]
    I --> J["Keep the largest 96 per folder"]
    J --> K["Summary and pruned tree in SQLite"]
```

- **Sizes are allocated blocks**, not logical length, so a sparse `Docker.raw` reports what it really occupies.
- **Loops can't happen.** Symlinks are never followed, and a visited set of device and inode pairs breaks any directory cycle.
- **Memory is bounded per folder.** Each keeps its 96 largest children and folds the rest into "Smaller items" without losing bytes. A scan of 5.7 million files persists as a tree of under 1 MB.
- **Rescans skip work.** Folders with many small files are cached by modification time, but their subfolders are still visited, so deeper changes are seen. Deep rescan ignores the cache.
- **Failures are counted, not fatal.** Unreadable items are skipped and reported in the summary.

### Destructive operations

The interface never sends paths at execution time. It receives a plan held in Rust, shows it, and confirms it by ID. What runs is exactly what was validated and shown.

```mermaid
sequenceDiagram
    actor User
    participant UI as React UI
    participant Cmd as Commands
    participant Plan as plan.rs
    participant Safety as safety.rs
    participant Disk as Filesystem and Trash
    participant Log as SQLite audit log

    User->>UI: Select items, then Review
    UI->>Cmd: prepare_cleanup(item ids)
    Cmd->>Plan: Resolve ids from the held scan
    Plan->>Safety: Validate every path
    Safety-->>Plan: Allowed, or blocked with a reason
    Plan-->>UI: Plan held in Rust, plus preview
    UI-->>User: Items, actions, totals, warnings
    User->>UI: Hold to confirm
    UI->>Cmd: execute_plan(plan id, confirmed)
    loop For each item
        Cmd->>Safety: Validate again, just before acting
        alt Dry run
            Safety-->>Log: Record what would happen
        else Allowed
            Safety->>Disk: Trash, or delete if permitted
            Disk-->>Log: Record the outcome
        else Blocked or gone
            Safety-->>Log: Skip and record why
        end
    end
    Cmd-->>UI: Result and space recovered
```

Plans expire after 15 minutes, because a preview of an older disk state shouldn't be trusted.

### Duplicate detection

```mermaid
flowchart LR
    A["Selected folders"] --> B["Index files above minimum size"]
    B --> C["Skip placeholders, packages, Library, .git, node_modules"]
    C --> D["Group by exact size"]
    D --> E["Remove hard links to the same inode"]
    E --> F["Hash first and last 16 KB"]
    F --> G["Full BLAKE3 hash of remaining candidates"]
    G --> H["Groups, sorted by recoverable space"]
```

iCloud placeholders are skipped so a scan never triggers a download. Package formats such as `.app` and `.photoslibrary` are treated as opaque, since removing a file inside one can break it.

### Health and advice

`system/health.rs` scores storage, memory, CPU, battery and performance from 0 to 100, then combines them with weights of 30, 25, 15, 15 and 15 percent. A Mac without a battery is scored on the remaining signals. The overall score is capped at 35 points above the weakest subsystem, so one failing part can't be hidden by four healthy ones.

`advisor.rs` turns local facts into recommendations using deterministic rules. Each one has an issue, why it matters, the impact, an estimate, an action, a risk level and a destination screen.

`AdvisorProvider` is the extension point for an optional LLM. A provider receives recommendations, never file contents, and can reword the summary but cannot add actions. None is connected today.

### Frontend

- `src/lib/ipc.ts` is the only code that calls the backend, and `src/lib/types.ts` mirrors the Rust types.
- Screens read Zustand stores, and `bootstrap()` loads initial state and subscribes to events once.
- `usePlan` drives the single deletion preview used by every destructive flow.
- A screen-level error boundary keeps a failure in one screen from taking down the app.

More detail is in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Data model

SQLite lives in the app data folder. Migrations are in [src-tauri/migrations](src-tauri/migrations), tracked by `PRAGMA user_version`.

```mermaid
erDiagram
    scans ||--o| scan_trees : "latest result"
    cleanup_runs ||--o{ cleanup_items : "audit log"

    settings {
        TEXT key PK
        TEXT value
    }
    scans {
        INTEGER id PK
        TEXT kind
        TEXT status
        INTEGER finished_at
        INTEGER total_bytes
        INTEGER reclaimable
        REAL health_score
    }
    scan_trees {
        INTEGER scan_id PK
        BLOB tree
    }
    dir_cache {
        TEXT path PK
        INTEGER mtime_ns
        INTEGER direct_size
        TEXT subdirs
    }
    cleanup_runs {
        INTEGER id PK
        TEXT source
        INTEGER dry_run
        INTEGER bytes_freed
        INTEGER bytes_trashed
    }
    cleanup_items {
        INTEGER id PK
        INTEGER run_id FK
        TEXT path
        TEXT action
        TEXT outcome
    }
    metric_samples {
        INTEGER ts PK
        REAL cpu
        INTEGER memory_used
        INTEGER disk_used
        REAL health
    }
```

| Table | Purpose |
| --- | --- |
| `settings` | Preferences and approved folders, as JSON |
| `scans` | One row per completed storage, cleaner or duplicate scan |
| `scan_trees` | The latest storage tree, so the map survives a restart |
| `dir_cache` | Listings of unchanged folders, to speed up rescans |
| `cleanup_runs` | One row per cleanup, including dry runs |
| `cleanup_items` | What happened to every item in a cleanup |
| `metric_samples` | Five-minute rollups for long-term trends |

Retention is configurable, and history can be cleared from Settings.

## IPC reference

### Commands

| Area | Commands |
| --- | --- |
| Settings and access | `get_settings`, `save_settings`, `approve_root`, `revoke_root`, `get_permissions`, `open_privacy_settings` |
| Jobs | `cancel_job`, `job_running` |
| System | `get_overview`, `get_telemetry`, `get_history`, `get_metric_trends` |
| Processes | `list_processes`, `terminate_process`, `open_activity_monitor` |
| Storage | `start_storage_scan`, `get_storage_summary`, `get_storage_tree`, `list_directory`, `measure_directory`, `search_files`, `reveal_in_finder` |
| Cleaner | `start_cleaner_scan`, `get_cleaner_report` |
| Duplicates | `start_duplicate_scan`, `get_duplicate_report`, `image_preview` |
| Applications | `list_apps`, `app_icon`, `app_associated` |
| Plans | `prepare_cleanup`, `prepare_empty_trash`, `prepare_paths_plan`, `prepare_duplicate_plan`, `prepare_uninstall`, `discard_plan`, `execute_plan` |
| Intelligence | `get_advisor_report` |
| History and reports | `list_scans`, `list_cleanup_runs`, `get_cleanup_items`, `clear_history`, `export_report` |

### Events

| Event | Payload |
| --- | --- |
| `telemetry://tick` | Full telemetry sample, every two seconds |
| `storage://progress`, `storage://done` | Counters, current path, discoveries, final summary |
| `cleaner://progress`, `cleaner://done` | Percent, phase, findings, final report |
| `duplicates://progress`, `duplicates://done` | Phase, hashing progress, final report |
| `cleanup://progress` | Items done, current path, bytes processed |
| `apps://size`, `apps://done` | Size of each application as it is measured |

Errors serialize as `{ kind, message }`. Kinds are `io`, `database`, `serialization`, `safety`, `permission`, `not_found`, `invalid`, `busy`, `unavailable` and `cancelled`.

## Design system

The look aims for an Apple-quality control room: deep navy surfaces, a restrained neon accent, luminous edges and fine grids.

| Element | Detail |
| --- | --- |
| Themes | NEXUS Dark (default), Arctic Blue, Cyber Violet, Obsidian, Holographic |
| Accents | Cyan, blue, violet, magenta, emerald |
| Type | Inter for the interface, Space Grotesk for headings, JetBrains Mono for metrics |
| Surfaces | Layered panels with a gradient edge, defined as CSS variables on the root element |
| Background | Grid, ambient lighting, scan line and sparse particles |
| Motion | Full, Reduced or Off, and it respects the system's reduced-motion setting |

**Charts:** category colors are checked for color-vision deficiency on the dark surface and stay bound to the category, never its rank. Charts always carry direct labels or legends, and status is never shown by color alone.

**Holographic Map:** a squarified treemap (Bruls, Huizing, van Wijk) drawn as a capped number of DOM tiles, so each block stays a real element for hover and context menus. 3D mode tilts the plane with CSS transforms and lifts tiles by their share of the parent.

**Ambient motion runs in pulses.** Continuous animation kept WebKit at 60 Hz and measured about 18% of a core while idle, against under 1% with it off. Motion now runs for 6 seconds in every 24, and only while the window has focus.

## Safety model

NEXUS is built so that a bug in the interface cannot delete the wrong file.

- **Scanning never changes anything.** It reads names, sizes and dates. File contents are read only to hash them in Duplicate Finder.
- **Nothing is read without approval.** Read and write access are limited to folders you approve. The whole disk can't be approved.
- **Previews are binding.** The interface confirms a plan by ID and never supplies paths at execution time.
- **Paths are checked twice,** when the plan is built and again immediately before each item is touched.
- **Symlinks can't redirect anything.** Parents are resolved and the item isn't, so a link is acted on as a link, even if it was swapped in after the preview.
- **The Trash is the default.** Permanent deletion is opt-in and applies only to low-risk catalogued caches.
- **Duplicates always keep a copy.** A selection covering every copy of a file is refused by the backend.
- **Age never decides.** Old files are shown as a hint and never preselected.
- **Every run is logged,** dry runs included, item by item.

### Policies

| Policy | How a path qualifies | Allowed action |
| --- | --- | --- |
| `CleanupTarget` | Inside a location from [the target catalog](src-tauri/src/cleaner/catalog.rs) | Trash, or delete if low risk and enabled |
| `Leftover` | Directly in a Library folder, with a third-party bundle-ID name | Trash only |
| `UserSelected` | Inside an approved folder | Trash only |
| `Application` | An app bundle, or a Library entry NEXUS associated with it | Trash only |

### Always protected

These are refused under every policy:

- macOS system paths: `/System`, `/usr`, `/bin`, `/sbin`, `/etc`, `/Library`, and everything in `/private` except the temporary areas
- The disk root, whole volumes, your home folder and the standard folders themselves
- Keychains, `.ssh`, `.gnupg`, `.aws`, `.kube` and `.netrc`
- Mail, Messages, Safari data, Calendars, Reminders, Photos, Contacts, iOS backups and password manager data
- Browser passwords, cookies, history and sessions: inside a browser profile, only recognised cache folders can be reached at all
- NEXUS's own data

### Processes

System processes, other users' processes and NEXUS itself can't be ended. Before signaling, NEXUS rechecks the process and its start time, which guards against a reused PID.

## Privacy and permissions

- **Local-first:** no account, no server, no telemetry, no analytics.
- **No file contents leave the Mac.** No AI provider is connected.
- **Network:** the [content security policy](src-tauri/tauri.conf.json) restricts the webview to the app itself and Tauri IPC.

NEXUS can't run in the App Sandbox, because a storage analyzer has to read outside its own container. It relies on macOS privacy controls instead.

| Permission | When it's needed |
| --- | --- |
| Desktop, Documents, Downloads, other apps' data | macOS may ask the first time each is read |
| Full Disk Access | Optional: only to measure the Trash and data macOS otherwise hides. Without it those items are reported as skipped |
| Automation (Finder) | Only for the Empty Trash fallback when the Trash can't be read directly |

## Performance

Measured on an Apple M4 MacBook Pro with 16 GB of RAM.

| Measure | Result |
| --- | --- |
| Full home scan, debug build | 5.74 million files, 422 GB, in 43 seconds |
| Persisted scan tree | Under 1 MB |
| Idle CPU, window focused | About 6.5% of one core, averaged over 48 seconds |
| Idle CPU, unfocused | About 2% of one core |
| Idle CPU, animation off | Under 1% of one core |
| Memory, including the webview | About 190 MB |
| App bundle | About 9 MB |
| Disk image | About 5 MB |

## Getting started

### Requirements

- macOS 13.3 or later
- Xcode Command Line Tools: `xcode-select --install`
- [Rust](https://rustup.rs) stable
- Node 20 or later, and [pnpm](https://pnpm.io)

### Run

```bash
git clone https://github.com/aqkprogrammer/nexus.git
```

```bash
cd nexus
```

```bash
pnpm install
```

```bash
pnpm tauri dev
```

### Build

```bash
pnpm tauri build
```

This writes `NEXUS.app` and a `.dmg` to `src-tauri/target/release/bundle/`. Local builds are ad-hoc signed. To distribute to other Macs you need a Developer ID certificate and notarization, and without them macOS may also ask for permissions again after each rebuild.

### Work on the interface

```bash
pnpm dev
```

This opens the interface in a browser at `http://localhost:1420` with demo data from `src/lib/mock.ts`, and a banner says so. Add `?fresh` to the URL to preview first run.

### Troubleshooting

If `rustc` fails with a `dyld` "Symbol not found" error, Homebrew's Rust and LLVM are out of step:

```bash
brew upgrade rust
```

## Testing

| Layer | Count | Covers |
| --- | --- | --- |
| Rust unit tests | 131, plus 1 opt-in | Safety rules, symlink swaps, plan execution, scanner edge cases, duplicates, health, advisor, migrations |
| Frontend unit tests | 17 | Treemap geometry, formatting, scoring |
| End-to-end, real app | 26 checks, 15 disk assertions | The real binary and webview in a throwaway home, including a real cleanup |

```bash
pnpm test
```

```bash
pnpm test:rust
```

```bash
pnpm typecheck
```

The end-to-end test drives the real interface against the real backend, then checks what happened on disk. Everything destructive stays inside a throwaway home folder, and its hook only exists in debug builds.

```bash
pnpm tauri build --debug --no-bundle
```

```bash
e2e/run.sh src-tauri/target/debug/nexus /tmp/nexus-e2e
```

To see what the native collectors report on your Mac without changing anything:

```bash
cargo run --manifest-path src-tauri/Cargo.toml --example probe -- ~/Downloads
```

One test puts a tiny file in your real Trash, so it only runs on request:

```bash
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored trash_action
```

## Data accuracy

NEXUS shows "Unavailable" instead of guessing.

- **Health and optimization scores** are NEXUS's own heuristics, not Apple diagnostics.
- **CPU temperature** has no supported macOS API. NEXUS uses what the system's sensors expose, and the thermal state is always available.
- **Other users' processes,** such as `WindowServer`, can report zero CPU and disk because macOS restricts those figures without root.
- **Battery fields** vary by macOS version. NEXUS checks both the current and older IOKit layouts, and takes condition and maximum capacity from Apple's own report.
- **"System & unscanned"** is used space minus what was scanned. It isn't a measurement of macOS itself.
- **Duplicate savings** can be lower than shown, because APFS clones share storage and can't be told apart.
- **Moving to the Trash doesn't free space** until the Trash is emptied, and results say so.
- **Software can't restore battery capacity.** NEXUS never claims otherwise.

## Project structure

```text
nexus/
├── src/                        React frontend
│   ├── components/
│   │   ├── shell/              Sidebar, title bar, palette, deletion preview
│   │   ├── ui/                 Design system primitives
│   │   └── viz/                Reactor, radar, charts, Holographic Map
│   ├── features/               One file per screen
│   ├── lib/                    IPC client, types, treemap, scoring, dev-only mock
│   ├── stores/                 Zustand stores and event wiring
│   └── styles/                 Tokens, themes, effects
├── src-tauri/
│   ├── migrations/             SQLite schema
│   ├── examples/probe.rs       Read-only hardware diagnostic
│   └── src/
│       ├── commands/           IPC surface
│       ├── system/             IOKit, memory, battery, processes, telemetry, health
│       ├── storage/            Scanner, classification, explorer
│       ├── cleaner/            Target catalog, Docker
│       ├── apps/               Bundles, associated files, leftovers
│       ├── safety.rs           Path validation
│       ├── plan.rs             Preview and execution engine
│       ├── duplicates.rs       Hashing pipeline
│       ├── advisor.rs          Recommendation rules
│       └── db.rs               Persistence
├── e2e/                        Real-app smoke test
└── docs/                       Architecture notes and screenshots
```

## Project status

| Phase | Scope | Status |
| --- | --- | --- |
| 1 | Architecture, Tauri app, interface, native system data, Command Center | Done |
| 2 | Storage scanner, explorer, large files, treemap | Done |
| 3 | Smart Cleaner, previews, safe deletion engine, cleanup history | Done |
| 4 | Duplicate Finder, developer cleanup, Applications | Done |
| 5 | CPU and RAM monitoring, process manager, Battery Intelligence | Done |
| 6 | NEXUS AI, recommendations, analytics, reports | Done, with local rules |

**Built but not yet exercised against a real system:** Docker prune, a real application uninstall, Finder's Empty Trash fallback, scheduled scans and launch at login. Each has unit coverage or a safety check where it can, but hasn't run for real.

**Not yet tested:** Intel Macs, and macOS versions other than 27.

**Planned:** an optional LLM provider behind the existing `AdvisorProvider` interface, and a signed, notarized release.
