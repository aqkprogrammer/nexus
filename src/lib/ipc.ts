import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type * as T from "./types";

export const isTauri = "__TAURI_INTERNALS__" in window;
/** True only in a plain browser during UI development. Never true in a built app. */
export const usingMockData = !isTauri && import.meta.env.DEV;

async function call<R>(command: string, args?: Record<string, unknown>): Promise<R> {
  if (isTauri) return invoke<R>(command, args);
  if (import.meta.env.DEV) {
    const { mockInvoke } = await import("./mock");
    return mockInvoke(command, args) as Promise<R>;
  }
  throw new Error("NEXUS must run inside its desktop app.");
}

export type Unlisten = () => void;

export function on<P>(event: string, handler: (payload: P) => void): Unlisten {
  let stop: Unlisten | undefined;
  let stopped = false;
  const attach = isTauri
    ? listen<P>(event, (e) => handler(e.payload))
    : import.meta.env.DEV
      ? import("./mock").then((m) => m.mockListen(event, handler as (p: unknown) => void))
      : Promise.resolve(() => {});
  attach.then((fn) => {
    if (stopped) fn();
    else stop = fn;
  });
  return () => {
    stopped = true;
    stop?.();
  };
}

export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error) return String(error.message);
  return "Something went wrong.";
}

export const api = {
  getSettings: () => call<T.Settings>("get_settings"),
  saveSettings: (settings: T.Settings) => call<T.Settings>("save_settings", { settings }),
  approveRoot: (path: string) => call<T.Settings>("approve_root", { path }),
  revokeRoot: (path: string) => call<T.Settings>("revoke_root", { path }),
  getPermissions: () => call<T.Permissions>("get_permissions"),
  openPrivacySettings: () => call<void>("open_privacy_settings"),
  cancelJob: (kind: string) => call<boolean>("cancel_job", { kind }),

  getOverview: () => call<T.SystemOverview>("get_overview"),
  getTelemetry: () => call<T.TelemetrySample | null>("get_telemetry"),
  getHistory: () => call<T.HistoryPoint[]>("get_history"),
  getMetricTrends: (days: number) => call<T.MetricRow[]>("get_metric_trends", { days }),
  listProcesses: () => call<T.ProcessSnapshot>("list_processes"),
  terminateProcess: (pid: number, startTime: number, force: boolean) =>
    call<void>("terminate_process", { pid, startTime, force }),
  openActivityMonitor: () => call<void>("open_activity_monitor"),
  getAdvisorReport: () => call<T.AdvisorReport>("get_advisor_report"),
  listScans: (kind: string | null, limit: number) => call<T.ScanRow[]>("list_scans", { kind, limit }),
  listCleanupRuns: (limit: number) => call<T.CleanupRun[]>("list_cleanup_runs", { limit }),
  getCleanupItems: (runId: number) => call<T.CleanupLogItem[]>("get_cleanup_items", { runId }),
  clearHistory: () => call<void>("clear_history"),
  exportReport: (path: string) => call<void>("export_report", { path }),

  startStorageScan: (deep: boolean) => call<void>("start_storage_scan", { deep }),
  getStorageSummary: () => call<T.ScanSummary | null>("get_storage_summary"),
  getStorageTree: (path: string | null, depth: number) => call<T.TreeNode | null>("get_storage_tree", { path, depth }),
  listDirectory: (path: string, showHidden: boolean) => call<T.Listing>("list_directory", { path, showHidden }),
  measureDirectory: (path: string) => call<[number, number]>("measure_directory", { path }),
  searchFiles: (query: string) => call<T.DirEntry[]>("search_files", { query }),
  revealInFinder: (path: string) => call<void>("reveal_in_finder", { path }),

  startCleanerScan: () => call<void>("start_cleaner_scan"),
  getCleanerReport: () => call<T.CleanerReport | null>("get_cleaner_report"),
  startDuplicateScan: (roots: string[], excluded: string[], minSize: number) =>
    call<void>("start_duplicate_scan", { roots, excluded, minSize }),
  getDuplicateReport: () => call<T.DuplicateReport | null>("get_duplicate_report"),
  imagePreview: (path: string) => call<string | null>("image_preview", { path }),

  listApps: () => call<T.AppInfo[]>("list_apps"),
  appIcon: (path: string) => call<string | null>("app_icon", { path }),
  appAssociated: (path: string) => call<T.AssociatedItem[]>("app_associated", { path }),

  prepareCleanup: (itemIds: string[], dockerIds: string[], dry: boolean) =>
    call<T.Plan>("prepare_cleanup", { itemIds, dockerIds, dry }),
  prepareEmptyTrash: (dry: boolean) => call<T.Plan>("prepare_empty_trash", { dry }),
  preparePathsPlan: (source: string, title: string, paths: string[], dry: boolean) =>
    call<T.Plan>("prepare_paths_plan", { source, title, paths, dry }),
  prepareDuplicatePlan: (paths: string[], dry: boolean) => call<T.Plan>("prepare_duplicate_plan", { paths, dry }),
  prepareUninstall: (appPath: string, associated: string[], dry: boolean) =>
    call<T.Plan>("prepare_uninstall", { appPath, associated, dry }),
  discardPlan: (planId: number) => call<void>("discard_plan", { planId }),
  executePlan: (planId: number) => call<T.ExecResult>("execute_plan", { planId, confirmed: true }),
};
