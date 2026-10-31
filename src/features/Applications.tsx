import { useEffect, useMemo, useState } from "react";
import { AppWindow, ExternalLink, PackageX, Search } from "lucide-react";
import { Button, Check, EmptyState, ErrorState, Input, KeyValue, Panel, ScreenHeader, Segmented, Skeleton, Status, Tag } from "../components/ui";
import { api, on } from "../lib/ipc";
import { bytes, count, dateFromSeconds, shortPath } from "../lib/format";
import { usePlan } from "../stores";
import type { AppInfo, AssociatedItem } from "../lib/types";
import { Drawer, reveal, useHome, useLoad } from "./shared";

const icons = new Map<string, string | null>();
let active = 0;
const waiting: (() => void)[] = [];

/** Icon extraction shells out, so at most four run at once. */
function fetchIcon(path: string): Promise<string | null> {
  if (icons.has(path)) return Promise.resolve(icons.get(path)!);
  return new Promise((resolve) => {
    const run = () => {
      active += 1;
      api.appIcon(path)
        .then((url) => { icons.set(path, url); resolve(url); })
        .catch(() => resolve(null))
        .finally(() => { active -= 1; waiting.shift()?.(); });
    };
    if (active < 4) run(); else waiting.push(run);
  });
}

function AppIcon({ path, size = 28 }: { path: string; size?: number }) {
  const [src, setSrc] = useState<string | null>(icons.get(path) ?? null);
  useEffect(() => {
    let alive = true;
    fetchIcon(path).then((url) => alive && setSrc(url));
    return () => { alive = false; };
  }, [path]);
  return src
    ? <img src={src} alt="" width={size} height={size} className="shrink-0" />
    : <div className="grid shrink-0 place-items-center rounded-md border border-line text-faint" style={{ width: size, height: size }}><AppWindow size={size * 0.55} /></div>;
}

function AppDrawer({ app, onClose, onRemoved }: { app: AppInfo | null; onClose: () => void; onRemoved: () => void }) {
  const home = useHome();
  const associated = useLoad<AssociatedItem[]>(() => (app ? api.appAssociated(app.path) : Promise.resolve([])), [app?.path]);
  const [chosen, setChosen] = useState<Set<string>>(new Set());
  useEffect(() => setChosen(new Set()), [app?.path]);
  const items = associated.data ?? [];
  const uninstall = () => {
    if (!app) return;
    const paths = [...chosen];
    onClose();
    void usePlan.getState().open((dry) => api.prepareUninstall(app.path, paths, dry), onRemoved);
  };
  return (
    <Drawer open={app != null} onClose={onClose} title={app?.name ?? ""} subtitle={app?.path}
      footer={app && (
        <>
          <Button icon={ExternalLink} onClick={() => reveal(app.path)}>Show in Finder</Button>
          <Button variant="danger" icon={PackageX} disabled={app.isSystem || app.running} onClick={uninstall}
            title={app.isSystem ? "Part of macOS" : app.running ? "Quit the app first" : undefined}>Review uninstall</Button>
        </>
      )}>
      {app && (
        <>
          <div className="mb-3 flex items-center gap-3">
            <AppIcon path={app.path} size={52} />
            <div className="min-w-0">
              <div className="flex flex-wrap gap-1.5">
                {app.isSystem && <Tag>macOS app</Tag>}
                {app.running && <Tag className="border-accent/30 text-accent">Running</Tag>}
              </div>
            </div>
          </div>
          <KeyValue label="Version" value={app.version} />
          <KeyValue label="Bundle ID" value={app.bundleId} />
          <KeyValue label="App size" value={app.size != null ? bytes(app.size) : "Measuring…"} />
          <KeyValue label="App data" value={app.dataSize != null ? bytes(app.dataSize) : "Measuring…"} />
          <KeyValue label="Modified" value={dateFromSeconds(app.modified)} />

          <div className="label mb-1 mt-5">Associated files</div>
          {associated.loading ? <Skeleton className="h-16" /> : associated.error ? <Status tone="warn">{associated.error}</Status> : items.length === 0 ? (
            <p className="text-xs text-soft">No matching support files were found.</p>
          ) : (
            <ul>
              {items.map((item) => (
                <li key={item.path} className="flex items-center gap-2.5 border-b border-line/50 py-1.5">
                  {!app.isSystem && <Check label={`Include ${item.path}`} checked={chosen.has(item.path)} onChange={(v) => setChosen((set) => {
                    const next = new Set(set);
                    if (v) next.add(item.path); else next.delete(item.path);
                    return next;
                  })} />}
                  <div className="min-w-0 flex-1">
                    <div className="text-xs text-ink">{item.label}</div>
                    <div className="mono truncate text-[10px] text-faint" title={item.path}>{shortPath(item.path, home)}</div>
                  </div>
                  <span className="mono shrink-0 text-xs text-soft">{bytes(item.size)}</span>
                </li>
              ))}
            </ul>
          )}
          {!app.isSystem && items.length > 0 && <p className="mt-2 text-xs text-soft">Tick items to include them in an uninstall. Unticked data stays where it is.</p>}
        </>
      )}
    </Drawer>
  );
}

export function Applications() {
  const list = useLoad(() => api.listApps(), []);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState("size");
  const [showSystem, setShowSystem] = useState(false);
  const [open, setOpen] = useState<string | null>(null);

  useEffect(() => on<{ path: string; size: number | null; dataSize: number | null }>("apps://size", (event) => {
    list.setData((apps) => apps?.map((a) => (a.path === event.path ? { ...a, size: event.size, dataSize: event.dataSize } : a)) ?? apps);
  }), []); // eslint-disable-line react-hooks/exhaustive-deps

  const apps = useMemo(() => {
    const needle = query.toLowerCase();
    return (list.data ?? [])
      .filter((a) => (showSystem || !a.isSystem) && (!needle || a.name.toLowerCase().includes(needle) || a.bundleId?.toLowerCase().includes(needle)))
      .sort((a, b) => {
        if (sort === "name") return a.name.localeCompare(b.name);
        if (sort === "data") return (b.dataSize ?? 0) - (a.dataSize ?? 0);
        return (b.size ?? 0) + (b.dataSize ?? 0) - ((a.size ?? 0) + (a.dataSize ?? 0));
      });
  }, [list.data, query, sort, showSystem]);

  const total = apps.reduce((sum, a) => sum + (a.size ?? 0), 0);
  const data = apps.reduce((sum, a) => sum + (a.dataSize ?? 0), 0);
  const selected = list.data?.find((a) => a.path === open) ?? null;

  return (
    <div className="space-y-4">
      <ScreenHeader title="Applications" subtitle="What each app occupies, including the data it keeps elsewhere.">
        <div className="relative">
          <Search size={13} className="absolute left-3 top-3 text-faint" />
          <Input aria-label="Search applications" value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Search applications" className="w-56 pl-8" />
        </div>
        <Segmented layoutId="apps-sort" value={sort} onChange={setSort}
          options={[{ value: "size", label: "Total size" }, { value: "data", label: "App data" }, { value: "name", label: "Name" }]} />
        <Button onClick={() => setShowSystem((v) => !v)} aria-pressed={showSystem}>{showSystem ? "Hide macOS apps" : "Show macOS apps"}</Button>
      </ScreenHeader>

      <div className="grid grid-cols-3 gap-3">
        {[["Applications", count(apps.length)], ["App bundles", bytes(total)], ["App data", bytes(data)]].map(([label, value]) => (
          <div key={label} className="glass p-4">
            <div className="label">{label}</div>
            <div className="display mt-1 text-2xl text-ink">{value}</div>
          </div>
        ))}
      </div>

      <Panel padded={false}>
        <div className="label flex items-center gap-3 border-b border-line px-5 py-2.5">
          <span className="w-7" />
          <span className="min-w-0 flex-1">Application</span>
          <span className="w-52">Bundle identifier</span>
          <span className="w-20">Version</span>
          <span className="w-24 text-right">Modified</span>
          <span className="w-20 text-right">App</span>
          <span className="w-20 text-right">Data</span>
        </div>
        {list.error ? <ErrorState title="Applications could not be listed" body={list.error} onRetry={() => void list.reload()} /> : list.loading && !list.data ? (
          <div className="space-y-2 p-5">{Array.from({ length: 8 }, (_, i) => <Skeleton key={i} className="h-7" />)}</div>
        ) : apps.length === 0 ? <EmptyState icon={AppWindow} title="No applications match" body="Try a different search." /> : (
          <div className="max-h-[600px] overflow-y-auto">
            {apps.map((app) => (
              <button key={app.path} type="button" onClick={() => setOpen(app.path)} className="row-hover flex h-[44px] w-full items-center gap-3 border-b border-line/40 px-5 text-left">
                <AppIcon path={app.path} />
                <span className="flex min-w-0 flex-1 items-center gap-2">
                  <span className="truncate text-[13px] text-ink">{app.name}</span>
                  {app.running && <span className="size-1.5 shrink-0 rounded-full bg-good" title="Running" />}
                </span>
                <span className="mono w-52 truncate text-[11px] text-faint">{app.bundleId ?? "Unavailable"}</span>
                <span className="mono w-20 truncate text-xs text-soft">{app.version ?? "—"}</span>
                <span className="mono w-24 text-right text-xs text-soft">{dateFromSeconds(app.modified)}</span>
                <span className="mono w-20 text-right text-xs text-ink">{app.size != null ? bytes(app.size) : "…"}</span>
                <span className="mono w-20 text-right text-xs text-soft">{app.dataSize != null ? bytes(app.dataSize) : "…"}</span>
              </button>
            ))}
          </div>
        )}
      </Panel>
      <AppDrawer app={selected} onClose={() => setOpen(null)} onRemoved={() => void list.reload()} />
    </div>
  );
}
