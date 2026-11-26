import { create } from "zustand";
import type { SortDir, SortKey } from "./sort";

export type Req = "scan" | "design" | "board" | "whiteboard" | "journal" | "files" | "collect" | "remote" | "exhibit";
export type View = "thumbs" | "icons" | "details";
type Prefs = {
  size: number; autoFit: boolean; showUser: boolean; showGroup: boolean; detail: boolean;
  view: View; sortKey: SortKey; sortDir: SortDir;
  /** true: wall follows the sort; false: saved/manual seating (layout.ts) */
  autoArrange: boolean; bgCenter: boolean; lessonOpen: boolean;
};

// "2": the default name changed to the computer name, so prefs saved by the old layout are not reused
const KEY = "ui-prefs2";
const DEF: Prefs = {
  size: 140, autoFit: false, showUser: false, showGroup: true, detail: true,
  view: "thumbs", sortKey: "name", sortDir: "asc", autoArrange: false, bgCenter: false, lessonOpen: false,
};
const load = (): Prefs => { try { return { ...DEF, ...JSON.parse(localStorage.getItem(KEY) ?? "{}") }; } catch { return DEF; } };

/** View preferences (remembered) plus a one-shot "open this dialog" request that App's nav rail / tiles send to ExtraBar. */
export const useUi = create<Prefs & { req: { k: Req; n: number } | null; /** lesson title for the window title row (LessonPanel keeps it current; not saved here) */ lesson: string; set: (p: Partial<Prefs>) => void; request: (k: Req) => void }>((set, get) => ({
  ...load(),
  req: null,
  lesson: "",
  set: (p) => {
    set(p);
    const g = get();
    const out = Object.fromEntries(Object.keys(DEF).map((k) => [k, g[k as keyof Prefs]]));
    try { localStorage.setItem(KEY, JSON.stringify(out)); } catch { /* not remembered */ }
  },
  request: (k) => set({ req: { k, n: Date.now() + Math.random() } }),
}));
