import { useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import {
  ArrowDownToLine, ArrowUpFromLine, Battery, CheckCircle2, Cpu, CircleAlert, FileText, Folder, HardDrive,
  Info, MemoryStick, Palette, Radar as RadarIcon, Search, TriangleAlert, X, type LucideIcon,
} from "lucide-react";
import { api, usingMockData } from "../../lib/ipc";
import { bytes, memory, percent, rate } from "../../lib/format";
import { toast, useApp, useCleaner, useDuplicates, useMenu, useStorage, useTelemetry, useToasts, type Route } from "../../stores";
import type { DirEntry } from "../../lib/types";
import { cx } from "../ui";
import { NAV } from "./Sidebar";

export function Background() {
  const motes = useMemo(
    () => Array.from({ length: 9 }, (_, i) => ({
      left: `${(i * 37 + 11) % 100}%`, top: `${(i * 53 + 23) % 100}%`, delay: `${i * 0.17}s`,
    })),
    [],
  );
  return (
    <div className="pointer-events-none fixed inset-0 overflow-hidden" aria-hidden>
      <div className="bg-orb" style={{ width: 760, height: 760, left: "12%", top: "-34%", background: "radial-gradient(circle, var(--ambient-a), transparent 66%)" }} />
      <div className="bg-orb" style={{ width: 900, height: 900, right: "-18%", top: "12%", animationDelay: "-9s", background: "radial-gradient(circle, var(--ambient-b), transparent 66%)" }} />
      <div className="bg-orb" style={{ width: 620, height: 620, left: "28%", bottom: "-36%", animationDelay: "-17s", background: "radial-gradient(circle, var(--ambient-c), transparent 66%)" }} />
      <div className="bg-grid" />
      <div className="bg-scan" />
      {motes.map((m, i) => <span key={i} className="bg-mote" style={{ left: m.left, top: m.top, animationDelay: m.delay }} />)}
    </div>
  );
}

function Chip({ icon: Icon, label, value, onClick }: { icon: LucideIcon; label: string; value: string; onClick: () => void }) {
  return (
    <button type="button" onClick={onClick} title={label} aria-label={`${label}: ${value}`}
      className="flex h-7 items-center gap-1.5 rounded-md border border-line bg-black/20 px-2 text-soft transition-colors hover:border-accent/40 hover:text-ink">
      <Icon size={12} strokeWidth={1.8} className="text-accent" />
      <span className="mono text-[11px]">{value}</span>
    </button>
  );
}

export function TitleBar() {
  const sample = useTelemetry((s) => s.sample);
  const navigate = useApp((s) => s.navigate);
  const route = useApp((s) => s.route);
  const collapsed = useApp((s) => s.sidebarCollapsed);
  const setPalette = useApp((s) => s.setPalette);
  const label = NAV.flatMap((g) => g.items).find((i) => i.route === route)?.label;

  return (
    <header data-tauri-drag-region className={cx("relative z-10 flex h-[52px] shrink-0 items-center gap-3 border-b border-line bg-black/10 pr-4", collapsed ? "pl-5" : "pl-6")}>
      <span data-tauri-drag-region className="label truncate">NEXUS / {label}</span>
      <div data-tauri-drag-region className="flex-1" />
      {sample && (
        <div className="hidden items-center gap-1.5 lg:flex">
          <Chip icon={Cpu} label="CPU" value={percent(sample.cpu)} onClick={() => navigate("performance")} />
          <Chip icon={MemoryStick} label="Memory in use" value={memory(sample.memory.used)} onClick={() => navigate("performance")} />
          <Chip icon={HardDrive} label="Disk available" value={bytes(sample.volume.available)} onClick={() => navigate("storage")} />
          {sample.battery && <Chip icon={Battery} label="Battery" value={percent(sample.battery.percent)} onClick={() => navigate("battery")} />}
        </div>
      )}
      <button type="button" onClick={() => setPalette(true)} className="flex h-7 w-56 shrink-0 items-center gap-2 rounded-md border border-line bg-black/25 px-2.5 text-faint transition-colors hover:border-accent/40 hover:text-soft">
        <Search size={13} className="shrink-0" />
        <span className="truncate text-xs">Search or run a command</span>
        <span className="mono ml-auto text-[10px]">⌘K</span>
      </button>
    </header>
  );
}

/** Floating indicator for work running in the background on any screen. */
export function JobDock() {
  const storage = useStorage();
  const cleaner = useCleaner();
  const duplicates = useDuplicates();
  const io = useTelemetry((s) => s.sample);
  const navigate = useApp((s) => s.navigate);
  const route = useApp((s) => s.route);
  const jobs: { id: Route; label: string; detail: string; cancel: () => void }[] = [];
  if (storage.scanning && route !== "storage") {
    jobs.push({ id: "storage", label: "Storage scan", detail: `${bytes(storage.progress?.bytes ?? 0)} analyzed`, cancel: storage.cancel });
  }
  if (cleaner.scanning && route !== "cleaner") {
    jobs.push({ id: "cleaner", label: "Cleaner scan", detail: `${Math.round(cleaner.progress?.percent ?? 0)}%`, cancel: cleaner.cancel });
  }
  if (duplicates.scanning && route !== "duplicates") {
    jobs.push({ id: "duplicates", label: "Duplicate scan", detail: duplicates.progress?.phase ?? "Starting", cancel: duplicates.cancel });
  }
  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-30 flex flex-col items-end gap-2">
      <AnimatePresence>
        {jobs.map((job) => (
          <motion.div key={job.id} initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: 10 }}
            className="glass pointer-events-auto flex items-center gap-3 px-3 py-2">
            <RadarIcon size={15} className="spin-radar text-accent" />
            <button type="button" className="text-left" onClick={() => navigate(job.id)}>
              <div className="text-xs text-ink">{job.label}</div>
              <div className="mono text-[10px] text-soft">{job.detail}</div>
            </button>
            <button type="button" aria-label={`Cancel ${job.label}`} onClick={job.cancel} className="text-faint hover:text-ink"><X size={13} /></button>
          </motion.div>
        ))}
      </AnimatePresence>
      {io && jobs.length === 0 && (io.diskReadBps ?? 0) + (io.diskWriteBps ?? 0) > 5_000_000 && (
        <div className="glass flex items-center gap-2 px-2.5 py-1.5 text-soft">
          <ArrowDownToLine size={11} /><span className="mono text-[10px]">{rate(io.diskReadBps)}</span>
          <ArrowUpFromLine size={11} /><span className="mono text-[10px]">{rate(io.diskWriteBps)}</span>
        </div>
      )}
    </div>
  );
}

const TOAST_ICON = { info: Info, good: CheckCircle2, warn: TriangleAlert, bad: CircleAlert };
const TOAST_COLOR = { info: "text-accent", good: "text-good", warn: "text-warn", bad: "text-bad" };

export function Toasts() {
  const toasts = useToasts((s) => s.toasts);
  const dismiss = useToasts((s) => s.dismiss);
  return (
    <div className="pointer-events-none fixed right-4 top-14 z-50 flex w-80 flex-col gap-2" role="status" aria-live="polite">
      <AnimatePresence>
        {toasts.map((t) => {
          const Icon = TOAST_ICON[t.tone];
          return (
            <motion.div key={t.id} layout initial={{ opacity: 0, x: 28, scale: 0.97 }} animate={{ opacity: 1, x: 0, scale: 1 }} exit={{ opacity: 0, x: 28 }}
              className="pointer-events-auto flex gap-2.5 rounded-xl border border-[var(--line-strong)] bg-[var(--panel-solid)] px-3.5 py-3 shadow-2xl">
              <Icon size={15} className={cx("mt-px shrink-0", TOAST_COLOR[t.tone])} />
              <div className="min-w-0 flex-1">
                <div className="text-[13px] text-ink">{t.title}</div>
                {t.body && <div className="mt-0.5 break-words text-xs text-soft">{t.body}</div>}
              </div>
              <button type="button" aria-label="Dismiss" onClick={() => dismiss(t.id)} className="self-start text-faint hover:text-ink"><X size={13} /></button>
            </motion.div>
          );
        })}
      </AnimatePresence>
    </div>
  );
}

export function ContextMenu() {
  const menu = useMenu((s) => s.menu);
  const hide = useMenu((s) => s.hide);
  useEffect(() => {
    if (!menu) return;
    const close = () => hide();
    window.addEventListener("click", close);
    window.addEventListener("blur", close);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("blur", close);
    };
  }, [menu, hide]);
  if (!menu) return null;
  return (
    <motion.ul role="menu" initial={{ opacity: 0, scale: 0.97 }} animate={{ opacity: 1, scale: 1 }} transition={{ duration: 0.09 }}
      className="fixed z-50 min-w-48 rounded-lg border border-[var(--line-strong)] bg-[var(--panel-solid)] p-1 shadow-2xl"
      style={{ left: Math.min(menu.x, window.innerWidth - 210), top: Math.min(menu.y, window.innerHeight - menu.items.length * 34 - 12) }}>
      {menu.items.map((item) => (
        <li key={item.label}>
          <button type="button" role="menuitem" onClick={item.onSelect}
            className={cx("flex h-[30px] w-full items-center rounded-md px-2.5 text-[13px] hover:bg-accent/10", item.danger ? "text-bad" : "text-ink")}>
            {item.label}
          </button>
        </li>
      ))}
    </motion.ul>
  );
}

interface PaletteItem { id: string; label: string; hint?: string; icon: LucideIcon; run: () => void }

export function CommandPalette() {
  const open = useApp((s) => s.paletteOpen);
  const setOpen = useApp((s) => s.setPalette);
  const navigate = useApp((s) => s.navigate);
  const updateSettings = useApp((s) => s.updateSettings);
  const [query, setQuery] = useState("");
  const [files, setFiles] = useState<DirEntry[]>([]);
  const [active, setActive] = useState(0);
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setQuery("");
      setFiles([]);
      setActive(0);
      setTimeout(() => input.current?.focus(), 20);
    }
  }, [open]);

  useEffect(() => {
    setActive(0);
    if (query.trim().length < 2) {
      setFiles([]);
      return;
    }
    const handle = setTimeout(() => api.searchFiles(query).then(setFiles).catch(() => setFiles([])), 200);
    return () => clearTimeout(handle);
  }, [query]);

  const items = useMemo<PaletteItem[]>(() => {
    const commands: PaletteItem[] = [
      ...NAV.flatMap((g) => g.items).map((i) => ({ id: i.route, label: i.label, hint: "Go to", icon: i.icon, run: () => navigate(i.route) })),
      { id: "scan", label: "Scan storage", hint: "Action", icon: RadarIcon, run: () => { navigate("storage"); void useStorage.getState().start(false); } },
      { id: "clean", label: "Scan for cleanup", hint: "Action", icon: RadarIcon, run: () => { navigate("cleaner"); void useCleaner.getState().start(); } },
      { id: "am", label: "Open Activity Monitor", hint: "Action", icon: Cpu, run: () => void api.openActivityMonitor() },
      ...["nexus", "arctic", "violet", "obsidian", "holographic"].map((theme) => ({
        id: `theme-${theme}`, label: `Theme: ${theme[0].toUpperCase()}${theme.slice(1)}`, hint: "Appearance", icon: Palette,
        run: () => void updateSettings({ theme }),
      })),
    ];
    const needle = query.trim().toLowerCase();
    const matched = needle ? commands.filter((c) => c.label.toLowerCase().includes(needle)) : commands.slice(0, 13);
    const fileItems: PaletteItem[] = files.slice(0, 12).map((f) => ({
      id: f.path, label: f.name, hint: f.size != null ? bytes(f.size) : undefined, icon: f.isDir ? Folder : FileText,
      run: () => api.revealInFinder(f.path).catch((e) => toast.error("Could not reveal item", e)),
    }));
    return [...matched, ...fileItems];
  }, [query, files, navigate, updateSettings]);

  const run = (item: PaletteItem) => {
    setOpen(false);
    item.run();
  };

  return (
    <AnimatePresence>
      {open && (
        <motion.div className="fixed inset-0 z-50 flex items-start justify-center bg-black/55 pt-[14vh] backdrop-blur-sm"
          initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.12 }}
          onClick={() => setOpen(false)}>
          <motion.div role="dialog" aria-label="Command palette" initial={{ y: -10, scale: 0.98 }} animate={{ y: 0, scale: 1 }} exit={{ y: -6, scale: 0.98 }} transition={{ duration: 0.14 }}
            className="glass glass-edge w-[600px] overflow-hidden bg-[var(--panel-solid)]" onClick={(e) => e.stopPropagation()}>
            <div className="flex items-center gap-2.5 border-b border-line px-4">
              <Search size={16} className="text-accent" />
              <input ref={input} value={query} aria-label="Search" onChange={(e) => setQuery(e.target.value)}
                placeholder="Search files, screens and commands"
                className="h-12 flex-1 bg-transparent text-[15px] text-ink outline-none placeholder:text-faint"
                onKeyDown={(e) => {
                  if (e.key === "ArrowDown") { e.preventDefault(); setActive((a) => Math.min(a + 1, items.length - 1)); }
                  if (e.key === "ArrowUp") { e.preventDefault(); setActive((a) => Math.max(a - 1, 0)); }
                  if (e.key === "Enter" && items[active]) run(items[active]);
                  if (e.key === "Escape") setOpen(false);
                }} />
            </div>
            <ul className="max-h-[380px] overflow-y-auto p-1.5">
              {items.length === 0 && <li className="px-3 py-6 text-center text-soft">No matches.</li>}
              {items.map((item, i) => {
                const Icon = item.icon;
                return (
                  <li key={item.id}>
                    <button type="button" onMouseEnter={() => setActive(i)} onClick={() => run(item)}
                      className={cx("flex h-9 w-full items-center gap-2.5 rounded-lg px-2.5 text-left", i === active ? "bg-accent/12 text-ink" : "text-soft")}>
                      <Icon size={15} strokeWidth={1.7} className={i === active ? "text-accent" : ""} />
                      <span className="truncate text-[13px]">{item.label}</span>
                      {item.hint && <span className="mono ml-auto shrink-0 text-[10px] text-faint">{item.hint}</span>}
                    </button>
                  </li>
                );
              })}
            </ul>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

export function MockBanner() {
  if (!usingMockData) return null;
  return (
    <div className="fixed bottom-3 left-1/2 z-50 -translate-x-1/2 rounded-full border border-warn/50 bg-warn/15 px-3 py-1 mono text-[11px] text-warn backdrop-blur">
      Browser preview: mock data. The desktop app uses live system data.
    </div>
  );
}
