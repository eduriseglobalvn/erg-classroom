/** Pure helpers for the journal dialog (no React/Tauri, so node can test them). */
export type JEntry = { ts: number; kind: string; who: string; detail: string };

/** Newest first; keeps entries where every space-separated word appears in time/who/kind/detail (case-insensitive). */
export function filterEntries(entries: JEntry[], q: string, kinds?: ReadonlySet<string>): JEntry[] {
  const words = q.toLowerCase().split(/\s+/).filter(Boolean);
  return entries
    .filter((e) => !kinds?.size || kinds.has(e.kind))
    .filter((e) => {
      const hay = `${clockOf(e.ts)} ${e.who} ${e.kind} ${e.detail}`.toLowerCase();
      return words.every((w) => hay.includes(w));
    })
    .sort((a, b) => b.ts - a.ts);
}

/** HH:MM:SS in the teacher's local time. */
export function clockOf(ts: number): string {
  const d = new Date(ts);
  return [d.getHours(), d.getMinutes(), d.getSeconds()].map((n) => String(n).padStart(2, "0")).join(":");
}

/** True when `ts` falls on the same local calendar day as `now`. */
export const isToday = (ts: number, now = Date.now()) => new Date(ts).toDateString() === new Date(now).toDateString();
