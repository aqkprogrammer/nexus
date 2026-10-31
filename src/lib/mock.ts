// DEVELOPMENT ONLY. Imported dynamically behind `import.meta.env.DEV` when running in a plain
// browser, so it is excluded from production builds. The desktop app never reaches this file.
import type * as T from "./types";

const GB = 1e9;
const MB = 1e6;
const listeners = new Map<string, Set<(payload: unknown) => void>>();

function emit(event: string, payload: unknown) {
  listeners.get(event)?.forEach((fn) => fn(payload));
}

export async function mockListen(event: string, handler: (payload: unknown) => void) {
  if (!listeners.has(event)) listeners.set(event, new Set());
  listeners.get(event)!.add(handler);
  return () => listeners.get(event)?.delete(handler);
}

/** Add ?fresh to the URL to preview the first-run experience. */
const fresh = new URLSearchParams(window.location.search).has("fresh");

let settings: T.Settings = {
  onboarded: !fresh, theme: "nexus", accent: "cyan", animation: "full", approvedRoots: fresh ? [] : ["/Users/demo", "/Applications"],
  exclusions: ["/Users/demo/Library/CloudStorage"], scanFrequency: "manual", notifications: true, cleanupMode: "trash",
  dryRun: false, tempMinAgeDays: 3, oldFileMonths: 12, recordHistory: true, retentionDays: 180, launchAtLogin: false,
  aiProvider: "none", aiModel: "", aiEndpoint: "", aiShareAggregates: false,
};

const part = (score: number, detail: string) => ({
  score, status: score >= 85 ? "Excellent" : score >= 70 ? "Good" : score >= 50 ? "Fair" : "Attention", detail,
});

let tick = 0;
function sample(): T.TelemetrySample {
  tick += 1;
  const cpu = 18 + 12 * Math.sin(tick / 6) + Math.random() * 8;
  return {
    ts: Date.now(), cpu, perCore: Array.from({ length: 10 }, (_, i) => Math.max(1, cpu + 22 * Math.sin(tick / 4 + i) )),
    loadAverage: [2.41, 2.2, 1.98],
    memory: {
      total: 24 * GB, used: 15.2 * GB, app: 8.9 * GB, wired: 3.4 * GB, compressed: 2.9 * GB, cached: 6.1 * GB, free: 1.2 * GB,
      available: 8.8 * GB, swapUsed: 0.8 * GB, swapTotal: 2 * GB, pressure: "normal", pressureLevel: 62,
    },
    diskReadBps: 2e6 + Math.random() * 6e7, diskWriteBps: 1e6 + Math.random() * 2e7,
    volume: { mountPoint: "/System/Volumes/Data", total: 494 * GB, available: 61 * GB, used: 433 * GB },
    battery: {
      percent: 78, isCharging: false, externalPower: false, fullyCharged: false, powerSource: "Battery", cycleCount: 214,
      designCapacityMah: 6075, maxCapacityMah: 5620, maxCapacityPercent: 92, condition: "Good", temperatureC: 31.4,
      voltageV: 12.31, powerW: -9.2, timeRemainingMin: 312,
    },
    cpuTemperature: null, thermalState: "nominal", processCount: 512, uptime: 412_000,
    health: {
      overall: 81, status: "Good", storage: part(72, "12% of the disk is available."), memory: part(96, "Memory pressure is normal."),
      cpu: part(100, "Averaging 24% over the last minute."), battery: part(86, "Maximum capacity is 92% of design."),
      performance: part(100, "Thermal state is nominal; load is 0.24 per core."),
    },
  };
}

const CATEGORIES: [T.Category, number][] = [
  ["developer", 118 * GB], ["media", 74 * GB], ["documents", 61 * GB], ["applications", 39 * GB], ["downloads", 31 * GB], ["caches", 22 * GB], ["other", 18 * GB],
];

function leaf(name: string, size: number, category: T.Category, kind: T.FileKind = "other"): T.TreeNode {
  return { name, size, files: 1, modified: 1_690_000_000, kind, category };
}

function folder(name: string, category: T.Category, children: T.TreeNode[]): T.TreeNode {
  return {
    name, size: children.reduce((s, c) => s + c.size, 0), files: children.reduce((s, c) => s + c.files, 0),
    modified: 1_720_000_000, kind: "folder", category, children,
  };
}

const TREE = folder("Scanned locations", "other", [
  folder("/Users/demo", "other", [
    folder("Developer", "developer", [
      folder("nexus", "developer", [leaf("node_modules", 3.1 * GB, "developer"), leaf("target", 7.4 * GB, "developer")]),
      folder("platform", "developer", [leaf("node_modules", 11 * GB, "developer"), leaf("dist", 2.2 * GB, "developer")]),
      folder("ml-lab", "developer", [leaf("weights.safetensors", 24 * GB, "developer"), leaf("datasets", 19 * GB, "developer")]),
    ]),
    folder("Library", "other", [
      folder("Caches", "caches", [leaf("com.spotify.client", 3.8 * GB, "caches"), leaf("Google", 6.1 * GB, "caches"), leaf("Homebrew", 4.2 * GB, "caches")]),
      folder("Developer", "developer", [leaf("DerivedData", 16.4 * GB, "developer"), leaf("CoreSimulator", 21 * GB, "developer")]),
      folder("Containers", "other", [leaf("com.docker.docker", 28 * GB, "developer"), leaf("com.apple.mail", 4 * GB, "other")]),
    ]),
    folder("Movies", "media", [leaf("Keynote recap.mov", 18 * GB, "media", "video"), leaf("Iceland.mp4", 9.1 * GB, "media", "video"), leaf("Drone raw.mov", 12.4 * GB, "media", "video")]),
    folder("Pictures", "media", [leaf("Photos Library.photoslibrary", 31 * GB, "media")]),
    folder("Documents", "documents", [folder("Projects", "documents", [leaf("Archive 2021.zip", 12 * GB, "documents", "archive"), leaf("Client assets", 21 * GB, "documents")]), leaf("Taxes", 2.4 * GB, "documents"), leaf("Design system.sketch", 1.1 * GB, "documents")]),
    folder("Downloads", "downloads", [leaf("ubuntu-24.04.iso", 6.2 * GB, "downloads", "diskImage"), leaf("Xcode_16.xip", 11.3 * GB, "downloads", "installer"), leaf("recording.mov", 4.4 * GB, "downloads", "video"), { ...leaf("Smaller items", 3 * GB, "other"), aggregate: true, files: 412 }]),
  ]),
  folder("/Applications", "applications", [leaf("Xcode.app", 14.2 * GB, "applications", "application"), leaf("Docker.app", 2.4 * GB, "applications", "application"), leaf("Google Chrome.app", 1.4 * GB, "applications", "application"), leaf("Figma.app", 0.6 * GB, "applications", "application")]),
]);

function findNode(path: string | null): T.TreeNode | null {
  if (!path) return TREE;
  for (const root of TREE.children!) {
    if (path === root.name) return root;
    if (path.startsWith(root.name)) {
      let node: T.TreeNode | undefined = root;
      for (const seg of path.slice(root.name.length + 1).split("/")) node = node?.children?.find((c) => c.name === seg);
      return node ?? null;
    }
  }
  return null;
}

const file = (path: string, size: number, kind: T.FileKind, category: T.Category): T.FileItem => ({
  path, name: path.split("/").pop()!, size, kind, category, modified: 1_680_000_000, accessed: 1_700_000_000,
});

const summary: T.ScanSummary = {
  startedAt: Date.now() - 300_000, finishedAt: Date.now() - 280_000, roots: settings.approvedRoots, completed: true,
  totalBytes: 363 * GB, files: 2_184_411, directories: 412_081, inaccessible: 38, reusedDirectories: 18_204,
  categories: CATEGORIES.map(([category, bytes]) => ({ category, bytes })),
  over100Mb: [212, 148 * GB], over1Gb: [31, 96 * GB],
  old: { cutoffMonths: 12, bytes: 84 * GB, files: 41_033, downloadsBytes: 18.4 * GB, downloadsFiles: 214 },
  topFiles: [
    file("/Users/demo/Developer/ml-lab/weights.safetensors", 24 * GB, "other", "developer"),
    file("/Users/demo/Movies/Keynote recap.mov", 18 * GB, "video", "media"),
    file("/Users/demo/Movies/Drone raw.mov", 12.4 * GB, "video", "media"),
    file("/Users/demo/Documents/Projects/Archive 2021.zip", 12 * GB, "archive", "documents"),
    file("/Users/demo/Downloads/Xcode_16.xip", 11.3 * GB, "installer", "downloads"),
    file("/Users/demo/Movies/Iceland.mp4", 9.1 * GB, "video", "media"),
    file("/Users/demo/Downloads/ubuntu-24.04.iso", 6.2 * GB, "diskImage", "downloads"),
  ],
  hogs: ([
    ["/Users/demo/Library/Containers/com.docker.docker/Data/vms", "docker", 28 * GB],
    ["/Users/demo/Library/Developer/Xcode/DerivedData", "xcodeDerivedData", 16.4 * GB],
    ["/Users/demo/Movies/Keynote recap.mov", "largeVideo", 18 * GB],
    ["/Users/demo/Movies/Drone raw.mov", "largeVideo", 12.4 * GB],
    ["/Users/demo/Developer/platform/node_modules", "nodeModules", 11 * GB],
    ["/Users/demo/Developer/nexus/node_modules", "nodeModules", 3.1 * GB],
    ["/Users/demo/Developer/nexus/target", "buildArtifact", 7.4 * GB],
    ["/Users/demo/Documents/Projects/Archive 2021.zip", "zipArchive", 12 * GB],
    ["/Users/demo/Downloads/Xcode_16.xip", "oldInstaller", 11.3 * GB],
    ["/Users/demo/Downloads/ubuntu-24.04.iso", "diskImage", 6.2 * GB],
    ["/Users/demo/VMs/ubuntu.utm", "virtualMachine", 22 * GB],
    ["/Users/demo/Downloads/old-export.bin", "unusedDownload", 3.3 * GB],
    ["/Users/demo/.ollama/models", "aiModels", 14 * GB],
    ["/Users/demo/Documents/site-backup", "projectBackup", 5.2 * GB],
  ] as [string, T.HogKind, number][]).map(([path, kind, size]) => ({
    path, name: path.split("/").pop()!, kind, size, files: 1200, modified: 1_680_000_000, accessed: 1_700_000_000, isDir: true,
  })),
};

let scanSummary: T.ScanSummary | null = fresh ? null : summary;

function runStorageScan() {
  let n = 0;
  const handle = setInterval(() => {
    n += 1;
    emit("storage://progress", {
      files: n * 41_000, directories: n * 7_800, bytes: n * 8.2 * GB, inaccessible: n, current: `/Users/demo/Library/Caches/item-${n}/data.bin`,
      discoveries: n % 3 === 0 ? [{ path: `/Users/demo/Movies/clip-${n}.mov`, size: (n + 2) * 140 * MB }] : [],
    } satisfies T.ScanProgress);
    if (n >= 44) {
      clearInterval(handle);
      scanSummary = { ...summary, finishedAt: Date.now() };
      emit("storage://done", { summary: scanSummary });
    }
  }, 120);
}

const item = (id: string, path: string, size: number): T.CleanerItem => ({
  id, path, name: path.split("/").pop()!, size, files: 240, modified: 1_710_000_000, isDir: true,
});

const target = (
  id: string, group: T.CleanerGroup, title: string, description: string, risk: T.Risk, selectedByDefault: boolean, items: T.CleanerItem[],
  appRunning: string | null = null,
): T.CleanerTarget => ({
  id, group, title, description, effect: "This data is recreated automatically when it is next needed.", risk, selectedByDefault,
  policy: "cleanupTarget", items, total: items.reduce((s, i) => s + i.size, 0), appRunning, unavailable: null,
});

const cleanerReport: T.CleanerReport = {
  scannedAt: Date.now() - 60_000, durationMs: 4200, completed: true, reclaimable: 41.3 * GB, recommended: 12.7 * GB,
  targets: [
    target("app-caches", "systemJunk", "Application caches", "Data apps saved in ~/Library/Caches.", "low", true, [
      item("app-caches:0", "/Users/demo/Library/Caches/com.spotify.client", 3.8 * GB),
      item("app-caches:1", "/Users/demo/Library/Caches/com.figma.Desktop", 1.2 * GB),
      item("app-caches:2", "/Users/demo/Library/Caches/com.tinyspeck.slackmacgap", 0.7 * GB),
    ]),
    target("user-logs", "systemJunk", "Log files", "Diagnostic logs written by apps.", "low", true, [item("user-logs:0", "/Users/demo/Library/Logs/JetBrains", 1.1 * GB)]),
    target("crash-reports", "systemJunk", "Crash reports", "Reports from apps that quit unexpectedly.", "low", true, [item("crash-reports:0", "/Users/demo/Library/Logs/DiagnosticReports/Slack.ips", 40 * MB)]),
    target("chrome-cache", "browser", "Chrome cache", "Web and code caches for each profile.", "low", true, [item("chrome-cache:0", "/Users/demo/Library/Caches/Google/Chrome/Default", 5.9 * GB)], "Google Chrome"),
    target("xcode-derived", "developer", "Xcode DerivedData", "Build products and indexes.", "low", false, [
      item("xcode-derived:0", "/Users/demo/Library/Developer/Xcode/DerivedData/Nexus-abc", 9.2 * GB),
      item("xcode-derived:1", "/Users/demo/Library/Developer/Xcode/DerivedData/Demo-def", 7.2 * GB),
    ]),
    target("npm-cache", "developer", "npm cache", "Downloaded package tarballs.", "low", false, [item("npm-cache:0", "/Users/demo/.npm/_cacache", 3.1 * GB)]),
    { ...target("orphan-prefs", "leftovers", "Orphaned preference files", "Settings left by apps no longer installed.", "medium", false, [item("orphan-prefs:0", "/Users/demo/Library/Preferences/com.gone.tool.plist", 24_000)]), policy: "leftover" },
  ],
  trash: { accessible: true, count: 3, total: 4.2 * GB, items: [item("trash:0", "/Users/demo/.Trash/old-project", 3.9 * GB), item("trash:1", "/Users/demo/.Trash/installer.dmg", 0.3 * GB)] },
  docker: {
    installed: true, running: true, message: "",
    resources: [
      { id: "images", label: "Unused images", total: "24", active: "4", size: 18 * GB, reclaimable: 11 * GB },
      { id: "builder", label: "Build cache", total: "120", active: "0", size: 9 * GB, reclaimable: 9 * GB },
      { id: "volumes", label: "Unused volumes", total: "6", active: "2", size: 4 * GB, reclaimable: 1.2 * GB },
    ],
  },
};

let cleaner: T.CleanerReport | null = fresh ? null : cleanerReport;

function runCleanerScan() {
  let n = 0;
  const titles = ["Application caches", "Log files", "Chrome cache", "Xcode DerivedData", "npm cache"];
  const handle = setInterval(() => {
    n += 1;
    emit("cleaner://progress", {
      percent: n * 5, phase: "Analyzing cleanup locations", current: titles[n % titles.length], foundBytes: n * 2 * GB,
      findings: n % 4 === 0 ? [{ title: titles[n % titles.length], bytes: n * 0.4 * GB }] : [],
    } satisfies T.CleanerProgress);
    if (n >= 20) {
      clearInterval(handle);
      cleaner = cleanerReport;
      emit("cleaner://done", { report: cleaner });
    }
  }, 120);
}

const duplicates: T.DuplicateReport = {
  scannedAt: Date.now() - 9_000, durationMs: 8200, completed: true, roots: ["/Users/demo"], filesIndexed: 182_004,
  bytesHashed: 41 * GB, inaccessible: 3, recoverable: 6.4 * GB,
  groups: [
    {
      id: "a1", hash: "9f1c3b7e2d4a5b6c7d8e9f0a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e", size: 3.2 * GB, kind: "video", recoverable: 3.2 * GB,
      files: [
        { path: "/Users/demo/Movies/Iceland.mp4", name: "Iceland.mp4", modified: 1_660_000_000, created: 1_660_000_000 },
        { path: "/Users/demo/Downloads/Iceland (1).mp4", name: "Iceland (1).mp4", modified: 1_700_000_000, created: 1_700_000_000 },
      ],
    },
    {
      id: "b2", hash: "1a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f809", size: 1.6 * GB, kind: "archive", recoverable: 3.2 * GB,
      files: [
        { path: "/Users/demo/Documents/assets.zip", name: "assets.zip", modified: 1_650_000_000, created: 1_650_000_000 },
        { path: "/Users/demo/Desktop/assets.zip", name: "assets.zip", modified: 1_690_000_000, created: 1_690_000_000 },
        { path: "/Users/demo/Downloads/assets.zip", name: "assets.zip", modified: 1_695_000_000, created: 1_695_000_000 },
      ],
    },
  ],
};

let dupes: T.DuplicateReport | null = null;

const proc = (pid: number, name: string, app: string | null, cpu: number, memory: number, opts: Partial<T.ProcessInfo> = {}): T.ProcessInfo => ({
  pid, parentPid: 1, name, app, exe: `/Applications/${app ?? name}.app/Contents/MacOS/${name}`, cpu, memory, diskRead: 120_000, diskWrite: 40_000,
  status: "Running", startTime: 1_700_000_000, runTime: 8400, owned: true, protected: false, ...opts,
});

const processes: T.ProcessInfo[] = [
  proc(811, "Google Chrome", "Google Chrome", 8.4, 1.4 * GB), proc(812, "Google Chrome Helper", "Google Chrome", 11.1, 2.0 * GB),
  proc(1024, "Xcode", "Xcode", 6.2, 2.8 * GB), proc(1302, "Figma", "Figma", 4.1, 1.1 * GB), proc(4410, "node", null, 12.9, 0.9 * GB, { exe: "/opt/homebrew/bin/node" }),
  proc(1, "launchd", null, 0.1, 42 * MB, { owned: false, protected: true, exe: "/sbin/launchd" }),
  proc(201, "WindowServer", null, 5.8, 0.7 * GB, { owned: false, protected: true, exe: "/System/Library/WindowServer" }),
];

const apps: T.AppInfo[] = [
  { name: "Xcode", path: "/Applications/Xcode.app", bundleId: "com.apple.dt.Xcode", version: "16.1", size: 14.2 * GB, dataSize: 42 * GB, modified: 1_730_000_000, isSystem: false, running: true },
  { name: "Docker", path: "/Applications/Docker.app", bundleId: "com.docker.docker", version: "4.34", size: 2.4 * GB, dataSize: 28 * GB, modified: 1_720_000_000, isSystem: false, running: false },
  { name: "Google Chrome", path: "/Applications/Google Chrome.app", bundleId: "com.google.Chrome", version: "130.0", size: 1.4 * GB, dataSize: 6.4 * GB, modified: 1_730_000_000, isSystem: false, running: true },
  { name: "Figma", path: "/Applications/Figma.app", bundleId: "com.figma.Desktop", version: "124.5", size: 0.6 * GB, dataSize: 1.9 * GB, modified: 1_725_000_000, isSystem: false, running: false },
  { name: "Safari", path: "/System/Applications/Safari.app", bundleId: "com.apple.Safari", version: "18.1", size: 0.1 * GB, dataSize: 0.4 * GB, modified: 1_725_000_000, isSystem: true, running: false },
];

const rec = (id: string, domain: string, title: string, issue: string, estimateBytes: number | null, risk: T.Risk, severity: number, route: string, actionLabel: string): T.Recommendation => ({
  id, domain, title, issue, why: "Left alone, this keeps using space you could reclaim.", impact: "Frees storage with limited side effects.",
  estimate: estimateBytes ? `Up to ${(estimateBytes / GB).toFixed(1)} GB` : "Depends on what you review", estimateBytes,
  action: "Review the items and confirm what to remove.", risk, severity, actionLabel, route,
});

const advisor: T.AdvisorReport = {
  generatedAt: Date.now(), engine: "Local rules", summary: "4 recommendations ready, covering up to 47.5 GB of storage.", missingData: ["Run Duplicate Finder to include duplicate files."],
  recommendations: [
    rec("low-disk", "storage", "Disk space is getting low", "Your available storage is 12% (61 GB free of 494 GB).", null, "low", 75, "optimize", "Open Optimization Center"),
    rec("safe-cleanup", "storage", "Low-risk cleanup available", "12.7 GB of caches, logs and temporary files can be cleared.", 12.7 * GB, "low", 60, "cleaner", "Open Smart Cleaner"),
    rec("old-downloads", "storage", "Old files in Downloads", "Your Downloads folder contains 18.4 GB across 214 files that have not been modified in more than 12 months.", 18.4 * GB, "medium", 55, "hogs", "Review in Space Hogs"),
    rec("derived-data", "developer", "Xcode DerivedData is large", "Xcode DerivedData is consuming 16.4 GB.", 16.4 * GB, "low", 50, "cleaner", "Open Smart Cleaner"),
    rec("heavy-app-google-chrome", "performance", "Google Chrome is using a lot of memory", "Google Chrome is currently consuming 3.4 GB of RAM across 2 processes.", null, "low", 35, "performance", "Open Performance Monitor"),
  ],
};

const entry = (name: string, base: string, size: number | null, isDir: boolean, kind: T.FileKind = isDir ? "folder" : "other"): T.DirEntry => ({
  name, path: `${base}/${name}`, isDir, isSymlink: false, isHidden: name.startsWith("."), size, files: isDir ? 1200 : null, kind,
  modified: 1_690_000_000, accessed: 1_720_000_000, created: 1_650_000_000,
});

function listing(path: string): T.Listing {
  const node = findNode(path);
  const entries = node?.children
    ? node.children.filter((c) => !c.aggregate).map((c) => entry(c.name, path, c.size, c.kind === "folder", c.kind))
    : [entry("notes.txt", path, 12_000, false, "document"), entry("assets", path, null, true)];
  return { path, parent: path.split("/").slice(0, -1).join("/") || "/", entries, skipped: 0 };
}

let planId = 0;
const plans = new Map<number, T.Plan>();

function plan(source: string, title: string, items: T.PlanItem[], dryRun: boolean): T.Plan {
  const created: T.Plan = {
    id: ++planId, source, title, dryRun, createdAt: Date.now(), items, blockedCount: items.filter((i) => i.blocked).length,
    trashBytes: items.filter((i) => i.action === "trash" && !i.blocked).reduce((s, i) => s + i.size, 0),
    deleteBytes: items.filter((i) => i.action !== "trash" && !i.blocked).reduce((s, i) => s + i.size, 0), warnings: [],
  };
  plans.set(created.id, created);
  return created;
}

const pause = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

const HISTORY_LEN = 90;

export async function mockInvoke(command: string, args: Record<string, unknown> = {}): Promise<unknown> {
  switch (command) {
    case "get_settings": return settings;
    case "save_settings": settings = args.settings as T.Settings; return settings;
    case "approve_root": settings = { ...settings, approvedRoots: [...settings.approvedRoots, args.path as string] }; return settings;
    case "revoke_root": settings = { ...settings, approvedRoots: settings.approvedRoots.filter((r) => r !== args.path) }; return settings;
    case "get_permissions": return { fullDiskAccess: "denied", home: "/Users/demo", suggestedRoots: ["/Users/demo", "/Applications"] };
    case "open_privacy_settings": case "open_activity_monitor": case "reveal_in_finder": case "discard_plan":
    case "clear_history": case "export_report": case "terminate_process": return null;
    case "cancel_job": return true;
    case "get_overview": return {
      modelName: "MacBook Pro", modelIdentifier: "Mac16,1", chip: "Apple M4", architecture: "aarch64", cpuBrand: "Apple M4",
      physicalCores: 10, logicalCores: 10, performanceCores: 4, efficiencyCores: 6, memoryTotal: 24 * GB, osName: "macOS", osVersion: "27.0.1",
      osBuild: "26A434", kernel: "27.0.0", hostName: "demo", bootTime: 1_700_000_000,
      disk: { name: "APPLE SSD", fileSystem: "APFS", protocol: "Apple Fabric", smartStatus: "Verified", solidState: true, encrypted: true },
    } satisfies T.SystemOverview;
    case "get_telemetry": return sample();
    case "get_history": return Array.from({ length: HISTORY_LEN }, (_, i) => {
      const s = sample();
      return {
        ts: Date.now() - (HISTORY_LEN - i) * 2000, cpu: s.cpu, memoryUsed: s.memory.used + Math.sin(i / 5) * 3e8, diskReadBps: s.diskReadBps, diskWriteBps: s.diskWriteBps,
        diskUsed: s.volume.used - (HISTORY_LEN - i) * 2e6, diskAvailable: s.volume.available + (HISTORY_LEN - i) * 2e6, battery: 78 + (HISTORY_LEN - i) * 0.02, temperature: null,
        processCount: 505 + (i % 9), cycleCount: 214, uptime: s.uptime - (HISTORY_LEN - i) * 2, health: s.health.overall,
      } satisfies T.HistoryPoint;
    });
    case "get_metric_trends": return Array.from({ length: 48 }, (_, i) => ({
      ts: Date.now() - (48 - i) * 3_600_000, cpu: 20 + (i % 7) * 4, memoryUsed: 14 * GB + (i % 5) * 4e8, memoryTotal: 24 * GB, diskUsed: 420 * GB + i * 2.5e8,
      diskTotal: 494 * GB, battery: 40 + (i * 7) % 60, health: 78 + (i % 6), storageScore: 74 - i * 0.05, memoryScore: 90 + (i % 8), cpuScore: 96, batteryScore: 86, performanceScore: 98,
    } satisfies T.MetricRow));
    case "list_processes": return {
      processes: processes.map((p) => ({ ...p, cpu: Math.max(0, p.cpu + (Math.random() - 0.5)) })),
      groups: [{ name: "Google Chrome", processes: 2, cpu: 19.5, memory: 3.4 * GB }, { name: "Xcode", processes: 1, cpu: 6.2, memory: 2.8 * GB }, { name: "Figma", processes: 1, cpu: 4.1, memory: 1.1 * GB }],
    } satisfies T.ProcessSnapshot;
    case "get_advisor_report": return advisor;
    case "list_scans": return [
      { id: 2, kind: "storage", startedAt: Date.now() - 400_000, finishedAt: Date.now() - 380_000, status: "completed", roots: [], totalBytes: 363 * GB, files: 2_184_411, reclaimable: 0, inaccessible: 38, diskUsed: 433 * GB, diskTotal: 494 * GB, healthScore: 81 },
      { id: 1, kind: "cleaner", startedAt: Date.now() - 90_000_000, finishedAt: Date.now() - 89_000_000, status: "completed", roots: [], totalBytes: 41 * GB, files: 88, reclaimable: 41 * GB, inaccessible: 0, diskUsed: 441 * GB, diskTotal: 494 * GB, healthScore: 77 },
    ] satisfies T.ScanRow[];
    case "list_cleanup_runs": return [{ id: 1, source: "cleaner", startedAt: Date.now() - 90_000_000, finishedAt: Date.now() - 89_900_000, dryRun: false, mode: "trash", status: "completed", bytesFreed: 0, bytesTrashed: 8.2 * GB, itemsOk: 42, itemsFailed: 0, itemsSkipped: 1 }] satisfies T.CleanupRun[];
    case "get_cleanup_items": return [{ path: "/Users/demo/Library/Caches/com.spotify.client", size: 3.8 * GB, action: "trash", outcome: "trashed", detail: null }] satisfies T.CleanupLogItem[];
    case "start_storage_scan": setTimeout(runStorageScan, 100); return null;
    case "get_storage_summary": return scanSummary;
    case "get_storage_tree": {
      const node = findNode((args.path as string | null) ?? null);
      return node;
    }
    case "list_directory": return listing(args.path as string);
    case "measure_directory": await pause(300); return [812 * MB, 4210];
    case "search_files": return [entry(`${args.query}-notes.txt`, "/Users/demo/Documents", 4 * MB, false, "document"), entry(`${args.query} assets`, "/Users/demo/Documents", 3.1 * GB, true)];
    case "start_cleaner_scan": setTimeout(runCleanerScan, 100); return null;
    case "get_cleaner_report": return cleaner;
    case "start_duplicate_scan": {
      let n = 0;
      const handle = setInterval(() => {
        n += 1;
        emit("duplicates://progress", { phase: n < 10 ? "Indexing files" : "Verifying with BLAKE3", files: n * 9000, candidates: 1400, bytesHashed: n > 10 ? (n - 10) * 2 * GB : 0, bytesTotal: n > 10 ? 41 * GB : 0, current: `/Users/demo/Documents/file-${n}.bin` } satisfies T.DuplicateProgress);
        if (n >= 26) {
          clearInterval(handle);
          dupes = duplicates;
          emit("duplicates://done", { report: dupes });
        }
      }, 110);
      return null;
    }
    case "get_duplicate_report": return dupes;
    case "image_preview": case "app_icon": return null;
    case "list_apps": return apps;
    case "app_associated": return [
      { path: "/Users/demo/Library/Application Support/Demo", label: "Application data", size: 1.2 * GB, files: 400, isDir: true },
      { path: "/Users/demo/Library/Caches/com.demo.app", label: "Cache", size: 310 * MB, files: 90, isDir: true },
      { path: "/Users/demo/Library/Preferences/com.demo.app.plist", label: "Preferences", size: 8000, files: 1, isDir: false },
    ] satisfies T.AssociatedItem[];
    case "prepare_cleanup": {
      const ids = new Set(args.itemIds as string[]);
      const items = (cleaner?.targets ?? []).flatMap((t) => t.items.filter((i) => ids.has(i.id)).map((i): T.PlanItem => ({
        path: i.path, size: i.size, action: "trash", policy: t.policy, risk: t.risk, group: t.title, blocked: null,
      })));
      return plan("cleaner", "Smart Cleaner", items, args.dry as boolean);
    }
    case "prepare_empty_trash": return plan("trash", "Empty Trash", cleanerReport.trash.items.map((i) => ({ path: i.path, size: i.size, action: "delete", policy: "cleanupTarget", risk: "medium", group: "Trash", blocked: null })), args.dry as boolean);
    case "prepare_paths_plan": return plan(args.source as string, args.title as string, (args.paths as string[]).map((path) => ({ path, size: 2 * GB, action: "trash", policy: "userSelected", risk: "medium", group: args.title as string, blocked: null })), args.dry as boolean);
    case "prepare_duplicate_plan": return plan("duplicates", "Duplicate Finder", (args.paths as string[]).map((path) => ({ path, size: 1.6 * GB, action: "trash", policy: "userSelected", risk: "medium", group: "Duplicate files", blocked: null })), args.dry as boolean);
    case "prepare_uninstall": return plan("uninstall", "Uninstall app", [{ path: args.appPath as string, size: 2 * GB, action: "trash", policy: "application", risk: "medium", group: "Application", blocked: null }], args.dry as boolean);
    case "execute_plan": {
      const current = plans.get(args.planId as number)!;
      for (let i = 0; i < current.items.length; i += 1) {
        emit("cleanup://progress", { done: i, total: current.items.length, current: current.items[i].path, bytes: 0 } satisfies T.ExecProgress);
        await pause(160);
      }
      return {
        runId: 2, dryRun: current.dryRun, status: "completed", bytesFreed: current.deleteBytes, bytesTrashed: current.trashBytes,
        itemsOk: current.items.length, itemsFailed: 0, itemsSkipped: 0, failures: [],
      } satisfies T.ExecResult;
    }
    default: throw new Error(`Mock has no handler for ${command}`);
  }
}

setInterval(() => emit("telemetry://tick", sample()), 2000);
