import { useState, type ReactNode } from "react";
import { disable as disableAutostart, enable as enableAutostart } from "@tauri-apps/plugin-autostart";
import { Check, FolderMinus, FolderPlus, ShieldCheck, X } from "lucide-react";
import { Button, Input, Panel, ScreenHeader, Segmented, Select, Tag, Toggle, cx } from "../components/ui";
import { api, isTauri } from "../lib/ipc";
import { toast, useApp, useTelemetry } from "../stores";
import { addFolder, pickFolder, useHome } from "./shared";
import { shortPath } from "../lib/format";

function Row({ label, hint, children }: { label: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6 border-b border-line/50 py-3 last:border-0">
      <div className="min-w-0">
        <div className="text-[13px] text-ink">{label}</div>
        {hint && <div className="mt-0.5 text-xs text-soft">{hint}</div>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

const THEMES = [
  { id: "nexus", label: "NEXUS Dark", swatch: ["#05070d", "#22e3ff"] },
  { id: "arctic", label: "Arctic Blue", swatch: ["#0b1623", "#78d2ff"] },
  { id: "violet", label: "Cyber Violet", swatch: ["#100b1c", "#a47bff"] },
  { id: "obsidian", label: "Obsidian", swatch: ["#0c0c0e", "#9c9ca3"] },
  { id: "holographic", label: "Holographic", swatch: ["#0b0d1e", "#ff5ec8"] },
];

const ACCENTS = [
  { id: "cyan", color: "#22e3ff" }, { id: "blue", color: "#6aa5ff" }, { id: "violet", color: "#b794ff" },
  { id: "magenta", color: "#ff78d0" }, { id: "emerald", color: "#4cf0b4" },
];

const SECTIONS = ["Appearance", "Folders", "Scanning", "Notifications", "Cleanup", "Privacy", "Startup", "AI", "About"] as const;
type Section = (typeof SECTIONS)[number];

function FolderList({ items, onRemove, empty }: { items: string[]; onRemove: (path: string) => void; empty: string }) {
  const home = useHome();
  if (items.length === 0) return <p className="py-2 text-xs text-faint">{empty}</p>;
  return (
    <ul className="space-y-1">
      {items.map((item) => (
        <li key={item} className="flex h-8 items-center gap-2 rounded-lg border border-line px-2.5">
          <span className="mono min-w-0 flex-1 truncate text-xs text-soft" title={item}>{shortPath(item, home)}</span>
          <button type="button" aria-label={`Remove ${item}`} className="text-faint hover:text-ink" onClick={() => onRemove(item)}><X size={13} /></button>
        </li>
      ))}
    </ul>
  );
}

export function Settings() {
  const settings = useApp((s) => s.settings);
  const update = useApp((s) => s.updateSettings);
  const permissions = useApp((s) => s.permissions);
  const overview = useTelemetry((s) => s.overview);
  const [section, setSection] = useState<Section>("Appearance");
  if (!settings) return null;

  const revoke = (path: string) => api.revokeRoot(path).then(useApp.getState().setSettings).catch((e) => toast.error("Could not remove folder", e));
  const setLogin = async (launchAtLogin: boolean) => {
    try {
      if (isTauri) await (launchAtLogin ? enableAutostart() : disableAutostart());
      await update({ launchAtLogin });
    } catch (error) {
      toast.error("Could not change startup behavior", error);
    }
  };
  const clearHistory = () => api.clearHistory().then(() => toast.good("History cleared")).catch((e) => toast.error("Could not clear history", e));

  return (
    <div className="space-y-4">
      <ScreenHeader title="Settings" subtitle="Everything NEXUS does is under your control." />
      <div className="grid grid-cols-[180px_1fr] items-start gap-4">
        <nav aria-label="Settings sections" className="glass sticky top-0 p-1.5">
          {SECTIONS.map((name) => (
            <button key={name} type="button" onClick={() => setSection(name)} aria-current={section === name ? "page" : undefined}
              className={cx("flex h-8 w-full items-center rounded-lg px-2.5 text-[13px] transition-colors", section === name ? "bg-accent/12 text-ink" : "text-soft hover:text-ink")}>
              {name}
            </button>
          ))}
        </nav>

        <Panel edge>
          {section === "Appearance" && (
            <>
              <div className="label mb-2">Theme</div>
              <div className="grid grid-cols-2 gap-2.5 lg:grid-cols-5">
                {THEMES.map((theme) => {
                  const active = settings.theme === theme.id;
                  return (
                    <button key={theme.id} type="button" aria-pressed={active} onClick={() => void update({ theme: theme.id })}
                      className={cx("rounded-xl border p-2 text-left transition-colors", active ? "border-accent/70" : "border-line hover:border-[var(--line-strong)]")}>
                      <div className="relative h-14 overflow-hidden rounded-lg" style={{ background: theme.swatch[0] }}>
                        <div className="absolute -right-3 -top-3 size-14 rounded-full opacity-60 blur-md" style={{ background: theme.swatch[1] }} />
                        <div className="absolute bottom-2 left-2 h-1 w-8 rounded" style={{ background: theme.swatch[1] }} />
                        {active && <Check size={13} className="absolute right-1.5 top-1.5 text-white" />}
                      </div>
                      <div className="mt-1.5 text-xs text-ink">{theme.label}</div>
                    </button>
                  );
                })}
              </div>
              <div className="mt-5">
                <Row label="Accent color">
                  <div className="flex gap-2">
                    {ACCENTS.map((accent) => (
                      <button key={accent.id} type="button" aria-label={`${accent.id} accent`} aria-pressed={settings.accent === accent.id}
                        onClick={() => void update({ accent: accent.id })}
                        className={cx("size-6 rounded-full ring-offset-2 ring-offset-[var(--bg-1)]", settings.accent === accent.id && "ring-2 ring-white/70")}
                        style={{ background: accent.color }} />
                    ))}
                  </div>
                </Row>
                <Row label="Animation intensity" hint="Off pauses the ambient background entirely.">
                  <Segmented layoutId="set-motion" value={settings.animation} onChange={(animation) => void update({ animation })}
                    options={[{ value: "full", label: "Full" }, { value: "reduced", label: "Reduced" }, { value: "off", label: "Off" }]} />
                </Row>
              </div>
            </>
          )}

          {section === "Folders" && (
            <>
              <div className="label mb-2">Approved folders</div>
              <p className="mb-2 text-xs text-soft">NEXUS reads and offers changes only inside these folders.</p>
              <FolderList items={settings.approvedRoots} onRemove={(p) => void revoke(p)} empty="No folders approved yet." />
              <Button size="sm" className="mt-2" icon={FolderPlus} onClick={() => void addFolder()}>Add folder</Button>
              <div className="label mb-2 mt-6">Scan exclusions</div>
              <p className="mb-2 text-xs text-soft">Skipped by storage and duplicate scans. Cloud folders are excluded by default so placeholders stay untouched.</p>
              <FolderList items={settings.exclusions} empty="Nothing excluded." onRemove={(p) => void update({ exclusions: settings.exclusions.filter((e) => e !== p) })} />
              <Button size="sm" className="mt-2" icon={FolderMinus}
                onClick={() => void pickFolder().then((p) => { if (p) void update({ exclusions: [...settings.exclusions, p] }); })}>Add exclusion</Button>
            </>
          )}

          {section === "Scanning" && (
            <>
              <Row label="Automatic storage scan" hint="Scheduled scans only read. They never clean anything.">
                <Select label="Scan frequency" value={settings.scanFrequency} onChange={(scanFrequency) => void update({ scanFrequency })}
                  options={[{ value: "manual", label: "Manual only" }, { value: "daily", label: "Daily" }, { value: "weekly", label: "Weekly" }]} />
              </Row>
              <Row label="Treat files as old after" hint="Used for insights only. Age never decides what is removed.">
                <Select label="Old file age" value={String(settings.oldFileMonths)} onChange={(v) => void update({ oldFileMonths: Number(v) })}
                  options={[6, 12, 18, 24, 36].map((m) => ({ value: String(m), label: `${m} months` }))} />
              </Row>
            </>
          )}

          {section === "Notifications" && (
            <Row label="Native notifications" hint="Alerts when a scan or cleanup finishes.">
              <Toggle label="Native notifications" checked={settings.notifications} onChange={(notifications) => void update({ notifications })} />
            </Row>
          )}

          {section === "Cleanup" && (
            <>
              <Row label="Default action" hint="Applies to low-risk caches only. Everything else always goes to the Trash.">
                <Segmented layoutId="set-mode" value={settings.cleanupMode} onChange={(cleanupMode) => void update({ cleanupMode })}
                  options={[{ value: "trash", label: "Move to Trash" }, { value: "delete", label: "Delete permanently" }]} />
              </Row>
              <Row label="Dry run by default" hint="Previews and logs each cleanup without changing anything.">
                <Toggle label="Dry run by default" checked={settings.dryRun} onChange={(dryRun) => void update({ dryRun })} />
              </Row>
              <Row label="Temporary file minimum age" hint="Newer temporary files may still be in use, so they are skipped.">
                <Select label="Temporary file age" value={String(settings.tempMinAgeDays)} onChange={(v) => void update({ tempMinAgeDays: Number(v) })}
                  options={[1, 3, 7, 14].map((d) => ({ value: String(d), label: `${d} ${d === 1 ? "day" : "days"}` }))} />
              </Row>
            </>
          )}

          {section === "Privacy" && (
            <>
              <div className="mb-3 flex items-start gap-2 rounded-xl border border-line bg-accent/5 p-3 text-xs text-soft">
                <ShieldCheck size={15} className="shrink-0 text-accent" />
                NEXUS runs entirely on this Mac. It has no accounts, no analytics and no server. File contents are read only to hash them for duplicate detection.
              </div>
              <Row label="Full Disk Access" hint="Only needed to measure the Trash and data macOS otherwise hides.">
                <div className="flex items-center gap-2">
                  <Tag>{permissions?.fullDiskAccess ?? "unknown"}</Tag>
                  <Button size="sm" onClick={() => void api.openPrivacySettings()}>Open Privacy Settings</Button>
                </div>
              </Row>
              <Row label="Record history" hint="Saves a metrics sample every five minutes for trend charts.">
                <Toggle label="Record history" checked={settings.recordHistory} onChange={(recordHistory) => void update({ recordHistory })} />
              </Row>
              <Row label="Data retention">
                <Select label="Data retention" value={String(settings.retentionDays)} onChange={(v) => void update({ retentionDays: Number(v) })}
                  options={[30, 90, 180, 365, 730].map((d) => ({ value: String(d), label: `${d} days` }))} />
              </Row>
              <Row label="Clear history" hint="Removes scans, cleanup logs and trends from this Mac.">
                <Button size="sm" variant="danger" onClick={() => void clearHistory()}>Clear history</Button>
              </Row>
            </>
          )}

          {section === "Startup" && (
            <Row label="Open NEXUS at login">
              <Toggle label="Open NEXUS at login" checked={settings.launchAtLogin} onChange={(v) => void setLogin(v)} />
            </Row>
          )}

          {section === "AI" && (
            <>
              <p className="mb-3 text-xs text-soft">
                Recommendations come from a local, deterministic rules engine. The provider layer is ready for an optional LLM, which is not connected in this version.
              </p>
              <Row label="AI provider">
                <Select label="AI provider" value={settings.aiProvider} onChange={(aiProvider) => void update({ aiProvider })}
                  options={[{ value: "none", label: "Local rules only" }]} />
              </Row>
              <Row label="Model"><Input aria-label="Model" className="w-56" disabled placeholder="Not configured" value={settings.aiModel} onChange={() => {}} /></Row>
              <Row label="Endpoint"><Input aria-label="Endpoint" className="w-56" disabled placeholder="Not configured" value={settings.aiEndpoint} onChange={() => {}} /></Row>
              <Row label="Share aggregate statistics" hint="Off by default. File contents are never sent to any provider.">
                <Toggle label="Share aggregate statistics" checked={settings.aiShareAggregates} disabled onChange={() => {}} />
              </Row>
            </>
          )}

          {section === "About" && (
            <div className="flex items-center gap-5 py-2">
              <div className="grid size-16 place-items-center rounded-2xl border border-accent/30 bg-accent/10">
                <span className="display text-2xl text-accent">N</span>
              </div>
              <div>
                <div className="display text-xl tracking-[0.18em] text-ink">NEXUS</div>
                <div className="text-soft">Understand. Optimize. Evolve.</div>
                <div className="mono mt-1 text-xs text-faint">Version 0.1.0 · {overview?.osName} {overview?.osVersion}</div>
              </div>
            </div>
          )}
        </Panel>
      </div>
    </div>
  );
}
