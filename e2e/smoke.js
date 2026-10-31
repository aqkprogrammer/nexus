// Runs inside the real webview against the real Rust backend. HOME is a throwaway fixture.
(async () => {
  const { invoke } = window.__TAURI_INTERNALS__;
  const log = (line) => invoke("e2e_log", { line: String(line) });
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  let failures = 0;
  const check = async (name, ok, detail = "") => {
    if (!ok) failures += 1;
    await log(`${ok ? "PASS" : "FAIL"} ${name}${detail ? `  (${detail})` : ""}`);
  };
  const shot = async (name) => {
    await sleep(700);
    await log(`SHOT ${name}`);
    await sleep(900);
  };
  const button = (label, exact = true) =>
    [...document.querySelectorAll("button")].find((b) => (exact ? b.textContent.trim() === label : b.textContent.includes(label)));
  const until = async (fn, what, ms = 45000) => {
    const end = Date.now() + ms;
    while (Date.now() < end) {
      const value = await fn();
      if (value) return value;
      await sleep(120);
    }
    throw new Error(`timed out waiting for ${what}`);
  };
  const go = async (label) => {
    [...document.querySelectorAll("aside nav button")].find((b) => b.textContent.includes(label)).click();
    await sleep(350);
  };
  const drawerOnScreen = async (name) => {
    const drawer = await until(() => document.querySelector("aside[role=dialog]"), `${name} drawer`);
    await sleep(600);
    const box = drawer.getBoundingClientRect();
    const position = getComputedStyle(drawer).position;
    await check(`${name} drawer is visible`, position === "fixed" && box.left >= 0 && box.right <= innerWidth && box.top >= 0 && box.bottom <= innerHeight,
      `${position} ${Math.round(box.left)},${Math.round(box.top)} ${Math.round(box.width)}x${Math.round(box.height)}`);
  };
  const closeDrawer = async () => {
    document.querySelector("aside[role=dialog] [aria-label=Close]").click();
    await until(() => !document.querySelector("aside[role=dialog]"), "drawer to close", 3000);
  };
  const hold = async () => {
    const target = await until(() => button("Hold to", false), "hold button");
    target.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
    await sleep(1400);
    target.dispatchEvent(new PointerEvent("pointerup", { bubbles: true }));
  };

  try {
    await until(() => document.querySelector("aside nav"), "shell");
    if ((await invoke("get_settings")).onboarded) {
      throw new Error("onboarding was completed before the test began: was the window clicked?");
    }
    await until(() => button("Approve Home and Applications"), "onboarding");
    await check("first run asks before reading any folder", (await invoke("get_settings")).approvedRoots.length === 0);
    await shot("01-onboarding");
    button("Approve Home and Applications").click();
    await until(async () => (await invoke("get_settings")).onboarded, "onboarding to finish");

    await until(async () => (await invoke("get_telemetry"))?.health, "telemetry");
    const sample = await invoke("get_telemetry");
    await check("telemetry reports real memory and disk", sample.memory.total > 1e9 && sample.volume.total > 1e10);
    await check("health score is in range", sample.health.overall >= 0 && sample.health.overall <= 100, sample.health.overall);
    await until(() => !document.body.textContent.includes("Reading…"), "sidebar health");
    await shot("02-command-center");
    await check("ambient motion stays off while the window is unfocused", document.hasFocus() || !document.documentElement.dataset.pulse);

    await go("Storage Intelligence");
    await until(() => button("Start scan"), "scan button").then((b) => b.click());
    const summary = await until(() => invoke("get_storage_summary"), "storage scan");
    await check("scan found the fixture files", summary.files >= 8, `${summary.files} files`);
    await check("wire format: size buckets", Array.isArray(summary.over100Mb) && Array.isArray(summary.over1Gb));
    const homeNode = await invoke("get_storage_tree", { path: (await invoke("get_permissions")).home, depth: 1 });
    await check("sparse 8 GB disk image counted by allocation", homeNode.size < 1e9, `${homeNode.size} bytes in home`);
    await check("hogs detected", summary.hogs.some((h) => h.kind === "nodeModules"), summary.hogs.map((h) => h.kind).join(","));
    await until(() => document.querySelector(".tile"), "treemap tiles");
    await check("map rendered real tiles", document.querySelectorAll(".tile").length > 2, document.querySelectorAll(".tile").length);
    document.querySelector("main").scrollTo(0, 380);
    await shot("03-storage-map");
    button("Holographic").click();
    await sleep(900);
    await shot("04-storage-holo");

    button("Explorer").click();
    await until(() => document.body.textContent.includes("Documents"), "explorer rows");
    await check("explorer lists the approved folder", !document.body.textContent.includes("could not be read"));
    await shot("05-explorer");
    [...document.querySelectorAll("main .row-hover")].find((r) => r.textContent.includes("Documents")).click();
    await drawerOnScreen("item details");
    await closeDrawer();

    const refused = await invoke("list_directory", { path: "/etc", showHidden: false }).catch((e) => e);
    await check("unapproved folders are refused", refused.kind === "permission", refused.message);

    const settings = await invoke("get_settings");
    await invoke("save_settings", { settings: { ...settings, cleanupMode: "delete" } });

    await go("Smart Cleaner");
    await until(() => button("Scan System"), "scan button").then((b) => b.click());
    const report = await until(() => invoke("get_cleaner_report"), "cleaner scan");
    await check("cleaner found the fixture cache", report.targets.some((t) => t.id === "app-caches" && t.items.length > 0));
    await check("developer caches are not preselected", report.targets.filter((t) => t.group === "developer").every((t) => !t.selectedByDefault));
    await until(() => button("Review cleanup"), "review button");
    await shot("06-cleaner-results");

    button("Review cleanup").click();
    await until(() => button("Hold to", false), "deletion preview");
    await shot("07-deletion-preview");
    button("Dry run", false);
    await hold();
    await until(() => button("Done"), "cleanup result");
    await shot("08-cleanup-done");
    const runs = await invoke("list_cleanup_runs", { limit: 5 });
    await check("cleanup recorded to the log", runs.length === 1 && runs[0].itemsOk > 0 && runs[0].itemsFailed === 0, JSON.stringify(runs[0]));
    button("Done").click();
    await until(async () => (await invoke("job_running", { kind: "cleaner" })) === false, "rescan");

    await invoke("start_duplicate_scan", { roots: [(await invoke("get_permissions")).home], excluded: [], minSize: 1024 });
    const dupes = await until(() => invoke("get_duplicate_report"), "duplicate scan");
    await check("duplicates found", dupes.groups.length === 1 && dupes.groups[0].files.length === 3, `${dupes.groups.length} groups`);
    const all = dupes.groups[0].files.map((f) => f.path);
    const blocked = await invoke("prepare_duplicate_plan", { paths: all, dry: true }).catch((e) => e);
    await check("removing every copy is refused", blocked.kind === "safety", blocked.message);
    const dry = await invoke("prepare_duplicate_plan", { paths: all.slice(1), dry: true });
    await check("duplicate plan is trash-only", dry.items.every((i) => i.action === "trash"));
    await invoke("execute_plan", { planId: dry.id, confirmed: true });

    const escape = await invoke("prepare_paths_plan", {
      source: "test", title: "Escape", paths: ["/etc/hosts", "/System/Library", `${settings.approvedRoots[0]}/.ssh/id_ed25519`], dry: false,
    });
    await check("protected paths are blocked in plans", escape.blockedCount === 3, escape.items.map((i) => i.blocked).join(" | "));
    const unconfirmed = await invoke("execute_plan", { planId: dry.id + 1, confirmed: false }).catch((e) => e);
    await check("execution requires confirmation", unconfirmed.kind === "safety");
    await invoke("discard_plan", { planId: escape.id });

    await go("Duplicate Finder");
    await shot("09-duplicates");
    [...document.querySelectorAll("main button")].find((b) => b.textContent.includes("copy.bin")).click();
    await drawerOnScreen("duplicate compare");
    await closeDrawer();
    await go("Performance Monitor");
    await until(() => document.querySelectorAll("main button.row-hover").length > 20, "process table");
    await shot("10-performance");
    document.querySelector("main button.row-hover").click();
    await drawerOnScreen("process details");
    await closeDrawer();
    const procs = await invoke("list_processes");
    const server = procs.processes.find((p) => p.name === "WindowServer");
    const denied = await invoke("terminate_process", { pid: server.pid, startTime: server.startTime, force: false }).catch((e) => e);
    await check("system processes cannot be terminated", denied.kind === "safety", denied.message);

    await go("Battery Intelligence");
    await shot("11-battery");
    await go("Applications");
    await until(() => document.querySelector("main img"), "app icons", 20000);
    document.querySelector("main button.row-hover").click();
    await drawerOnScreen("application details");
    await closeDrawer();
    const apps = await invoke("list_apps");
    await check("applications listed with metadata", apps.some((a) => a.bundleId));
    button("Show macOS apps").click();
    await sleep(1800);
    await shot("12-applications");
    await go("NEXUS AI");
    await until(() => document.body.textContent.includes("System assessment"), "advisor");
    await shot("13-advisor");
    await go("Optimization Center");
    await shot("14-optimization");
    await go("Scan History");
    await shot("15-history");
    await invoke("export_report", { path: `${settings.approvedRoots[0]}/report.md` });
    await check("health report exported", true);
  } catch (error) {
    failures += 1;
    await log(`FAIL harness: ${error?.message ?? JSON.stringify(error)} [screen: ${document.querySelector("h1")?.textContent}]`);
    await log("SHOT 99-failure");
    await sleep(4000);
  }
  await log(`DONE failures=${failures}`);
  await invoke("e2e_exit", { code: failures ? 1 : 0 });
})();
