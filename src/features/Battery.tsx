import { useMemo } from "react";
import { BatteryCharging, BatteryWarning, Cable, Info, Plug } from "lucide-react";
import { TimeChart } from "../components/viz/charts";
import { Ring } from "../components/viz/gauges";
import { Button, EmptyState, KeyValue, Panel, ScreenHeader, Skeleton, Status, Tag } from "../components/ui";
import { api } from "../lib/ipc";
import { clock, count, dateTime, duration, memory, percent } from "../lib/format";
import { useApp, useTelemetry } from "../stores";
import { useLoad } from "./shared";

export function Battery() {
  const sample = useTelemetry((s) => s.sample);
  const history = useTelemetry((s) => s.history);
  const navigate = useApp((s) => s.navigate);
  const trends = useLoad(() => api.getMetricTrends(14), []);
  const heavy = useLoad(() => api.listProcesses(), []);

  const topApps = useMemo(
    () => [...(heavy.data?.groups ?? [])].sort((a, b) => b.cpu - a.cpu).slice(0, 5),
    [heavy.data],
  );

  if (!sample) return <Skeleton className="h-96" />;
  const { battery } = sample;
  const header = <ScreenHeader title="Battery Intelligence" subtitle="Live condition and capacity, as macOS reports them." />;

  if (!battery) {
    return (
      <div className="space-y-4">
        {header}
        <Panel edge>
          <EmptyState icon={Plug} title="No battery detected" body="This Mac runs on mains power, so there is nothing to report here." />
        </Panel>
      </div>
    );
  }

  const live = history.filter((p) => p.battery != null).map((p) => ({ ts: p.ts, level: p.battery }));
  const longTerm = (trends.data ?? []).filter((t) => t.battery != null).map((t) => ({ ts: t.ts, level: t.battery }));
  const chart = longTerm.length >= 4 ? longTerm : live;
  const StateIcon = battery.isCharging ? BatteryCharging : battery.externalPower ? Cable : BatteryWarning;
  const remaining = battery.timeRemainingMin != null ? duration(battery.timeRemainingMin * 60) : null;
  const condition = battery.condition?.toLowerCase();
  const conditionGood = condition === "good" || condition === "normal";

  return (
    <div className="space-y-4">
      {header}
      <div className="grid grid-cols-12 gap-4">
        <Panel edge className="col-span-12 lg:col-span-4" title="Charge">
          <div className="flex flex-col items-center py-2">
            <Ring value={battery.percent} size={196} stroke={12}>
              <div className="text-center">
                <div className="display text-[44px] leading-none text-ink">{battery.percent.toFixed(0)}<span className="text-lg text-soft">%</span></div>
                <div className="mt-1 flex items-center justify-center gap-1 text-xs text-accent">
                  <StateIcon size={13} />{battery.isCharging ? "Charging" : battery.fullyCharged ? "Fully charged" : battery.powerSource}
                </div>
              </div>
            </Ring>
            <div className="mono mt-3 text-xs text-soft">{remaining ? `${remaining} remaining` : battery.externalPower ? "On power adapter" : "Estimating time remaining"}</div>
          </div>
        </Panel>

        <Panel className="col-span-12 lg:col-span-4" title="Health">
          <div className="mb-2 flex items-center justify-between">
            <span className="display text-2xl text-ink">{battery.condition ?? "Unavailable"}</span>
            {battery.condition && <Status tone={conditionGood ? "good" : "warn"}>{conditionGood ? "Operating normally" : "Check with Apple Support"}</Status>}
          </div>
          <KeyValue label="Maximum capacity" value={percent(battery.maxCapacityPercent)} />
          <KeyValue label="Full charge capacity" value={battery.maxCapacityMah != null ? `${count(battery.maxCapacityMah)} mAh` : "Unavailable"} />
          <KeyValue label="Design capacity" value={battery.designCapacityMah != null ? `${count(battery.designCapacityMah)} mAh` : "Unavailable"} />
          <KeyValue label="Cycle count" value={count(battery.cycleCount)} />
        </Panel>

        <Panel className="col-span-12 lg:col-span-4" title="Power">
          <KeyValue label="Power source" value={battery.powerSource} mono={false} />
          <KeyValue label="Charging" value={battery.isCharging ? "Yes" : "No"} mono={false} />
          <KeyValue label="Temperature" value={battery.temperatureC != null ? `${battery.temperatureC.toFixed(1)} °C` : "Unavailable"} />
          <KeyValue label="Voltage" value={battery.voltageV != null ? `${battery.voltageV.toFixed(2)} V` : "Unavailable"} />
          <KeyValue label="Power flow" value={battery.powerW != null ? `${battery.powerW.toFixed(1)} W` : "Unavailable"} />
          <p className="mt-2 text-xs text-soft">Positive power flow charges the battery. Negative flow drains it.</p>
        </Panel>
      </div>

      <Panel title={longTerm.length >= 4 ? "Charge history: last 14 days" : "Charge history: this session"}>
        {chart.length < 2 ? <div className="grid h-40 place-items-center text-soft">Collecting battery readings…</div> : (
          <TimeChart data={chart} height={170} domain={[0, 100]} series={[{ key: "level", label: "Charge", color: "var(--accent)" }]}
            format={(v) => percent(v)} labelFormat={longTerm.length >= 4 ? dateTime : clock} />
        )}
      </Panel>

      <div className="grid grid-cols-12 gap-4">
        <Panel className="col-span-12 lg:col-span-6" title="Highest CPU apps right now"
          aside={<Button size="sm" onClick={() => navigate("performance")}>Open monitor</Button>}>
          {topApps.length === 0 ? <Skeleton className="h-24" /> : (
            <ul>
              {topApps.map((app) => (
                <li key={app.name} className="flex h-7 items-center gap-3">
                  <span className="min-w-0 flex-1 truncate text-[13px] text-ink">{app.name}</span>
                  <Tag>{app.processes} proc</Tag>
                  <span className="mono w-14 text-right text-xs text-soft">{percent(app.cpu, 1)}</span>
                  <span className="mono w-16 text-right text-xs text-soft">{memory(app.memory)}</span>
                </li>
              ))}
            </ul>
          )}
          <p className="mt-2 text-xs text-soft">CPU use is the closest available proxy for energy impact.</p>
        </Panel>
        <Panel className="col-span-12 lg:col-span-6" title="Recommendations">
          <ul className="space-y-2 text-[13px]">
            <li><span className="text-ink">Review high-energy applications.</span> <span className="text-soft">Sustained CPU use is the biggest drain.</span></li>
            <li><span className="text-ink">Reduce unnecessary background processes.</span> <span className="text-soft">Quit helpers and menu bar tools you do not need.</span></li>
            <li><span className="text-ink">Investigate abnormal battery drain.</span> <span className="text-soft">A steep slope above while idle points to something running in the background.</span></li>
          </ul>
          <p className="mt-3 flex items-start gap-1.5 text-xs text-soft">
            <Info size={12} className="mt-0.5 shrink-0" />
            Battery wear is physical. No software, including NEXUS, can recalibrate a battery or restore lost capacity.
          </p>
        </Panel>
      </div>
    </div>
  );
}
