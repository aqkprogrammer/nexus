import { useCallback, useEffect, useMemo, useState } from "react";
import { motion } from "framer-motion";
import {
  ArrowUp, ChevronDown, ChevronRight, Eye, EyeOff, File, Folder, FolderPlus, Gauge, HardDrive, Layers, ListTree,
  Map as MapIcon, Radar as RadarIcon, RotateCw, Search, Trash2,
} from "lucide-react";
import { Donut, type DonutSegment } from "../components/viz/gauges";
import { HoloMap, type MapSelection } from "../components/viz/HoloMap";
import { Button, Check, EmptyState, ErrorState, Input, Panel, ScreenHeader, Segmented, Select, Skeleton, Status, Tag, cx } from "../components/ui";
import { api, errorMessage } from "../lib/ipc";
import { ago, ageMonths, baseName, bytes, count, dateFromSeconds, shortPath, splitBytes } from "../lib/format";
import { CATEGORY_COLOR, CATEGORY_LABEL, SYSTEM_COLOR, storageSegments } from "../lib/score";
import { useApp, useStorage, useTelemetry, toast } from "../stores";
import type { Category, DirEntry, FileItem, FileKind, ScanSummary } from "../lib/types";
import { ScanExperience } from "./ScanExperience";
import { FolderAccess, FullDiskNotice, ItemDrawer, KIND_LABEL, addFolder, itemMenu, reviewTrash, useHome, type ItemDetails } from "./shared";

type Tab = "map" | "explorer" | "largest";

function ScanView() {
  const { progress, discoveries, cancel } = useStorage();
  const used = useTelemetry((s) => s.sample?.volume.used ?? 0);
  const home = useHome();
  // Total size is unknown until the walk ends, so estimate against used disk and cap below 100.
  const percent = progress && used ? Math.min(99, (progress.bytes / used) * 100) : null;
  return (
    <ScanExperience
      title="Deep scan in progress"
      phase="Mapping your storage"
      percent={percent}
      sublabel="estimated"
      current={progress ? shortPath(progress.current, home) : ""}
      onCancel={cancel}
      stats={[
        { label: "Space analyzed", value: bytes(progress?.bytes ?? 0) },
        { label: "Files", value: count(progress?.files ?? 0) },
        { label: "Folders", value: count(progress?.directories ?? 0) },
        { label: "Inaccessible", value: count(progress?.inaccessible ?? 0) },
      ]}
      feedTitle="Live discovery: files over 100 MB"
      feed={discoveries.map((d) => ({ key: d.path, title: shortPath(d.path, home), detail: bytes(d.size) }))}
    />
  );
}

function Overview({ summary, selected, onSelect }: {
  summary: ScanSummary; selected: Category | null; onSelect: (category: Category | null) => void;
}) {
  const volume = useTelemetry((s) => s.sample?.volume);
  if (!volume) return <Skeleton className="h-[372px]" />;
  const segments = storageSegments(volume, summary);
  const donut: DonutSegment[] = segments.map((s) => ({
    id: s.id, label: s.label, value: s.bytes, color: s.id === "system" ? SYSTEM_COLOR : CATEGORY_COLOR[s.id as Category],
  }));
  const [used, unit] = splitBytes(volume.used);
  return (
    <Panel edge title="Storage overview" className="col-span-12 xl:col-span-7">
      <div className="grid grid-cols-[auto_1fr] items-center gap-5">
        <Donut segments={donut} total={volume.total} selected={selected} size={252}
          onSelect={(id) => id !== "system" && onSelect(selected === id ? null : (id as Category))}
          center={
            <>
              <span className="label">Used</span>
              <span className="display mt-1 text-[32px] leading-none text-ink">{used}</span>
              <span className="mono mt-1 text-xs text-soft">{unit} of {bytes(volume.total)}</span>
            </>
          } />
        <ul className="min-w-0 space-y-0.5">
          {segments.map((segment, i) => {
            const interactive = segment.id !== "system";
            const active = selected === segment.id;
            return (
              <li key={segment.id}>
                <button type="button" disabled={!interactive} onClick={() => onSelect(active ? null : (segment.id as Category))}
                  className={cx("flex h-7 w-full items-center gap-2 rounded-md px-1.5 text-left transition-colors", interactive && "hover:bg-white/5", active && "bg-accent/10")}>
                  <span className="size-2.5 shrink-0 rounded-sm" style={{ background: donut[i].color }} />
                  <span className="truncate text-xs text-soft">{segment.label}</span>
                  <span className="mono ml-auto shrink-0 text-xs text-ink">{bytes(segment.bytes)}</span>
                </button>
              </li>
            );
          })}
          <li className="flex h-7 items-center gap-2 px-1.5">
            <span className="size-2.5 shrink-0 rounded-sm border border-line" />
            <span className="text-xs text-soft">Available</span>
            <span className="mono ml-auto text-xs text-ink">{bytes(volume.available)}</span>
          </li>
        </ul>
      </div>
    </Panel>
  );
}

function Insights({ summary }: { summary: ScanSummary }) {
  const stats = [
    { label: "Files over 100 MB", value: count(summary.over100Mb[0]), hint: bytes(summary.over100Mb[1]) },
    { label: "Files over 1 GB", value: count(summary.over1Gb[0]), hint: bytes(summary.over1Gb[1]) },
    { label: `Untouched ${summary.old.cutoffMonths}+ months`, value: bytes(summary.old.bytes), hint: `${count(summary.old.files)} files` },
    { label: "Scanned", value: bytes(summary.totalBytes), hint: `${count(summary.files)} files` },
  ];
  return (
    <Panel title="Scan insights" className="col-span-12 xl:col-span-5"
      aside={<span className="mono text-[10px] text-faint">{ago(summary.finishedAt)} · {((summary.finishedAt - summary.startedAt) / 1000).toFixed(1)}s</span>}>
      <div className="grid grid-cols-2 gap-2.5">
        {stats.map((s) => (
          <div key={s.label} className="rounded-xl border border-line bg-black/15 px-3.5 py-2.5">
            <div className="label">{s.label}</div>
            <div className="display mt-1 text-xl text-ink">{s.value}</div>
            <div className="mono text-[11px] text-soft">{s.hint}</div>
          </div>
        ))}
      </div>
      <div className="mt-3 flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-soft">
        <span>Locations:</span>
        {summary.roots.map((root) => <Tag key={root}>{root}</Tag>)}
        {summary.inaccessible > 0 && <Status tone="neutral">{count(summary.inaccessible)} items skipped: macOS denied access</Status>}
        {summary.reusedDirectories > 0 && <span className="text-faint">{count(summary.reusedDirectories)} unchanged folders reused from cache</span>}
      </div>
      <div className="mt-3"><FullDiskNotice /></div>
    </Panel>
  );
}

function ageFlags(size: number | null, modified: number | null) {
  const flags: string[] = [];
  if (size != null && size >= 1e9) flags.push("1 GB+");
  else if (size != null && size >= 1e8) flags.push("100 MB+");
  if (modified && ageMonths(modified) >= 12) flags.push("Old");
  return flags;
}

function SizeCell({ size, max }: { size: number | null; max: number }) {
  return (
    <div className="w-28 shrink-0">
      <div className="mono text-right text-xs text-ink">{size == null ? "—" : bytes(size)}</div>
      {size != null && max > 0 && (
        <div className="mt-0.5 h-[3px] rounded-full bg-white/5">
          <div className="ml-auto h-full rounded-full bg-accent/70" style={{ width: `${Math.max(2, (size / max) * 100)}%` }} />
        </div>
      )}
    </div>
  );
}

const KIND_FILTERS: { value: string; label: string }[] = [
  { value: "all", label: "All types" },
  ...(["folder", "video", "audio", "image", "document", "archive", "installer", "diskImage", "code", "application", "other"] as FileKind[])
    .map((k) => ({ value: k, label: KIND_LABEL[k] })),
];

function ExplorerRow({ entry, depth, expanded, onToggle, onOpen, onDetails, onMeasure, max, selected, onSelect, showHidden, sort, kind, onDone }: {
  entry: DirEntry; depth: number; expanded: Record<string, DirEntry[] | "loading">; onToggle: (e: DirEntry) => void;
  onOpen: (e: DirEntry) => void; onDetails: (e: DirEntry) => void; onMeasure: (e: DirEntry) => void; max: number;
  selected: Set<string>; onSelect: (path: string, on: boolean) => void; showHidden: boolean; sort: string; kind: string; onDone: () => void;
}) {
  const children = expanded[entry.path];
  const open = children !== undefined;
  const flags = ageFlags(entry.size, entry.modified);
  return (
    <>
      <div onDoubleClick={() => (entry.isDir && !entry.isSymlink ? onOpen(entry) : onDetails(entry))} onClick={() => onDetails(entry)}
        onContextMenu={itemMenu(entry.path, "explorer", () => onDetails(entry), onDone)}
        className="row-hover flex h-[34px] items-center gap-3 border-b border-line/40 px-5">
        <Check label={`Select ${entry.name}`} checked={selected.has(entry.path)} onChange={(on) => onSelect(entry.path, on)} />
        <div className="flex min-w-0 flex-1 items-center gap-1.5" style={{ paddingLeft: depth * 16 }}>
          {entry.isDir && !entry.isSymlink ? (
            <button type="button" aria-label={open ? "Collapse" : "Expand"} className="text-faint hover:text-ink" onClick={(e) => { e.stopPropagation(); onToggle(entry); }}>
              {open ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
            </button>
          ) : <span className="w-[13px]" />}
          {entry.isDir ? <Folder size={14} className="shrink-0 text-accent" strokeWidth={1.7} /> : <File size={14} className="shrink-0 text-soft" strokeWidth={1.7} />}
          <span className={cx("truncate text-[13px]", entry.isHidden ? "text-soft" : "text-ink")}>{entry.name}</span>
          {entry.isSymlink && <Tag>Link</Tag>}
          {flags.map((flag) => <Tag key={flag} className={flag === "Old" ? "border-warn/30 text-warn" : "border-accent/30 text-accent"}>{flag}</Tag>)}
        </div>
        <span className="w-20 shrink-0 truncate text-xs text-soft">{KIND_LABEL[entry.kind]}</span>
        <span className="mono w-14 shrink-0 text-right text-xs text-soft">{entry.files != null ? count(entry.files) : ""}</span>
        <span className="mono w-24 shrink-0 text-right text-xs text-soft">{dateFromSeconds(entry.modified)}</span>
        <span className="mono w-24 shrink-0 text-right text-xs text-faint">{dateFromSeconds(entry.accessed)}</span>
        {entry.size == null && entry.isDir && !entry.isSymlink ? (
          <div className="flex w-28 shrink-0 justify-end">
            <button type="button" className="text-xs text-accent hover:underline" onClick={(e) => { e.stopPropagation(); onMeasure(entry); }}>Measure</button>
          </div>
        ) : <SizeCell size={entry.size} max={max} />}
      </div>
      {children === "loading" && <div className="px-5 py-2 text-xs text-faint" style={{ paddingLeft: 60 + (depth + 1) * 16 }}>Loading…</div>}
      {Array.isArray(children) && arrange(children, sort, kind).map((child) => (
        <ExplorerRow key={child.path} entry={child} depth={depth + 1} expanded={expanded} onToggle={onToggle} onOpen={onOpen} onDetails={onDetails}
          onMeasure={onMeasure} max={max} selected={selected} onSelect={onSelect} showHidden={showHidden} sort={sort} kind={kind} onDone={onDone} />
      ))}
    </>
  );
}

function arrange(entries: DirEntry[], sort: string, kind: string) {
  const filtered = kind === "all" ? entries : entries.filter((e) => e.kind === kind);
  return [...filtered].sort((a, b) => {
    if (sort === "modified") return (b.modified ?? 0) - (a.modified ?? 0);
    if (sort === "name") return a.name.localeCompare(b.name);
    return (b.size ?? 0) - (a.size ?? 0);
  });
}

function Explorer() {
  const roots = useApp((s) => s.settings?.approvedRoots ?? []);
  const home = useHome();
  const [cwd, setCwd] = useState(roots[0] ?? "");
  const [entries, setEntries] = useState<DirEntry[] | null>(null);
  const [parent, setParent] = useState<string | null>(null);
  const [skipped, setSkipped] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<Record<string, DirEntry[] | "loading">>({});
  const [showHidden, setShowHidden] = useState(false);
  const [sort, setSort] = useState("size");
  const [kind, setKind] = useState("all");
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<DirEntry[] | null>(null);
  const [details, setDetails] = useState<DirEntry | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());

  const load = useCallback(() => {
    if (!cwd) return;
    setEntries(null);
    setError(null);
    setExpanded({});
    api.listDirectory(cwd, showHidden).then((listing) => {
      setEntries(listing.entries);
      setParent(listing.parent);
      setSkipped(listing.skipped);
    }).catch((e) => setError(errorMessage(e)));
  }, [cwd, showHidden]);

  useEffect(load, [load]);

  useEffect(() => {
    const needle = query.trim();
    if (needle.length < 2) {
      setResults(null);
      return;
    }
    const handle = setTimeout(() => api.searchFiles(needle).then(setResults).catch(() => setResults([])), 260);
    return () => clearTimeout(handle);
  }, [query]);

  const patchEntry = (path: string, patch: Partial<DirEntry>) => {
    const apply = (list: DirEntry[]) => list.map((e) => (e.path === path ? { ...e, ...patch } : e));
    setEntries((list) => (list ? apply(list) : list));
    setExpanded((map) => Object.fromEntries(Object.entries(map).map(([k, v]) => [k, Array.isArray(v) ? apply(v) : v])));
  };

  const measure = (entry: DirEntry) => {
    api.measureDirectory(entry.path)
      .then(([size, files]) => patchEntry(entry.path, { size, files }))
      .catch((e) => toast.error("Could not measure folder", e));
  };

  const toggle = (entry: DirEntry) => {
    if (expanded[entry.path] !== undefined) {
      setExpanded(({ [entry.path]: _, ...rest }) => rest);
      return;
    }
    setExpanded((map) => ({ ...map, [entry.path]: "loading" }));
    api.listDirectory(entry.path, showHidden)
      .then((listing) => setExpanded((map) => ({ ...map, [entry.path]: listing.entries })))
      .catch((e) => {
        setExpanded(({ [entry.path]: _, ...rest }) => rest);
        toast.error("Could not open folder", e);
      });
  };

  const rows = useMemo(() => arrange(results ?? entries ?? [], sort, kind), [results, entries, sort, kind]);
  const max = rows.reduce((m, e) => Math.max(m, e.size ?? 0), 0);
  const atRoot = roots.includes(cwd);

  return (
    <Panel padded={false}>
      <div className="flex flex-wrap items-center gap-2 border-b border-line px-5 py-3">
        <Button size="sm" icon={ArrowUp} disabled={atRoot || !parent} aria-label="Up one folder" onClick={() => parent && setCwd(parent)}>Up</Button>
        <Select label="Location" className="max-w-52 !h-7 text-xs" value={roots.includes(cwd) ? cwd : ""} onChange={(v) => v && setCwd(v)}
          options={[...(roots.includes(cwd) ? [] : [{ value: "", label: baseName(cwd) }]), ...roots.map((r) => ({ value: r, label: shortPath(r, home) }))]} />
        <span className="mono min-w-0 flex-1 truncate text-xs text-soft">{shortPath(cwd, home)}</span>
        <div className="relative">
          <Search size={13} className="absolute left-2.5 top-2 text-faint" />
          <Input aria-label="Search files and folders" value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Search all approved folders" className="!h-7 w-56 pl-7 text-xs" />
        </div>
        <Select label="Filter by type" className="!h-7 text-xs" value={kind} onChange={setKind} options={KIND_FILTERS} />
        <Select label="Sort" className="!h-7 text-xs" value={sort} onChange={setSort}
          options={[{ value: "size", label: "Largest first" }, { value: "modified", label: "Recently modified" }, { value: "name", label: "Name" }]} />
        <Button size="sm" icon={showHidden ? Eye : EyeOff} onClick={() => setShowHidden((v) => !v)} aria-pressed={showHidden}>Hidden files</Button>
        <Button size="sm" icon={FolderPlus} onClick={() => void addFolder()}>Add folder</Button>
      </div>
      <div className="label flex items-center gap-3 border-b border-line px-5 py-2">
        <span className="w-[17px]" />
        <span className="flex-1">Name</span>
        <span className="w-20">Type</span>
        <span className="w-14 text-right">Files</span>
        <span className="w-24 text-right">Modified</span>
        <span className="w-24 text-right">Accessed</span>
        <span className="w-28 text-right">Size</span>
      </div>
      <div className="h-[460px] overflow-y-auto">
        {error ? <ErrorState title="This folder could not be read" body={error} onRetry={load} /> : entries == null ? (
          <div className="space-y-2 p-5">{Array.from({ length: 8 }, (_, i) => <Skeleton key={i} className="h-6" />)}</div>
        ) : rows.length === 0 ? (
          <div className="grid h-40 place-items-center text-soft">{results ? "No files match that search." : "This folder is empty."}</div>
        ) : rows.slice(0, 600).map((entry) => (
          <ExplorerRow key={entry.path} entry={entry} depth={0} expanded={expanded} onToggle={toggle}
            onOpen={(e) => { setQuery(""); setCwd(e.path); }} onDetails={setDetails} onMeasure={measure} max={max}
            selected={selected} showHidden={showHidden} sort={sort} kind={kind} onDone={load}
            onSelect={(path, on) => setSelected((s) => { const next = new Set(s); if (on) next.add(path); else next.delete(path); return next; })} />
        ))}
      </div>
      <div className="flex items-center gap-3 border-t border-line px-5 py-2 text-xs text-soft">
        <span>{count(rows.length)} items</span>
        {skipped > 0 && <span className="text-faint">{skipped} skipped: macOS denied access</span>}
        {selected.size > 0 && (
          <Button size="sm" variant="danger" className="ml-auto" icon={Trash2}
            onClick={() => reviewTrash([...selected], "explorer", "Selected items", () => { setSelected(new Set()); load(); })}>
            Review {selected.size} selected
          </Button>
        )}
      </div>
      <ItemDrawer source="explorer" onClose={() => setDetails(null)} onDone={load}
        item={details && { path: details.path, size: details.size, files: details.files, kind: details.kind, modified: details.modified, accessed: details.accessed }} />
    </Panel>
  );
}

function LargestFiles({ summary, category, onClearCategory }: { summary: ScanSummary; category: Category | null; onClearCategory: () => void }) {
  const home = useHome();
  const [band, setBand] = useState("all");
  const [kind, setKind] = useState("all");
  const [details, setDetails] = useState<FileItem | null>(null);
  const files = useMemo(() => summary.topFiles.filter((f) =>
    (!category || f.category === category) && (kind === "all" || f.kind === kind) &&
    (band === "all" || (band === "1g" && f.size >= 1e9) || (band === "100m" && f.size >= 1e8) || (band === "old" && ageMonths(f.modified) >= summary.old.cutoffMonths))
  ), [summary, category, kind, band]);
  const max = files[0]?.size ?? 0;
  return (
    <Panel padded={false}>
      <div className="flex flex-wrap items-center gap-2 border-b border-line px-5 py-3">
        <Segmented layoutId="largest-band" value={band} onChange={setBand}
          options={[{ value: "all", label: "All" }, { value: "100m", label: "Over 100 MB" }, { value: "1g", label: "Over 1 GB" }, { value: "old", label: "Old" }]} />
        <Select label="Filter by type" className="!h-[30px] text-xs" value={kind} onChange={setKind} options={KIND_FILTERS.filter((k) => k.value !== "folder")} />
        {category && <Button size="sm" onClick={onClearCategory}>Showing {CATEGORY_LABEL[category]}: clear</Button>}
        <span className="mono ml-auto text-xs text-faint">{count(files.length)} files, 10 MB and up</span>
      </div>
      <div className="h-[500px] overflow-y-auto">
        {files.length === 0 ? <div className="grid h-40 place-items-center text-soft">No files match these filters.</div> : files.slice(0, 400).map((file) => (
          <div key={file.path} onClick={() => setDetails(file)} onContextMenu={itemMenu(file.path, "largest", () => setDetails(file))}
            className="row-hover flex h-[42px] items-center gap-3 border-b border-line/40 px-5">
            <span className="size-2 shrink-0 rounded-sm" style={{ background: CATEGORY_COLOR[file.category] }} />
            <div className="min-w-0 flex-1">
              <div className="truncate text-[13px] text-ink">{file.name}</div>
              <div className="mono truncate text-[10px] text-faint">{shortPath(file.path, home)}</div>
            </div>
            {ageFlags(null, file.modified).map((f) => <Tag key={f} className="border-warn/30 text-warn">{f}</Tag>)}
            <span className="w-20 shrink-0 text-xs text-soft">{KIND_LABEL[file.kind]}</span>
            <span className="mono w-24 shrink-0 text-right text-xs text-soft">{dateFromSeconds(file.modified)}</span>
            <SizeCell size={file.size} max={max} />
          </div>
        ))}
      </div>
      <ItemDrawer source="largest" item={details} onClose={() => setDetails(null)} />
    </Panel>
  );
}

export function Storage() {
  const { summary, scanning, start } = useStorage();
  const roots = useApp((s) => s.settings?.approvedRoots);
  const filter = useApp((s) => s.storageFilter);
  const setFilter = useApp((s) => s.setStorageFilter);
  const [tab, setTab] = useState<Tab>("map");
  const [mapItem, setMapItem] = useState<ItemDetails | null>(null);

  const onMap = (selection: MapSelection): ItemDetails => ({
    path: selection.path, size: selection.node.size, files: selection.node.files,
    kind: selection.node.kind, modified: selection.node.modified || null,
  });

  const header = (
    <ScreenHeader title="Storage Intelligence" subtitle="See exactly how your disk is being used.">
      {summary && !scanning && <Button icon={Gauge} onClick={() => void start(true)} title="Ignores the cache and re-reads every folder">Deep rescan</Button>}
      {!scanning && roots && roots.length > 0 && <Button variant="primary" icon={summary ? RotateCw : RadarIcon} onClick={() => void start(false)}>{summary ? "Scan again" : "Start scan"}</Button>}
    </ScreenHeader>
  );

  if (roots && roots.length === 0) {
    return <div className="space-y-4">{header}<Panel edge><FolderAccess /></Panel></div>;
  }
  if (scanning) return <div className="space-y-4">{header}<ScanView /></div>;
  if (!summary) {
    return (
      <div className="space-y-4">
        {header}
        <Panel edge>
          <EmptyState icon={HardDrive} title="Map your storage" body="NEXUS walks your approved folders, measures what each one really occupies, and builds an interactive map. Nothing is changed.">
            <Button variant="primary" icon={RadarIcon} onClick={() => void start(false)}>Start scan</Button>
          </EmptyState>
        </Panel>
      </div>
    );
  }

  return (
    <motion.div className="space-y-4" initial={{ opacity: 0 }} animate={{ opacity: 1 }}>
      {header}
      <div className="grid grid-cols-12 gap-4">
        <Overview summary={summary} selected={filter} onSelect={(category) => { setFilter(category); if (category) setTab("largest"); }} />
        <Insights summary={summary} />
      </div>
      <Segmented layoutId="storage-tab" value={tab} onChange={setTab}
        options={[{ value: "map", label: "Holographic map", icon: MapIcon }, { value: "explorer", label: "Explorer", icon: ListTree }, { value: "largest", label: "Largest files", icon: Layers }]} />
      {tab === "map" && (
        <Panel edge>
          <HoloMap height={500} onSelect={(s) => setMapItem(onMap(s))}
            onContext={(s, x, y) => itemMenu(s.path, "map", () => setMapItem(onMap(s)))({ preventDefault() {}, clientX: x, clientY: y } as React.MouseEvent)} />
          <ItemDrawer source="map" item={mapItem} onClose={() => setMapItem(null)} />
        </Panel>
      )}
      {tab === "explorer" && <Explorer />}
      {tab === "largest" && <LargestFiles summary={summary} category={filter} onClearCategory={() => setFilter(null)} />}
    </motion.div>
  );
}
