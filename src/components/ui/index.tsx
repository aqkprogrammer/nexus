import type { ButtonHTMLAttributes, ReactNode } from "react";
import { motion } from "framer-motion";
import { AlertTriangle, CheckCircle2, Info, OctagonAlert, ShieldAlert, type LucideIcon } from "lucide-react";
import type { Risk } from "../../lib/types";

export function cx(...parts: (string | false | null | undefined)[]) {
  return parts.filter(Boolean).join(" ");
}

export function Panel({
  children, className, edge, title, aside, padded = true,
}: {
  children: ReactNode; className?: string; edge?: boolean; title?: ReactNode; aside?: ReactNode; padded?: boolean;
}) {
  return (
    <section className={cx("glass", edge && "glass-edge", className)}>
      {(title || aside) && (
        <header className={cx("flex items-center justify-between gap-3", padded ? "px-5 pt-4" : "px-5 py-3.5 border-b border-line")}>
          <h3 className="label">{title}</h3>
          {aside}
        </header>
      )}
      <div className={cx(padded && "p-5", padded && Boolean(title || aside) && "pt-3")}>{children}</div>
    </section>
  );
}

const BUTTON_VARIANT = {
  primary:
    "bg-accent/15 text-accent border-accent/50 hover:bg-accent/25 shadow-[0_0_24px_-10px_var(--accent)]",
  ghost: "bg-transparent text-soft border-line hover:text-ink hover:border-[var(--line-strong)]",
  danger: "bg-bad/10 text-bad border-bad/45 hover:bg-bad/20",
} as const;

export function Button({
  variant = "ghost", icon: Icon, children, className, size = "md", ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: keyof typeof BUTTON_VARIANT; icon?: LucideIcon; size?: "sm" | "md";
}) {
  return (
    <button
      type="button"
      className={cx(
        "inline-flex items-center justify-center gap-2 rounded-lg border font-medium transition-colors active:translate-y-px disabled:opacity-40 disabled:pointer-events-none whitespace-nowrap",
        size === "md" ? "h-9 px-3.5 text-[13px]" : "h-7 px-2.5 text-xs",
        BUTTON_VARIANT[variant],
        className,
      )}
      {...rest}
    >
      {Icon && <Icon size={size === "md" ? 15 : 13} strokeWidth={1.8} />}
      {children}
    </button>
  );
}

const RISK = {
  low: { label: "Low risk", className: "text-good border-good/35 bg-good/10", icon: CheckCircle2 },
  medium: { label: "Review first", className: "text-warn border-warn/35 bg-warn/10", icon: AlertTriangle },
  high: { label: "High risk", className: "text-bad border-bad/35 bg-bad/10", icon: ShieldAlert },
};

export function RiskBadge({ risk }: { risk: Risk }) {
  const { label, className, icon: Icon } = RISK[risk];
  return (
    <span className={cx("inline-flex items-center gap-1 rounded-md border px-1.5 py-0.5 text-[11px] font-medium", className)}>
      <Icon size={11} strokeWidth={2} />
      {label}
    </span>
  );
}

export type Tone = "good" | "warn" | "serious" | "bad" | "neutral";

const TONE = {
  good: { color: "text-good", icon: CheckCircle2 },
  warn: { color: "text-warn", icon: AlertTriangle },
  serious: { color: "text-serious", icon: AlertTriangle },
  bad: { color: "text-bad", icon: OctagonAlert },
  neutral: { color: "text-soft", icon: Info },
};

/** Status always pairs its colour with an icon and words, never colour alone. */
export function Status({ tone, children }: { tone: Tone; children: ReactNode }) {
  const { color, icon: Icon } = TONE[tone];
  return (
    <span className={cx("inline-flex items-center gap-1.5 text-xs min-w-0", color)}>
      <Icon size={12} strokeWidth={2} className="shrink-0" />
      <span className="truncate">{children}</span>
    </span>
  );
}

export function scoreTone(score: number | null | undefined): Tone {
  if (score == null) return "neutral";
  if (score >= 70) return "good";
  if (score >= 50) return "warn";
  if (score >= 30) return "serious";
  return "bad";
}

export function Tag({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <span className={cx("inline-flex items-center rounded border border-line px-1.5 py-px mono text-[10px] text-soft", className)}>
      {children}
    </span>
  );
}

export function Toggle({ checked, onChange, label, disabled }: {
  checked: boolean; onChange: (value: boolean) => void; label: string; disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cx(
        "relative h-[22px] w-[38px] shrink-0 rounded-full border transition-colors disabled:opacity-40",
        checked ? "bg-accent/30 border-accent/60" : "bg-white/5 border-line",
      )}
    >
      <span
        className={cx(
          "absolute top-[2px] size-4 rounded-full transition-all",
          checked ? "left-[18px] bg-accent shadow-[0_0_10px_var(--accent)]" : "left-[2px] bg-soft",
        )}
      />
    </button>
  );
}

export function Check({ checked, indeterminate, onChange, label }: {
  checked: boolean; indeterminate?: boolean; onChange: (value: boolean) => void; label: string;
}) {
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={indeterminate ? "mixed" : checked}
      aria-label={label}
      onClick={(e) => {
        e.stopPropagation();
        onChange(!checked);
      }}
      className={cx(
        "grid size-[17px] shrink-0 place-items-center rounded-[5px] border transition-colors",
        checked || indeterminate ? "bg-accent/25 border-accent/70 text-accent" : "border-[var(--line-strong)] hover:border-accent/50",
      )}
    >
      {checked ? (
        <svg width="11" height="11" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round"><path d="M2.5 6.5 5 9l4.5-6" /></svg>
      ) : indeterminate ? (
        <span className="h-0.5 w-2 rounded bg-current" />
      ) : null}
    </button>
  );
}

export function Segmented<V extends string>({ value, options, onChange, layoutId }: {
  value: V; options: { value: V; label: string; icon?: LucideIcon }[]; onChange: (value: V) => void; layoutId: string;
}) {
  return (
    <div role="tablist" className="inline-flex rounded-lg border border-line bg-black/20 p-0.5">
      {options.map((option) => {
        const active = option.value === value;
        const Icon = option.icon;
        return (
          <button
            key={option.value}
            role="tab"
            aria-selected={active}
            type="button"
            onClick={() => onChange(option.value)}
            className={cx("relative flex h-7 items-center gap-1.5 rounded-md px-3 text-xs font-medium transition-colors", active ? "text-ink" : "text-soft hover:text-ink")}
          >
            {active && (
              <motion.span layoutId={layoutId} transition={{ type: "spring", stiffness: 420, damping: 36 }} className="absolute inset-0 rounded-md bg-accent/15 ring-1 ring-accent/40" />
            )}
            <span className="relative flex items-center gap-1.5">
              {Icon && <Icon size={13} strokeWidth={1.8} />}
              {option.label}
            </span>
          </button>
        );
      })}
    </div>
  );
}

export function Input({ className, ...rest }: React.InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      className={cx(
        "h-9 rounded-lg border border-line bg-black/25 px-3 text-[13px] text-ink placeholder:text-faint focus:border-accent/60 focus:outline-none",
        className,
      )}
      {...rest}
    />
  );
}

export function Select({ value, onChange, options, label, className }: {
  value: string; onChange: (value: string) => void; options: { value: string; label: string }[]; label: string; className?: string;
}) {
  return (
    <select
      aria-label={label}
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className={cx("h-9 rounded-lg border border-line bg-[var(--panel-solid)] px-2.5 text-[13px] text-ink focus:border-accent/60 focus:outline-none", className)}
    >
      {options.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
    </select>
  );
}

export function Skeleton({ className }: { className?: string }) {
  return <div className={cx("skeleton", className)} />;
}

export function EmptyState({ icon: Icon, title, body, children }: {
  icon: LucideIcon; title: string; body: ReactNode; children?: ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 px-8 py-14 text-center">
      <div className="grid size-14 place-items-center rounded-2xl border border-line bg-accent/5 text-accent">
        <Icon size={24} strokeWidth={1.5} />
      </div>
      <h3 className="display text-lg text-ink">{title}</h3>
      <p className="max-w-md text-soft">{body}</p>
      {children && <div className="mt-2 flex flex-wrap items-center justify-center gap-2">{children}</div>}
    </div>
  );
}

export function ErrorState({ title, body, onRetry }: { title: string; body: string; onRetry?: () => void }) {
  return (
    <div className="flex flex-col items-center gap-2 px-6 py-10 text-center">
      <OctagonAlert size={22} className="text-bad" strokeWidth={1.6} />
      <h3 className="display text-base">{title}</h3>
      <p className="max-w-md text-soft">{body}</p>
      {onRetry && <Button className="mt-2" onClick={onRetry}>Try again</Button>}
    </div>
  );
}

export function ScreenHeader({ title, subtitle, children }: { title: string; subtitle: string; children?: ReactNode }) {
  return (
    <div className="flex flex-wrap items-end justify-between gap-4">
      <div className="min-w-0">
        <h1 className="display text-[26px] font-medium leading-tight text-ink">{title}</h1>
        <p className="mt-0.5 text-soft">{subtitle}</p>
      </div>
      {children && <div className="flex items-center gap-2">{children}</div>}
    </div>
  );
}

export function Bar({ value, className, color }: { value: number; className?: string; color?: string }) {
  return (
    <div className={cx("h-1.5 overflow-hidden rounded-full bg-white/[0.06]", className)}>
      <div
        className="h-full rounded-full transition-[width] duration-500"
        style={{ width: `${Math.max(0, Math.min(100, value))}%`, background: color ?? "linear-gradient(90deg, var(--accent-2), var(--accent))" }}
      />
    </div>
  );
}

export function KeyValue({ label, value, mono = true }: { label: string; value: ReactNode; mono?: boolean }) {
  return (
    <div className="flex items-baseline justify-between gap-4 border-b border-line/60 py-1.5 last:border-0">
      <span className="text-soft">{label}</span>
      <span className={cx("min-w-0 truncate text-right text-ink", mono && "mono text-xs")}>{value ?? "Unavailable"}</span>
    </div>
  );
}

export function Stat({ label, value, unit, hint }: { label: string; value: string; unit?: string; hint?: ReactNode }) {
  return (
    <div>
      <div className="label">{label}</div>
      <div className="mt-1 flex items-baseline gap-1">
        <span className="display text-2xl text-ink">{value}</span>
        {unit && <span className="mono text-xs text-soft">{unit}</span>}
      </div>
      {hint && <div className="mt-0.5 text-xs text-soft">{hint}</div>}
    </div>
  );
}

export function TableHead({ children }: { children: ReactNode }) {
  return <div className="label flex items-center gap-3 border-b border-line px-5 py-2.5">{children}</div>;
}
