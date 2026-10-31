export interface Rect { x: number; y: number; w: number; h: number }
export interface Tile<T> extends Rect { item: T }

interface Weighted<T> { area: number; item: T }

function worstRatio(row: Weighted<unknown>[], side: number): number {
  const sum = row.reduce((total, r) => total + r.area, 0);
  const max = Math.max(...row.map((r) => r.area));
  const min = Math.min(...row.map((r) => r.area));
  const s2 = sum * sum;
  const w2 = side * side;
  return Math.max((w2 * max) / s2, s2 / (w2 * min));
}

/** Squarified treemap (Bruls, Huizing, van Wijk): keeps tiles close to square. */
export function squarify<T>(items: { value: number; item: T }[], bounds: Rect): Tile<T>[] {
  const total = items.reduce((sum, i) => sum + Math.max(i.value, 0), 0);
  const area = bounds.w * bounds.h;
  if (total <= 0 || area <= 0) return [];
  const scale = area / total;
  const queue: Weighted<T>[] = items
    .filter((i) => i.value > 0)
    .sort((a, b) => b.value - a.value)
    .map((i) => ({ area: i.value * scale, item: i.item }));

  const out: Tile<T>[] = [];
  const free: Rect = { ...bounds };
  let row: Weighted<T>[] = [];

  const flush = () => {
    if (row.length === 0) return;
    const sum = row.reduce((total, r) => total + r.area, 0);
    if (free.w >= free.h) {
      const width = free.h > 0 ? sum / free.h : 0;
      let y = free.y;
      for (const r of row) {
        const h = width > 0 ? r.area / width : 0;
        out.push({ x: free.x, y, w: width, h, item: r.item });
        y += h;
      }
      free.x += width;
      free.w -= width;
    } else {
      const height = free.w > 0 ? sum / free.w : 0;
      let x = free.x;
      for (const r of row) {
        const w = height > 0 ? r.area / height : 0;
        out.push({ x, y: free.y, w, h: height, item: r.item });
        x += w;
      }
      free.y += height;
      free.h -= height;
    }
    row = [];
  };

  for (const next of queue) {
    const side = Math.min(free.w, free.h);
    if (row.length === 0 || worstRatio(row, side) >= worstRatio([...row, next], side)) {
      row.push(next);
    } else {
      flush();
      row.push(next);
    }
  }
  flush();
  return out;
}
