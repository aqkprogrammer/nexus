import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { ChevronRight, Box, Grid2x2, Home } from "lucide-react";
import { motion } from "framer-motion";
import { squarify, type Rect } from "../../lib/treemap";
import { CATEGORY_COLOR, CATEGORY_LABEL } from "../../lib/score";
import { api } from "../../lib/ipc";
import { useStorage } from "../../stores";
import { bytes, count } from "../../lib/format";
import { Segmented, Skeleton } from "../ui";
import type { Category, TreeNode } from "../../lib/types";

export interface MapSelection { path: string; node: TreeNode }

interface Tooltip { x: number; y: number; node: TreeNode; color: string }

const MAX_TILES = 180;

function shortName(name: string) {
  return name.split("/").filter(Boolean).pop() ?? name;
}

function join(base: string | null, name: string) {
  if (base == null) return name;
  return base.endsWith("/") ? `${base}${name}` : `${base}/${name}`;
}

interface MapTile extends Rect {
  key: string; node: TreeNode; path: string; depth: number; color: string; share: number; nest: boolean;
}

function colorFor(node: TreeNode, inherited: Category) {
  if (node.aggregate) return "#39425a";
  return CATEGORY_COLOR[node.category === "other" ? inherited : node.category];
}

/**
 * Holographic Storage Map. A squarified treemap drawn as DOM tiles (bounded in count) so
 * each block stays a real element for hover and context menus. Holo mode tilts the plane
 * and lifts tiles by share of the parent, using only compositor-friendly 3D transforms.
 */
export function HoloMap({ onSelect, onContext, height }: {
  onSelect: (selection: MapSelection) => void;
  onContext: (selection: MapSelection, x: number, y: number) => void;
  height: number;
}) {
  const [trail, setTrail] = useState<{ path: string | null; label: string }[]>([{ path: null, label: "All locations" }]);
  const [node, setNode] = useState<TreeNode | null>(null);
  const [loading, setLoading] = useState(true);
  const [mode, setMode] = useState<"holo" | "flat">("flat");
  const [tooltip, setTooltip] = useState<Tooltip | null>(null);
  const stage = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);

  const current = trail[trail.length - 1];
  const scanVersion = useStorage((s) => s.summary?.adjustedAt ?? s.summary?.finishedAt);

  useLayoutEffect(() => {
    if (!stage.current) return;
    const observer = new ResizeObserver(([entry]) => setWidth(entry.contentRect.width));
    observer.observe(stage.current);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    let alive = true;
    setLoading(true);
    api.getStorageTree(current.path, 3).then((tree) => {
      if (!alive) return;
      // The folder being viewed may have just been cleaned up.
      if (!tree && current.path) {
        setTrail((t) => t.slice(0, 1));
        return;
      }
      setNode(tree);
      setLoading(false);
    });
    return () => {
      alive = false;
    };
  }, [current.path, scanVersion]);

  const tiles = useMemo(() => {
    if (!node?.children || width === 0) return [];
    const total = node.size;
    const out: MapTile[] = [];
    const place = (children: TreeNode[], bounds: Rect, base: string | null, depth: number, inherited: Category) => {
      const laid = squarify(children.filter((c) => c.size > 0).slice(0, 40).map((c) => ({ value: c.size, item: c })), bounds);
      for (const tile of laid) {
        if (out.length >= MAX_TILES) return;
        const path = join(base, tile.item.name);
        const color = colorFor(tile.item, inherited);
        const inner = tile.item.children?.filter((c) => c.size > 0) ?? [];
        const nest = inner.length > 1 && tile.w > 118 && tile.h > 84 && depth < 2;
        out.push({ ...tile, key: path, node: tile.item, path, depth, color, share: tile.item.size / total, nest });
        // Containers keep a header strip that drills into the folder itself.
        if (nest) {
          place(inner, { x: tile.x + 3, y: tile.y + 21, w: tile.w - 6, h: tile.h - 24 }, path, depth + 1,
            tile.item.category === "other" ? inherited : tile.item.category);
        }
      }
    };
    place(node.children, { x: 0, y: 0, w: width, h: height }, current.path, 0, "other");
    return out;
  }, [node, width, height, current.path]);

  const legend = useMemo(() => {
    const present = new Set<Category>();
    node?.children?.forEach((c) => {
      if (!c.aggregate) present.add(c.category);
      c.children?.forEach((g) => !g.aggregate && present.add(g.category));
    });
    return [...present];
  }, [node]);

  const activate = (tile: MapTile) => {
    if (tile.node.aggregate) return;
    if (tile.node.kind === "folder" && (tile.node.children?.length ?? 0) > 0) {
      setTrail((t) => [...t, { path: tile.path, label: shortName(tile.node.name) }]);
    } else {
      onSelect({ path: tile.path, node: tile.node });
    }
  };

  return (
    <div>
      <div className="mb-3 flex flex-wrap items-center justify-between gap-3">
        <nav aria-label="Map location" className="flex min-w-0 items-center gap-1 text-[13px]">
          {trail.map((step, i) => (
            <span key={step.path ?? "root"} className="flex min-w-0 items-center gap-1">
              {i > 0 && <ChevronRight size={13} className="shrink-0 text-faint" />}
              <button type="button" onClick={() => setTrail((t) => t.slice(0, i + 1))}
                className={i === trail.length - 1 ? "truncate text-ink" : "truncate text-soft hover:text-accent"}>
                {i === 0 ? <span className="inline-flex items-center gap-1.5"><Home size={13} />{step.label}</span> : step.label}
              </button>
            </span>
          ))}
          {node && <span className="mono ml-2 shrink-0 text-xs text-faint">{bytes(node.size)}</span>}
        </nav>
        <Segmented layoutId="map-mode" value={mode} onChange={setMode}
          options={[{ value: "flat", label: "Map", icon: Grid2x2 }, { value: "holo", label: "Holographic", icon: Box }]} />
      </div>

      <div ref={stage} className="holo-stage relative overflow-hidden rounded-xl" style={{ height }} onMouseLeave={() => setTooltip(null)}>
        {loading && <Skeleton className="absolute inset-0" />}
        {!loading && tiles.length === 0 && (
          <div className="grid h-full place-items-center text-soft">This location has no items large enough to map.</div>
        )}
        <div className="holo-plane absolute inset-0" data-holo={mode === "holo"}>
          {tiles.map((tile, i) => (
            <motion.button
              type="button"
              key={tile.key}
              className="tile text-left"
              aria-label={`${tile.node.name}, ${bytes(tile.node.size)}`}
              initial={{ opacity: 0, scale: 0.94 }}
              animate={{ opacity: 1, scale: 1 }}
              transition={{ duration: 0.32, delay: Math.min(i * 0.006, 0.3) }}
              data-container={tile.nest}
              style={{
                left: tile.x + 1, top: tile.y + 1, width: Math.max(tile.w - 2, 1), height: Math.max(tile.h - 2, 1),
                zIndex: tile.depth + 1,
                ["--lift" as string]: `${(mode === "holo" ? 5 + Math.sqrt(tile.share) * 70 : 0) + tile.depth * 10}px`,
                ["--tile" as string]: tile.color,
              }}
              onClick={() => activate(tile)}
              onContextMenu={(e) => {
                e.preventDefault();
                if (!tile.node.aggregate) onContext({ path: tile.path, node: tile.node }, e.clientX, e.clientY);
              }}
              onMouseMove={(e) => {
                const box = stage.current!.getBoundingClientRect();
                setTooltip({ x: e.clientX - box.left, y: e.clientY - box.top, node: tile.node, color: tile.color });
              }}
            >
              {tile.w > 52 && tile.h > 20 && <span className="tile-name">{shortName(tile.node.name)}</span>}
              {tile.w > 62 && tile.h > 40 && !tile.nest && <span className="tile-size">{bytes(tile.node.size)}</span>}
            </motion.button>
          ))}
        </div>
        {tooltip && (
          <div className="pointer-events-none absolute z-20 max-w-64 rounded-lg border border-[var(--line-strong)] bg-[var(--panel-solid)]/95 px-3 py-2 shadow-2xl"
            style={{ left: Math.min(tooltip.x + 14, width - 260), top: Math.min(tooltip.y + 14, height - 76) }}>
            <div className="flex items-center gap-2">
              <span className="size-2 shrink-0 rounded-sm" style={{ background: tooltip.color }} />
              <span className="truncate text-[13px] text-ink">{shortName(tooltip.node.name)}</span>
            </div>
            <div className="mono mt-1 text-xs text-soft">
              {bytes(tooltip.node.size)}
              {tooltip.node.files > 1 && ` · ${count(tooltip.node.files)} files`}
            </div>
          </div>
        )}
      </div>

      <div className="mt-3 flex flex-wrap gap-x-4 gap-y-1">
        {legend.map((c) => (
          <span key={c} className="inline-flex items-center gap-1.5 text-xs text-soft">
            <span className="size-2.5 rounded-sm" style={{ background: CATEGORY_COLOR[c] }} />
            {CATEGORY_LABEL[c]}
          </span>
        ))}
        <span className="inline-flex items-center gap-1.5 text-xs text-soft">
          <span className="size-2.5 rounded-sm" style={{ background: "#39425a" }} />
          Smaller items
        </span>
      </div>
    </div>
  );
}
