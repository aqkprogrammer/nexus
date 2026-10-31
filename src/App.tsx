import { Component, useEffect, useRef, type ComponentType, type ReactNode } from "react";
import { motion } from "framer-motion";
import { Background, CommandPalette, ContextMenu, JobDock, MockBanner, TitleBar, Toasts } from "./components/shell/Chrome";
import { PlanDialog } from "./components/shell/PlanDialog";
import { NAV, Sidebar } from "./components/shell/Sidebar";
import { Button, ErrorState } from "./components/ui";
import { Advisor } from "./features/Advisor";
import { Applications } from "./features/Applications";
import { Battery } from "./features/Battery";
import { Cleaner } from "./features/Cleaner";
import { CommandCenter } from "./features/CommandCenter";
import { Duplicates } from "./features/Duplicates";
import { History } from "./features/History";
import { Optimize } from "./features/Optimize";
import { Performance } from "./features/Performance";
import { Settings } from "./features/Settings";
import { SpaceHogs } from "./features/SpaceHogs";
import { Storage } from "./features/Storage";
import { FolderAccess } from "./features/shared";
import { useApp, useMenu, usePlan, type Route } from "./stores";

const SCREENS: Record<Route, ComponentType> = {
  command: CommandCenter, storage: Storage, hogs: SpaceHogs, optimize: Optimize, cleaner: Cleaner, duplicates: Duplicates,
  performance: Performance, battery: Battery, apps: Applications, ai: Advisor, history: History, settings: Settings,
};

/** Contains a failure to one screen so the shell, and any running job, keep working. */
class ScreenBoundary extends Component<{ children: ReactNode }, { error: string | null }> {
  state = { error: null };

  static getDerivedStateFromError(error: unknown) {
    return { error: error instanceof Error ? error.message : "This screen stopped unexpectedly." };
  }

  render() {
    if (this.state.error) {
      return <ErrorState title="This screen hit a problem" body={this.state.error} onRetry={() => this.setState({ error: null })} />;
    }
    return this.props.children;
  }
}

function Onboarding() {
  const settings = useApp((s) => s.settings);
  const update = useApp((s) => s.updateSettings);
  if (!settings || settings.onboarded) return null;
  const finish = () => void update({ onboarded: true });
  return (
    <motion.div className="fixed inset-0 z-40 grid place-items-center bg-black/65 backdrop-blur-md" initial={{ opacity: 0 }} animate={{ opacity: 1 }}>
      <motion.div role="dialog" aria-label="Welcome to NEXUS" initial={{ y: 16, scale: 0.98 }} animate={{ y: 0, scale: 1 }}
        className="glass glass-edge w-[620px] bg-[var(--panel-solid)]">
        <FolderAccess onDone={finish} />
        <div className="flex items-center justify-between border-t border-line px-6 py-3">
          <span className="text-xs text-soft">The dashboard works without any folder access.</span>
          <Button onClick={finish}>Continue without scanning</Button>
        </div>
      </motion.div>
    </motion.div>
  );
}

const PULSE_MS = 6000;
const PULSE_EVERY_MS = { full: 24_000, reduced: 60_000 };

/** Runs ambient motion in short pulses, only while the window has focus. */
function useAmbient() {
  const animation = useApp((s) => s.settings?.animation ?? "full");
  useEffect(() => {
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (animation === "off" || reduced) return;
    const root = document.documentElement;
    let ending: ReturnType<typeof setTimeout> | undefined;
    const stop = () => {
      clearTimeout(ending);
      delete root.dataset.pulse;
    };
    const pulse = () => {
      if (!document.hasFocus() || root.dataset.pulse) return;
      root.dataset.pulse = "on";
      ending = setTimeout(stop, PULSE_MS);
    };
    pulse();
    const every = setInterval(pulse, animation === "full" ? PULSE_EVERY_MS.full : PULSE_EVERY_MS.reduced);
    window.addEventListener("focus", pulse);
    window.addEventListener("blur", stop);
    return () => {
      clearInterval(every);
      window.removeEventListener("focus", pulse);
      window.removeEventListener("blur", stop);
      stop();
    };
  }, [animation]);
}

function useShortcuts() {
  useEffect(() => {
    const routes = NAV.flatMap((g) => g.items).filter((i) => i.shortcut);
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        useMenu.getState().hide();
        if (usePlan.getState().stage !== "running" && usePlan.getState().plan) usePlan.getState().close();
      }
      if (!e.metaKey) return;
      const app = useApp.getState();
      const target = routes.find((r) => r.shortcut === e.key);
      if (target) {
        e.preventDefault();
        app.navigate(target.route);
      } else if (e.key === "k") {
        e.preventDefault();
        app.setPalette(!app.paletteOpen);
      } else if (e.key === "b") {
        e.preventDefault();
        app.toggleSidebar();
      } else if (e.key === ",") {
        e.preventDefault();
        app.navigate("settings");
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);
}

export function App() {
  const route = useApp((s) => s.route);
  const Screen = SCREENS[route];
  const main = useRef<HTMLElement>(null);
  useShortcuts();
  useAmbient();
  useEffect(() => {
    main.current?.scrollTo(0, 0);
  }, [route]);
  return (
    <div className="relative flex h-full bg-base" onContextMenu={(e) => e.preventDefault()}>
      <Background />
      <Sidebar />
      <div className="relative flex min-w-0 flex-1 flex-col">
        <TitleBar />
        <main ref={main} className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
          <div className="mx-auto max-w-[1500px]">
            <div key={route} className="screen-in">
              <ScreenBoundary>
                <Screen />
              </ScreenBoundary>
            </div>
          </div>
        </main>
      </div>
      <JobDock />
      <PlanDialog />
      <CommandPalette />
      <ContextMenu />
      <Toasts />
      <Onboarding />
      <MockBanner />
    </div>
  );
}
