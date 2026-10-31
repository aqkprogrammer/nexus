import { Area, AreaChart, CartesianGrid, Line, LineChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";

const AXIS = { fontSize: 10, fontFamily: "var(--font-mono)", fill: "var(--ink-faint)" };

export interface Series { key: string; label: string; color: string }

function ChartTooltip({ active, payload, label, format, labelFormat }: {
  active?: boolean;
  payload?: { dataKey: string; name: string; value: number; color: string }[];
  label?: number;
  format: (value: number) => string;
  labelFormat: (value: number) => string;
}) {
  if (!active || !payload?.length) return null;
  return (
    <div className="rounded-lg border border-[var(--line-strong)] bg-[var(--panel-solid)] px-2.5 py-2 shadow-xl">
      <div className="mono text-[10px] text-faint">{label != null ? labelFormat(label) : ""}</div>
      {payload.map((entry) => (
        <div key={entry.dataKey} className="mt-1 flex items-center gap-2 text-xs">
          <span className="size-2 rounded-sm" style={{ background: entry.color }} />
          <span className="text-soft">{entry.name}</span>
          <span className="mono ml-auto pl-3 text-ink">{format(entry.value)}</span>
        </div>
      ))}
    </div>
  );
}

export function Legend({ series }: { series: Series[] }) {
  return (
    <div className="flex flex-wrap gap-x-4 gap-y-1">
      {series.map((s) => (
        <span key={s.key} className="inline-flex items-center gap-1.5 text-xs text-soft">
          <span className="h-0.5 w-3 rounded" style={{ background: s.color }} />
          {s.label}
        </span>
      ))}
    </div>
  );
}

/** One y-axis, one unit. Measures of different scale go in separate charts. */
export function TimeChart<D extends { ts: number }>({
  data, series, height = 170, format, axisFormat, labelFormat, domain, kind = "area",
}: {
  data: D[]; series: Series[]; height?: number; format: (value: number) => string;
  axisFormat?: (value: number) => string; labelFormat: (ts: number) => string;
  domain?: [number | "auto", number | "auto"]; kind?: "area" | "line";
}) {
  const shared = { data, margin: { top: 6, right: 6, bottom: 0, left: 0 } };
  const axes = (
    <>
      <CartesianGrid vertical={false} stroke="var(--line)" strokeDasharray="2 6" />
      <XAxis dataKey="ts" type="number" domain={["dataMin", "dataMax"]} scale="time" hide />
      <YAxis tick={AXIS} axisLine={false} tickLine={false} width={50} domain={domain} tickFormatter={axisFormat ?? format} tickCount={4} />
      <Tooltip
        isAnimationActive={false}
        cursor={{ stroke: "var(--ink-soft)", strokeDasharray: "2 3" }}
        content={<ChartTooltip format={format} labelFormat={labelFormat} />}
      />
    </>
  );
  return (
    <ResponsiveContainer width="100%" height={height}>
      {kind === "area" ? (
        <AreaChart {...shared}>
          <defs>
            {series.map((s) => (
              <linearGradient key={s.key} id={`fill-${s.key}`} x1="0" y1="0" x2="0" y2="1">
                <stop offset="0" stopColor={s.color} stopOpacity=".28" />
                <stop offset="1" stopColor={s.color} stopOpacity="0" />
              </linearGradient>
            ))}
          </defs>
          {axes}
          {series.map((s) => (
            <Area key={s.key} type="monotone" dataKey={s.key} name={s.label} stroke={s.color} strokeWidth={1.75}
              fill={`url(#fill-${s.key})`} isAnimationActive={false} connectNulls dot={false}
              activeDot={{ r: 3.5, stroke: "var(--panel-solid)", strokeWidth: 2 }} />
          ))}
        </AreaChart>
      ) : (
        <LineChart {...shared}>
          {axes}
          {series.map((s) => (
            <Line key={s.key} type="monotone" dataKey={s.key} name={s.label} stroke={s.color} strokeWidth={2}
              isAnimationActive={false} connectNulls dot={false}
              activeDot={{ r: 3.5, stroke: "var(--panel-solid)", strokeWidth: 2 }} />
          ))}
        </LineChart>
      )}
    </ResponsiveContainer>
  );
}
