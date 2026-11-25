import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

/** Seating order (by host name) and groups. Saved in the "layout" document, loaded at start. */
interface LayoutState {
  order: string[];
  groups: Record<string, string>;
  /** group name -> host of its leader */
  leaders: Record<string, string>;
  /** group name -> team points */
  scores: Record<string, number>;
  filter: string | null;
  loaded: boolean;
  load: () => Promise<void>;
  /** Put `from` where `to` is (swap) and remember it. Unknown hosts are appended first. */
  swap: (from: string, to: string, all: string[]) => void;
  setFilter: (g: string | null) => void;
  assign: (hosts: string[], group: string) => void;
  /** Split `hosts` (in current seating order) into `n` balanced groups. */
  autoGroup: (hosts: string[], n: number) => void;
  clearGroups: () => void;
  setLeader: (host: string) => boolean;
  addScore: (group: string, delta: number) => void;
  resetScores: () => void;
}

const save = (s: Pick<LayoutState, "order" | "groups" | "leaders" | "scores">) =>
  void invoke("save_doc", { name: "layout", json: JSON.stringify({ order: s.order, groups: s.groups, leaders: s.leaders, scores: s.scores }) }).catch(() => {});
const persist = (get: () => LayoutState) => save(get());

/** Teams by points, best first (ties by name). */
export function ranking(scores: Record<string, number>): [string, number][] {
  return Object.entries(scores).sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
}

/** Pure helpers, exported for the test script. */
export function ordered<T extends { hostname: string }>(list: T[], order: string[]): T[] {
  const pos = new Map(order.map((h, i) => [h, i]));
  return [...list].sort((a, b) => {
    const pa = pos.get(a.hostname), pb = pos.get(b.hostname);
    if (pa !== undefined && pb !== undefined) return pa - pb;
    if (pa !== undefined) return -1;
    if (pb !== undefined) return 1;
    return a.hostname.localeCompare(b.hostname, undefined, { numeric: true });
  });
}

export function swapOrder(order: string[], all: string[], from: string, to: string): string[] {
  if (from === to) return order;
  // every currently known host gets a position so a swap is well defined
  const full = [...order.filter((h) => all.includes(h)), ...all.filter((h) => !order.includes(h))];
  const i = full.indexOf(from), j = full.indexOf(to);
  if (i < 0 || j < 0) return order;
  [full[i], full[j]] = [full[j], full[i]];
  return full;
}

export function balancedGroups(hosts: string[], n: number): Record<string, string> {
  const k = Math.max(1, Math.min(n, hosts.length || 1));
  return Object.fromEntries(hosts.map((h, i) => [h, `Group ${(i % k) + 1}`]));
}

export const useLayout = create<LayoutState>((set, get) => ({
  order: [],
  groups: {},
  leaders: {},
  scores: {},
  filter: null,
  loaded: false,
  load: async () => {
    try {
      const raw = await invoke<string | null>("load_doc", { name: "layout" });
      if (raw) {
        const d = JSON.parse(raw);
        set({ order: Array.isArray(d.order) ? d.order.filter((x: unknown) => typeof x === "string") : [], groups: d.groups && typeof d.groups === "object" ? d.groups : {}, leaders: d.leaders && typeof d.leaders === "object" ? d.leaders : {}, scores: d.scores && typeof d.scores === "object" ? d.scores : {} });
      }
    } catch { /* first run or damaged file: start empty */ }
    set({ loaded: true });
  },
  swap: (from, to, all) => {
    const order = swapOrder(get().order, all, from, to);
    set({ order });
    persist(get);
  },
  setFilter: (filter) => set({ filter }),
  assign: (hosts, group) => {
    const groups = { ...get().groups };
    hosts.forEach((h) => (groups[h] = group));
    set({ groups });
    persist(get);
  },
  autoGroup: (hosts, n) => {
    const groups = balancedGroups(hosts, n);
    set({ groups, leaders: {}, filter: null });
    persist(get);
  },
  clearGroups: () => {
    set({ groups: {}, leaders: {}, filter: null });
    persist(get);
  },
  setLeader: (host) => {
    const g = get().groups[host];
    if (!g) return false;
    set({ leaders: { ...get().leaders, [g]: host } });
    persist(get);
    return true;
  },
  addScore: (group, delta) => {
    set({ scores: { ...get().scores, [group]: (get().scores[group] ?? 0) + delta } });
    persist(get);
  },
  resetScores: () => {
    set({ scores: {} });
    persist(get);
  },
}));
