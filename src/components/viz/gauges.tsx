import { useId, useState, type ReactNode } from "react";
import { motion } from "framer-motion";

function polar(cx: number, cy: number, r: number, degrees: number) {
  const a = ((degrees - 90) * Math.PI) / 180;
  return [cx + r * Math.cos(a), cy + r * Math.sin(a)];
}

function arc(cx: number, cy: number, r: number, start: number, end: number) {
  const [x0, y0] = polar(cx, cy, r, start);
  const [x1, y1] = polar(cx, cy, r, end);
  return `M ${x0} ${y0} A ${r} ${r} 0 ${end - start > 180 ? 1 : 0} 1 ${x1} ${y1}`;
}

export function Ring({ value, size = 56, stroke = 5, children, color = "var(--accent)" }: {
  value: number | null; size?: number; stroke?: number; children?: ReactNode; color?: string;
}) {
  const r = (size - stroke) / 2;
  const c = 2 * Math.PI * r;
  const pct = Math.max(0, Math.min(100, value ?? 0)) / 100;
  return (
    <div className="relative grid place-items-center" style={{ width: size, height: size }}>
      <svg width={size} height={size} className="-rotate-90" aria-hidden>
        <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="var(--line)" strokeWidth={stroke} />
        <motion.circle
          cx={size / 2} cy={size / 2} r={r} fill="none" stroke={color} strokeWidth={stroke} strokeLinecap="round"
          strokeDasharray={c}
          initial={{ strokeDashoffset: c }}
          animate={{ strokeDashoffset: c * (1 - pct) }}
          transition={{ duration: 0.9, ease: [0.2, 0.8, 0.2, 1] }}
        />
      </svg>
      <div className="absolute inset-0 grid place-items-center">{children}</div>
    </div>
  );
}

export interface ReactorRing { label: string; value: number | null; color: string }

/** The Command Center's main instrument: overall score in the centre, one arc per subsystem. */
export function HealthReactor({ score, status, rings }: { score: number | null; status: string; rings: ReactorRing[] }) {
  const size = 372;
  const center = size / 2;
  const outer = 158;
  const gap = 15;
  const sweep = 300;
  const start = 30;
  const ticks = Array.from({ length: 72 });
  return (
    <div className="relative mx-auto" style={{ width: size, height: size }}>
      <div className="absolute inset-10 rounded-full bg-accent/[0.06] blur-3xl" />
      <svg viewBox={`0 0 ${size} ${size}`} width={size} height={size} className="spin-slow absolute inset-0" aria-hidden>
        {ticks.map((_, i) => {
          const [x0, y0] = polar(center, center, 180, i * 5);
          const [x1, y1] = polar(center, center, i % 6 === 0 ? 172 : 176, i * 5);
          return <line key={i} x1={x0} y1={y0} x2={x1} y2={y1} stroke="var(--accent)" strokeOpacity={i % 6 === 0 ? 0.55 : 0.2} strokeWidth="1" />;
        })}
      </svg>
      <svg viewBox={`0 0 ${size} ${size}`} width={size} height={size} className="spin-rev absolute inset-0" aria-hidden>
        <circle cx={center} cy={center} r="168" fill="none" stroke="var(--accent-3)" strokeOpacity=".25" strokeWidth="1" strokeDasharray="2 14 40 14" />
      </svg>
      <svg viewBox={`0 0 ${size} ${size}`} width={size} height={size} className="relative" role="img"
        aria-label={score == null ? "Health score loading" : `Mac health score ${score} of 100, ${status}`}>
        {rings.map((ring, i) => {
          const r = outer - i * gap;
          const length = (2 * Math.PI * r * sweep) / 360;
          const fraction = Math.max(0, Math.min(100, ring.value ?? 0)) / 100;
          const path = arc(center, center, r, start, start + sweep);
          return (
            <g key={ring.label}>
              <path d={path} fill="none" stroke="var(--line)" strokeWidth="7" strokeLinecap="round" />
              {ring.value != null && (
                <>
                  <path d={path} fill="none" stroke={ring.color} strokeOpacity=".16" strokeWidth="13" strokeLinecap="round"
                    strokeDasharray={`${length * fraction} ${length}`} />
                  <motion.path d={path} fill="none" stroke={ring.color} strokeWidth="6" strokeLinecap="round"
                    strokeDasharray={length}
                    initial={{ strokeDashoffset: length }}
                    animate={{ strokeDashoffset: length * (1 - fraction) }}
                    transition={{ duration: 1.3, delay: i * 0.08, ease: [0.2, 0.8, 0.2, 1] }} />
                </>
              )}
            </g>
          );
        })}
      </svg>
      <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center pt-1">
        <span className="label">Mac health</span>
        <span className="display text-[64px] font-medium leading-none tabular-nums text-ink">
          {score == null ? "--" : Math.round(score)}
        </span>
        <span className="mt-1 text-[13px] text-accent">{status}</span>
      </div>
    </div>
  );
}

export function Radar({ percent, size = 280, label, sublabel }: {
  percent: number | null; size?: number; label?: string; sublabel?: string;
}) {
  const id = useId().replace(/:/g, "");
  const c = size / 2;
  const blips = [[.32, .36], [.68, .3], [.74, .66], [.28, .7], [.54, .2], [.46, .78]];
  return (
    <div className="relative" style={{ width: size, height: size }} role="progressbar"
      aria-valuenow={percent ?? undefined} aria-valuemin={0} aria-valuemax={100} aria-label={label ?? "Scan progress"}>
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} aria-hidden>
        <defs>
          <linearGradient id={`${id}-sweep`} x1="0" y1="0" x2="1" y2="0">
            <stop offset="0" stopColor="var(--accent)" stopOpacity="0" />
            <stop offset="1" stopColor="var(--accent)" stopOpacity=".5" />
          </linearGradient>
        </defs>
        {[0.98, 0.75, 0.5, 0.25].map((f) => (
          <circle key={f} cx={c} cy={c} r={(c - 2) * f} fill="none" stroke="var(--accent)" strokeOpacity=".2" />
        ))}
        <line x1={c} y1="2" x2={c} y2={size - 2} stroke="var(--accent)" strokeOpacity=".1" />
        <line x1="2" y1={c} x2={size - 2} y2={c} stroke="var(--accent)" strokeOpacity=".1" />
        {blips.map(([x, y], i) => (
          <circle key={i} cx={x * size} cy={y * size} r="2.5" fill="var(--accent)" className="pulse-dot" style={{ animationDelay: `${i * 0.37}s` }} />
        ))}
        <g className="spin-radar">
          <path d={`M ${c} ${c} L ${c} 2 A ${c - 2} ${c - 2} 0 0 1 ${c + (c - 2) * Math.sin(1)} ${c - (c - 2) * Math.cos(1)} Z`} fill={`url(#${id}-sweep)`} />
          <line x1={c} y1={c} x2={c} y2="2" stroke="var(--accent)" strokeWidth="1.5" />
        </g>
        {percent != null && (
          <circle cx={c} cy={c} r={c - 2} fill="none" stroke="var(--accent)" strokeWidth="2.5"
            strokeDasharray={2 * Math.PI * (c - 2)} strokeDashoffset={2 * Math.PI * (c - 2) * (1 - percent / 100)}
            transform={`rotate(-90 ${c} ${c})`} style={{ transition: "stroke-dashoffset .3s" }} />
        )}
      </svg>
      <div className="absolute inset-0 flex flex-col items-center justify-center">
        <div className="rounded-xl bg-[var(--bg-0)]/70 px-3 py-1.5 text-center backdrop-blur-sm">
          <div className="display text-[34px] leading-none tabular-nums text-ink">{percent == null ? "···" : `${Math.round(percent)}%`}</div>
          {sublabel && <div className="mono mt-1 text-[11px] text-soft">{sublabel}</div>}
        </div>
      </div>
    </div>
  );
}

export function Spark({ data, color = "var(--accent)", height = 34, format }: {
  data: (number | null)[]; color?: string; height?: number; format?: (value: number) => string;
}) {
  const id = useId().replace(/:/g, "");
  const [hover, setHover] = useState<number | null>(null);
  const width = 200;
  const points = data.map((v, i) => ({ v, i })).filter((p): p is { v: number; i: number } => p.v != null);
  if (points.length < 2) {
    return <div className="flex items-center text-[10px] text-faint" style={{ height }}>Collecting…</div>;
  }
  const values = points.map((p) => p.v);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = max - min || 1;
  const x = (i: number) => (i / (data.length - 1)) * width;
  const y = (v: number) => height - 3 - ((v - min) / span) * (height - 8);
  const line = points.map((p, n) => `${n ? "L" : "M"}${x(p.i).toFixed(1)} ${y(p.v).toFixed(1)}`).join(" ");
  const area = `${line} L${x(points.at(-1)!.i).toFixed(1)} ${height} L${x(points[0].i).toFixed(1)} ${height} Z`;
  const active = hover == null ? null : points.reduce((best, p) => (Math.abs(p.i - hover) < Math.abs(best.i - hover) ? p : best));
  return (
    <div className="relative" onMouseLeave={() => setHover(null)}
      onMouseMove={(e) => {
        const box = e.currentTarget.getBoundingClientRect();
        setHover(Math.round(((e.clientX - box.left) / box.width) * (data.length - 1)));
      }}>
      <svg viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="none" width="100%" height={height} aria-hidden>
        <defs>
          <linearGradient id={id} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor={color} stopOpacity=".26" />
            <stop offset="1" stopColor={color} stopOpacity="0" />
          </linearGradient>
        </defs>
        <path d={area} fill={`url(#${id})`} />
        <path d={line} fill="none" stroke={color} strokeWidth="1.5" vectorEffect="non-scaling-stroke" strokeLinejoin="round" />
        {active && <line x1={x(active.i)} x2={x(active.i)} y1="0" y2={height} stroke="var(--ink-soft)" strokeWidth="1" vectorEffect="non-scaling-stroke" strokeDasharray="2 2" />}
      </svg>
      {active && format && (
        <div className="mono pointer-events-none absolute -top-7 z-10 -translate-x-1/2 rounded border border-line bg-[var(--panel-solid)] px-1.5 py-0.5 text-[10px] whitespace-nowrap text-ink"
          style={{ left: `${Math.min(84, Math.max(16, (active.i / (data.length - 1)) * 100))}%` }}>
          {format(active.v)}
        </div>
      )}
    </div>
  );
}

export interface DonutSegment { id: string; label: string; value: number; color: string }

/** Storage radial. Gaps separate segments; the legend beside it carries identity. */
export function Donut({ segments, total, center, selected, onSelect, size = 288 }: {
  segments: DonutSegment[]; total: number; center: ReactNode; selected?: string | null;
  onSelect?: (id: string) => void; size?: number;
}) {
  const [hover, setHover] = useState<string | null>(null);
  const c = size / 2;
  const r = c - 20;
  const gapDegrees = 1.4;
  let cursor = 0;
  const arcs = segments.map((segment) => {
    const span = (segment.value / total) * 360;
    const shape = { ...segment, start: cursor + gapDegrees / 2, end: cursor + Math.max(span - gapDegrees / 2, gapDegrees) };
    cursor += span;
    return shape;
  });
  const focused = arcs.find((a) => a.id === (hover ?? selected));
  return (
    <div className="relative mx-auto" style={{ width: size, height: size }}>
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} role="img" aria-label="Storage distribution by category">
        <circle cx={c} cy={c} r={r + 13} fill="none" stroke="var(--accent)" strokeOpacity=".14" strokeDasharray="1 7" />
        <circle cx={c} cy={c} r={r} fill="none" stroke="var(--line)" strokeWidth="20" />
        {arcs.map((a, i) => (
          <motion.path
            key={a.id}
            d={arc(c, c, r, a.start, Math.min(a.end, a.start + 359.9))}
            fill="none" stroke={a.color} strokeLinecap="butt"
            strokeWidth={focused?.id === a.id ? 26 : 20}
            initial={{ pathLength: 0, opacity: 0 }}
            animate={{ pathLength: 1, opacity: focused && focused.id !== a.id ? 0.45 : 1 }}
            transition={{ duration: 0.75, delay: i * 0.05, ease: [0.2, 0.8, 0.2, 1] }}
            onMouseEnter={() => setHover(a.id)}
            onMouseLeave={() => setHover(null)}
            onClick={() => onSelect?.(a.id)}
          />
        ))}
      </svg>
      <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center px-14 text-center">
        {focused ? (
          <>
            <span className="label">{focused.label}</span>
            <span className="display mt-1 text-[30px] leading-none text-ink">{((focused.value / total) * 100).toFixed(1)}%</span>
          </>
        ) : center}
      </div>
    </div>
  );
}
