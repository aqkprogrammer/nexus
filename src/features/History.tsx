import { useMemo, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { ChevronDown, ChevronRight, FileDown, History as HistoryIcon } from "lucide-react";
import { TimeChart } from "../components/viz/charts";
import { Button, EmptyState, Panel, ScreenHeader, Segmented, Skeleton, Status, Tag } from "../components/ui";
import { api, isTauri } from "../lib/ipc";
import { bytes, count, dateTime, memory, percent } from "../lib/format";
import { toast } from "../stores";
import type { CleanupLogItem, CleanupRun } from "../lib/types";
import { useLoad } from "./shared";

const SCAN_LABEL = { storage: "Storage", cleaner: "Cleaner", duplicates: "Duplicates" };

function RunRow({ run }: { run: CleanupRun }) {
  const [items, setItems] = useState<CleanupLogItem[] | null>(null);
  const [open, setOpen] = useState(false);
  const toggle = () => {
    setOpen((v) => !v);
    if (!items) api.getCleanupItems(run.id).then(setItems).catch((e) => toast.error("Could not load log", e));
  };
  return (
    <div className="border-b border-line/50 last:border-0">
      <button type="button" aria-expanded={open} onClick={toggle} className="row-hover flex h-10 w-full items-center gap-3 px-5 text-left">
        {open ? <ChevronDown size={14} className="text-faint" /> : <ChevronRight size={14} className="text-faint" />}
        <span className="w-28 text-[13px] capitalize text-ink">{run.source}</span>
        {run.dryRun && <Tag className="border-accent/30 text-accent">Dry run</Tag>}
        {run.status !== "completed" && <Status tone="warn">{run.status}</Status>}
        <span className="mono ml-auto text-xs text-soft">{dateTime(run.finishedAt)}</span>
        <span className="mono w-36 text-right text-xs text-soft">{count(run.itemsOk)} ok · {count(run.itemsFailed)} failed</span>
        <span className="mono w-20 text-right text-xs text-ink">{bytes(run.bytesFreed)}</span>
        <span className="mono w-20 text-right text-xs text-soft">{bytes(run.bytesTrashed)}</span>
      </button>
      {open && (
        <ul className="max-h-60 overflow-y-auto px-5 pb-2 pl-11">
          {items == null ? <Skeleton className="h-10" /> : items.map((item, i) => (
            <li key={`${item.path}-${i}`} className="flex h-6 items-center gap-3">
              <Tag>{item.outcome}</Tag>
              <span className="mono min-w-0 flex-1 truncate text-[11px] text-soft" title={item.detail ?? item.path}>{item.path}</span>
              <span className="mono text-[11px] text-soft">{bytes(item.size)}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

export function History() {
  const [days, setDays] = useState("30");
  const scans = useLoad(() => api.listScans(null, 200), []);
  const runs = useLoad(() => api.listCleanupRuns(100), []);
  const trends = useLoad(() => api.getMetricTrends(Number(days)), [days]);

  const realRuns = (runs.data ?? []).filter((r) => !r.dryRun);
  const recovered = realRuns.reduce((sum, r) => sum + r.bytesFreed, 0);
  const trashed = realRuns.reduce((sum, r) => sum + r.bytesTrashed, 0);
  const points = trends.data ?? [];

  const storageChange = useMemo(() => {
    const storageScans = (scans.data ?? []).filter((s) => s.kind === "storage" && s.diskUsed != null);
    if (storageScans.length < 2) return null;
    return storageScans[0].diskUsed! - storageScans[storageScans.length - 1].diskUsed!;
  }, [scans.data]);

  const exportReport = async (format: "md" | "json") => {
    if (!isTauri) {
      toast.info("Export runs in the desktop app");
      return;
    }
    const path = await save({
      title: "Save system health report",
      defaultPath: `NEXUS-health-report.${format}`,
      filters: [{ name: format === "md" ? "Markdown" : "JSON", extensions: [format] }],
    });
    if (!path) return;
    api.exportReport(path).then(() => toast.good("Report saved", path)).catch((e) => toast.error("Could not save report", e));
  };

  const healthSeries = [
    { key: "health", label: "Overall", color: "var(--accent)" },
    { key: "storageScore", label: "Storage", color: "#e4569f" },
    { key: "memoryScore", label: "Memory", color: "#b0851a" },
  ];

  return (
    <div className="space-y-4">
      <ScreenHeader title="Scan History" subtitle="How this Mac has changed over time. Stored locally.">
        <Button icon={FileDown} onClick={() => void exportReport("md")}>Export Markdown</Button>
        <Button icon={FileDown} onClick={() => void exportReport("json")}>Export JSON</Button>
      </ScreenHeader>

      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        {[
          ["Space recovered", bytes(recovered), "Permanently removed"],
          ["Moved to Trash", bytes(trashed), "Recoverable until emptied"],
          ["Cleanups", count(realRuns.length), `${count((runs.data ?? []).length - realRuns.length)} dry runs`],
          ["Disk use change", storageChange == null ? "Unavailable" : `${storageChange >= 0 ? "+" : "−"}${bytes(Math.abs(storageChange))}`, "Across recorded scans"],
        ].map(([label, value, hint]) => (
          <div key={label} className="glass p-4">
            <div className="label">{label}</div>
            <div className="display mt-1 text-2xl text-ink">{value}</div>
            <div className="text-xs text-soft">{hint}</div>
          </div>
        ))}
      </div>

      <div className="flex justify-end">
        <Segmented layoutId="history-range" value={days} onChange={setDays}
          options={[{ value: "7", label: "7 days" }, { value: "30", label: "30 days" }, { value: "180", label: "180 days" }]} />
      </div>
      <div className="grid gap-4 xl:grid-cols-2">
        <Panel title="Health scores">
          {points.length < 2 ? <div className="grid h-44 place-items-center px-6 text-center text-soft">Trends build up as NEXUS runs. A sample is saved every five minutes.</div> : (
            <>
              <TimeChart data={points} height={176} kind="line" domain={[0, 100]} series={healthSeries} format={(v) => v.toFixed(0)} labelFormat={dateTime} />
              <div className="mt-2 flex flex-wrap gap-x-4 text-xs text-soft">
                {healthSeries.map((s) => <span key={s.key} className="inline-flex items-center gap-1.5"><span className="h-0.5 w-3 rounded" style={{ background: s.color }} />{s.label}</span>)}
              </div>
            </>
          )}
        </Panel>
        <Panel title="Storage used">
          {points.length < 2 ? <div className="grid h-44 place-items-center text-soft">Not enough samples yet.</div> : (
            <TimeChart data={points} height={196} series={[{ key: "diskUsed", label: "Used", color: "var(--accent)" }]} format={(v) => bytes(v)} axisFormat={(v) => bytes(v, 0)} labelFormat={dateTime} />
          )}
        </Panel>
        <Panel title="CPU trend">
          {points.length < 2 ? <div className="grid h-36 place-items-center text-soft">Not enough samples yet.</div> : (
            <TimeChart data={points} height={150} domain={[0, 100]} series={[{ key: "cpu", label: "CPU", color: "var(--accent-3)" }]} format={(v) => percent(v)} labelFormat={dateTime} />
          )}
        </Panel>
        <Panel title="Memory trend">
          {points.length < 2 ? <div className="grid h-36 place-items-center text-soft">Not enough samples yet.</div> : (
            <TimeChart data={points} height={150} series={[{ key: "memoryUsed", label: "Memory used", color: "var(--accent-2)" }]} format={(v) => memory(v)} axisFormat={(v) => memory(v, 0)} labelFormat={dateTime} />
          )}
        </Panel>
      </div>

      <Panel padded={false} title="Cleanup log" aside={<span className="mono text-[10px] text-faint">Freed · Trash</span>}>
        {runs.loading && !runs.data ? <Skeleton className="m-5 h-16" /> : (runs.data ?? []).length === 0 ? (
          <p className="px-5 py-5 text-soft">No cleanups yet. Every run, including dry runs, is recorded here item by item.</p>
        ) : (runs.data ?? []).map((run) => <RunRow key={run.id} run={run} />)}
      </Panel>

      <Panel padded={false} title="Previous scans">
        {scans.loading && !scans.data ? <Skeleton className="m-5 h-16" /> : (scans.data ?? []).length === 0 ? (
          <EmptyState icon={HistoryIcon} title="No scans yet" body="Completed scans appear here." />
        ) : (
          <div className="max-h-80 overflow-y-auto">
            {(scans.data ?? []).map((scan) => (
              <div key={scan.id} className="flex h-9 items-center gap-3 border-b border-line/40 px-5">
                <span className="w-24 text-[13px] text-ink">{SCAN_LABEL[scan.kind]}</span>
                <span className="mono text-xs text-soft">{dateTime(scan.finishedAt)}</span>
                <span className="mono ml-auto w-28 text-right text-xs text-soft">{count(scan.files)} {scan.kind === "cleaner" ? "items" : "files"}</span>
                <span className="mono w-20 text-right text-xs text-ink">{bytes(scan.totalBytes)}</span>
                <span className="mono w-24 text-right text-xs text-soft">{scan.reclaimable ? `${bytes(scan.reclaimable)} free` : ""}</span>
                <span className="mono w-14 text-right text-xs text-soft">{scan.healthScore != null ? Math.round(scan.healthScore) : "—"}</span>
              </div>
            ))}
          </div>
        )}
      </Panel>
    </div>
  );
}
