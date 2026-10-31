import { AnimatePresence, motion } from "framer-motion";
import { X } from "lucide-react";
import type { ReactNode } from "react";
import { Radar } from "../components/viz/gauges";
import { Button, Panel, Tag } from "../components/ui";

export interface ScanStat { label: string; value: string }
export interface ScanFeedItem { key: string; title: string; detail: string }

/** The cinematic scan view shared by storage, cleaner and duplicate scans. */
export function ScanExperience({
  title, percent, sublabel, phase, current, stats, feed, feedTitle, onCancel, children,
}: {
  title: string; percent: number | null; sublabel?: string; phase: string; current: string;
  stats: ScanStat[]; feed: ScanFeedItem[]; feedTitle: string; onCancel: () => void; children?: ReactNode;
}) {
  return (
    <Panel edge padded={false} className="overflow-hidden">
      <div className="grid gap-6 p-7 lg:grid-cols-[320px_1fr]">
        <div className="flex flex-col items-center justify-center gap-3">
          <Radar percent={percent} size={280} label={title} sublabel={sublabel} />
        </div>
        <div className="flex min-w-0 flex-col gap-4">
          <div className="flex items-start justify-between gap-4">
            <div className="min-w-0">
              <div className="label">{phase}</div>
              <h2 className="display mt-0.5 text-2xl text-ink">{title}</h2>
              <p className="mono mt-1 truncate text-[11px] text-faint">{current || "Preparing…"}</p>
            </div>
            <Button icon={X} onClick={onCancel}>Cancel</Button>
          </div>
          <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
            {stats.map((stat) => (
              <div key={stat.label} className="rounded-xl border border-line bg-black/20 px-3.5 py-2.5">
                <div className="label">{stat.label}</div>
                <div className="display mt-0.5 text-xl tabular-nums text-ink">{stat.value}</div>
              </div>
            ))}
          </div>
          <div className="min-h-0 flex-1 rounded-xl border border-line bg-black/20">
            <div className="label border-b border-line px-3.5 py-2">{feedTitle}</div>
            <ul className="h-[148px] overflow-hidden px-3.5 py-1.5">
              <AnimatePresence initial={false}>
                {feed.slice(0, 7).map((item) => (
                  <motion.li key={item.key} layout initial={{ opacity: 0, x: -10 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0 }}
                    className="flex items-center justify-between gap-4 py-[3px]">
                    <span className="mono truncate text-[11px] text-soft">{item.title}</span>
                    <Tag className="shrink-0">{item.detail}</Tag>
                  </motion.li>
                ))}
              </AnimatePresence>
              {feed.length === 0 && <li className="py-2 text-xs text-faint">Nothing notable yet.</li>}
            </ul>
          </div>
          {children}
        </div>
      </div>
    </Panel>
  );
}
