import { useEffect, useRef, useState } from "react";
import { Button, Checkbox, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Input, Skeleton, SkeletonItem, tokens } from "@fluentui/react-components";
import { ArrowDown16Regular, ArrowUp16Regular, ArrowSync16Regular, Dismiss16Regular, Document16Regular, Folder16Filled, Home16Regular, LockClosed16Regular } from "@fluentui/react-icons";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useClass } from "./store";

type Entry = { name: string; is_dir: boolean; size: number; modified_s: number };

// ---- v2 dialog look, shared by every Tutor dialog: flush 40px header, body, grey footer with right-aligned buttons
const line = `1px solid ${tokens.colorNeutralStroke2}`;
export const mut = tokens.colorNeutralForeground3;
export const D = {
  surface: { padding: 0, borderRadius: 8, overflow: "hidden", border: `1px solid ${tokens.colorNeutralStroke1}` },
  body: { gap: 0 },
  head: { display: "flex", alignItems: "center", gap: 8, height: 40, boxSizing: "border-box", padding: "0 6px 0 16px", fontSize: 14, lineHeight: "20px", fontWeight: 600 },
  content: { padding: "4px 16px 16px", display: "flex", flexDirection: "column", gap: 12 },
  foot: { gridColumn: "1 / 4", justifySelf: "stretch", justifyContent: "flex-end", alignItems: "center", padding: "12px 16px", background: tokens.colorNeutralBackground2, borderTop: line },
  box: { border: line, borderRadius: 4, overflow: "auto", background: tokens.colorNeutralBackground1 },
} satisfies Record<string, React.CSSProperties>;
export const th: React.CSSProperties = { textAlign: "left", fontWeight: 400, fontSize: 12, color: mut, height: 28, padding: "0 12px", position: "sticky", top: 0, zIndex: 1, background: tokens.colorNeutralBackground2, borderBottom: line };
export const td: React.CSSProperties = { height: 28, padding: "0 12px", fontSize: 13, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis", borderBottom: `1px solid ${tokens.colorNeutralBackground4}` };
export const GREEN = tokens.colorPaletteGreenForeground1, RED = tokens.colorPaletteRedForeground1, BLUE = tokens.colorBrandBackground, GREY = tokens.colorNeutralForeground4;

/** Dialog header: title, optional grey subtitle, close button on the right. */
export function DlgHead({ title, sub, onClose, off }: { title: React.ReactNode; sub?: React.ReactNode; onClose: () => void; off?: boolean }) {
  return (
    <DialogTitle style={D.head}>
      <span>{title}</span>
      {sub && <span style={{ fontWeight: 400, color: mut, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{sub}</span>}
      <span style={{ flex: 1 }} />
      <Button appearance="subtle" size="small" aria-label="Close" icon={<Dismiss16Regular />} disabled={off} onClick={onClose} />
    </DialogTitle>
  );
}
export const Bar = ({ v, color, h = 8, label }: { v: number; color: string; h?: number; label?: string }) => (
  <div role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(v * 100)} style={{ height: h, borderRadius: h / 2, background: tokens.colorNeutralBackground5 }}>
    <div style={{ width: `${v * 100}%`, height: h, borderRadius: h / 2, background: color, transition: "width .2s" }} />
  </div>
);
/** Coloured dot + text, the status cell of the student tables. */
export const Status = ({ color, children }: { color: string; children: React.ReactNode }) => (
  <span style={{ display: "inline-flex", alignItems: "center", gap: 7, maxWidth: "100%" }}>
    <span style={{ width: 8, height: 8, borderRadius: 4, flex: "none", background: color }} />
    <span style={{ overflow: "hidden", textOverflow: "ellipsis" }}>{children}</span>
  </span>
);

const DEST = "Documents\\ERG\\Received"; // fixed by the student program
const card: React.CSSProperties = { display: "grid", gap: 6, padding: "10px 12px", borderRadius: 4, minWidth: 0 };
const cardRow: React.CSSProperties = { display: "flex", gap: 8, alignItems: "baseline", whiteSpace: "nowrap", overflow: "hidden" };
const tag = (bg: string, fg: string): React.CSSProperties => ({ display: "inline-flex", alignItems: "center", height: 20, padding: "0 8px", borderRadius: 10, fontSize: 12, background: bg, color: fg });
type St = { done: number; bad: number; cur: string };

/** File Transfer: folders and files of this computer on top, the receiving students below; Copy sends the ticked files to all of them. */
export function FileDist({ onClose, say, fail }: { onClose: () => void; say: (s: string) => void; fail: (s: string) => void }) {
  const { students, selected } = useClass();
  const [whole, setWhole] = useState(false);
  const to = Object.values(students).filter((s) => s.online && (whole || selected.size === 0 || selected.has(s.id)));
  const [path, setPath] = useState("");
  const [home, setHome] = useState("");
  const [filter, setFilter] = useState("");
  const [typed, setTyped] = useState("");
  const [rows, setRows] = useState<Entry[] | null>(null);
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState("");
  // the last (or running) copy: its files, its computers and how far each one is
  const [job, setJob] = useState<{ names: string[]; list: typeof to; dir: string; st: Record<string, St>; stopped: boolean } | null>(null);
  const stop = useRef(false); // Stop finishes the file being copied, then sends nothing more
  const [now, setNow] = useState("");
  const go = (p: string) => {
    setRows(null);
    invoke<[string, Entry[]]>("local_dir", { path: p }).then(([real, list]) => { if (!p) setHome(real); setPath(real); setTyped(real); setRows(list); setPicked(new Set()); },
      (e) => { fail(String(e)); setRows([]); });
  };
  useEffect(() => go(""), []); // eslint-disable-line react-hooks/exhaustive-deps
  // free space comes with the inventory, asked for once
  useEffect(() => { if (to.length) void invoke("send_command", { ids: to.map((s) => s.id), cmd: "RequestInventory" }).catch(() => { /* column stays empty */ }); }, []); // eslint-disable-line react-hooks/exhaustive-deps
  // right pane: one student's folder (click a student row below); asked only once the answers can be heard
  const [ask, setAsk] = useState<{ id: string; path: string } | null>(null);
  const [far, setFar] = useState<{ rows: Entry[] | null; msg: string }>({ rows: null, msg: "" });
  useEffect(() => {
    if (!ask) return;
    setFar({ rows: null, msg: "" });
    const a = listen<{ id: string; path: string; entries: Entry[] }>("student-dir", (e) => { if (e.payload.id === ask.id && e.payload.path === ask.path) setFar({ rows: e.payload.entries, msg: "" }); });
    const b = listen<{ id: string; ok: boolean; message: string }>("student-fs", (e) => { if (e.payload.id === ask.id) e.payload.ok ? setAsk({ ...ask }) : setFar({ rows: [], msg: e.payload.message }); }); // ok: list again
    void Promise.all([a, b]).then(() => invoke("send_command", { ids: [ask.id], cmd: { ListDir: ask.path } })).catch((e) => setFar({ rows: [], msg: String(e) }));
    return () => { void a.then((f) => f()); void b.then((f) => f()); };
  }, [ask]);
  // shortcut: guesses the usual profile folder; a moved Documents (OneDrive) shows the error, then Up works
  const browse = (s: (typeof to)[number]) => setAsk({ id: s.id, path: `C:\\Users\\${s.user.split("\\").pop() || ""}\\${DEST}` });
  const farUp = () => { if (!ask) return; const q = ask.path.replace(/\\$/, ""); const i = q.lastIndexOf("\\"); if (i > 1) setAsk({ ...ask, path: i === 2 ? q.slice(0, 3) : q.slice(0, i) }); };
  const peek = ask && students[ask.id];
  const farDo = (cmd: unknown) => ask && void invoke("send_command", { ids: [ask.id], cmd }).catch((e) => fail(String(e)));
  const farAt = (n: string) => (ask?.path ?? "").replace(/\\$/, "") + "\\" + n;
  const freeOf = (id: string) => { const d = students[id]?.inventory?.disks; const c = d?.find((x) => /^c:/i.test(x[0])) ?? d?.[0]; return c ? `${(c[2] / 2 ** 30).toFixed(c[2] < 10 * 2 ** 30 ? 1 : 0)} GB` : "–"; };
  const join = (n: string) => path.replace(/\\$/, "") + "\\" + n;
  const up = () => { const p = path.replace(/\\$/, ""); const i = p.lastIndexOf("\\"); if (i > 1) go(i === 2 ? p.slice(0, 3) : p.slice(0, i)); };
  const tick = (n: string) => setPicked((s) => { const x = new Set(s); x.has(n) ? x.delete(n) : x.add(n); return x; });
  // "*.docx" style patterns, or plain text anywhere in the name
  const f = filter.trim().toLowerCase();
  const match = (n: string) => !f || (f.includes("*") ? new RegExp(`^${f.split("*").map((x) => x.replace(/[.+?^${}()|[\]\\]/g, "\\$&")).join(".*")}$`).test(n.toLowerCase()) : n.toLowerCase().includes(f));
  const shown = rows?.filter((r) => r.is_dir || match(r.name)) ?? null;
  const files = shown?.filter((r) => !r.is_dir) ?? [];
  const bytes = files.filter((f) => picked.has(f.name)).reduce((a, f) => a + f.size, 0);
  // folder pane: the folders above the open one, then the folders inside it (a network path is shown as one entry)
  const parts = path.replace(/\\$/, "").split("\\");
  const crumbs = !path ? [] : path.startsWith("\\\\") ? [{ name: path, full: path }] : parts.map((p, i) => ({ name: i ? p : p + "\\", full: i ? parts.slice(0, i + 1).join("\\") : p + "\\" }));
  const copy = async (names: string[], list: typeof to, dir: string) => {
    const queue = [...list];
    stop.current = false;
    const st: Record<string, St> = Object.fromEntries(list.map((s) => [s.id, { done: 0, bad: 0, cur: "" }]));
    const show = () => setJob({ names, list, dir, st: { ...st }, stopped: stop.current });
    setBusy(true); setDone(""); show();
    let ok = 0, bad = 0;
    // one send_file per computer, so every row shows its own result; 10 at a time, as the backend does for a whole-class send
    const worker = async () => {
      for (let s = queue.shift(); s && !stop.current; s = queue.shift()) {
        const r = st[s.id];
        setNow(s.id);
        for (const n of names) {
          if (stop.current) break;
          r.cur = n; show();
          try {
            const [a] = await invoke<[number, number]>("send_file", { ids: [s.id], path: dir + "\\" + n });
            a ? r.done++ : r.bad++;
          } catch (e) { r.bad++; fail(String(e)); }
        }
        r.cur = ""; ok += r.done; bad += r.bad; show();
      }
    };
    await Promise.all(Array.from({ length: 10 }, worker));
    setBusy(false); show();
    const text = stop.current ? `Stopped: ${ok} copies delivered, ${bad} failed` : bad ? `${names.length} file(s): ${ok} copies delivered, ${bad} failed` : `${names.length} file(s) copied to ${list.length} PC(s)`;
    setDone(text); say(text);
  };
  const copyNow = () => void copy([...picked], to, path.replace(/\\$/, ""));
  const size = (n: number) => n >= 1 << 20 ? `${(n / (1 << 20)).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`;
  const list = job?.list ?? to;
  const total = job?.names.length ?? 0;
  const sts = job ? Object.values(job.st) : [];
  const fin = sts.filter((r) => r.done + r.bad === total), failed = fin.filter((r) => r.bad).length;
  const cur = job?.st[now], curPc = job?.list.find((s) => s.id === now);
  const retry = job && !busy ? job.list.filter((s) => job.st[s.id].bad) : [];
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !busy && onClose()}>
      <DialogSurface style={{ ...D.surface, maxWidth: "none", width: "min(1300px, 94vw)" }}>
        <DialogBody style={D.body}>
          <DlgHead title="File Transfer" sub={`This computer → ${to.length} student${to.length === 1 ? "" : "s"}${selected.size ? "" : " (whole class)"}`} onClose={onClose} off={busy} />
          <DialogContent style={{ padding: 0, display: "grid", gridTemplateRows: "auto minmax(0, 1fr) auto", height: "calc(100vh - 170px)", overflow: "hidden", borderTop: line }}>
            <div role="toolbar" aria-label="File commands" style={{ display: "flex", alignItems: "center", gap: 4, height: 44, padding: "0 12px", borderBottom: line, background: tokens.colorNeutralBackground2 }}>
              <Button appearance="primary" size="small" icon={<ArrowDown16Regular />} iconPosition="after" disabled={!picked.size || !to.length || busy} onClick={copyNow}>Copy to students</Button>
              <span style={{ width: 1, height: 20, margin: "0 4px", background: "#E0E0E0" }} />
              <Button appearance="subtle" size="small" disabled={!files.length} onClick={() => setPicked(new Set(files.map((x) => x.name)))}>Select all</Button>
              <Button appearance="subtle" size="small" onClick={() => go(path)}>Refresh</Button>
              <span style={{ flex: 1 }} />
              <Input size="small" aria-label="Filter files" placeholder="Filter, e.g. *.docx" style={{ width: 160 }} value={filter} onChange={(_, d) => { setFilter(d.value); setPicked(new Set()); }} />
            </div>
            <div style={{ display: "grid", gridTemplateColumns: "220px minmax(0, 1fr) 400px", minHeight: 0 }}>
              <nav aria-label="Folders on this computer" style={{ display: "flex", flexDirection: "column", padding: "8px 6px", overflow: "auto", borderRight: line, background: tokens.colorNeutralBackground2 }}>
                {home && ([["Home", home], ["Desktop", home + "\\Desktop"], ["Downloads", home + "\\Downloads"], ["Documents", home + "\\Documents"]] as const).map(([n, p], i) => (
                  <Button key={n} appearance="subtle" size="small" icon={i ? <Folder16Filled style={{ color: "#B7791F" }} /> : <Home16Regular />} onClick={() => go(p)}
                    style={{ flex: "none", justifyContent: "flex-start", minHeight: 26, paddingLeft: 6, fontWeight: 400 }}>{n}</Button>
                ))}
                {home && <div style={{ height: 1, flex: "none", margin: "6px 4px", background: "#E0E0E0" }} />}
                {crumbs.map((c, i) => {
                  const here = i === crumbs.length - 1;
                  return (
                    <Button key={c.full} appearance="subtle" size="small" icon={<Folder16Filled style={{ color: "#B7791F" }} />} aria-current={here || undefined} onClick={() => go(c.full)}
                      style={{ flex: "none", justifyContent: "flex-start", minHeight: 26, paddingLeft: 6 + i * 12, fontWeight: here ? 600 : 400, background: here ? tokens.colorSubtleBackgroundSelected : undefined, boxShadow: here ? `inset 3px 0 0 ${BLUE}` : undefined }}>
                      <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{c.name}</span>
                    </Button>
                  );
                })}
                {rows?.filter((r) => r.is_dir).map((r) => (
                  <Button key={r.name} appearance="subtle" size="small" icon={<Folder16Filled style={{ color: "#B7791F" }} />} onClick={() => go(join(r.name))}
                    style={{ flex: "none", justifyContent: "flex-start", minHeight: 26, paddingLeft: 6 + crumbs.length * 12, fontWeight: 400 }}>
                    <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{r.name}</span>
                  </Button>
                ))}
              </nav>
              <section aria-label="This computer" style={{ display: "flex", flexDirection: "column", minWidth: 0, minHeight: 0 }}>
                <div style={{ display: "flex", gap: 6, alignItems: "center", height: 40, flex: "none", padding: "0 10px" }}>
                  <Button appearance="subtle" size="small" icon={<ArrowUp16Regular />} aria-label="Up one folder" onClick={up} />
                  <span style={{ fontWeight: 600, whiteSpace: "nowrap" }}>This computer</span>
                  <Input size="small" style={{ flex: 1, minWidth: 0 }} aria-label="Folder on this computer" value={typed} onChange={(_, d) => setTyped(d.value)} onKeyDown={(e) => e.key === "Enter" && go(typed)} />
                  <Button appearance="subtle" size="small" icon={<ArrowSync16Regular />} aria-label="Refresh" onClick={() => go(path)} />
                </div>
                <div style={{ flex: 1, minHeight: 0, overflow: "auto", borderTop: line }}>
                  <table style={{ width: "100%", borderCollapse: "collapse", tableLayout: "fixed" }}>
                    <thead><tr>
                      <th style={{ ...th, width: 44 }}><Checkbox aria-label="All files" checked={files.length > 0 && picked.size === files.length ? true : picked.size ? "mixed" : false}
                        onChange={(_, d) => setPicked(d.checked ? new Set(files.map((f) => f.name)) : new Set())} /></th>
                      <th style={th}>Name</th><th style={{ ...th, width: 180 }}>Modified</th><th style={{ ...th, width: 100, textAlign: "right" }}>Size</th>
                    </tr></thead>
                    <tbody>
                      {rows === null && Array.from({ length: 8 }, (_, i) => <tr key={i}><td colSpan={4} style={td}><Skeleton><SkeletonItem size={16} /></Skeleton></td></tr>)}
                      {shown?.map((r) => (
                        <tr key={r.name} style={{ cursor: "default", background: picked.has(r.name) ? tokens.colorBrandBackground2 : undefined }}
                          onClick={() => !r.is_dir && tick(r.name)} onDoubleClick={() => r.is_dir && go(join(r.name))}>
                          <td style={td}>{!r.is_dir && <Checkbox aria-label={r.name} checked={picked.has(r.name)} onChange={() => tick(r.name)} onClick={(e) => e.stopPropagation()} />}</td>
                          <td style={td} title={r.name}>
                            <span style={{ display: "inline-flex", alignItems: "center", gap: 6, maxWidth: "100%" }}>
                              {r.is_dir ? <Folder16Filled style={{ color: "#B7791F", flex: "none" }} /> : <Document16Regular style={{ flex: "none", color: mut }} />}
                              {r.is_dir ? <a href="#" style={{ color: "inherit", textDecoration: "none" }} onClick={(e) => { e.preventDefault(); go(join(r.name)); }}>{r.name}</a> : r.name}
                            </span>
                          </td>
                          <td style={{ ...td, color: mut, fontSize: 12 }}>{r.modified_s ? new Date(r.modified_s * 1000).toLocaleString() : ""}</td>
                          <td style={{ ...td, color: mut, fontSize: 12, textAlign: "right" }}>{r.is_dir ? "" : size(r.size)}</td>
                        </tr>
                      ))}
                      {rows?.length === 0 && <tr><td colSpan={4} style={{ ...td, color: mut }}>This folder is empty.</td></tr>}
                    </tbody>
                  </table>
                </div>
                <div style={{ display: "flex", alignItems: "center", height: 26, flex: "none", padding: "0 12px", borderTop: line, fontSize: 12, color: mut }}>
                  {picked.size} of {files.length} files selected{picked.size ? ` · ${size(bytes)}` : ""}
                </div>
              </section>
              <section aria-label={peek ? peek.hostname : "Student computer"} style={{ display: "flex", flexDirection: "column", minWidth: 0, minHeight: 0, borderLeft: line }}>
                <div style={{ display: "flex", gap: 6, alignItems: "center", height: 40, flex: "none", padding: "0 10px" }}>
                  <Button appearance="subtle" size="small" icon={<ArrowUp16Regular />} aria-label="Up one folder" disabled={!ask} onClick={farUp} />
                  <span style={{ fontWeight: 600, whiteSpace: "nowrap", maxWidth: 150, overflow: "hidden", textOverflow: "ellipsis" }}>{peek ? peek.hostname : "Student computer"}</span>
                  <Input size="small" style={{ flex: 1, minWidth: 0 }} aria-label="Folder on the student computer" readOnly value={ask?.path ?? ""} />
                  <Button appearance="subtle" size="small" disabled={!ask} onClick={() => { const n = window.prompt("New folder name"); if (n?.trim()) farDo({ MakeDir: farAt(n.trim()) }); }}>New folder</Button>
                </div>
                <div style={{ flex: 1, minHeight: 0, overflow: "auto", borderTop: line }}>
                  {!ask && <div style={{ padding: 12, color: mut }}>Click a student below to see that computer's folder.</div>}
                  {ask && far.rows === null && <div style={{ padding: 12 }}><Skeleton><SkeletonItem size={16} /></Skeleton></div>}
                  {far.msg && <div style={{ padding: 12, color: RED }}>{far.msg}</div>}
                  {ask && far.rows?.map((r) => (
                    <div key={r.name} onDoubleClick={() => r.is_dir && setAsk({ ...ask, path: ask.path.replace(/\\$/, "") + "\\" + r.name })}
                      style={{ display: "flex", alignItems: "center", gap: 6, height: 28, padding: "0 12px", borderBottom: `1px solid ${tokens.colorNeutralBackground4}`, fontSize: 13 }}>
                      {r.is_dir ? <Folder16Filled style={{ color: "#B7791F", flex: "none" }} /> : <Document16Regular style={{ flex: "none", color: mut }} />}
                      <span style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }} title={r.name}>{r.name}</span>
                      <span style={{ width: 60, textAlign: "right", color: mut, fontSize: 12 }}>{r.is_dir ? "" : size(r.size)}</span>
                      <Button appearance="subtle" size="small" style={{ minWidth: 0 }} onClick={() => { const n = window.prompt("New name", r.name)?.trim(); if (n && n !== r.name) farDo({ RenamePath: { from: farAt(r.name), to: farAt(n) } }); }}>Rename</Button>
                      <Button appearance="subtle" size="small" style={{ minWidth: 0 }} onClick={() => window.confirm(`Delete ${r.name} on ${peek ? peek.hostname : "the student computer"}?`) && farDo({ DeletePath: farAt(r.name) })}>Delete</Button>
                    </div>
                  ))}
                </div>
                <div style={{ display: "flex", alignItems: "center", height: 26, flex: "none", padding: "0 12px", borderTop: line, fontSize: 12, color: mut }}>
                  {ask && far.rows ? `${far.rows.length} items · free space ${freeOf(ask.id)}` : ""}
                </div>
              </section>
            </div>
            <section aria-label="Copy to" style={{ display: "flex", flexDirection: "column", height: job ? 300 : 214, borderTop: `1px solid ${tokens.colorNeutralStroke1}` }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8, height: 38, flex: "none", padding: "0 12px", background: tokens.colorNeutralBackground2, borderBottom: line }}>
                <span style={{ fontWeight: 600 }}>Copy to</span><span style={{ color: mut }}>{list.length} student{list.length === 1 ? "" : "s"}</span>
                {selected.size > 0 && <Button size="small" appearance="subtle" aria-pressed={whole} disabled={busy} onClick={() => setWhole(!whole)} style={whole ? { background: "#E8EFFA" } : undefined}>Whole class</Button>}
                <span style={{ flex: 1 }} />
                <span title="Every student gets the files in this folder; it cannot be changed here." style={{ display: "inline-flex", alignItems: "center", gap: 6, fontSize: 12, color: mut }}>
                  Folder for all <LockClosed16Regular /><span style={{ color: tokens.colorNeutralForeground1 }}>{DEST}</span>
                </span>
              </div>
              {job && (
                <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12, padding: "10px 12px", flex: "none" }}>
                  <div style={{ ...card, border: line, background: tokens.colorNeutralBackground2 }}>
                    <div style={cardRow}>
                      <span style={{ fontSize: 12, color: mut }}>This computer</span><span style={{ fontWeight: 600 }}>{curPc?.hostname}</span><span style={{ color: mut }}>{curPc?.user}</span>
                      <span style={{ flex: 1 }} /><span style={{ fontWeight: 600 }}>{cur ? Math.round(((cur.done + cur.bad) / total) * 100) : 0}%</span>
                    </div>
                    <Bar label="Progress on this computer" v={cur ? (cur.done + cur.bad) / total : 0} color={cur?.bad ? RED : GREEN} />
                    <div style={{ ...cardRow, fontSize: 12, color: mut }}>
                      <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis" }}>{cur?.cur ? `${cur.cur} → ${DEST}` : busy ? "" : "Finished"}</span>
                      <span>{cur ? `file ${Math.min(cur.done + cur.bad + 1, total)} of ${total}` : ""}</span>
                    </div>
                  </div>
                  <div style={{ ...card, border: `1px solid ${tokens.colorBrandStroke2}`, background: tokens.colorBrandBackground2 }}>
                    <div style={cardRow}>
                      <span style={{ fontSize: 12, color: mut }}>All computers</span><span style={{ fontWeight: 600 }}>{fin.length} of {sts.length}</span>
                      <span style={{ flex: 1 }} /><span style={{ fontWeight: 600 }}>{sts.length ? Math.round((fin.length / sts.length) * 100) : 0}%</span>
                    </div>
                    <Bar label="Overall progress" v={sts.length ? fin.length / sts.length : 0} color={BLUE} />
                    <div style={{ ...cardRow, fontSize: 12, alignItems: "center" }}>
                      <span style={tag(tokens.colorPaletteGreenBackground2, tokens.colorPaletteGreenForeground2)}>{fin.length - failed} done</span>
                      <span style={tag(tokens.colorPaletteRedBackground2, tokens.colorPaletteRedForeground2)}>{failed} failed</span>
                      <span style={tag(tokens.colorNeutralBackground4, tokens.colorNeutralForeground1)}>{sts.length - fin.length} waiting</span>
                      <span style={{ flex: 1 }} /><span style={{ color: mut }}>{sts.reduce((a, r) => a + r.done + r.bad, 0)} of {total * sts.length} files</span>
                    </div>
                  </div>
                </div>
              )}
              <div style={{ flex: 1, minHeight: 0, overflow: "auto", borderTop: job ? line : undefined }}>
                <table style={{ width: "100%", borderCollapse: "collapse", tableLayout: "fixed" }}>
                  <thead><tr><th style={{ ...th, width: 130 }}>Computer</th><th style={{ ...th, width: 190 }}>Student</th><th style={th}>Destination folder</th><th style={{ ...th, width: 90, textAlign: "right" }}>Free space</th>{job && <th style={{ ...th, width: 130 }}>Progress</th>}<th style={{ ...th, width: 260 }}>Status</th></tr></thead>
                  <tbody>
                    {list.map((s) => {
                      const r = job?.st[s.id], n = r ? r.done + r.bad : 0;
                      const [text, color] = !r ? ["Ready", GREEN] : n === total ? (r.bad ? [`Failed: ${r.bad} of ${total} file(s) not delivered`, RED] : ["Copied", GREEN]) : r.cur ? [`Copying… ${r.cur}`, BLUE] : job.stopped && !busy ? ["Stopped", GREY] : ["Waiting", GREY];
                      return (
                        <tr key={s.id} onClick={() => browse(s)} title="Click to see this computer's folder on the right"
                          style={{ cursor: "pointer", background: (busy && s.id === now) || ask?.id === s.id ? tokens.colorBrandBackground2 : undefined }}>
                          <td style={td}>{s.hostname}</td>
                          <td style={td}>{s.user}</td>
                          <td style={{ ...td, color: mut }}><span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}><LockClosed16Regular />{DEST}</span></td>
                          <td style={{ ...td, color: mut, textAlign: "right" }}>{freeOf(s.id)}</td>
                          {job && <td style={td}><Bar h={4} v={n / total} color={color} /></td>}
                          <td style={td} title={text}><Status color={color}>{text}</Status></td>
                        </tr>
                      );
                    })}
                    {!list.length && <tr><td colSpan={5} style={{ ...td, color: mut }}>No student computers are connected.</td></tr>}
                  </tbody>
                </table>
              </div>
            </section>
          </DialogContent>
          <DialogActions style={D.foot}>
            <span style={{ marginRight: "auto", fontSize: 12, color: mut }}>
              {done || (picked.size ? `${picked.size} file${picked.size === 1 ? "" : "s"} · ${size(bytes)} to each of ${to.length} student${to.length === 1 ? "" : "s"} · ${size(bytes * to.length)} in total` : "Tick the files to copy · click a student row to see that computer's folder")}
            </span>
            {busy && <Button disabled={job?.stopped} onClick={() => { stop.current = true; setJob((j) => j && { ...j, stopped: true }); }}>Stop</Button>}
            {retry.length > 0 && job && <Button onClick={() => void copy(job.names, retry, job.dir)}>Retry {retry.length} failed</Button>}
            <Button appearance="primary" style={{ minWidth: 140 }} disabled={!picked.size || !to.length || busy} onClick={copyNow}>
              {busy ? "Copying…" : `Copy to ${to.length} student${to.length === 1 ? "" : "s"}`}
            </Button>
            <Button style={{ minWidth: 80 }} disabled={busy} onClick={onClose}>Close</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
