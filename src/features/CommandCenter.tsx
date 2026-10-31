import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { ArrowRight, Info, Sparkles } from "lucide-react";
import { HealthReactor, Spark, type ReactorRing } from "../components/viz/gauges";
import { Button, KeyValue, Panel, RiskBadge, Skeleton, Status, cx, scoreTone, type Tone } from "../components/ui";
import { api } from "../lib/ipc";
import { ago, bytes, count, duration, memory as formatMemory, percent, splitBytes } from "../lib/format";
import { useApp, useTelemetry } from "../stores";
import type { HealthComponent, HistoryPoint, Recommendation } from "../lib/types";
import { useLoad } from "./shared";

const RING_COLORS = {
  storage: "var(--accent)", memory: "var(--accent-2)", cpu: "var(--accent-3)", battery: "var(--accent-4)", performance: "#e9c46a",
};

interface Metric {
  label: string; value: string; unit?: string; tone: Tone; status: string;
  series: (p: HistoryPoint) => number | null; format: (v: number) => string;
}

function MetricCard({ metric, history, ts, index }: { metric: Metric; history: HistoryPoint[]; ts: number; index: number }) {
  return (
    <motion.article initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: index * 0.035, duration: 0.35 }}
      className="glass group flex min-w-0 flex-col p-4 transition-colors hover:border-accent/30">
      <div className="label truncate">{metric.label}</div>
      <div className="mt-1.5 flex items-baseline gap-1">
        <span className={cx("display font-medium leading-none tabular-nums text-ink", metric.value.length > 9 ? "text-lg" : "text-[26px]")}>{metric.value}</span>
        {metric.unit && <span className="mono text-[11px] text-soft">{metric.unit}</span>}
      </div>
      <div className="mt-2.5">
        <Spark data={history.map(metric.series)} format={metric.format} />
      </div>
      <div className="mt-2 flex items-center justify-between gap-2">
        <Status tone={metric.tone}>{metric.status}</Status>
      </div>
      <div className="mono mt-1 text-[10px] text-faint">Updated {ago(ts)}</div>
    </motion.article>
  );
}

function Subsystem({ label, color, part }: { label: string; color: string; part: HealthComponent | null }) {
  return (
    <div className="py-2.5">
      <div className="flex items-center gap-2">
        <span className="size-2 rounded-full" style={{ background: color, boxShadow: `0 0 8px ${color}` }} />
        <span className="text-[13px] text-ink">{label}</span>
        <span className="mono ml-auto text-[13px] text-ink">{part ? Math.round(part.score) : "—"}</span>
      </div>
      <div className="mt-1 pl-4">
        {part ? <Status tone={scoreTone(part.score)}>{part.status}</Status> : <Status tone="neutral">Unavailable</Status>}
        <p className="mt-0.5 text-xs text-soft">{part?.detail ?? "This Mac does not report this signal."}</p>
      </div>
    </div>
  );
}

function RecommendationRow({ rec }: { rec: Recommendation }) {
  const navigate = useApp((s) => s.navigate);
  return (
    <button type="button" onClick={() => navigate("ai")} className="row-hover flex w-full items-center gap-3 rounded-lg px-2 py-2 text-left">
      <div className="min-w-0 flex-1">
        <div className="truncate text-[13px] text-ink">{rec.title}</div>
        <div className="truncate text-xs text-soft">{rec.issue}</div>
      </div>
      <RiskBadge risk={rec.risk} />
      <ArrowRight size={14} className="shrink-0 text-faint" />
    </button>
  );
}

export function CommandCenter() {
  const sample = useTelemetry((s) => s.sample);
  const history = useTelemetry((s) => s.history);
  const overview = useTelemetry((s) => s.overview);
  const navigate = useApp((s) => s.navigate);
  const advisor = useLoad(() => api.getAdvisorReport(), []);
  const [, tick] = useState(0);
  useEffect(() => {
    const handle = setInterval(() => tick((n) => n + 1), 5000);
    return () => clearInterval(handle);
  }, []);

  if (!sample) {
    return (
      <div className="grid grid-cols-12 gap-4">
        <Skeleton className="col-span-5 h-[430px]" /><Skeleton className="col-span-3 h-[430px]" /><Skeleton className="col-span-4 h-[430px]" />
        {Array.from({ length: 5 }, (_, i) => <Skeleton key={i} className="col-span-2 h-40" />)}
      </div>
    );
  }

  const { health, volume, memory, battery } = sample;
  const rings: ReactorRing[] = [
    { label: "Storage", value: health.storage.score, color: RING_COLORS.storage },
    { label: "Memory", value: health.memory.score, color: RING_COLORS.memory },
    { label: "CPU", value: health.cpu.score, color: RING_COLORS.cpu },
    { label: "Battery", value: health.battery?.score ?? null, color: RING_COLORS.battery },
    { label: "Performance", value: health.performance.score, color: RING_COLORS.performance },
  ];
  const [used, usedUnit] = splitBytes(volume.used);
  const [free, freeUnit] = splitBytes(volume.available);
  const [total, totalUnit] = splitBytes(volume.total);
  const [ram, ramUnit] = formatMemory(memory.used).split(" ");
  const usedPct = volume.total ? (volume.used / volume.total) * 100 : 0;
  const memoryTone: Tone = memory.pressure === "critical" ? "bad" : memory.pressure === "warning" ? "warn" : memory.pressure === "normal" ? "good" : "neutral";
  const cpuTone: Tone = sample.cpu > 90 ? "serious" : sample.cpu > 70 ? "warn" : "good";
  const thermalTone: Tone = sample.thermalState === "critical" ? "bad" : sample.thermalState === "serious" ? "serious" : sample.thermalState === "fair" ? "warn" : "good";

  const metrics: Metric[] = [
    { label: "Storage used", value: used, unit: usedUnit, tone: scoreTone(health.storage.score), status: `${usedPct.toFixed(0)}% of capacity`, series: (p) => p.diskUsed, format: (v) => bytes(v) },
    { label: "Available storage", value: free, unit: freeUnit, tone: scoreTone(health.storage.score), status: health.storage.status, series: (p) => p.diskAvailable, format: (v) => bytes(v) },
    { label: "Total storage", value: total, unit: totalUnit, tone: "neutral", status: overview?.disk.fileSystem ?? "Data volume", series: (p) => p.diskUsed + p.diskAvailable, format: (v) => bytes(v) },
    { label: "RAM usage", value: ram, unit: ramUnit, tone: memoryTone, status: `Pressure ${memory.pressure}`, series: (p) => p.memoryUsed, format: (v) => formatMemory(v) },
    { label: "CPU usage", value: sample.cpu.toFixed(0), unit: "%", tone: cpuTone, status: `Load ${sample.loadAverage[0].toFixed(2)}`, series: (p) => p.cpu, format: (v) => percent(v) },
    {
      label: "Battery", value: battery ? battery.percent.toFixed(0) : "Unavailable", unit: battery ? "%" : undefined,
      tone: battery ? (battery.percent < 15 && !battery.externalPower ? "warn" : "good") : "neutral",
      status: battery ? (battery.isCharging ? "Charging" : battery.powerSource) : "No battery reported",
      series: (p) => p.battery, format: (v) => percent(v),
    },
    {
      label: "Battery cycles", value: count(battery?.cycleCount), tone: "neutral",
      status: battery?.condition ? `Condition ${battery.condition}` : "Condition unavailable",
      series: (p) => p.cycleCount, format: (v) => count(v),
    },
    {
      label: "CPU temperature", value: sample.cpuTemperature != null ? sample.cpuTemperature.toFixed(0) : "Unavailable",
      unit: sample.cpuTemperature != null ? "°C" : undefined, tone: thermalTone, status: `Thermal ${sample.thermalState}`,
      series: (p) => p.temperature, format: (v) => `${v.toFixed(0)}°C`,
    },
    { label: "Running processes", value: count(sample.processCount), tone: "neutral", status: "Across all users", series: (p) => p.processCount, format: (v) => count(v) },
    { label: "System uptime", value: duration(sample.uptime), tone: "neutral", status: "Since last restart", series: (p) => p.uptime, format: (v) => duration(v) },
  ];

  const recommendations = advisor.data?.recommendations.slice(0, 3) ?? [];

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-12 gap-4">
        <Panel edge className="col-span-12 lg:col-span-5" title="Overall Mac health" padded>
          <HealthReactor score={health.overall} status={health.status} rings={rings} />
          <p className="mt-2 flex items-start gap-1.5 text-[11px] text-faint">
            <Info size={12} className="mt-px shrink-0" />
            This score is NEXUS's own heuristic from live readings. It is not an official Apple diagnostic.
          </p>
        </Panel>

        <Panel className="col-span-6 lg:col-span-3" title="Subsystems">
          <div className="-my-1 divide-y divide-line/60">
            <Subsystem label="Storage" color={RING_COLORS.storage} part={health.storage} />
            <Subsystem label="Memory" color={RING_COLORS.memory} part={health.memory} />
            <Subsystem label="CPU" color={RING_COLORS.cpu} part={health.cpu} />
            <Subsystem label="Battery" color={RING_COLORS.battery} part={health.battery} />
            <Subsystem label="Performance" color={RING_COLORS.performance} part={health.performance} />
          </div>
        </Panel>

        <div className="col-span-6 flex flex-col gap-4 lg:col-span-4">
          <Panel title="System overview" className="flex-1">
            <KeyValue label="Mac" value={overview?.modelName ? `${overview.modelName} (${overview.modelIdentifier})` : overview?.modelIdentifier} mono={false} />
            <KeyValue label="macOS" value={overview ? `${overview.osName} ${overview.osVersion ?? ""} ${overview.osBuild ? `(${overview.osBuild})` : ""}` : null} mono={false} />
            <KeyValue label="Chip" value={overview?.chip ?? overview?.cpuBrand} mono={false} />
            <KeyValue label="Architecture" value={overview?.architecture} />
            <KeyValue label="Cores" value={overview?.logicalCores != null
              ? `${overview.logicalCores}${overview.performanceCores ? ` (${overview.performanceCores}P + ${overview.efficiencyCores}E)` : ""}` : null} />
            <KeyValue label="Memory" value={overview ? formatMemory(overview.memoryTotal, 0) : null} />
            <KeyValue label="Disk" value={overview ? `${overview.disk.name ?? "Internal"}, SMART ${overview.disk.smartStatus ?? "unavailable"}` : null} mono={false} />
            <KeyValue label="Battery" value={battery ? `${battery.condition ?? "Condition unavailable"}, ${percent(battery.maxCapacityPercent)} capacity` : "No battery reported"} mono={false} />
            <KeyValue label="Uptime" value={duration(sample.uptime)} />
          </Panel>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-5">
        {metrics.map((metric, i) => <MetricCard key={metric.label} metric={metric} history={history} ts={sample.ts} index={i} />)}
      </div>

      <Panel title="NEXUS AI" aside={<Button size="sm" icon={Sparkles} onClick={() => navigate("ai")}>Open advisor</Button>}>
        {advisor.loading && !advisor.data ? <Skeleton className="h-16" /> : recommendations.length > 0 ? (
          <div>{recommendations.map((rec) => <RecommendationRow key={rec.id} rec={rec} />)}</div>
        ) : (
          <p className="text-soft">{advisor.data?.missingData[0] ?? advisor.data?.summary ?? "Advisor data is unavailable."}</p>
        )}
      </Panel>
    </div>
  );
}
