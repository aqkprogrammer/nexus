import { useMemo, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { ArrowRight, BrainCircuit, ChevronDown, Cpu, RotateCw, ShieldCheck, Sparkles } from "lucide-react";
import { Button, EmptyState, ErrorState, Panel, RiskBadge, ScreenHeader, Segmented, Skeleton, Tag, cx } from "../components/ui";
import { api } from "../lib/ipc";
import { ago, bytes } from "../lib/format";
import { useApp, type Route } from "../stores";
import type { Recommendation } from "../lib/types";
import { useLoad } from "./shared";

function Orb() {
  return (
    <div className="relative size-[108px] shrink-0" aria-hidden>
      <div className="absolute inset-0 rounded-full border border-accent/25 spin-slow" style={{ borderStyle: "dashed" }} />
      <div className="absolute inset-3 rounded-full border border-[color:var(--accent-3)]/25 spin-rev" />
      <div className="absolute inset-6 grid place-items-center rounded-full bg-[radial-gradient(circle_at_35%_30%,var(--accent),var(--accent-2)_55%,transparent_78%)] opacity-90">
        <Sparkles size={22} className="text-white/90" strokeWidth={1.5} />
      </div>
    </div>
  );
}

function RecommendationCard({ rec, index }: { rec: Recommendation; index: number }) {
  const navigate = useApp((s) => s.navigate);
  const [open, setOpen] = useState(index === 0);
  return (
    <motion.article initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: index * 0.04 }} className="glass">
      <button type="button" aria-expanded={open} onClick={() => setOpen((v) => !v)} className="flex w-full items-center gap-3 px-5 py-3.5 text-left">
        <span className="mono w-7 shrink-0 text-center text-xs text-faint">{String(index + 1).padStart(2, "0")}</span>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <h3 className="truncate text-sm font-medium text-ink">{rec.title}</h3>
            <RiskBadge risk={rec.risk} />
          </div>
          <p className="truncate text-xs text-soft">{rec.issue}</p>
        </div>
        <span className="mono shrink-0 text-xs text-accent">{rec.estimateBytes != null ? bytes(rec.estimateBytes) : ""}</span>
        <ChevronDown size={15} className={cx("shrink-0 text-faint transition-transform", open && "rotate-180")} />
      </button>
      <AnimatePresence initial={false}>
        {open && (
          <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }} className="overflow-hidden">
            <div className="grid gap-4 border-t border-line px-5 py-4 pl-[60px] md:grid-cols-3">
              <div><div className="label mb-1">Issue detected</div><p className="text-[13px] text-ink">{rec.issue}</p></div>
              <div><div className="label mb-1">Why it matters</div><p className="text-[13px] text-soft">{rec.why}</p></div>
              <div><div className="label mb-1">Potential impact</div><p className="text-[13px] text-soft">{rec.impact}</p></div>
              <div><div className="label mb-1">Estimated improvement</div><p className="text-[13px] text-ink">{rec.estimate}</p></div>
              <div className="md:col-span-2"><div className="label mb-1">Recommended action</div><p className="text-[13px] text-soft">{rec.action}</p></div>
            </div>
            <div className="flex items-center gap-3 border-t border-line px-5 py-3 pl-[60px]">
              <Button variant="primary" onClick={() => navigate(rec.route as Route)}>{rec.actionLabel}<ArrowRight size={14} /></Button>
              <span className="text-xs text-faint">Opens the screen where you review and confirm. Nothing runs automatically.</span>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.article>
  );
}

export function Advisor() {
  const report = useLoad(() => api.getAdvisorReport(), []);
  const provider = useApp((s) => s.settings?.aiProvider ?? "none");
  const navigate = useApp((s) => s.navigate);
  const [domain, setDomain] = useState("all");
  const data = report.data;
  const recommendations = useMemo(
    () => (data?.recommendations ?? []).filter((r) => domain === "all" || r.domain === domain),
    [data, domain],
  );

  return (
    <div className="space-y-4">
      <ScreenHeader title="NEXUS AI" subtitle="An analyst for your Mac, working from local data only.">
        <Button icon={RotateCw} onClick={() => void report.reload()}>Re-analyze</Button>
      </ScreenHeader>

      {report.error ? <ErrorState title="Analysis failed" body={report.error} onRetry={() => void report.reload()} /> : !data ? <Skeleton className="h-44" /> : (
        <>
          <Panel edge>
            <div className="flex items-center gap-6">
              <Orb />
              <div className="min-w-0 flex-1">
                <div className="label">System assessment</div>
                <p className="display mt-1 text-xl leading-snug text-ink">{data.summary}</p>
                <div className="mt-2 flex flex-wrap items-center gap-2">
                  <Tag><Cpu size={10} className="mr-1" />{data.engine}</Tag>
                  <Tag>Provider: {provider === "none" ? "off" : provider}</Tag>
                  <Tag>{ago(data.generatedAt)}</Tag>
                  <span className="flex items-center gap-1 text-xs text-soft"><ShieldCheck size={12} className="text-accent" />No data leaves this Mac. NEXUS AI cannot delete, stop or change anything.</span>
                </div>
              </div>
            </div>
          </Panel>

          {data.missingData.length > 0 && (
            <Panel title="Improve this analysis">
              <ul className="space-y-1.5">
                {data.missingData.map((line) => (
                  <li key={line} className="flex items-center gap-2 text-[13px] text-soft">
                    <BrainCircuit size={14} className="shrink-0 text-accent" />{line}
                  </li>
                ))}
              </ul>
              <div className="mt-3 flex gap-2">
                <Button size="sm" onClick={() => navigate("storage")}>Storage scan</Button>
                <Button size="sm" onClick={() => navigate("cleaner")}>Smart Cleaner</Button>
                <Button size="sm" onClick={() => navigate("duplicates")}>Duplicate Finder</Button>
              </div>
            </Panel>
          )}

          <Segmented layoutId="advisor-filter" value={domain} onChange={setDomain}
            options={[{ value: "all", label: "All" }, { value: "storage", label: "Storage" }, { value: "developer", label: "Developer" }, { value: "performance", label: "Performance" }, { value: "battery", label: "Battery" }]} />

          {recommendations.length === 0 ? (
            <Panel><EmptyState icon={Sparkles} title="Nothing to flag here" body="No rule fired for this category with the data collected so far." /></Panel>
          ) : (
            <div className="space-y-2.5">{recommendations.map((rec, i) => <RecommendationCard key={rec.id} rec={rec} index={i} />)}</div>
          )}
        </>
      )}
    </div>
  );
}
