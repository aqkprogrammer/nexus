// Mirrors the serde types in src-tauri. Field names are the camelCase wire names.

export type Risk = "low" | "medium" | "high";
export type Category = "applications" | "documents" | "downloads" | "media" | "developer" | "caches" | "other";
export type FileKind =
  | "folder" | "video" | "audio" | "image" | "document" | "archive"
  | "installer" | "diskImage" | "code" | "application" | "other";
export type HogKind =
  | "largeVideo" | "oldInstaller" | "zipArchive" | "diskImage" | "virtualMachine" | "docker"
  | "xcodeDerivedData" | "nodeModules" | "buildArtifact" | "projectBackup" | "unusedDownload" | "aiModels";

export interface HealthComponent { score: number; status: string; detail: string }
export interface HealthReport {
  overall: number;
  status: string;
  storage: HealthComponent;
  memory: HealthComponent;
  cpu: HealthComponent;
  battery: HealthComponent | null;
  performance: HealthComponent;
}

export interface MemorySnapshot {
  total: number; used: number; app: number; wired: number; compressed: number;
  cached: number; free: number; available: number; swapUsed: number; swapTotal: number;
  pressure: string; pressureLevel: number | null;
}
export interface VolumeSnapshot { mountPoint: string; total: number; available: number; used: number }
export interface BatterySnapshot {
  percent: number; isCharging: boolean; externalPower: boolean; fullyCharged: boolean; powerSource: string;
  cycleCount: number | null; designCapacityMah: number | null; maxCapacityMah: number | null;
  maxCapacityPercent: number | null; condition: string | null; temperatureC: number | null;
  voltageV: number | null; powerW: number | null; timeRemainingMin: number | null;
}
export interface TelemetrySample {
  ts: number; cpu: number; perCore: number[]; loadAverage: [number, number, number];
  memory: MemorySnapshot; diskReadBps: number | null; diskWriteBps: number | null;
  volume: VolumeSnapshot; battery: BatterySnapshot | null; cpuTemperature: number | null;
  thermalState: string; processCount: number | null; uptime: number; health: HealthReport;
}
export interface HistoryPoint {
  ts: number; cpu: number; memoryUsed: number; diskReadBps: number | null; diskWriteBps: number | null;
  diskUsed: number; diskAvailable: number; battery: number | null; temperature: number | null;
  processCount: number | null; cycleCount: number | null; uptime: number; health: number;
}
export interface MetricRow {
  ts: number; cpu: number; memoryUsed: number; memoryTotal: number; diskUsed: number; diskTotal: number;
  battery: number | null; health: number; storageScore: number; memoryScore: number; cpuScore: number;
  batteryScore: number | null; performanceScore: number;
}
export interface DiskInfo {
  name: string | null; fileSystem: string | null; protocol: string | null; smartStatus: string | null;
  solidState: boolean | null; encrypted: boolean | null;
}
export interface SystemOverview {
  modelName: string | null; modelIdentifier: string | null; chip: string | null; architecture: string;
  cpuBrand: string | null; physicalCores: number | null; logicalCores: number | null;
  performanceCores: number | null; efficiencyCores: number | null; memoryTotal: number; osName: string;
  osVersion: string | null; osBuild: string | null; kernel: string | null; hostName: string | null;
  bootTime: number | null; disk: DiskInfo;
}

export interface ProcessInfo {
  pid: number; parentPid: number | null; name: string; app: string | null; exe: string | null; cpu: number;
  memory: number; diskRead: number; diskWrite: number; status: string; startTime: number; runTime: number;
  owned: boolean; protected: boolean;
}
export interface ProcessGroup { name: string; processes: number; cpu: number; memory: number }
export interface ProcessSnapshot { processes: ProcessInfo[]; groups: ProcessGroup[] }

export interface TreeNode {
  name: string; size: number; files: number; modified: number; kind: FileKind; category: Category;
  aggregate?: boolean; denied?: boolean; children?: TreeNode[];
}
export interface FileItem {
  path: string; name: string; size: number; modified: number; accessed: number; kind: FileKind; category: Category;
}
export interface HogItem {
  path: string; name: string; kind: HogKind; size: number; files: number; modified: number; accessed: number; isDir: boolean;
}
export interface CategoryStat { category: Category; bytes: number }
export interface OldFileStat { cutoffMonths: number; bytes: number; files: number; downloadsBytes: number; downloadsFiles: number }
export interface ScanSummary {
  startedAt: number; finishedAt: number; roots: string[]; completed: boolean; totalBytes: number; files: number;
  directories: number; inaccessible: number; reusedDirectories: number; categories: CategoryStat[];
  over100Mb: [number, number]; over1Gb: [number, number]; old: OldFileStat; topFiles: FileItem[]; hogs: HogItem[];
}
export interface Discovery { path: string; size: number }
export interface ScanProgress {
  files: number; directories: number; bytes: number; inaccessible: number; current: string; discoveries: Discovery[];
}
export interface DirEntry {
  name: string; path: string; isDir: boolean; isSymlink: boolean; isHidden: boolean; size: number | null;
  files: number | null; kind: FileKind; modified: number | null; accessed: number | null; created: number | null;
}
export interface Listing { path: string; parent: string | null; entries: DirEntry[]; skipped: number }

export type CleanerGroup = "systemJunk" | "leftovers" | "browser" | "developer";
export type Policy = "cleanupTarget" | "leftover" | "userSelected" | "application";
export interface CleanerItem {
  id: string; path: string; name: string; size: number; files: number; modified: number; isDir: boolean;
}
export interface CleanerTarget {
  id: string; group: CleanerGroup; title: string; description: string; effect: string; risk: Risk;
  selectedByDefault: boolean; policy: Policy; items: CleanerItem[]; total: number;
  appRunning: string | null; unavailable: string | null;
}
export interface TrashStatus { accessible: boolean; items: CleanerItem[]; total: number; count: number }
export interface DockerResource { id: string; label: string; total: string; active: string; size: number; reclaimable: number }
export interface DockerStatus { installed: boolean; running: boolean; message: string; resources: DockerResource[] }
export interface CleanerReport {
  scannedAt: number; durationMs: number; completed: boolean; targets: CleanerTarget[]; trash: TrashStatus;
  docker: DockerStatus; reclaimable: number; recommended: number;
}
export interface CleanerProgress {
  percent: number; phase: string; current: string; foundBytes: number; findings: { title: string; bytes: number }[];
}

export type PlanAction = "trash" | "delete" | "docker-prune" | "finder-empty-trash";
export interface PlanItem {
  path: string; size: number; action: PlanAction; policy: Policy; risk: Risk; group: string; blocked: string | null;
}
export interface Plan {
  id: number; source: string; title: string; dryRun: boolean; createdAt: number; items: PlanItem[];
  trashBytes: number; deleteBytes: number; blockedCount: number; warnings: string[];
}
export interface ExecProgress { done: number; total: number; current: string; bytes: number }
export interface ExecResult {
  runId: number; dryRun: boolean; status: string; bytesFreed: number; bytesTrashed: number; itemsOk: number;
  itemsFailed: number; itemsSkipped: number; failures: { path: string; reason: string }[];
}

export interface DuplicateFile { path: string; name: string; modified: number; created: number }
export interface DuplicateGroup {
  id: string; hash: string; size: number; kind: FileKind; recoverable: number; files: DuplicateFile[];
}
export interface DuplicateReport {
  scannedAt: number; durationMs: number; completed: boolean; roots: string[]; filesIndexed: number;
  bytesHashed: number; inaccessible: number; recoverable: number; groups: DuplicateGroup[];
}
export interface DuplicateProgress {
  phase: string; files: number; candidates: number; bytesHashed: number; bytesTotal: number; current: string;
}

export interface AppInfo {
  name: string; path: string; bundleId: string | null; version: string | null; size: number | null;
  dataSize: number | null; modified: number | null; isSystem: boolean; running: boolean;
}
export interface AssociatedItem { path: string; label: string; size: number; files: number; isDir: boolean }

export interface Recommendation {
  id: string; domain: string; title: string; issue: string; why: string; impact: string; estimate: string;
  estimateBytes: number | null; action: string; risk: Risk; severity: number; actionLabel: string; route: string;
}
export interface AdvisorReport {
  generatedAt: number; engine: string; summary: string; recommendations: Recommendation[]; missingData: string[];
}

export interface ScanRow {
  id: number; kind: "storage" | "cleaner" | "duplicates"; startedAt: number; finishedAt: number; status: string;
  roots: string[]; totalBytes: number; files: number; reclaimable: number; inaccessible: number;
  diskUsed: number | null; diskTotal: number | null; healthScore: number | null;
}
export interface CleanupRun {
  id: number; source: string; startedAt: number; finishedAt: number; dryRun: boolean; mode: string; status: string;
  bytesFreed: number; bytesTrashed: number; itemsOk: number; itemsFailed: number; itemsSkipped: number;
}
export interface CleanupLogItem { path: string; size: number; action: string; outcome: string; detail: string | null }

export interface Settings {
  onboarded: boolean; theme: string; accent: string; animation: string; approvedRoots: string[]; exclusions: string[];
  scanFrequency: string; notifications: boolean; cleanupMode: string; dryRun: boolean; tempMinAgeDays: number;
  oldFileMonths: number; recordHistory: boolean; retentionDays: number; launchAtLogin: boolean; aiProvider: string;
  aiModel: string; aiEndpoint: string; aiShareAggregates: boolean;
}
export interface Permissions { fullDiskAccess: "granted" | "denied" | "unknown"; home: string; suggestedRoots: string[] }
export interface NexusErrorPayload { kind: string; message: string }
