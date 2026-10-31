import { useMemo, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import {
  AppWindow, ChevronDown, ChevronRight, Code2, Container, Globe, ListChecks, Radar as RadarIcon, ShieldCheck, Sparkles, Trash2, TriangleAlert, type LucideIcon,
} from "lucide-react";
import { Button, Check, EmptyState, Panel, RiskBadge, ScreenHeader, Status, Tag, cx } from "../components/ui";
import { api } from "../lib/ipc";
import { ago, bytes, count, dateFromSeconds, shortPath } from "../lib/format";
import { useCleaner, usePlan } from "../stores";
import type { CleanerGroup, CleanerReport, CleanerTarget, DockerResource } from "../lib/types";
import { ScanExperience } from "./ScanExperience";
import { FullDiskNotice, itemMenu, reveal, useHome } from "./shared";

const GROUPS: { id: CleanerGroup; title: string; icon: LucideIcon; note: string }[] = [
  { id: "systemJunk", title: "System junk", icon: Sparkles, note: "Temporary files, caches and logs that apps recreate." },
  { id: "browser", title: "Browser cleanup", icon: Globe, note: "Caches only. Saved passwords, cookies, history and sessions are never included." },
  { id: "developer", title: "Developer cleanup", icon: Code2, note: "Rebuildable caches. Off by default, since rebuilding takes time." },
  { id: "leftovers", title: "Application leftovers", icon: AppWindow, note: "Possible leftovers of removed apps. Review each one; they always go to the Trash." },
];

function TargetRow({ target }: { target: CleanerTarget }) {
  const { selected, setSelected } = useCleaner();
  const home = useHome();
  const [open, setOpen] = useState(false);
  const picked = target.items.filter((i) => selected.has(i.id));
  const all = picked.length === target.items.length && target.items.length > 0;
  const set = (ids: string[], on: boolean) => {
    const next = new Set(selected);
    ids.forEach((id) => (on ? next.add(id) : next.delete(id)));
    setSelected(next);
  };
  return (
    <div className="border-b border-line/50 last:border-0">
      <div className="row-hover flex items-center gap-3 px-5 py-2.5" onClick={() => target.items.length && setOpen((v) => !v)}>
        <Check label={`Select ${target.title}`} checked={all} indeterminate={picked.length > 0 && !all}
          onChange={(on) => set(target.items.map((i) => i.id), on)} />
        <span className="text-faint">{target.items.length > 0 ? (open ? <ChevronDown size={14} /> : <ChevronRight size={14} />) : <span className="inline-block w-3.5" />}</span>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="text-[13px] text-ink">{target.title}</span>
            <RiskBadge risk={target.risk} />
            {target.appRunning && <Status tone="warn">{target.appRunning} is running</Status>}
          </div>
          <p className="truncate text-xs text-soft" title={target.unavailable ?? target.description}>
            {target.items.length > 0 ? target.description : target.unavailable ?? target.description}
            {target.items.length > 0 && target.unavailable && <span className="text-faint"> Some locations were not accessible.</span>}
          </p>
        </div>
        <span className="mono shrink-0 text-xs text-faint">{count(target.items.length)}</span>
        <span className="mono w-20 shrink-0 text-right text-[13px] text-ink">{bytes(target.total)}</span>
      </div>
      <AnimatePresence initial={false}>
        {open && (
          <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }} className="overflow-hidden">
            <p className="flex items-start gap-1.5 px-5 pb-2 pl-[72px] text-xs text-soft">
              <ShieldCheck size={12} className="mt-0.5 shrink-0 text-accent" />{target.effect}
            </p>
            <ul className="max-h-64 overflow-y-auto pb-2">
              {target.items.slice(0, 300).map((item) => (
                <li key={item.id} onContextMenu={itemMenu(item.path, "cleaner")} onDoubleClick={() => reveal(item.path)}
                  className="row-hover flex h-8 items-center gap-3 pl-[72px] pr-5">
                  <Check label={`Select ${item.name}`} checked={selected.has(item.id)} onChange={(on) => set([item.id], on)} />
                  <span className="min-w-0 flex-1 truncate text-xs text-ink" title={item.path}>{shortPath(item.path, home)}</span>
                  <span className="mono shrink-0 text-[10px] text-faint">{dateFromSeconds(item.modified)}</span>
                  <span className="mono w-16 shrink-0 text-right text-xs text-soft">{bytes(item.size)}</span>
                </li>
              ))}
              {target.items.length > 300 && <li className="pl-[72px] pt-1 text-xs text-faint">Showing the 300 largest of {count(target.items.length)} items.</li>}
            </ul>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function DockerPanel({ report }: { report: CleanerReport }) {
  const { docker, setDocker } = useCleaner();
  const status = report.docker;
  if (!status.installed) return null;
  const toggle = (resource: DockerResource, on: boolean) => {
    const next = new Set(docker);
    if (on) next.add(resource.id); else next.delete(resource.id);
    setDocker(next);
  };
  return (
    <Panel padded={false} title={<span className="flex items-center gap-2"><Container size={13} />Docker</span>}
      aside={<span className="text-xs text-soft">Previewed with <span className="mono">docker system df</span>, cleaned with Docker's own prune</span>}>
      {!status.running ? <p className="px-5 py-4 text-soft">{status.message}</p> : status.resources.map((resource) => (
        <div key={resource.id} className="flex items-center gap-3 border-b border-line/50 px-5 py-2.5 last:border-0">
          <Check label={`Select ${resource.label}`} checked={docker.has(resource.id)} onChange={(on) => toggle(resource, on)} />
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <span className="text-[13px] text-ink">{resource.label}</span>
              <RiskBadge risk={resource.id === "volumes" ? "high" : "low"} />
            </div>
            <p className="text-xs text-soft">
              {resource.id === "volumes" ? "Volumes hold container data. Only prune if you know it is disposable." : `${resource.total} total, ${resource.active} active`}
            </p>
          </div>
          <span className="mono shrink-0 text-xs text-faint">of {bytes(resource.size)}</span>
          <span className="mono w-20 shrink-0 text-right text-[13px] text-ink">{bytes(resource.reclaimable)}</span>
        </div>
      ))}
    </Panel>
  );
}

function TrashPanel({ report, onDone }: { report: CleanerReport; onDone: () => void }) {
  const home = useHome();
  const [open, setOpen] = useState(false);
  const { trash } = report;
  return (
    <Panel padded={false} title={<span className="flex items-center gap-2"><Trash2 size={13} />Trash</span>}
      aside={<Button size="sm" variant="danger" disabled={trash.accessible && trash.count === 0}
        onClick={() => void usePlan.getState().open((dry) => api.prepareEmptyTrash(dry), onDone)}>Empty Trash…</Button>}>
      {trash.accessible ? (
        <>
          <button type="button" className="row-hover flex w-full items-center gap-3 px-5 py-3 text-left" onClick={() => setOpen((v) => !v)}>
            {open ? <ChevronDown size={14} className="text-faint" /> : <ChevronRight size={14} className="text-faint" />}
            <span className="text-[13px] text-ink">{count(trash.count)} {trash.count === 1 ? "item" : "items"}</span>
            <span className="mono ml-auto text-[13px] text-ink">{bytes(trash.total)}</span>
          </button>
          {open && (
            <ul className="max-h-56 overflow-y-auto pb-2">
              {trash.items.slice(0, 200).map((item) => (
                <li key={item.id} className="flex h-7 items-center gap-3 px-5 pl-11">
                  <span className="min-w-0 flex-1 truncate text-xs text-ink" title={shortPath(item.path, home)}>{item.name}</span>
                  <span className="mono text-xs text-soft">{bytes(item.size)}</span>
                </li>
              ))}
            </ul>
          )}
        </>
      ) : (
        <p className="px-5 py-3 text-xs text-soft">
          macOS does not let NEXUS list the Trash without Full Disk Access, so its size is unavailable. You can still ask Finder to empty it.
        </p>
      )}
    </Panel>
  );
}

function ScanView() {
  const { progress, findings, cancel } = useCleaner();
  return (
    <ScanExperience
      title="Analyzing cleanup locations"
      phase={progress?.phase ?? "Starting"}
      percent={progress?.percent ?? 0}
      current={progress?.current ?? ""}
      onCancel={cancel}
      stats={[
        { label: "Reclaimable found", value: bytes(progress?.foundBytes ?? 0) },
        { label: "Categories found", value: count(findings.length) },
        { label: "Files changed", value: "0" },
        { label: "Mode", value: "Read only" },
      ]}
      feedTitle="Findings"
      feed={findings.map((f) => ({ key: f.title, title: f.title, detail: bytes(f.bytes) }))}
    />
  );
}

export function Cleaner() {
  const { report, scanning, start, selected, docker, setSelected, setDocker } = useCleaner();
  const selectedBytes = useMemo(() => {
    if (!report) return 0;
    const items = report.targets.flatMap((t) => t.items).filter((i) => selected.has(i.id)).reduce((sum, i) => sum + i.size, 0);
    const dockerBytes = report.docker.resources.filter((r) => docker.has(r.id)).reduce((sum, r) => sum + r.reclaimable, 0);
    return items + dockerBytes;
  }, [report, selected, docker]);

  const rescan = () => {
    setSelected(new Set());
    setDocker(new Set());
    void start();
  };

  const review = () => {
    const items = [...selected];
    const dockerIds = [...docker];
    void usePlan.getState().open((dry) => api.prepareCleanup(items, dockerIds, dry), rescan);
  };

  const header = (
    <ScreenHeader title="Smart Cleaner" subtitle="Reclaim space safely. You approve every removal.">
      {report && !scanning && <Button icon={RadarIcon} onClick={() => void start()}>Scan again</Button>}
    </ScreenHeader>
  );

  if (scanning) return <div className="space-y-4">{header}<ScanView /></div>;
  if (!report) {
    return (
      <div className="space-y-4">
        {header}
        <Panel edge>
          <EmptyState icon={ListChecks} title="Find what can be safely reclaimed"
            body="NEXUS checks known cache, log and temporary locations, and shows every item before anything is removed. Scanning changes nothing.">
            <Button variant="primary" icon={RadarIcon} onClick={() => void start()}>Scan System</Button>
          </EmptyState>
        </Panel>
      </div>
    );
  }

  return (
    <div className="space-y-4 pb-20">
      {header}
      <div className="grid grid-cols-3 gap-3">
        {[
          ["Recommended", bytes(report.recommended), "Low risk, preselected"],
          ["Total reclaimable", bytes(report.reclaimable), "Including items to review"],
          ["Scanned", ago(report.scannedAt), `Finished in ${(report.durationMs / 1000).toFixed(1)}s`],
        ].map(([label, value, hint]) => (
          <div key={label} className="glass p-4">
            <div className="label">{label}</div>
            <div className="display mt-1 text-2xl text-ink">{value}</div>
            <div className="text-xs text-soft">{hint}</div>
          </div>
        ))}
      </div>
      <FullDiskNotice />

      {GROUPS.map((group) => {
        const targets = report.targets.filter((t) => t.group === group.id && (t.items.length > 0 || t.unavailable));
        const Icon = group.icon;
        const total = targets.reduce((sum, t) => sum + t.total, 0);
        return (
          <Panel key={group.id} padded={false}
            title={<span className="flex items-center gap-2"><Icon size={13} />{group.title}</span>}
            aside={<span className="mono text-xs text-soft">{bytes(total)}</span>}>
            <p className={cx("px-5 pb-2 pt-2 text-xs text-soft", targets.length > 0 && "border-b border-line/50")}>{group.note}</p>
            {targets.length === 0 ? <p className="px-5 py-3 text-xs text-faint">Nothing to clean here.</p> : targets.map((t) => <TargetRow key={t.id} target={t} />)}
          </Panel>
        );
      })}

      <DockerPanel report={report} />
      <TrashPanel report={report} onDone={rescan} />

      <div className="sticky bottom-0 z-10 flex items-center gap-4 rounded-xl border border-accent/30 bg-[var(--panel-solid)]/95 px-5 py-3 shadow-[0_-12px_32px_-18px_var(--accent)] backdrop-blur">
        <div>
          <div className="label">Selected</div>
          <div className="display text-xl text-ink">{bytes(selectedBytes)}</div>
        </div>
        <Tag>{count(selected.size + docker.size)} items</Tag>
        <span className="flex items-center gap-1.5 text-xs text-soft"><TriangleAlert size={12} />Nothing is removed until you confirm the preview.</span>
        <Button variant="primary" className="ml-auto" disabled={selected.size + docker.size === 0} onClick={review}>Review cleanup</Button>
      </div>
    </div>
  );
}
