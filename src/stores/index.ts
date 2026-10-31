import { create } from "zustand";
import { api, errorMessage, on } from "../lib/ipc";
import type * as T from "../lib/types";

export type Route =
  | "command" | "storage" | "hogs" | "optimize" | "cleaner" | "duplicates"
  | "performance" | "battery" | "apps" | "ai" | "history" | "settings";

interface Toast { id: number; tone: "info" | "good" | "warn" | "bad"; title: string; body?: string }
interface ToastState { toasts: Toast[]; push: (t: Omit<Toast, "id">) => void; dismiss: (id: number) => void }
let toastId = 0;

export const useToasts = create<ToastState>((set, get) => ({
  toasts: [],
  push: (toast) => {
    const id = ++toastId;
    set((s) => ({ toasts: [...s.toasts.slice(-3), { ...toast, id }] }));
    setTimeout(() => get().dismiss(id), toast.tone === "bad" ? 8000 : 4500);
  },
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));

export const toast = {
  info: (title: string, body?: string) => useToasts.getState().push({ tone: "info", title, body }),
  good: (title: string, body?: string) => useToasts.getState().push({ tone: "good", title, body }),
  warn: (title: string, body?: string) => useToasts.getState().push({ tone: "warn", title, body }),
  error: (title: string, error?: unknown) =>
    useToasts.getState().push({ tone: "bad", title, body: error === undefined ? undefined : errorMessage(error) }),
};

interface AppState {
  route: Route;
  sidebarCollapsed: boolean;
  paletteOpen: boolean;
  settings: T.Settings | null;
  permissions: T.Permissions | null;
  storageFilter: T.Category | null;
  navigate: (route: Route) => void;
  toggleSidebar: () => void;
  setPalette: (open: boolean) => void;
  setStorageFilter: (category: T.Category | null) => void;
  updateSettings: (patch: Partial<T.Settings>) => Promise<void>;
  setSettings: (settings: T.Settings) => void;
}

function applyAppearance(settings: T.Settings) {
  const root = document.documentElement;
  root.dataset.theme = settings.theme;
  root.dataset.accent = settings.accent;
  root.dataset.motion = settings.animation;
}

function readSidebar(): boolean {
  try {
    return localStorage.getItem("nexus.sidebar") === "1";
  } catch {
    return false;
  }
}

export const useApp = create<AppState>((set, get) => ({
  route: "command",
  sidebarCollapsed: readSidebar(),
  paletteOpen: false,
  settings: null,
  permissions: null,
  storageFilter: null,
  navigate: (route) => set({ route, paletteOpen: false }),
  toggleSidebar: () =>
    set((s) => {
      const next = !s.sidebarCollapsed;
      try {
        localStorage.setItem("nexus.sidebar", next ? "1" : "0");
      } catch {
        /* preference only */
      }
      return { sidebarCollapsed: next };
    }),
  setPalette: (paletteOpen) => set({ paletteOpen }),
  setStorageFilter: (storageFilter) => set({ storageFilter }),
  setSettings: (settings) => {
    applyAppearance(settings);
    set({ settings });
  },
  updateSettings: async (patch) => {
    const current = get().settings;
    if (!current) return;
    const optimistic = { ...current, ...patch };
    get().setSettings(optimistic);
    try {
      get().setSettings(await api.saveSettings(optimistic));
    } catch (error) {
      get().setSettings(current);
      toast.error("Could not save settings", error);
    }
  },
}));

const LIVE_POINTS = 150;

interface TelemetryState {
  sample: T.TelemetrySample | null;
  history: T.HistoryPoint[];
  overview: T.SystemOverview | null;
}

export const useTelemetry = create<TelemetryState>(() => ({ sample: null, history: [], overview: null }));

function toPoint(s: T.TelemetrySample): T.HistoryPoint {
  return {
    ts: s.ts, cpu: s.cpu, memoryUsed: s.memory.used, diskReadBps: s.diskReadBps, diskWriteBps: s.diskWriteBps,
    diskUsed: s.volume.used, diskAvailable: s.volume.available, battery: s.battery?.percent ?? null,
    temperature: s.cpuTemperature, processCount: s.processCount, cycleCount: s.battery?.cycleCount ?? null,
    uptime: s.uptime, health: s.health.overall,
  };
}

interface StorageState {
  summary: T.ScanSummary | null;
  scanning: boolean;
  progress: T.ScanProgress | null;
  discoveries: T.Discovery[];
  error: string | null;
  start: (deep: boolean) => Promise<void>;
  cancel: () => void;
}

export const useStorage = create<StorageState>((set) => ({
  summary: null,
  scanning: false,
  progress: null,
  discoveries: [],
  error: null,
  start: async (deep) => {
    set({ scanning: true, progress: null, discoveries: [], error: null });
    try {
      await api.startStorageScan(deep);
    } catch (error) {
      set({ scanning: false, error: errorMessage(error) });
      toast.error("Scan could not start", error);
    }
  },
  cancel: () => void api.cancelJob("storage"),
}));

interface CleanerState {
  report: T.CleanerReport | null;
  scanning: boolean;
  progress: T.CleanerProgress | null;
  findings: { title: string; bytes: number }[];
  selected: Set<string>;
  docker: Set<string>;
  start: () => Promise<void>;
  cancel: () => void;
  setSelected: (selected: Set<string>) => void;
  setDocker: (docker: Set<string>) => void;
}

function defaultSelection(report: T.CleanerReport): Set<string> {
  const ids = new Set<string>();
  for (const target of report.targets) {
    if (target.selectedByDefault && !target.appRunning) target.items.forEach((i) => ids.add(i.id));
  }
  return ids;
}

export const useCleaner = create<CleanerState>((set) => ({
  report: null,
  scanning: false,
  progress: null,
  findings: [],
  selected: new Set(),
  docker: new Set(),
  start: async () => {
    set({ scanning: true, progress: null, findings: [] });
    try {
      await api.startCleanerScan();
    } catch (error) {
      set({ scanning: false });
      toast.error("Scan could not start", error);
    }
  },
  cancel: () => void api.cancelJob("cleaner"),
  setSelected: (selected) => set({ selected }),
  setDocker: (docker) => set({ docker }),
}));

interface DuplicateState {
  report: T.DuplicateReport | null;
  scanning: boolean;
  progress: T.DuplicateProgress | null;
  selected: Set<string>;
  start: (roots: string[], excluded: string[], minSize: number) => Promise<void>;
  cancel: () => void;
  setSelected: (selected: Set<string>) => void;
}

export const useDuplicates = create<DuplicateState>((set) => ({
  report: null,
  scanning: false,
  progress: null,
  selected: new Set(),
  start: async (roots, excluded, minSize) => {
    set({ scanning: true, progress: null, selected: new Set() });
    try {
      await api.startDuplicateScan(roots, excluded, minSize);
    } catch (error) {
      set({ scanning: false });
      toast.error("Scan could not start", error);
    }
  },
  cancel: () => void api.cancelJob("duplicates"),
  setSelected: (selected) => set({ selected }),
}));

type PlanStage = "preview" | "running" | "done";

interface PlanState {
  plan: T.Plan | null;
  stage: PlanStage;
  progress: T.ExecProgress | null;
  result: T.ExecResult | null;
  rebuild: ((dry: boolean) => Promise<T.Plan>) | null;
  onDone: (() => void) | null;
  open: (build: (dry: boolean) => Promise<T.Plan>, onDone?: () => void) => Promise<void>;
  setDryRun: (dry: boolean) => Promise<void>;
  execute: () => Promise<void>;
  close: () => void;
}

/** Every destructive action in the app goes through this preview-then-confirm flow. */
export const usePlan = create<PlanState>((set, get) => ({
  plan: null,
  stage: "preview",
  progress: null,
  result: null,
  rebuild: null,
  onDone: null,
  open: async (build, onDone) => {
    try {
      const plan = await build(false);
      set({ plan, stage: "preview", progress: null, result: null, rebuild: build, onDone: onDone ?? null });
    } catch (error) {
      toast.error("Could not prepare a preview", error);
    }
  },
  setDryRun: async (dry) => {
    const { rebuild, plan } = get();
    if (!rebuild || !plan) return;
    try {
      void api.discardPlan(plan.id);
      set({ plan: await rebuild(dry) });
    } catch (error) {
      toast.error("Could not update the preview", error);
    }
  },
  execute: async () => {
    const { plan } = get();
    if (!plan) return;
    set({ stage: "running" });
    try {
      const result = await api.executePlan(plan.id);
      set({ stage: "done", result });
    } catch (error) {
      set({ stage: "preview" });
      toast.error("Cleanup did not run", error);
    }
  },
  close: () => {
    const { plan, stage, onDone } = get();
    if (plan && stage === "preview") void api.discardPlan(plan.id);
    if (stage === "done") onDone?.();
    set({ plan: null, stage: "preview", progress: null, result: null, rebuild: null, onDone: null });
  },
}));

interface MenuItem { label: string; onSelect: () => void; danger?: boolean }
interface MenuState {
  menu: { x: number; y: number; items: MenuItem[] } | null;
  show: (x: number, y: number, items: MenuItem[]) => void;
  hide: () => void;
}

export const useMenu = create<MenuState>((set) => ({
  menu: null,
  show: (x, y, items) => set({ menu: { x, y, items } }),
  hide: () => set({ menu: null }),
}));

export async function bootstrap() {
  on<T.TelemetrySample>("telemetry://tick", (sample) => {
    useTelemetry.setState((s) => ({
      sample,
      history: [...s.history.slice(-(LIVE_POINTS - 1)), toPoint(sample)],
    }));
  });

  on<T.ScanProgress>("storage://progress", (progress) => {
    useStorage.setState((s) => ({
      progress,
      discoveries: progress.discoveries.length
        ? [...progress.discoveries.reverse(), ...s.discoveries].slice(0, 40)
        : s.discoveries,
    }));
  });
  on<{ summary?: T.ScanSummary | null; error?: string }>("storage://done", (done) => {
    useStorage.setState((s) => ({ scanning: false, summary: done.summary ?? s.summary, error: done.error ?? null }));
    if (done.error) toast.error("Storage scan failed", done.error);
  });

  on<T.CleanerProgress>("cleaner://progress", (progress) => {
    useCleaner.setState((s) => ({ progress, findings: [...progress.findings, ...s.findings].slice(0, 24) }));
  });
  on<{ report?: T.CleanerReport; error?: string }>("cleaner://done", ({ report, error }) => {
    if (error) toast.error("Cleaner scan failed", error);
    useCleaner.setState((s) => ({
      scanning: false,
      report: report?.completed ? report : s.report,
      selected: report?.completed ? defaultSelection(report) : s.selected,
      docker: new Set(),
    }));
  });

  on<T.DuplicateProgress>("duplicates://progress", (progress) => useDuplicates.setState({ progress }));
  on<{ report?: T.DuplicateReport; error?: string }>("duplicates://done", ({ report, error }) => {
    if (error) toast.error("Duplicate scan failed", error);
    useDuplicates.setState((s) => ({ scanning: false, report: report?.completed ? report : s.report }));
  });

  on<T.ExecProgress>("cleanup://progress", (progress) => usePlan.setState({ progress }));

  const [settings, permissions, overview, sample, history, summary, cleaner, duplicates] = await Promise.all([
    api.getSettings(), api.getPermissions(), api.getOverview(), api.getTelemetry(), api.getHistory(),
    api.getStorageSummary(), api.getCleanerReport(), api.getDuplicateReport(),
  ]);
  useApp.getState().setSettings(settings);
  useApp.setState({ permissions });
  useTelemetry.setState({ overview, sample, history: history.slice(-LIVE_POINTS) });
  useStorage.setState({ summary });
  if (cleaner) useCleaner.setState({ report: cleaner, selected: defaultSelection(cleaner) });
  useDuplicates.setState({ report: duplicates });
}
