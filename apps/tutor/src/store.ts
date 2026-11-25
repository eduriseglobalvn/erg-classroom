import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { lossPct } from "./showstats";

export interface Inventory {
  os: string; cpu: string; cores: number; ram_total: number; ram_used: number;
  disks: [string, number, number][]; uptime_s: number; processes: string[];
}

export interface Student {
  id: string;
  hostname: string;
  user: string;
  room: string;
  online: boolean;
  /** App version of the student ("0.1" = before 0.2.0); outdated = older than this Tutor; legacy = disconnects on commands it does not know. */
  version?: string;
  outdated?: boolean;
  legacy?: boolean;
  locked?: boolean;
  help?: boolean;
  /** % of Show frames lost in the last stats window; only meaningful while a Show is active */
  showLoss?: number;
  /** data: URL of the latest screen thumbnail */
  thumb?: string;
  inventory?: Inventory;
}

interface State {
  students: Record<string, Student>;
  selected: Set<string>;
  toggle: (id: string, additive: boolean) => void;
  selectAll: () => void;
  clear: () => void;
  showing: boolean;
  startShow: (window?: boolean) => Promise<void>;
  stopShow: () => Promise<void>;
  /** Ask for a file and send it; resolves to a Vietnamese status line (or null if cancelled). */
  sendFile: () => Promise<string | null>;
  send: (cmd: unknown) => Promise<number>;
  /** Room the Tutor is connected to ("" = every room). */
  room: string;
  /** Waiting for the first students after starting or (re)connecting: the wall shows placeholders. */
  loading: boolean;
  /** Connect to a room: everyone is dropped and that room's students come back. Same room = refresh the list. */
  connect: (room: string) => Promise<void>;
  init: () => Promise<() => void>;
}

const clearLoss = (m: Record<string, Student>) => Object.fromEntries(Object.entries(m).map(([k, v]) => [k, { ...v, showLoss: undefined }]));

export const useClass = create<State>((set, get) => ({
  students: {},
  selected: new Set(),
  showing: false,
  startShow: async (window?: boolean) => {
    const ids = [...get().selected];
    await invoke<number>("start_show", { ids: ids.length ? ids : null, window: window ?? false });
    set((s) => ({ showing: true, students: clearLoss(s.students) }));
  },
  stopShow: async () => {
    await invoke("stop_show");
    set((s) => ({ showing: false, students: clearLoss(s.students) }));
  },
  sendFile: async () => {
    const path = await open({ multiple: false, title: "Choose a file to send to students" });
    if (!path || Array.isArray(path)) return null;
    const ids = [...get().selected];
    const [ok, failed] = await invoke<[number, number]>("send_file", { ids: ids.length ? ids : null, path });
    return failed ? `Sent to ${ok} PC(s), ${failed} failed` : `Sent to ${ok} PC(s)`;
  },
  toggle: (id, additive) =>
    set((s) => {
      const n = new Set(additive ? s.selected : []);
      n.has(id) ? n.delete(id) : n.add(id);
      return { selected: n };
    }),
  selectAll: () => set((s) => ({ selected: new Set(Object.keys(s.students)) })),
  clear: () => set({ selected: new Set() }),
  // No selection means the whole class, matching the command-bar labels.
  send: (cmd) => {
    const ids = [...get().selected];
    return invoke<number>("send_command", { ids: ids.length ? ids : null, cmd });
  },
  room: "",
  loading: true,
  connect: async (room) => {
    await invoke("set_room", { room });
    set({ room: room.trim(), students: {}, selected: new Set(), showing: false, loading: true });
    setTimeout(() => set({ loading: false }), 6000); // an empty room stops looking busy
  },
  init: async () => {
    invoke<string | null>("get_room").then((r) => set({ room: r ?? "" }), () => { /* browser dev */ });
    // Batch updates every 200 ms so a 50-tile grid does not re-render per beacon.
    let pending: Record<string, Partial<Student>> = {};
    const flush = () => {
      if (!Object.keys(pending).length) return;
      const p = pending;
      pending = {};
      set((s) => {
        const next = { ...s.students };
        for (const [id, patch] of Object.entries(p))
          // an unknown id without a name is a leftover of a list cleared by `connect`: do not bring it back
          if (next[id] || patch.hostname) next[id] ={ ...(next[id] ?? { id, hostname: id, user: "", room: "", online: false }), ...patch } as Student;
        return { students: next, loading: false };
      });
    };
    const timer = setInterval(flush, 200);
    const un = [
      await listen<Student>("student-online", (e) => (pending[e.payload.id] = { ...e.payload, online: true })),
      await listen<{ id: string }>("student-offline", (e) => (pending[e.payload.id] = { ...pending[e.payload.id], online: false })),
      await listen<{ id: string }>("student-help", (e) => (pending[e.payload.id] = { ...pending[e.payload.id], help: true })),
      await listen<{ id: string; jpeg: string }>("student-thumb", (e) => (pending[e.payload.id] = { ...pending[e.payload.id], thumb: "data:image/jpeg;base64," + e.payload.jpeg })),
      await listen<{ id: string; recv: number; lost: number; fecFixed: number }>("student-show-stats", (e) => {
        const p = lossPct(e.payload.recv, e.payload.lost);
        if (p !== null) pending[e.payload.id] = { ...pending[e.payload.id], showLoss: p };
      }),
      await listen<{ id: string; inv: Inventory }>("student-inventory", (e) => (pending[e.payload.id] = { ...pending[e.payload.id], inventory: e.payload.inv })),
    ];
    await invoke("start_roster");
    setTimeout(() => set({ loading: false }), 6000);
    return () => {
      clearInterval(timer);
      un.forEach((f) => f());
    };
  },
}));
