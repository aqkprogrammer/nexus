import { useMemo } from "react";
import { motion } from "framer-motion";
import {
  AppWindow, ArrowRight, Code2, Copy, Download, HardDrive, Info, Sparkles, Trash2, Wand2, type LucideIcon,
} from "lucide-react";
import { Ring } from "../components/viz/gauges";
import { Button, Panel, RiskBadge, ScreenHeader, Skeleton, Status, scoreTone } from "../components/ui";
import { api } from "../lib/ipc";
import { bytes } from "../lib/format";
import { CATEGORY_COLOR, SYSTEM_COLOR, optimizationScore, storageSegments } from "../lib/score";
import { useApp, useCleaner, useDuplicates, usePlan, useStorage, useTelemetry, type Route } from "../stores";
import type { Category, CleanerGroup, Risk } from "../lib/types";

interface Action {
  id: string; title: string; detail: string; icon: LucideIcon; risk: Risk; bytes: number | null;
  needsScan?: { label: string; route: Route };
  run?: { label: string; go: () => void };
}

/** Safe, reversible wins rank above larger items that need human judgement. */
const WEIGHT: Record<Risk, number> = { low: 8, medium: 1, high: 0.4 };

export function Optimize() {
  const volume = useTelemetry((s) => s.sample?.volume ?? null);
  const storage = useStorage((s) => s.summary);
  const cleaner = useCleaner((s) => s.report);
  const startCleaner = useCleaner((s) => s.start);
  const duplicates = useDuplicates((s) => s.report);
  const navigate = useApp((s) => s.navigate);

  const score = optimizationScore({ volume, storage, cleaner, duplicates });

  const actions = useMemo<Action[]>(() => {
    const group = (id: CleanerGroup) => (cleaner?.targets ?? []).filter((t) => t.group === id);
    const total = (id: CleanerGroup) => group(id).reduce((sum, t) => sum + t.total, 0);
    const refresh = () => void startCleaner();
    const cleanItems = (ids: string[]) => () => void usePlan.getState().open((dry) => api.prepareCleanup(ids, [], dry), refresh);
    const recommendedIds = (cleaner?.targets ?? [])
      .filter((t) => t.selectedByDefault && !t.appRunning)
      .flatMap((t) => t.items.map((i) => i.id));
    const developerIds = group("developer").filter((t) => t.risk === "low").flatMap((t) => t.items.map((i) => i.id));
    const scanCleaner = { label: "Scan first", route: "cleaner" as Route };
    const scanStorage = { label: "Scan first", route: "storage" as Route };

    const list: Action[] = [
      {
        id: "recommended", title: "Recommended cleanup", detail: "Caches, logs and temporary files apps can recreate.", icon: Sparkles, risk: "low",
        bytes: cleaner?.recommended ?? null, needsScan: cleaner ? undefined : scanCleaner,
        run: recommendedIds.length ? { label: "Preview", go: cleanItems(recommendedIds) } : undefined,
      },
      {
        id: "developer", title: "Developer cache", detail: "Rebuildable caches from Xcode, npm, Cargo and friends.", icon: Code2, risk: "low",
        bytes: cleaner ? total("developer") : null, needsScan: cleaner ? undefined : scanCleaner,
        run: developerIds.length ? { label: "Preview", go: cleanItems(developerIds) } : undefined,
      },
      {
        id: "trash", title: "Trash", detail: cleaner && !cleaner.trash.accessible ? "Size needs Full Disk Access." : "Files stay on disk until the Trash is emptied.", icon: Trash2, risk: "medium",
        bytes: cleaner?.trash.accessible ? cleaner.trash.total : null, needsScan: cleaner ? undefined : scanCleaner,
        run: cleaner && (!cleaner.trash.accessible || cleaner.trash.count > 0)
          ? { label: "Preview", go: () => void usePlan.getState().open((dry) => api.prepareEmptyTrash(dry), refresh) } : undefined,
      },
      {
        id: "leftovers", title: "Application leftovers", detail: "Possible leftovers from removed apps, for manual review.", icon: AppWindow, risk: "medium",
        bytes: cleaner ? total("leftovers") : null, needsScan: cleaner ? undefined : scanCleaner,
        run: { label: "Review", go: () => navigate("cleaner") },
      },
      {
        id: "duplicates", title: "Duplicate files", detail: "Identical copies. You choose which to keep.", icon: Copy, risk: "medium",
        bytes: duplicates?.recoverable ?? null, needsScan: duplicates ? undefined : { label: "Scan first", route: "duplicates" },
        run: { label: "Review", go: () => navigate("duplicates") },
      },
      {
        id: "downloads", title: "Old downloads", detail: storage ? `Unchanged for ${storage.old.cutoffMonths}+ months. Age alone does not make them unneeded.` : "Files you may have finished with.", icon: Download, risk: "medium",
        bytes: storage?.old.downloadsBytes ?? null, needsScan: storage ? undefined : scanStorage,
        run: { label: "Review", go: () => navigate("hogs") },
      },
      {
        id: "large", title: "Large files", detail: "Files over 1 GB, worth moving or removing.", icon: HardDrive, risk: "medium",
        bytes: storage?.over1Gb[1] ?? null, needsScan: storage ? undefined : scanStorage,
        run: { label: "Review", go: () => navigate("storage") },
      },
    ];
    return list.sort((a, b) => (b.bytes ?? 0) * WEIGHT[b.risk] - (a.bytes ?? 0) * WEIGHT[a.risk]);
  }, [cleaner, duplicates, storage, navigate, startCleaner]);

  const potential = actions.filter((a) => a.id !== "large").reduce((sum, a) => sum + (a.bytes ?? 0), 0);
  const segments = volume && storage ? storageSegments(volume, storage) : [];

  return (
    <div className="space-y-4">
      <ScreenHeader title="Optimization Center" subtitle="One prioritized list. Every action shows its preview before it runs." />
      <div className="grid grid-cols-12 gap-4">
        <Panel edge className="col-span-12 lg:col-span-4" title="Optimization score">
          <div className="flex items-center gap-5">
            <Ring value={score} size={132} stroke={9}>
              <span className="display text-[38px] leading-none text-ink">{score ?? "--"}</span>
            </Ring>
            <div className="min-w-0">
              <Status tone={scoreTone(score)}>{score == null ? "Reading disk" : score >= 85 ? "Well optimized" : score >= 60 ? "Room to improve" : "Needs attention"}</Status>
              <div className="display mt-2 text-2xl text-ink">{bytes(potential)}</div>
              <div className="text-xs text-soft">potential recovery measured so far</div>
            </div>
          </div>
          <p className="mt-3 flex items-start gap-1.5 text-[11px] text-faint">
            <Info size={12} className="mt-px shrink-0" />A NEXUS heuristic from free space and measured reclaimable data.
          </p>
        </Panel>

        <Panel className="col-span-12 lg:col-span-8" title="Storage distribution">
          {!volume ? <Skeleton className="h-20" /> : segments.length === 0 ? (
            <div className="flex items-center justify-between gap-4">
              <p className="text-soft">Run a storage scan to see where your space goes.</p>
              <Button onClick={() => navigate("storage")}>Open Storage Intelligence</Button>
            </div>
          ) : (
            <>
              <div role="img" aria-label="Storage distribution" className="flex h-4 gap-[2px] overflow-hidden rounded-md bg-white/5">
                {segments.map((s) => (
                  <motion.div key={s.id} title={`${s.label}: ${bytes(s.bytes)}`} initial={{ width: 0 }} animate={{ width: `${(s.bytes / volume.total) * 100}%` }} transition={{ duration: 0.7 }}
                    style={{ background: s.id === "system" ? SYSTEM_COLOR : CATEGORY_COLOR[s.id as Category] }} />
                ))}
              </div>
              <ul className="mt-3 grid grid-cols-2 gap-x-6 gap-y-1 md:grid-cols-3">
                {segments.map((s) => (
                  <li key={s.id} className="flex items-center gap-2 text-xs">
                    <span className="size-2.5 shrink-0 rounded-sm" style={{ background: s.id === "system" ? SYSTEM_COLOR : CATEGORY_COLOR[s.id as Category] }} />
                    <span className="truncate text-soft">{s.label}</span>
                    <span className="mono ml-auto text-ink">{bytes(s.bytes)}</span>
                  </li>
                ))}
              </ul>
            </>
          )}
        </Panel>
      </div>

      <Panel padded={false} title="Prioritized actions" aside={<span className="flex items-center gap-1.5 text-xs text-soft"><Wand2 size={12} />Ranked by space and risk</span>}>
        {actions.map((action, i) => {
          const Icon = action.icon;
          return (
            <motion.div key={action.id} layout initial={{ opacity: 0, x: -8 }} animate={{ opacity: 1, x: 0 }} transition={{ delay: i * 0.03 }}
              className="flex items-center gap-3.5 border-b border-line/50 px-5 py-3 last:border-0">
              <span className="mono w-5 shrink-0 text-xs text-faint">{i + 1}</span>
              <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-accent/10 text-accent"><Icon size={16} strokeWidth={1.6} /></span>
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="text-[13px] text-ink">{action.title}</span>
                  <RiskBadge risk={action.risk} />
                </div>
                <p className="truncate text-xs text-soft">{action.detail}</p>
              </div>
              <span className="mono w-24 shrink-0 text-right text-[13px] text-ink">{action.bytes != null ? bytes(action.bytes) : "Unavailable"}</span>
              <div className="flex w-28 shrink-0 justify-end">
                {action.needsScan ? (
                  <Button size="sm" onClick={() => navigate(action.needsScan!.route)}>{action.needsScan.label}</Button>
                ) : action.run ? (
                  <Button size="sm" variant="primary" onClick={action.run.go}>{action.run.label}<ArrowRight size={12} /></Button>
                ) : <span className="text-xs text-faint">Nothing to do</span>}
              </div>
            </motion.div>
          );
        })}
      </Panel>
    </div>
  );
}
