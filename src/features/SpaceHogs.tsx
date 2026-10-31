import { useMemo, useState } from "react";
import { motion } from "framer-motion";
import {
  Archive, Box, Clapperboard, Container, CopyMinus, Disc3, Download, Folder, Hammer, Info, Package, Sparkles, Trash2, Weight, Wrench, type LucideIcon,
} from "lucide-react";
import { Button, Check, EmptyState, Panel, ScreenHeader, Tag, cx } from "../components/ui";
import { ageMonths, bytes, count, dateFromSeconds, shortPath } from "../lib/format";
import { useApp, useStorage } from "../stores";
import type { HogItem, HogKind } from "../lib/types";
import { ItemDrawer, KIND_LABEL, itemMenu, reviewTrash, useHome, type ItemDetails } from "./shared";

const HOG: Record<HogKind, { label: string; icon: LucideIcon; note: string }> = {
  largeVideo: { label: "Large videos", icon: Clapperboard, note: "Video files over 100 MB." },
  oldInstaller: { label: "Installers", icon: Package, note: "Installer packages you may no longer need after installing." },
  zipArchive: { label: "ZIP archives", icon: Archive, note: "Compressed archives over 50 MB." },
  diskImage: { label: "Disk images", icon: Disc3, note: "DMG and ISO images. Check they are not your only copy." },
  virtualMachine: { label: "Virtual machines", icon: Box, note: "VM bundles and disks. Removing one deletes that whole machine." },
  docker: { label: "Docker", icon: Container, note: "Docker's disk image. Use Smart Cleaner to prune through Docker itself." },
  xcodeDerivedData: { label: "Xcode DerivedData", icon: Hammer, note: "Rebuildable, but the next build takes longer." },
  nodeModules: { label: "Node modules", icon: Folder, note: "Reinstallable from a lockfile. Keep the ones you are actively using." },
  buildArtifact: { label: "Build artifacts", icon: Wrench, note: "Output folders such as target, dist and .next." },
  projectBackup: { label: "Project backups", icon: CopyMinus, note: "Names suggest a backup, which NEXUS cannot confirm. Check before removing." },
  unusedDownload: { label: "Unused downloads", icon: Download, note: "In Downloads and unchanged for 6 months or more." },
  aiModels: { label: "AI models", icon: Sparkles, note: "Local model weights. They can be downloaded again." },
};

export function SpaceHogs() {
  const summary = useStorage((s) => s.summary);
  const navigate = useApp((s) => s.navigate);
  const home = useHome();
  const [kind, setKind] = useState<HogKind | "all">("all");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [details, setDetails] = useState<HogItem | null>(null);

  const groups = useMemo(() => {
    const totals = new Map<HogKind, { bytes: number; items: number }>();
    summary?.hogs.forEach((h) => {
      const entry = totals.get(h.kind) ?? { bytes: 0, items: 0 };
      totals.set(h.kind, { bytes: entry.bytes + h.size, items: entry.items + 1 });
    });
    return [...totals.entries()].sort((a, b) => b[1].bytes - a[1].bytes);
  }, [summary]);

  const visible = useMemo(
    () => (summary?.hogs ?? []).filter((h) => kind === "all" || h.kind === kind).sort((a, b) => b.size - a.size),
    [summary, kind],
  );
  const max = visible[0]?.size ?? 0;
  const selectedBytes = visible.filter((h) => selected.has(h.path)).reduce((sum, h) => sum + h.size, 0);

  if (!summary) {
    return (
      <div className="space-y-4">
        <ScreenHeader title="Space Hogs" subtitle="The largest things consuming your disk." />
        <Panel edge>
          <EmptyState icon={Weight} title="No scan yet" body="Space Hogs reads from your latest storage scan.">
            <Button variant="primary" onClick={() => navigate("storage")}>Go to Storage Intelligence</Button>
          </EmptyState>
        </Panel>
      </div>
    );
  }

  const details2: ItemDetails | null = details && {
    path: details.path, size: details.size, files: details.isDir ? details.files : null,
    kind: details.isDir ? "folder" : "other", modified: details.modified, accessed: details.accessed,
  };

  return (
    <div className="space-y-4">
      <ScreenHeader title="Space Hogs" subtitle="The largest things consuming your disk." />
      <div className="flex items-start gap-2 rounded-xl border border-line bg-white/[0.02] px-4 py-2.5 text-xs text-soft">
        <Info size={14} className="mt-px shrink-0 text-accent" />
        Age and size are only hints. NEXUS never assumes an old file is unnecessary, so nothing here is preselected.
      </div>

      <div className="grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-4">
        {groups.map(([id, group], i) => {
          const { icon: Icon, label } = HOG[id];
          const active = kind === id;
          return (
            <motion.button key={id} type="button" initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: i * 0.03 }}
              onClick={() => setKind(active ? "all" : id)} aria-pressed={active}
              className={cx("glass flex items-center gap-3 p-3.5 text-left transition-colors hover:border-accent/30", active && "!border-accent/60")}>
              <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-accent/10 text-accent"><Icon size={17} strokeWidth={1.6} /></span>
              <span className="min-w-0">
                <span className="block truncate text-xs text-soft">{label}</span>
                <span className="display block text-lg leading-tight text-ink">{bytes(group.bytes)}</span>
                <span className="mono block text-[10px] text-faint">{count(group.items)} {group.items === 1 ? "item" : "items"}</span>
              </span>
            </motion.button>
          );
        })}
      </div>

      <Panel padded={false} title={kind === "all" ? "All space hogs" : HOG[kind].label}
        aside={kind !== "all" ? <span className="text-xs text-soft">{HOG[kind].note}</span> : undefined}>
        {visible.length === 0 ? <div className="grid h-40 place-items-center text-soft">Nothing large enough was found in the scanned folders.</div> : (
          <div className="max-h-[520px] overflow-y-auto">
            {visible.slice(0, 300).map((hog) => {
              const Icon = HOG[hog.kind].icon;
              const old = hog.modified > 0 && ageMonths(hog.modified) >= 12;
              return (
                <div key={hog.path} onClick={() => setDetails(hog)} onContextMenu={itemMenu(hog.path, "hogs", () => setDetails(hog))}
                  className="row-hover flex h-[46px] items-center gap-3 border-b border-line/40 px-5">
                  <Check label={`Select ${hog.name}`} checked={selected.has(hog.path)} onChange={(on) => setSelected((s) => {
                    const next = new Set(s);
                    if (on) next.add(hog.path); else next.delete(hog.path);
                    return next;
                  })} />
                  <Icon size={15} strokeWidth={1.6} className="shrink-0 text-accent" />
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-[13px] text-ink">{hog.name}</div>
                    <div className="mono truncate text-[10px] text-faint">{shortPath(hog.path, home)}</div>
                  </div>
                  {old && <Tag className="border-warn/30 text-warn">Old</Tag>}
                  <span className="w-28 shrink-0 truncate text-xs text-soft">{hog.isDir ? KIND_LABEL.folder : HOG[hog.kind].label}</span>
                  <span className="mono w-24 shrink-0 text-right text-xs text-soft">{dateFromSeconds(hog.modified)}</span>
                  <div className="w-28 shrink-0">
                    <div className="mono text-right text-xs text-ink">{bytes(hog.size)}</div>
                    <div className="mt-0.5 h-[3px] rounded-full bg-white/5">
                      <div className="ml-auto h-full rounded-full bg-accent/70" style={{ width: `${Math.max(2, (hog.size / max) * 100)}%` }} />
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        )}
        {selected.size > 0 && (
          <div className="flex items-center gap-3 border-t border-line px-5 py-2.5">
            <span className="text-xs text-soft">{selected.size} selected · {bytes(selectedBytes)}</span>
            <Button size="sm" onClick={() => setSelected(new Set())}>Clear</Button>
            <Button size="sm" variant="danger" icon={Trash2} className="ml-auto"
              onClick={() => reviewTrash([...selected], "hogs", "Space Hogs", () => setSelected(new Set()))}>
              Review move to Trash
            </Button>
          </div>
        )}
      </Panel>
      <ItemDrawer source="hogs" item={details2} onClose={() => setDetails(null)} />
    </div>
  );
}
