import { useEffect, useMemo, useState } from "react";
import { Copy, FolderMinus, FolderPlus, Image as ImageIcon, Radar as RadarIcon, ShieldCheck, Trash2, X } from "lucide-react";
import { Button, Check, EmptyState, Panel, ScreenHeader, Select, Tag } from "../components/ui";
import { api } from "../lib/ipc";
import { baseName, bytes, count, dateFromSeconds, shortPath } from "../lib/format";
import { useApp, useDuplicates, usePlan } from "../stores";
import type { DuplicateGroup } from "../lib/types";
import { ScanExperience } from "./ScanExperience";
import { Drawer, FolderAccess, KIND_LABEL, addFolder, itemMenu, pickFolder, reveal, useHome } from "./shared";

function Preview({ path, kind }: { path: string; kind: string }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    if (kind !== "image") return;
    let alive = true;
    api.imagePreview(path).then((url) => alive && setSrc(url)).catch(() => {});
    return () => { alive = false; };
  }, [path, kind]);
  if (kind !== "image") return null;
  return src
    ? <img src={src} alt={`Preview of ${baseName(path)}`} className="h-28 w-full rounded-lg border border-line object-cover" />
    : <div className="grid h-28 place-items-center rounded-lg border border-line text-faint"><ImageIcon size={18} /></div>;
}

function GroupDrawer({ group, onClose }: { group: DuplicateGroup | null; onClose: () => void }) {
  const { selected, setSelected } = useDuplicates();
  const home = useHome();
  const toggle = (path: string, on: boolean) => {
    const next = new Set(selected);
    if (on) next.add(path); else next.delete(path);
    setSelected(next);
  };
  const keepOnly = (keep: string) => {
    if (!group) return;
    const next = new Set(selected);
    group.files.forEach((f) => (f.path === keep ? next.delete(f.path) : next.add(f.path)));
    setSelected(next);
  };
  return (
    <Drawer open={group != null} onClose={onClose} title={group?.files[0].name ?? ""} subtitle={group ? `BLAKE3 ${group.hash.slice(0, 24)}…` : undefined}
      footer={group && <span className="text-xs text-soft">Compare, then tick the copies to remove. At least one copy must stay.</span>}>
      {group && (
        <div className="space-y-3">
          <div className="flex gap-2">
            <Tag>{bytes(group.size)} each</Tag>
            <Tag>{KIND_LABEL[group.kind]}</Tag>
            <Tag>{group.files.length} copies</Tag>
          </div>
          {group.files.map((file, index) => {
            const oldest = index === 0;
            return (
              <div key={file.path} className="rounded-xl border border-line p-3">
                <div className="flex items-start gap-2.5">
                  <Check label={`Remove ${file.path}`} checked={selected.has(file.path)} onChange={(on) => toggle(file.path, on)} />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="truncate text-[13px] text-ink">{file.name}</span>
                      {oldest && <Tag className="border-accent/30 text-accent">Oldest</Tag>}
                    </div>
                    <div className="mono mt-0.5 break-all text-[10px] text-faint">{shortPath(file.path, home)}</div>
                    <div className="mono mt-1 text-[10px] text-soft">Modified {dateFromSeconds(file.modified)} · Created {dateFromSeconds(file.created)}</div>
                  </div>
                </div>
                <div className="mt-2"><Preview path={file.path} kind={group.kind} /></div>
                <div className="mt-2 flex gap-2">
                  <Button size="sm" onClick={() => reveal(file.path)}>Reveal</Button>
                  <Button size="sm" onClick={() => keepOnly(file.path)}>Keep only this</Button>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </Drawer>
  );
}

const SIZES = [
  { value: "1048576", label: "1 MB and larger" },
  { value: "10485760", label: "10 MB and larger" },
  { value: "104857600", label: "100 MB and larger" },
  { value: "65536", label: "64 KB and larger" },
];

export function Duplicates() {
  const { report, scanning, progress, cancel, start, selected, setSelected } = useDuplicates();
  const roots = useApp((s) => s.settings?.approvedRoots);
  const home = useHome();
  const [folders, setFolders] = useState<string[]>([]);
  const [excluded, setExcluded] = useState<string[]>([]);
  const [minSize, setMinSize] = useState(SIZES[0].value);
  const [open, setOpen] = useState<DuplicateGroup | null>(null);

  useEffect(() => {
    if (roots && folders.length === 0) setFolders(roots.filter((r) => r !== "/Applications").slice(0, 1));
  }, [roots]); // eslint-disable-line react-hooks/exhaustive-deps

  const selectedBytes = useMemo(() => {
    if (!report) return 0;
    return report.groups.reduce((sum, g) => sum + g.files.filter((f) => selected.has(f.path)).length * g.size, 0);
  }, [report, selected]);

  const header = (
    <ScreenHeader title="Duplicate Finder" subtitle="Byte-identical files, confirmed by BLAKE3 hash.">
      {report && !scanning && <Button icon={RadarIcon} onClick={() => void start(folders, excluded, Number(minSize))}>Scan again</Button>}
    </ScreenHeader>
  );

  if (roots && roots.length === 0) return <div className="space-y-4">{header}<Panel edge><FolderAccess /></Panel></div>;

  if (scanning) {
    const hashPercent = progress && progress.bytesTotal > 0 ? (progress.bytesHashed / progress.bytesTotal) * 100 : null;
    return (
      <div className="space-y-4">
        {header}
        <ScanExperience
          title="Looking for identical files"
          phase={progress?.phase ?? "Starting"}
          percent={hashPercent}
          sublabel={hashPercent != null ? "hashing" : "indexing"}
          current={progress ? shortPath(progress.current, home) : ""}
          onCancel={cancel}
          stats={[
            { label: "Files indexed", value: count(progress?.files ?? 0) },
            { label: "Candidates", value: count(progress?.candidates ?? 0) },
            { label: "Hashed", value: bytes(progress?.bytesHashed ?? 0) },
            { label: "To hash", value: bytes(progress?.bytesTotal ?? 0) },
          ]}
          feedTitle="How this works"
          feed={[
            { key: "1", title: "Group files by exact size", detail: "Fast" },
            { key: "2", title: "Compare the start and end of each candidate", detail: "Fast" },
            { key: "3", title: "Hash the full contents of what remains", detail: "BLAKE3" },
          ]}
        />
      </div>
    );
  }

  const setup = (
    <Panel title="Scan setup">
      <div className="grid gap-5 md:grid-cols-2">
        <div>
          <div className="mb-2 text-[13px] text-ink">Folders to scan</div>
          <ul className="space-y-1">
            {folders.map((folder) => (
              <li key={folder} className="flex h-8 items-center gap-2 rounded-lg border border-line px-2.5">
                <span className="mono min-w-0 flex-1 truncate text-xs text-soft">{shortPath(folder, home)}</span>
                <button type="button" aria-label={`Remove ${folder}`} onClick={() => setFolders((f) => f.filter((x) => x !== folder))} className="text-faint hover:text-ink"><X size={13} /></button>
              </li>
            ))}
          </ul>
          <div className="mt-2 flex flex-wrap gap-2">
            {(roots ?? []).filter((r) => !folders.includes(r)).map((root) => (
              <Button key={root} size="sm" icon={FolderPlus} onClick={() => setFolders((f) => [...f, root])}>{shortPath(root, home)}</Button>
            ))}
            <Button size="sm" icon={FolderPlus} onClick={() => void addFolder()}>Approve another folder</Button>
          </div>
        </div>
        <div>
          <div className="mb-2 text-[13px] text-ink">Exclude</div>
          <ul className="space-y-1">
            {excluded.map((folder) => (
              <li key={folder} className="flex h-8 items-center gap-2 rounded-lg border border-line px-2.5">
                <span className="mono min-w-0 flex-1 truncate text-xs text-soft">{shortPath(folder, home)}</span>
                <button type="button" aria-label={`Stop excluding ${folder}`} onClick={() => setExcluded((f) => f.filter((x) => x !== folder))} className="text-faint hover:text-ink"><X size={13} /></button>
              </li>
            ))}
          </ul>
          <Button size="sm" className="mt-2" icon={FolderMinus} onClick={() => void pickFolder().then((p) => p && setExcluded((f) => [...f, p]))}>Exclude a folder</Button>
          <p className="mt-2 text-xs text-soft">Library, .git, node_modules, app bundles and photo libraries are always skipped.</p>
        </div>
      </div>
      <div className="mt-4 flex items-center gap-3">
        <Select label="Minimum file size" value={minSize} onChange={setMinSize} options={SIZES} />
        <Button variant="primary" icon={RadarIcon} disabled={folders.length === 0} onClick={() => void start(folders, excluded, Number(minSize))}>Find duplicates</Button>
      </div>
    </Panel>
  );

  if (!report) {
    return (
      <div className="space-y-4">
        {header}
        {setup}
        <Panel edge><EmptyState icon={Copy} title="No duplicate scan yet" body="Files are compared by content, not name. iCloud placeholders are skipped so nothing gets downloaded." /></Panel>
      </div>
    );
  }

  return (
    <div className="space-y-4 pb-20">
      {header}
      <div className="grid grid-cols-4 gap-3">
        {[
          ["Recoverable", bytes(report.recoverable), "Keeping one copy of each"],
          ["Groups", count(report.groups.length), `${(report.durationMs / 1000).toFixed(1)}s scan`],
          ["Files indexed", count(report.filesIndexed), `${count(report.inaccessible)} folders denied`],
          ["Hashed", bytes(report.bytesHashed), "BLAKE3, full contents"],
        ].map(([label, value, hint]) => (
          <div key={label} className="glass p-4">
            <div className="label">{label}</div>
            <div className="display mt-1 text-2xl text-ink">{value}</div>
            <div className="text-xs text-soft">{hint}</div>
          </div>
        ))}
      </div>
      <div className="flex items-center gap-2 rounded-xl border border-line bg-white/[0.02] px-4 py-2.5 text-xs text-soft">
        <ShieldCheck size={14} className="text-accent" />
        Nothing is preselected. Open a group to compare copies and choose which to remove.
      </div>
      <Panel padded={false} title="Duplicate groups">
        {report.groups.length === 0 ? <EmptyState icon={Copy} title="No duplicates found" body="No identical files of this size in the scanned folders." /> : (
          <div className="max-h-[560px] overflow-y-auto">
            {report.groups.slice(0, 400).map((group) => {
              const picked = group.files.filter((f) => selected.has(f.path)).length;
              return (
                <button key={group.id} type="button" onClick={() => setOpen(group)} onContextMenu={itemMenu(group.files[0].path, "duplicates")}
                  className="row-hover flex w-full items-center gap-3 border-b border-line/40 px-5 py-2.5 text-left">
                  <Copy size={15} strokeWidth={1.6} className="shrink-0 text-accent" />
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-[13px] text-ink">{group.files[0].name}</div>
                    <div className="mono truncate text-[10px] text-faint">{group.files.map((f) => shortPath(f.path, home)).join("  ·  ")}</div>
                  </div>
                  {picked > 0 && <Tag className="border-accent/30 text-accent">{picked} to remove</Tag>}
                  <Tag>{group.files.length} copies</Tag>
                  <span className="mono w-20 shrink-0 text-right text-xs text-soft">{bytes(group.size)}</span>
                  <span className="mono w-20 shrink-0 text-right text-[13px] text-ink">{bytes(group.recoverable)}</span>
                </button>
              );
            })}
          </div>
        )}
      </Panel>
      {setup}
      {selected.size > 0 && (
        <div className="sticky bottom-0 z-10 flex items-center gap-4 rounded-xl border border-accent/30 bg-[var(--panel-solid)]/95 px-5 py-3 backdrop-blur">
          <div>
            <div className="label">Selected</div>
            <div className="display text-xl text-ink">{bytes(selectedBytes)}</div>
          </div>
          <Tag>{count(selected.size)} files</Tag>
          <Button size="sm" onClick={() => setSelected(new Set())}>Clear</Button>
          <Button variant="primary" className="ml-auto" icon={Trash2}
            onClick={() => {
              const paths = [...selected];
              void usePlan.getState().open((dry) => api.prepareDuplicatePlan(paths, dry), () => {
                setSelected(new Set());
                void start(folders, excluded, Number(minSize));
              });
            }}>
            Review removal
          </Button>
        </div>
      )}
      <GroupDrawer group={open} onClose={() => setOpen(null)} />
    </div>
  );
}
