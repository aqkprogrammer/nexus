import { useCallback, useEffect, useState, type DependencyList, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "framer-motion";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Copy, ExternalLink, FolderPlus, HardDrive, ShieldCheck, Trash2, X } from "lucide-react";
import { api, errorMessage, isTauri } from "../lib/ipc";
import { baseName, bytes, count, dateFromSeconds, shortPath } from "../lib/format";
import { toast, useApp, useMenu, usePlan } from "../stores";
import type { FileKind } from "../lib/types";
import { Button, KeyValue, Tag } from "../components/ui";

export function useLoad<R>(load: () => Promise<R>, deps: DependencyList) {
  const [data, setData] = useState<R | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const run = useCallback(() => {
    setLoading(true);
    return load()
      .then((value) => { setData(value); setError(null); })
      .catch((e) => setError(errorMessage(e)))
      .finally(() => setLoading(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
  useEffect(() => { void run(); }, [run]);
  return { data, error, loading, reload: run, setData };
}

export function useInterval(fn: () => void, ms: number) {
  useEffect(() => {
    const handle = setInterval(fn, ms);
    return () => clearInterval(handle);
  }, [fn, ms]);
}

export function useHome() {
  return useApp((s) => s.permissions?.home);
}

export const KIND_LABEL: Record<FileKind, string> = {
  folder: "Folder", video: "Video", audio: "Audio", image: "Image", document: "Document", archive: "Archive",
  installer: "Installer", diskImage: "Disk image", code: "Code", application: "Application", other: "File",
};

export function reveal(path: string) {
  api.revealInFinder(path).catch((e) => toast.error("Could not reveal item", e));
}

export function copyPath(path: string) {
  navigator.clipboard.writeText(path).then(() => toast.info("Path copied")).catch(() => toast.warn("Could not copy path"));
}

/** Items from explorer-style views always go through a Trash-only preview. */
export function reviewTrash(paths: string[], source: string, title: string, onDone?: () => void) {
  void usePlan.getState().open((dry) => api.preparePathsPlan(source, title, paths, dry), () => {
    toast.info("Storage changed", "Scan again to refresh these figures.");
    onDone?.();
  });
}

export function itemMenu(path: string, source: string, onDetails?: () => void, onDone?: () => void) {
  return (e: React.MouseEvent) => {
    e.preventDefault();
    useMenu.getState().show(e.clientX, e.clientY, [
      ...(onDetails ? [{ label: "Details", onSelect: onDetails }] : []),
      { label: "Reveal in Finder", onSelect: () => reveal(path) },
      { label: "Copy path", onSelect: () => copyPath(path) },
      { label: "Move to Trash…", danger: true, onSelect: () => reviewTrash([path], source, baseName(path), onDone) },
    ]);
  };
}

export function Drawer({ open, onClose, title, subtitle, children, footer }: {
  open: boolean; onClose: () => void; title: string; subtitle?: string; children: ReactNode; footer?: ReactNode;
}) {
  // Portaled: a backdrop-filter ancestor would otherwise become the containing block.
  return createPortal(
    <AnimatePresence>
      {open && (
        <>
          <motion.div className="fixed inset-0 z-[35] bg-black/35" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} onClick={onClose} />
          <motion.aside role="dialog" aria-label={title} initial={{ x: 40, opacity: 0 }} animate={{ x: 0, opacity: 1 }} exit={{ x: 40, opacity: 0 }}
            transition={{ type: "spring", stiffness: 380, damping: 38 }}
            className="glass glass-edge fixed bottom-3 right-3 top-[60px] z-[35] flex w-[420px] flex-col bg-[var(--panel-solid)]">
            <header className="flex items-start gap-3 border-b border-line px-5 py-4">
              <div className="min-w-0 flex-1">
                <h3 className="display truncate text-lg text-ink">{title}</h3>
                {subtitle && <p className="mono mt-0.5 break-all text-[11px] text-faint selectable">{subtitle}</p>}
              </div>
              <button type="button" aria-label="Close" onClick={onClose} className="text-faint hover:text-ink"><X size={17} /></button>
            </header>
            <div className="min-h-0 flex-1 overflow-y-auto px-5 py-3">{children}</div>
            {footer && <footer className="flex flex-wrap gap-2 border-t border-line px-5 py-3">{footer}</footer>}
          </motion.aside>
        </>
      )}
    </AnimatePresence>,
    document.body,
  );
}

export interface ItemDetails {
  path: string; size: number | null; files?: number | null; kind: FileKind; modified?: number | null; accessed?: number | null;
}

export function ItemDrawer({ item, source, onClose, onDone }: {
  item: ItemDetails | null; source: string; onClose: () => void; onDone?: () => void;
}) {
  const home = useHome();
  return (
    <Drawer open={item != null} onClose={onClose} title={item ? baseName(item.path) : ""} subtitle={item ? shortPath(item.path, home) : undefined}
      footer={item && (
        <>
          <Button icon={ExternalLink} onClick={() => reveal(item.path)}>Reveal in Finder</Button>
          <Button icon={Copy} onClick={() => copyPath(item.path)}>Copy path</Button>
          <Button variant="danger" icon={Trash2} onClick={() => { const { path } = item; onClose(); reviewTrash([path], source, baseName(path), onDone); }}>Move to Trash</Button>
        </>
      )}>
      {item && (
        <>
          <KeyValue label="Size on disk" value={bytes(item.size)} />
          <KeyValue label="Type" value={KIND_LABEL[item.kind]} mono={false} />
          {item.files != null && <KeyValue label="Files" value={count(item.files)} />}
          {item.modified != null && <KeyValue label="Modified" value={dateFromSeconds(item.modified)} />}
          {item.accessed != null && <KeyValue label="Last accessed" value={dateFromSeconds(item.accessed)} />}
          <p className="mt-4 text-xs text-soft">
            NEXUS never assumes a file is unnecessary. Moving to Trash shows a preview first and can be undone from the Trash.
          </p>
        </>
      )}
    </Drawer>
  );
}

export async function pickFolder(): Promise<string | null> {
  if (!isTauri) return null;
  const picked = await openDialog({ directory: true, multiple: false, title: "Choose a folder for NEXUS to read" });
  return typeof picked === "string" ? picked : null;
}

export async function approveFolder(path: string) {
  try {
    useApp.getState().setSettings(await api.approveRoot(path));
  } catch (error) {
    toast.error("Could not approve folder", error);
  }
}

export async function addFolder() {
  const path = await pickFolder();
  if (path) await approveFolder(path);
}

/** Shown wherever scanning needs folder access. Nothing is read until the user says so. */
export function FolderAccess({ onDone }: { onDone?: () => void }) {
  const permissions = useApp((s) => s.permissions);
  const [busy, setBusy] = useState(false);
  const approveSuggested = async () => {
    setBusy(true);
    for (const root of permissions?.suggestedRoots ?? []) await approveFolder(root);
    setBusy(false);
    onDone?.();
  };
  return (
    <div className="mx-auto flex max-w-xl flex-col items-center gap-4 px-6 py-12 text-center">
      <div className="grid size-16 place-items-center rounded-2xl border border-accent/30 bg-accent/10 text-accent">
        <ShieldCheck size={28} strokeWidth={1.4} />
      </div>
      <div>
        <h2 className="display text-[22px] text-ink">Choose what NEXUS may read</h2>
        <p className="mx-auto mt-1.5 max-w-md text-soft">
          Scans run on this Mac and read file names and sizes only. Nothing is uploaded. macOS may ask you to allow
          access to Desktop, Documents, Downloads and other apps' data the first time each is read.
        </p>
      </div>
      <div className="flex flex-wrap justify-center gap-2">
        <Button variant="primary" icon={HardDrive} disabled={busy} onClick={() => void approveSuggested()}>
          Approve Home and Applications
        </Button>
        <Button icon={FolderPlus} onClick={() => void addFolder()}>Choose a folder</Button>
      </div>
      {permissions?.suggestedRoots && (
        <div className="flex flex-wrap justify-center gap-1.5">
          {permissions.suggestedRoots.map((r) => <Tag key={r}>{r}</Tag>)}
        </div>
      )}
    </div>
  );
}

export function FullDiskNotice() {
  const access = useApp((s) => s.permissions?.fullDiskAccess);
  if (access !== "denied") return null;
  return (
    <div className="flex flex-wrap items-center gap-3 rounded-xl border border-line bg-white/[0.02] px-4 py-2.5 text-xs text-soft">
      <ShieldCheck size={14} className="text-accent" />
      <span className="min-w-60 flex-1">
        Optional: Full Disk Access lets NEXUS measure the Trash and app data macOS otherwise hides. Everything else works without it.
      </span>
      <Button size="sm" onClick={() => void api.openPrivacySettings()}>Open Privacy Settings</Button>
    </div>
  );
}
