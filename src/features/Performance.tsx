import { useCallback, useMemo, useState } from "react";
import { ArrowDown, ArrowUp, ExternalLink, OctagonX, Search } from "lucide-react";
import { Legend, TimeChart } from "../components/viz/charts";
import { Button, Input, KeyValue, Panel, ScreenHeader, Skeleton, Status, Tag, cx, type Tone } from "../components/ui";
import { api } from "../lib/ipc";
import { bytes, clock, count, duration, memory as mem, percent, rate } from "../lib/format";
import { toast, useTelemetry } from "../stores";
import type { ProcessInfo, ProcessSnapshot } from "../lib/types";
import { Drawer, reveal, useInterval, useLoad } from "./shared";

const MEMORY_PARTS = [
  { key: "app", label: "App memory", color: "#08a0bc" },
  { key: "wired", label: "Wired", color: "#3f7ff0" },
  { key: "compressed", label: "Compressed", color: "#9a68f0" },
  { key: "cached", label: "Cached files", color: "#1b9d74" },
] as const;

function coreColor(value: number) {
  return `color-mix(in srgb, var(--accent) ${Math.round(12 + value * 0.88)}%, transparent)`;
}

function pressureTone(pressure: string): Tone {
  return pressure === "critical" ? "bad" : pressure === "warning" ? "warn" : pressure === "normal" ? "good" : "neutral";
}

function TopList({ title, items, value }: { title: string; items: ProcessInfo[]; value: (p: ProcessInfo) => string }) {
  return (
    <div>
      <div className="label mb-1.5">{title}</div>
      <ul>
        {items.map((p) => (
          <li key={p.pid} className="flex h-6 items-center gap-2">
            <span className="min-w-0 flex-1 truncate text-xs text-ink">{p.name}</span>
            <span className="mono text-[10px] text-faint">{p.pid}</span>
            <span className="mono w-16 text-right text-xs text-soft">{value(p)}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

function ProcessDrawer({ process, onClose, onDone }: { process: ProcessInfo | null; onClose: () => void; onDone: () => void }) {
  const [confirming, setConfirming] = useState<null | "term" | "kill">(null);
  const close = () => { setConfirming(null); onClose(); };
  const terminate = (force: boolean) => {
    if (!process) return;
    api.terminateProcess(process.pid, process.startTime, force)
      .then(() => { toast.good(`${process.name} ${force ? "force quit" : "asked to quit"}`); onDone(); close(); })
      .catch((e) => { toast.error("Could not end process", e); setConfirming(null); });
  };
  const eligible = process?.owned && !process.protected;
  return (
    <Drawer open={process != null} onClose={close} title={process?.name ?? ""} subtitle={process?.exe ?? undefined}
      footer={process && (confirming ? (
        <div className="w-full">
          <p className="mb-2 text-xs text-soft">
            {confirming === "kill"
              ? `Force quit ${process.name}? It cannot save its work.`
              : `Ask ${process.name} to quit? Unsaved work may be lost.`}
          </p>
          <div className="flex gap-2">
            <Button onClick={() => setConfirming(null)}>Cancel</Button>
            <Button variant="danger" onClick={() => terminate(confirming === "kill")}>{confirming === "kill" ? "Force quit" : "Quit process"}</Button>
          </div>
        </div>
      ) : (
        <>
          {process.exe && <Button icon={ExternalLink} onClick={() => reveal(process.exe!)}>Reveal</Button>}
          <Button icon={OctagonX} variant="danger" disabled={!eligible} onClick={() => setConfirming("term")}>Quit</Button>
          <Button variant="danger" disabled={!eligible} onClick={() => setConfirming("kill")}>Force quit</Button>
        </>
      ))}>
      {process && (
        <>
          {!eligible && (
            <div className="mb-3">
              <Status tone="neutral">{process.protected ? "Protected system process" : "Belongs to another user"}: it cannot be ended here.</Status>
            </div>
          )}
          <KeyValue label="PID" value={process.pid} />
          <KeyValue label="Parent PID" value={process.parentPid} />
          <KeyValue label="Application" value={process.app} mono={false} />
          <KeyValue label="CPU" value={percent(process.cpu, 1)} />
          <KeyValue label="Memory" value={mem(process.memory)} />
          <KeyValue label="Status" value={process.status} mono={false} />
          <KeyValue label="Running for" value={duration(process.runTime)} />
          <KeyValue label="Disk read" value={rate(process.diskRead)} />
          <KeyValue label="Disk write" value={rate(process.diskWrite)} />
        </>
      )}
    </Drawer>
  );
}

type SortKey = "cpu" | "memory" | "name";
const PROCESS_ROWS = 800;

function ProcessTable({ snapshot, reload }: { snapshot: ProcessSnapshot | null; reload: () => void }) {
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<SortKey>("cpu");
  const [view, setView] = useState<"processes" | "apps">("processes");
  const [details, setDetails] = useState<number | null>(null);
  const rows = useMemo(() => {
    const needle = query.toLowerCase();
    return (snapshot?.processes ?? [])
      .filter((p) => !needle || p.name.toLowerCase().includes(needle) || String(p.pid).includes(needle) || p.app?.toLowerCase().includes(needle))
      .sort((a, b) => (sort === "cpu" ? b.cpu - a.cpu : sort === "memory" ? b.memory - a.memory : a.name.localeCompare(b.name)));
  }, [snapshot, query, sort]);
  const selected = snapshot?.processes.find((p) => p.pid === details) ?? null;
  const SortHead = ({ id, label, className }: { id: SortKey; label: string; className: string }) => (
    <button type="button" onClick={() => setSort(id)} className={cx(className, "label flex items-center gap-1 hover:text-ink", sort === id && "!text-accent")}>
      {sort === id && <ArrowDown size={10} />}{label}
    </button>
  );
  return (
    <Panel padded={false} title="Process manager" aside={
      <div className="flex items-center gap-2">
        <div className="relative">
          <Search size={13} className="absolute left-2.5 top-2 text-faint" />
          <Input aria-label="Search processes" value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Search processes" className="!h-7 w-48 pl-7 text-xs" />
        </div>
        <Button size="sm" onClick={() => setView((v) => (v === "apps" ? "processes" : "apps"))}>{view === "apps" ? "Show processes" : "Group by app"}</Button>
        <Button size="sm" icon={ExternalLink} onClick={() => void api.openActivityMonitor()}>Activity Monitor</Button>
      </div>
    }>
      {view === "apps" ? (
        <div className="h-[420px] overflow-y-auto">
          {(snapshot?.groups ?? []).slice(0, 80).map((group) => (
            <div key={group.name} className="flex h-8 items-center gap-3 border-b border-line/40 px-5">
              <span className="min-w-0 flex-1 truncate text-[13px] text-ink">{group.name}</span>
              <Tag>{group.processes} processes</Tag>
              <span className="mono w-16 text-right text-xs text-soft">{percent(group.cpu, 1)}</span>
              <span className="mono w-20 text-right text-xs text-ink">{mem(group.memory)}</span>
            </div>
          ))}
        </div>
      ) : (
        <>
          <div className="flex items-center gap-3 border-b border-line px-5 py-2">
            <SortHead id="name" label="Process" className="min-w-0 flex-1" />
            <span className="label w-16 text-right">PID</span>
            <span className="label w-20">Status</span>
            <SortHead id="cpu" label="CPU" className="w-16 justify-end" />
            <SortHead id="memory" label="Memory" className="w-20 justify-end" />
          </div>
          <div className="h-[420px] overflow-y-auto">
            {snapshot == null ? <div className="space-y-2 p-5">{Array.from({ length: 8 }, (_, i) => <Skeleton key={i} className="h-5" />)}</div> : rows.slice(0, PROCESS_ROWS).map((p) => (
              <button key={p.pid} type="button" onClick={() => setDetails(p.pid)} className="row-hover flex h-8 w-full items-center gap-3 border-b border-line/40 px-5 text-left">
                <span className="min-w-0 flex-1 truncate text-[13px] text-ink">{p.name}</span>
                {p.protected && <Tag>System</Tag>}
                <span className="mono w-16 text-right text-xs text-faint">{p.pid}</span>
                <span className="w-20 text-xs text-soft">{p.status}</span>
                <span className="mono w-16 text-right text-xs text-ink">{percent(p.cpu, 1)}</span>
                <span className="mono w-20 text-right text-xs text-ink">{mem(p.memory)}</span>
              </button>
            ))}
          </div>
          {rows.length > PROCESS_ROWS && (
            <div className="border-t border-line px-5 py-2 text-xs text-faint">Showing {PROCESS_ROWS} of {count(rows.length)}. Search to find the rest.</div>
          )}
        </>
      )}
      <ProcessDrawer process={selected} onClose={() => setDetails(null)} onDone={reload} />
    </Panel>
  );
}

export function Performance() {
  const sample = useTelemetry((s) => s.sample);
  const history = useTelemetry((s) => s.history);
  const processes = useLoad(() => api.listProcesses(), []);
  const refresh = useCallback(() => { api.listProcesses().then(processes.setData).catch(() => {}); }, []); // eslint-disable-line react-hooks/exhaustive-deps
  useInterval(refresh, 3000);

  const snapshot = processes.data;
  const topCpu = useMemo(() => [...(snapshot?.processes ?? [])].sort((a, b) => b.cpu - a.cpu).slice(0, 5), [snapshot]);
  const topMemory = useMemo(() => [...(snapshot?.processes ?? [])].sort((a, b) => b.memory - a.memory).slice(0, 5), [snapshot]);

  if (!sample) return <Skeleton className="h-96" />;
  const { memory } = sample;
  const reclaimShare = (value: number) => (value / memory.total) * 100;
  const cpuData = history.map((p) => ({ ts: p.ts, cpu: p.cpu }));
  const memData = history.map((p) => ({ ts: p.ts, used: p.memoryUsed }));
  const ioData = history.map((p) => ({ ts: p.ts, read: p.diskReadBps, write: p.diskWriteBps }));
  const ioSeries = [
    { key: "read", label: "Read", color: "#08a0bc" },
    { key: "write", label: "Write", color: "#e4569f" },
  ];

  return (
    <div className="space-y-4">
      <ScreenHeader title="Performance Monitor" subtitle="Live telemetry, sampled every two seconds." />

      <div className="grid grid-cols-12 gap-4">
        <Panel edge className="col-span-12 xl:col-span-7" title="CPU" aside={<span className="mono text-xs text-soft">Load {sample.loadAverage.map((l) => l.toFixed(2)).join(" · ")}</span>}>
          <div className="flex items-end gap-6">
            <div>
              <div className="display text-[40px] leading-none text-ink">{sample.cpu.toFixed(0)}<span className="text-lg text-soft">%</span></div>
              <div className="mt-1 text-xs text-soft">Total usage</div>
            </div>
            <div className="flex-1">
              <div className="label mb-1.5">Per core</div>
              <div className="grid gap-1" style={{ gridTemplateColumns: `repeat(${Math.min(sample.perCore.length, 16)}, minmax(0, 1fr))` }}>
                {sample.perCore.map((value, i) => (
                  <div key={i} title={`Core ${i + 1}: ${value.toFixed(0)}%`} className="mono grid h-9 place-items-center rounded-md border border-line text-[10px] text-ink" style={{ background: coreColor(value) }}>
                    {value.toFixed(0)}
                  </div>
                ))}
              </div>
            </div>
          </div>
          <div className="mt-4">
            <TimeChart data={cpuData} height={150} domain={[0, 100]} series={[{ key: "cpu", label: "CPU", color: "var(--accent)" }]} format={(v) => percent(v)} labelFormat={clock} />
          </div>
          <div className="mt-3 grid grid-cols-2 gap-6 border-t border-line pt-3">
            <TopList title="Top CPU" items={topCpu} value={(p) => percent(p.cpu, 1)} />
            <TopList title="Top memory" items={topMemory} value={(p) => mem(p.memory)} />
          </div>
        </Panel>

        <Panel className="col-span-12 xl:col-span-5" title="Memory" aside={<Status tone={pressureTone(memory.pressure)}>Pressure {memory.pressure}</Status>}>
          <div className="flex items-baseline justify-between">
            <div className="display text-[32px] leading-none text-ink">{mem(memory.used)}</div>
            <div className="mono text-xs text-soft">of {mem(memory.total, 0)}</div>
          </div>
          <div role="img" aria-label="Memory composition" className="mt-3 flex h-3 gap-[2px] overflow-hidden rounded-full bg-white/5">
            {MEMORY_PARTS.map((part) => <div key={part.key} className="transition-[width] duration-700" style={{ width: `${reclaimShare(memory[part.key])}%`, background: part.color }} />)}
          </div>
          <ul className="mt-3 grid grid-cols-2 gap-x-6 gap-y-1">
            {MEMORY_PARTS.map((part) => (
              <li key={part.key} className="flex items-center gap-2 text-xs">
                <span className="size-2.5 rounded-sm" style={{ background: part.color }} />
                <span className="text-soft">{part.label}</span>
                <span className="mono ml-auto text-ink">{mem(memory[part.key])}</span>
              </li>
            ))}
            <li className="flex items-center gap-2 text-xs">
              <span className="size-2.5 rounded-sm border border-line" />
              <span className="text-soft">Available</span>
              <span className="mono ml-auto text-ink">{mem(memory.available)}</span>
            </li>
            <li className="flex items-center gap-2 text-xs">
              <span className="size-2.5" />
              <span className="text-soft">Swap used</span>
              <span className="mono ml-auto text-ink">{mem(memory.swapUsed)}</span>
            </li>
          </ul>
          <p className="mt-3 text-xs text-soft">
            Cached memory is not wasted. macOS hands it back instantly, so memory pressure is the signal that matters.
          </p>
          <div className="mt-3">
            <TimeChart data={memData} height={110} domain={[0, memory.total]} series={[{ key: "used", label: "Memory used", color: "var(--accent-2)" }]} format={(v) => mem(v)} axisFormat={(v) => mem(v, 0)} labelFormat={clock} />
          </div>
        </Panel>
      </div>

      <Panel title="Disk activity" aside={<Legend series={ioSeries} />}>
        <div className="grid gap-4 lg:grid-cols-[220px_1fr]">
          <div className="space-y-2.5">
            <div>
              <div className="flex items-center gap-1.5 text-xs text-soft"><ArrowDown size={12} />Read</div>
              <div className="display text-xl text-ink">{rate(sample.diskReadBps)}</div>
            </div>
            <div>
              <div className="flex items-center gap-1.5 text-xs text-soft"><ArrowUp size={12} />Write</div>
              <div className="display text-xl text-ink">{rate(sample.diskWriteBps)}</div>
            </div>
            <div className="text-xs text-soft">{bytes(sample.volume.available)} available</div>
          </div>
          <TimeChart data={ioData} height={132} kind="line" series={ioSeries} format={rate} axisFormat={(v) => bytes(v, 0)} labelFormat={clock} />
        </div>
      </Panel>

      <ProcessTable snapshot={snapshot} reload={refresh} />
      <p className="mono text-center text-[10px] text-faint">{count(sample.processCount)} processes running</p>
    </div>
  );
}
