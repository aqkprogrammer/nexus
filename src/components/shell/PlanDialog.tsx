import { useMemo, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { CheckCircle2, CircleSlash, FlaskConical, OctagonPause, ShieldCheck, TriangleAlert, X } from "lucide-react";
import { baseName, bytes, count } from "../../lib/format";
import { useApp, usePlan } from "../../stores";
import type { PlanItem } from "../../lib/types";
import { Bar, Button, Toggle, cx } from "../ui";

function HoldButton({ label, danger, onConfirm }: { label: string; danger: boolean; onConfirm: () => void }) {
  const [holding, setHolding] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const start = () => {
    setHolding(true);
    timer.current = setTimeout(() => {
      setHolding(false);
      onConfirm();
    }, 1100);
  };
  const stop = () => {
    setHolding(false);
    clearTimeout(timer.current);
  };
  return (
    <button type="button" onPointerDown={start} onPointerUp={stop} onPointerLeave={stop}
      onKeyDown={(e) => { if ((e.key === " " || e.key === "Enter") && !e.repeat && !holding) start(); }}
      onKeyUp={stop}
      className={cx("relative h-10 overflow-hidden rounded-lg border px-5 text-[13px] font-medium", holding && "holding", danger ? "border-bad/55 text-bad" : "border-accent/55 text-accent")}>
      <span className={cx("hold-fill absolute inset-0", danger ? "bg-bad/25" : "bg-accent/25")} />
      <span className="relative">{holding ? "Keep holding…" : label}</span>
    </button>
  );
}

function ActionLabel({ item }: { item: PlanItem }) {
  if (item.blocked) return <span className="text-bad">Blocked</span>;
  switch (item.action) {
    case "trash": return <span className="text-soft">Move to Trash</span>;
    case "delete": return <span className="text-warn">Delete</span>;
    case "docker-prune": return <span className="text-soft">Docker prune</span>;
    default: return <span className="text-warn">Finder empties Trash</span>;
  }
}

export function PlanDialog() {
  const { plan, stage, progress, result, stopping, setDryRun, execute, stop, close } = usePlan();
  const navigate = useApp((s) => s.navigate);
  const groups = useMemo(() => {
    const map = new Map<string, PlanItem[]>();
    plan?.items.forEach((item) => map.set(item.group, [...(map.get(item.group) ?? []), item]));
    return [...map.entries()];
  }, [plan]);
  const runnable = plan ? plan.items.length - plan.blockedCount : 0;
  const permanent = plan ? plan.items.some((i) => !i.blocked && i.action !== "trash") : false;

  return (
    <AnimatePresence>
      {plan && (
        <motion.div className="fixed inset-0 z-40 grid place-items-center bg-black/60 backdrop-blur-sm" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
          <motion.div role="dialog" aria-modal="true" aria-label={`Review: ${plan.title}`} initial={{ y: 14, scale: 0.98 }} animate={{ y: 0, scale: 1 }}
            className="glass glass-edge flex max-h-[82vh] w-[760px] flex-col bg-[var(--panel-solid)]">
            <header className="flex items-start justify-between gap-4 border-b border-line px-6 py-4">
              <div>
                <div className="label">Deletion preview</div>
                <h2 className="display mt-0.5 text-xl text-ink">{plan.title}</h2>
              </div>
              {stage !== "running" && <button type="button" aria-label="Close" onClick={close} className="text-faint hover:text-ink"><X size={17} /></button>}
            </header>

            {stage === "done" && result ? (
              <div className="flex flex-col items-center gap-3 px-8 py-10 text-center">
                {result.status === "cancelled"
                  ? <OctagonPause size={38} strokeWidth={1.4} className="text-warn" />
                  : <CheckCircle2 size={38} strokeWidth={1.4} className="text-good" />}
                <h3 className="display text-2xl text-ink">
                  {result.status === "cancelled"
                    ? "Cleanup stopped"
                    : result.dryRun
                    ? `Dry run: ${bytes(result.bytesFreed + result.bytesTrashed)} would be recovered`
                    : result.bytesFreed > 0
                      ? `${bytes(result.bytesFreed)} recovered`
                      : `${bytes(result.bytesTrashed)} moved to Trash`}
                </h3>
                <p className="max-w-lg text-soft">
                  {result.status === "cancelled"
                    ? `${count(result.itemsOk)} finished before you stopped. ${count(result.itemsSkipped)} left untouched.`
                    : result.dryRun
                    ? "Nothing was changed. Every item was validated and written to the cleanup log."
                    : result.bytesTrashed > 0
                      ? `${bytes(result.bytesTrashed)} is in the Trash and can be restored. That space returns when the Trash is emptied.`
                      : "This run is recorded in the cleanup log."}
                </p>
                <div className="mono text-xs text-soft">
                  {count(result.itemsOk)} done · {count(result.itemsSkipped)} skipped · {count(result.itemsFailed)} failed
                </div>
                {result.failures.length > 0 && (
                  <ul className="mt-2 max-h-40 w-full overflow-y-auto rounded-lg border border-line p-2 text-left">
                    {result.failures.map((f) => (
                      <li key={f.path} className="py-1 text-xs">
                        <div className="mono truncate text-soft">{f.path}</div>
                        <div className="text-bad">{f.reason}</div>
                      </li>
                    ))}
                  </ul>
                )}
                <div className="mt-3 flex gap-2">
                  <Button onClick={() => { close(); navigate("history"); }}>View log</Button>
                  <Button variant="primary" onClick={close}>Done</Button>
                </div>
              </div>
            ) : (
              <>
                <div className="grid grid-cols-3 gap-px border-b border-line bg-line">
                  {[
                    ["Items", count(runnable)],
                    ["Move to Trash", bytes(plan.trashBytes)],
                    ["Removed permanently", bytes(plan.deleteBytes)],
                  ].map(([label, value]) => (
                    <div key={label} className="bg-[var(--panel-solid)] px-6 py-3">
                      <div className="label">{label}</div>
                      <div className="display mt-0.5 text-lg text-ink">{value}</div>
                    </div>
                  ))}
                </div>

                {plan.warnings.map((warning) => (
                  <div key={warning} className="flex items-center gap-2 border-b border-line bg-warn/5 px-6 py-2 text-xs text-warn">
                    <TriangleAlert size={13} className="shrink-0" />{warning}
                  </div>
                ))}
                {plan.blockedCount > 0 && (
                  <div className="flex items-center gap-2 border-b border-line bg-bad/5 px-6 py-2 text-xs text-bad">
                    <CircleSlash size={13} className="shrink-0" />
                    {plan.blockedCount} {plan.blockedCount === 1 ? "item was" : "items were"} blocked by a safety rule and will not be touched.
                  </div>
                )}

                <div className="min-h-0 flex-1 overflow-y-auto px-6 py-3">
                  {groups.map(([group, items]) => (
                    <div key={group} className="mb-3">
                      <div className="flex items-baseline justify-between py-1">
                        <span className="text-[13px] text-ink">{group}</span>
                        <span className="mono text-xs text-soft">{items.length} · {bytes(items.reduce((s, i) => s + (i.blocked ? 0 : i.size), 0))}</span>
                      </div>
                      <ul>
                        {items.slice(0, 200).map((item) => (
                          <li key={item.path} title={item.blocked ?? item.path} className="flex items-center gap-3 border-t border-line/50 py-1.5">
                            <div className="min-w-0 flex-1">
                              <div className="truncate text-xs text-ink">{baseName(item.path)}</div>
                              <div className="mono truncate text-[10px] text-faint">{item.blocked ?? item.path}</div>
                            </div>
                            <span className="shrink-0 text-[11px]"><ActionLabel item={item} /></span>
                            <span className="mono w-16 shrink-0 text-right text-xs text-soft">{bytes(item.size)}</span>
                          </li>
                        ))}
                        {items.length > 200 && <li className="py-1.5 text-xs text-faint">And {items.length - 200} more, all included above.</li>}
                      </ul>
                    </div>
                  ))}
                </div>

                <footer className="flex items-center gap-4 border-t border-line px-6 py-3.5">
                  {stage === "running" ? (
                    <>
                    <div className="min-w-0 flex-1">
                      <div className="mb-1.5 flex justify-between text-xs">
                        <span className="mono truncate text-soft">{progress?.current ? baseName(progress.current) : "Starting"}</span>
                        <span className="mono text-soft">{progress?.done ?? 0} / {progress?.total ?? plan.items.length}</span>
                      </div>
                      <Bar value={progress ? (progress.done / Math.max(progress.total, 1)) * 100 : 0} />
                    </div>
                    <Button disabled={stopping} onClick={() => void stop()}>{stopping ? "Stopping…" : "Stop after this item"}</Button>
                    </>
                  ) : (
                    <>
                      <label className="flex items-center gap-2 text-xs text-soft">
                        <Toggle label="Dry run" checked={plan.dryRun} onChange={(dry) => void setDryRun(dry)} />
                        <FlaskConical size={13} /> Dry run
                      </label>
                      <span className="flex items-center gap-1.5 text-xs text-faint">
                        <ShieldCheck size={13} /> Re-validated before each item
                      </span>
                      <div className="ml-auto flex items-center gap-2">
                        <Button onClick={close}>Cancel</Button>
                        {runnable > 0 && (
                          <HoldButton danger={permanent && !plan.dryRun} onConfirm={() => void execute()}
                            label={plan.dryRun ? "Hold to run dry run" : permanent ? "Hold to delete" : "Hold to move to Trash"} />
                        )}
                      </div>
                    </>
                  )}
                </footer>
              </>
            )}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
