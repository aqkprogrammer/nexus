import { motion } from "framer-motion";
import {
  Activity, AppWindow, BatteryCharging, Copy, Gauge, HardDrive, History, PanelLeftClose, PanelLeftOpen, Settings,
  Sparkles, Trash2, Wand2, Weight, type LucideIcon,
} from "lucide-react";
import { useApp, useTelemetry, type Route } from "../../stores";
import { Ring } from "../viz/gauges";
import { cx } from "../ui";

export interface NavItem { route: Route; label: string; icon: LucideIcon; shortcut?: string }

export const NAV: { group: string; items: NavItem[] }[] = [
  {
    group: "Overview",
    items: [{ route: "command", label: "Command Center", icon: Gauge, shortcut: "1" }],
  },
  {
    group: "Storage",
    items: [
      { route: "storage", label: "Storage Intelligence", icon: HardDrive, shortcut: "2" },
      { route: "hogs", label: "Space Hogs", icon: Weight, shortcut: "3" },
      { route: "optimize", label: "Optimization Center", icon: Wand2, shortcut: "4" },
      { route: "cleaner", label: "Smart Cleaner", icon: Trash2, shortcut: "5" },
      { route: "duplicates", label: "Duplicate Finder", icon: Copy, shortcut: "6" },
    ],
  },
  {
    group: "System",
    items: [
      { route: "performance", label: "Performance Monitor", icon: Activity, shortcut: "7" },
      { route: "battery", label: "Battery Intelligence", icon: BatteryCharging, shortcut: "8" },
      { route: "apps", label: "Applications", icon: AppWindow, shortcut: "9" },
    ],
  },
  {
    group: "Intelligence",
    items: [
      { route: "ai", label: "NEXUS AI", icon: Sparkles },
      { route: "history", label: "Scan History", icon: History },
      { route: "settings", label: "Settings", icon: Settings },
    ],
  },
];

export function Sidebar() {
  const route = useApp((s) => s.route);
  const navigate = useApp((s) => s.navigate);
  const collapsed = useApp((s) => s.sidebarCollapsed);
  const toggle = useApp((s) => s.toggleSidebar);
  const health = useTelemetry((s) => s.sample?.health);

  return (
    <aside className={cx("relative z-10 flex h-full shrink-0 flex-col border-r border-line bg-black/30 transition-[width] duration-300", collapsed ? "w-[68px]" : "w-[236px]")}>
      <div data-tauri-drag-region className="flex h-[58px] shrink-0 items-end px-[18px] pb-1" />
      <div className={cx("flex items-center gap-2.5 px-[18px] pb-4 pt-1", collapsed && "justify-center px-0")}>
        <svg width="28" height="28" viewBox="0 0 32 32" fill="none" aria-hidden className="shrink-0">
          <defs>
            <linearGradient id="logo" x1="0" y1="0" x2="1" y2="1">
              <stop stopColor="var(--accent)" /><stop offset="1" stopColor="var(--accent-3)" />
            </linearGradient>
          </defs>
          <circle cx="16" cy="16" r="14.5" stroke="url(#logo)" strokeOpacity=".5" strokeDasharray="60 31" />
          <path d="M10 22V10l12 12V10" stroke="url(#logo)" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
        {!collapsed && (
          <div className="min-w-0">
            <div className="display text-[15px] font-semibold tracking-[0.2em] text-ink">NEXUS</div>
            <div className="truncate text-[10px] text-faint">Understand. Optimize. Evolve.</div>
          </div>
        )}
      </div>

      <nav aria-label="Main" className="flex-1 space-y-4 overflow-y-auto px-2.5 pb-3">
        {NAV.map(({ group, items }) => (
          <div key={group}>
            {!collapsed && <div className="label px-2.5 pb-1.5">{group}</div>}
            <ul className="space-y-0.5">
              {items.map((item) => {
                const active = route === item.route;
                const Icon = item.icon;
                return (
                  <li key={item.route}>
                    <button type="button" title={collapsed ? item.label : undefined} aria-current={active ? "page" : undefined}
                      onClick={() => navigate(item.route)}
                      className={cx("relative flex h-[34px] w-full items-center gap-2.5 rounded-lg px-2.5 text-[13px] transition-colors", collapsed && "justify-center px-0", active ? "text-ink" : "text-soft hover:bg-white/[0.04] hover:text-ink")}>
                      {active && (
                        <motion.span layoutId="nav-active" transition={{ type: "spring", stiffness: 420, damping: 36 }}
                          className="absolute inset-0 rounded-lg border border-accent/30 bg-accent/10">
                          <span className="absolute -left-[3px] top-2 bottom-2 w-[2px] rounded bg-accent shadow-[0_0_10px_var(--accent)]" />
                        </motion.span>
                      )}
                      <Icon size={16} strokeWidth={1.7} className={cx("relative shrink-0", active && "text-accent")} />
                      {!collapsed && <span className="relative truncate">{item.label}</span>}
                      {!collapsed && item.shortcut && <span className="mono relative ml-auto text-[10px] text-faint">⌘{item.shortcut}</span>}
                    </button>
                  </li>
                );
              })}
            </ul>
          </div>
        ))}
      </nav>

      <div className={cx("flex items-center gap-2.5 border-t border-line p-3", collapsed && "flex-col")}>
        <Ring value={health?.overall ?? null} size={34} stroke={3}>
          <span className="mono text-[10px] text-ink">{health ? Math.round(health.overall) : "--"}</span>
        </Ring>
        {!collapsed && (
          <div className="min-w-0 flex-1">
            <div className="text-xs text-ink">{health?.status ?? "Reading…"}</div>
            <div className="text-[10px] text-faint">NEXUS heuristic</div>
          </div>
        )}
        <button type="button" aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"} title="Toggle sidebar (⌘B)" onClick={toggle} className="grid size-7 place-items-center rounded-md text-soft hover:bg-white/5 hover:text-ink">
          {collapsed ? <PanelLeftOpen size={15} /> : <PanelLeftClose size={15} />}
        </button>
      </div>
    </aside>
  );
}
