/** Per-student Show loss badge: pure helpers (no Tauri) so they can be tested in node. */
export type LossLevel = "success" | "warning" | "danger";

/** Percent of frames lost in a stats window; null when there was nothing to judge. */
export function lossPct(recv: number, lost: number): number | null {
  const n = recv + lost;
  return n > 0 ? Math.round((lost * 100) / n) : null;
}

/** green < 2%, amber < 10%, red otherwise. */
export function lossLevel(pct: number): LossLevel {
  return pct < 2 ? "success" : pct < 10 ? "warning" : "danger";
}
