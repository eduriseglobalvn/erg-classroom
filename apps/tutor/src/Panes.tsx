import { useEffect, useState } from "react";
import { Button, Checkbox, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, Input, Menu, MenuItem, MenuList, MenuPopover, Radio, RadioGroup, Select, tokens } from "@fluentui/react-components";
import { Dismiss16Regular, Document16Regular, Folder16Filled, Globe20Regular, Apps20Regular, DocumentText20Regular } from "@fluentui/react-icons";
import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useClass } from "./store";
import { useT } from "./i18n";
import { parseList } from "./guardlist";
import { useLayout } from "./layout";
import { D, DlgHead, mut, th, td, RED } from "./FileDist";

const line = `1px solid ${tokens.colorNeutralStroke2}`;
const load = <T,>(key: string, def: T): T => { try { return { ...def, ...JSON.parse(localStorage.getItem(key) ?? "{}") }; } catch { return def; } };
const save = (key: string, v: unknown) => { try { localStorage.setItem(key, JSON.stringify(v)); } catch { /* not remembered */ } };
const size = (n: number) => n >= 1 << 20 ? `${(n / (1 << 20)).toFixed(1)} MB` : n ? `${Math.max(1, Math.round(n / 1024))} KB` : "0 KB";
const pcs = (n: number, t: (s: string) => string) => `${n} ${t("học sinh")}`;

// ================= Web access (design N08): replaces the wall while open
export type Site = { u: string; d: string };
export type Mode = "Unrestricted" | "AllowListOnly" | "BlockList" | "BlockAll";
export type Web = { mode: Mode; ok: Site[]; no: Site[] };
/** The class's web mode and lists, shared by the Web access pane and the Web rules dialog (remembered). */
export const useWeb = create<{ w: Web; set: (w: Web) => void }>((set) => ({
  w: load("erg.web", { mode: "Unrestricted", ok: [], no: [] }),
  set: (w) => { save("erg.web", w); set({ w }); },
}));
/** `have` plus the new sites, without repeats, at most 500. */
export const mergeSites = (have: Site[], add: Site[]) => { const s = new Set(have.map((x) => x.u)); return [...have, ...add.filter((x) => !s.has(x.u) && s.add(x.u))].slice(0, 500); };
/** The command that puts a web mode in force with the list it uses. */
export const webCmd = (w: Web) => ({ SetWebPolicy: { mode: w.mode, list: (w.mode === "AllowListOnly" ? w.ok : w.mode === "BlockList" ? w.no : []).map((s) => s.u) } });
/** Sites from a text file the teacher picks; null = cancelled. */
export async function pickSites(): Promise<Site[] | null> {
  const f = await open({ multiple: false, filters: [{ name: "Text", extensions: ["txt", "csv"] }] });
  if (!f || Array.isArray(f)) return null;
  return parseSites((await invoke<string>("load_text", { path: f })).replace(/[,;]/g, " "));
}
const BROWSERS = ["chrome.exe", "msedge.exe", "firefox.exe", "browser.exe", "opera.exe", "brave.exe", "iexplore.exe"];
const COLORS = ["#0B7285", "#4A5568", "#6B4FBB", "#1F5FBF", "#B7791F", "#107C41", "#C42B1C"];
const hue = (s: string) => COLORS[[...s].reduce((a, c) => a + c.charCodeAt(0), 0) % COLORS.length];

/** "site.vn  what it is" per line (or comma separated sites): host names that pass parseList, with the rest of the line as description. */
export function parseSites(text: string): Site[] {
  return text.split(/\n+/).flatMap((l) => {
    const [u = "", ...d] = l.trim().split(/\s+/);
    const hosts = parseList(u, "site");
    return hosts.map((h) => ({ u: h, d: hosts.length === 1 ? d.join(" ").slice(0, 100) : "" }));
  });
}

export function WebPane({ onClose, onLink, onRules, say, fail }: { onClose: () => void; onLink: () => void; onRules: () => void; say: (s: string) => void; fail: (s: string) => void }) {
  const t = useT();
  const { students, selected, toggle, send } = useClass();
  const w = useWeb((s) => s.w);
  const [view, setView] = useState<"icons" | "details">("icons");
  const [hits, setHits] = useState<{ id: string; at: number }[]>([]);
  const [now, setNow] = useState(Date.now());
  const [ctx, setCtx] = useState<{ x: number; y: number; host: string } | null>(null);
  const [typed, setTyped] = useState({ ok: "", no: "" });
  useEffect(() => {
    const p = listen<{ id: string }>("student-violation", (e) => setHits((h) => [...h.slice(-200), { id: e.payload.id, at: Date.now() }]));
    const iv = setInterval(() => setNow(Date.now()), 30_000);
    return () => { void p.then((f) => f()); clearInterval(iv); };
  }, []);
  const recent = hits.filter((h) => now - h.at < 5 * 60_000);
  const list = Object.values(students).filter((s) => s.online).sort((a, b) => a.hostname.localeCompare(b.hostname, undefined, { numeric: true }));
  const host = (s: (typeof list)[number]) => (BROWSERS.includes((s.fgApp ?? "").toLowerCase()) ? s.fgTitle || "?" : "");
  const withBrowser = list.filter((s) => host(s)).length;
  const target = selected.size ? `${selected.size} ${t("đã chọn")}` : t("cả lớp");

  // the list a mode uses goes with it; editing that list while it is in force sends it again
  const apply = (next: Web, resend: boolean) => {
    useWeb.getState().set(next);
    if (!resend) return;
    send(webCmd(next)).then((n) => say(`${t("Đã áp dụng chế độ web cho")} ${n} ${t("máy")}`), (e) => fail(String(e)));
  };
  const inUse = (k: "ok" | "no") => (k === "ok" ? w.mode === "AllowListOnly" : w.mode === "BlockList");
  const add = (k: "ok" | "no", sites: Site[]) => {
    if (!sites.length) return fail(t("Không có tên trang web hợp lệ"));
    apply({ ...w, [k]: mergeSites(w[k], sites) }, inUse(k));
  };
  const importList = (k: "ok" | "no") => pickSites().then((l) => l && add(k, l), (e) => fail(String(e)));
  const MODES: [Mode, string][] = [["Unrestricted", t("Không hạn chế")], ["AllowListOnly", t("Chỉ trang được duyệt")], ["BlockList", t("Chặn trang bị hạn chế")], ["BlockAll", t("Chặn hết")]];
  const section = (k: "ok" | "no", title: string, color: string, bg: string, ph: string) => (
    <section aria-label={title} style={{ display: "flex", flexDirection: "column", minWidth: 0, minHeight: 0, borderRight: line }}>
      <div style={{ display: "flex", alignItems: "center", gap: 6, height: 36, flex: "none", padding: "0 6px 0 12px", borderBottom: line, background: bg }}>
        <span style={{ width: 8, height: 8, borderRadius: 2, background: color }} /><b>{title}</b><span style={{ color: mut }}>{w[k].length}</span>
        {inUse(k) && <span style={{ fontSize: 12, color }}>· {t("đang áp dụng")}</span>}
        <span style={{ flex: 1 }} />
        <Input size="small" style={{ width: 200 }} aria-label={title} placeholder={ph} value={typed[k]} onChange={(_, d) => setTyped({ ...typed, [k]: d.value })}
          onKeyDown={(e) => { if (e.key === "Enter" && typed[k].trim()) { add(k, parseSites(typed[k])); setTyped({ ...typed, [k]: "" }); } }} />
        <Button size="small" disabled={!typed[k].trim()} onClick={() => { add(k, parseSites(typed[k])); setTyped({ ...typed, [k]: "" }); }}>{t("Thêm")}</Button>
        <Button size="small" appearance="subtle" onClick={() => void importList(k)}>{t("Nhập…")}</Button>
      </div>
      <div style={{ flex: 1, minHeight: 0, overflow: "auto" }}>
        <table style={{ width: "100%", borderCollapse: "collapse", tableLayout: "fixed" }}>
          <thead><tr><th style={{ ...th, width: 240 }}>{t("Trang web")}</th><th style={th}>{t("Mô tả")}</th><th style={{ ...th, width: 36 }} /></tr></thead>
          <tbody>
            {w[k].map((s) => (
              <tr key={s.u}><td style={td}>{s.u}</td><td style={{ ...td, color: mut }}>{s.d}</td>
                <td style={td}><Button size="small" appearance="subtle" icon={<Dismiss16Regular />} aria-label={`${t("Xoá")} ${s.u}`} onClick={() => apply({ ...w, [k]: w[k].filter((x) => x.u !== s.u) }, inUse(k))} /></td></tr>
            ))}
            {!w[k].length && <tr><td colSpan={3} style={{ ...td, color: mut }}>{t("Chưa có trang nào")}</td></tr>}
          </tbody>
        </table>
      </div>
    </section>
  );
  return (
    <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column", background: tokens.colorNeutralBackground1 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8, height: 48, flex: "none", padding: "0 12px", borderBottom: line, background: tokens.colorNeutralBackground2 }}>
        <RadioGroup layout="horizontal" aria-label={t("Truy cập web của lớp")} value={w.mode} onChange={(_, d) => apply({ ...w, mode: d.value as Mode }, true)}>
          {MODES.map(([m, l]) => <Radio key={m} value={m} label={l} />)}
        </RadioGroup>
        <span style={{ flex: 1 }} />
        <span style={{ fontSize: 12, color: mut }}>{t("Lệnh áp dụng cho")}: <b style={{ color: tokens.colorNeutralForeground1 }}>{target}</b></span>
        <Button size="small" onClick={onLink}>{t("Mở trang web trên máy học sinh…")}</Button>
        <Button size="small" onClick={onRules}>{t("Quy tắc…")}</Button>
        <div role="group" aria-label={t("Xem")} style={{ display: "flex" }}>
          <Button size="small" appearance={view === "icons" ? "secondary" : "subtle"} aria-pressed={view === "icons"} onClick={() => setView("icons")}>{t("Biểu tượng")}</Button>
          <Button size="small" appearance={view === "details" ? "secondary" : "subtle"} aria-pressed={view === "details"} onClick={() => setView("details")}>{t("Chi tiết")}</Button>
        </div>
        <Button size="small" appearance="subtle" icon={<Dismiss16Regular />} aria-label={t("Đóng")} onClick={onClose} />
      </div>
      <div style={{ flex: 1, minHeight: 0, overflow: "auto" }}>
        {view === "icons" ? (
          <div style={{ padding: "18px 16px", display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(118px, 1fr))", gap: "16px 8px", alignContent: "start" }}>
            {list.map((s) => {
              const h = host(s), bad = recent.some((x) => x.id === s.id), on = selected.has(s.id);
              return (
                <button key={s.id} type="button" aria-pressed={on} title={h || t("Không mở trình duyệt")}
                  onClick={() => toggle(s.id, true)}
                  onContextMenu={(e) => { e.preventDefault(); if (h && h !== "?") setCtx({ x: e.clientX, y: e.clientY, host: h }); }}
                  style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 3, minWidth: 0, padding: "6px 4px", borderRadius: 4, cursor: "pointer", fontFamily: "inherit",
                    border: `1px solid ${on ? tokens.colorBrandStroke1 : bad ? "#E9A7A0" : "transparent"}`, background: on ? tokens.colorBrandBackground2 : bad ? "#FDF3F2" : "transparent", color: tokens.colorNeutralForeground1 }}>
                  <span style={{ display: "grid", placeItems: "center", width: 32, height: 32, borderRadius: 7, fontSize: 14, fontWeight: 600, color: "#fff", background: h ? hue(h) : "#BDBDBD" }}>{h ? h[0].toUpperCase() : "–"}</span>
                  <span style={{ fontSize: 12, fontWeight: 600 }}>{s.hostname}</span>
                  <span style={{ fontSize: 12, maxWidth: "100%", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", color: bad ? "#A32418" : mut }}>{bad ? `${t("Vi phạm")}: ${h || ""}` : h || t("Không mở trình duyệt")}</span>
                </button>
              );
            })}
            {!list.length && <span style={{ color: mut }}>{t("Chưa có máy học sinh nào kết nối")}</span>}
          </div>
        ) : (
          <table style={{ width: "100%", borderCollapse: "collapse", tableLayout: "fixed" }}>
            <thead><tr><th style={{ ...th, width: 140 }}>{t("Máy")}</th><th style={{ ...th, width: 160 }}>{t("Người dùng")}</th><th style={{ ...th, width: 140 }}>{t("Trình duyệt")}</th><th style={th}>{t("Trang web")}</th></tr></thead>
            <tbody>
              {list.map((s) => {
                const h = host(s);
                return (
                  <tr key={s.id} onClick={() => toggle(s.id, true)} onContextMenu={(e) => { e.preventDefault(); if (h && h !== "?") setCtx({ x: e.clientX, y: e.clientY, host: h }); }}
                    style={{ cursor: "default", background: selected.has(s.id) ? tokens.colorBrandBackground2 : undefined }}>
                    <td style={td}>{s.hostname}</td><td style={td}>{s.user}</td><td style={{ ...td, color: mut }}>{h ? s.fgApp : "–"}</td>
                    <td style={{ ...td, color: recent.some((x) => x.id === s.id) ? RED : undefined }}>{h || t("Không mở trình duyệt")}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 18, height: 30, flex: "none", padding: "0 16px", borderTop: line, fontSize: 12, color: mut }}>
        <span>{withBrowser} / {pcs(list.length, t)} {t("đang mở trình duyệt")}</span>
        <span style={{ color: recent.length ? "#A32418" : mut }}>{recent.length} {t("vi phạm trong 5 phút qua")}</span>
        <span style={{ flex: 1 }} />
        <span>{t("Nhấp chuột phải vào máy để duyệt hoặc hạn chế trang đang mở")}</span>
      </div>
      <div style={{ height: 232, flex: "none", display: "grid", gridTemplateColumns: "1fr 1fr", borderTop: `1px solid ${tokens.colorNeutralStroke1}` }}>
        {section("ok", t("Trang được duyệt"), "#107C41", tokens.colorPaletteGreenBackground1, "vd: violet.vn")}
        {section("no", t("Trang bị hạn chế"), "#C42B1C", tokens.colorPaletteRedBackground1, "vd: tiktok.com")}
      </div>
      <Menu open={!!ctx} onOpenChange={(_, d) => !d.open && setCtx(null)} positioning={{ target: { getBoundingClientRect: () => new DOMRect(ctx?.x ?? 0, ctx?.y ?? 0, 0, 0) } }}>
        <MenuPopover>
          <MenuList>
            <MenuItem onClick={() => ctx && add("ok", [{ u: ctx.host, d: "" }])}>{t("Duyệt")} {ctx?.host}</MenuItem>
            <MenuItem onClick={() => ctx && add("no", [{ u: ctx.host, d: "" }])}>{t("Hạn chế")} {ctx?.host}</MenuItem>
          </MenuList>
        </MenuPopover>
      </Menu>
    </div>
  );
}

// ================= Quick Launch (design N09): popover under the ribbon
type Kind = "web" | "app" | "doc";
type Launch = { name: string; kind: Kind; path: string };
const DEF_LAUNCH: Launch[] = [
  { name: "Word", kind: "app", path: "winword.exe" }, { name: "Excel", kind: "app", path: "excel.exe" }, { name: "PowerPoint", kind: "app", path: "powerpnt.exe" },
];
const KIND_ICON = { web: <Globe20Regular />, app: <Apps20Regular />, doc: <DocumentText20Regular /> };
// letter tiles in each program's own colour (exe or document extension -> letter, colour)
const TILE: Record<string, [string, string]> = {
  winword: ["W", "#185ABD"], doc: ["W", "#185ABD"], docx: ["W", "#185ABD"], excel: ["X", "#107C41"], xls: ["X", "#107C41"], xlsx: ["X", "#107C41"], csv: ["X", "#107C41"],
  powerpnt: ["P", "#C43E1C"], ppt: ["P", "#C43E1C"], pptx: ["P", "#C43E1C"], onenote: ["N", "#7719AA"], msaccess: ["A", "#A4373A"], outlook: ["O", "#0F6CBD"],
  msedge: ["e", "#0C7CD5"], chrome: ["G", "#1A73E8"], firefox: ["F", "#E66000"], notepad: ["N", "#2B88D8"], mspaint: ["P", "#E3008C"], calc: ["=", "#4C4C4C"],
  code: ["{}", "#007ACC"], python: ["Py", "#3776AB"], pythonw: ["Py", "#3776AB"], "scratch 3": ["S", "#FF8C1A"], scratch: ["S", "#FF8C1A"], explorer: ["E", "#F2B300"],
  pdf: ["PDF", "#D13438"], txt: ["T", "#616161"], png: ["▣", "#8764B8"], jpg: ["▣", "#8764B8"], mp4: ["▶", "#C239B3"],
};
const hostOf = (p: string) => p.trim().replace(/^https?:\/\//i, "").split(/[/?#]/)[0];
function LaunchIcon({ it }: { it: Launch }) {
  const [bad, setBad] = useState(false);
  const file = it.path.trim().replace(/^.*[\\/]/, "").toLowerCase();
  const key = it.kind === "app" ? file.replace(/\.exe$/, "") : file.replace(/^.*\./, "");
  const tile = it.kind !== "web" && TILE[key];
  if (tile) return <span aria-hidden style={{ width: 28, height: 28, borderRadius: 6, display: "grid", placeItems: "center", background: tile[1], color: "#fff", fontWeight: 700, fontSize: tile[0].length > 2 ? 9 : 14, boxShadow: tokens.shadow4 }}>{tile[0]}</span>;
  // the site's own icon, the globe when it has none
  if (it.kind === "web" && !bad && hostOf(it.path)) return <img alt="" width={24} height={24} style={{ margin: 2 }} src={`https://${hostOf(it.path)}/favicon.ico`} onError={() => setBad(true)} />;
  return <span style={{ color: tokens.colorBrandForeground1, height: 28, display: "grid", placeItems: "center" }}>{KIND_ICON[it.kind]}</span>;
}
const nameOf = (it: Launch) => it.name || (it.kind === "web" ? hostOf(it.path) : it.path.trim().replace(/^.*[\\/]/, ""));

/** The command that starts a Quick Launch item on the students. */
export const launchCmd = (it: Launch) => it.kind === "web"
  ? { OpenUrl: /^https?:\/\//i.test(it.path) ? it.path : `https://${it.path}` }
  : { RunApp: { path: it.path, args: [] as string[] } };

export function QuickLaunch({ close, say, fail }: { close: () => void; say: (s: string) => void; fail: (s: string) => void }) {
  const t = useT();
  const { selected } = useClass();
  const [items, setItems] = useState<Launch[]>(() => { try { const j = JSON.parse(localStorage.getItem("erg.launch") ?? "null"); return Array.isArray(j) ? j : DEF_LAUNCH; } catch { return DEF_LAUNCH; } });
  const [cur, setCur] = useState<number | null>(null);
  const [form, setForm] = useState<Launch>({ name: "", kind: "web", path: "" });
  const [to, setTo] = useState<"all" | "sel">(selected.size ? "sel" : "all");
  const keep = (l: Launch[]) => { setItems(l); save("erg.launch", l); };
  const valid = form.path.trim() && !/[\x00-\x1f]/.test(form.path) && form.path.length <= 260 && (form.kind !== "web" || parseList(form.path, "site").length === 1);
  const go = (it: Launch) => {
    const ids = to === "sel" ? [...selected] : null;
    if (to === "sel" && !ids?.length) return fail(t("Chọn ít nhất một máy"));
    invoke<number>("send_command", { ids, cmd: launchCmd(it) }).then((n) => { say(`${nameOf(it)}: ${t("đã gửi tới")} ${n} ${t("máy")}`); close(); }, (e) => fail(String(e)));
  };
  const saveForm = () => {
    const it = { ...form, name: form.name.trim(), path: form.path.trim() };
    keep(cur === null ? [...items, it] : items.map((x, i) => (i === cur ? it : x)));
    setCur(cur ?? items.length);
  };
  const pickFile = async () => {
    const f = await open({ multiple: false, filters: form.kind === "app" ? [{ name: "Program", extensions: ["exe"] }] : undefined });
    if (f && !Array.isArray(f)) setForm({ ...form, path: f });
  };
  return (
    <div style={{ width: 416, display: "flex", flexDirection: "column", gap: 10, padding: 8 }}>
      <div style={{ display: "flex", alignItems: "flex-start", gap: 8 }}>
        <div style={{ flex: 1 }}><b>{t("Khởi chạy nhanh")}</b><div style={{ fontSize: 12, color: mut }}>{t("Mở một ứng dụng hoặc trang web trên máy học sinh")}</div></div>
        <Button size="small" onClick={() => { setCur(null); setForm({ name: "", kind: "web", path: "" }); }}>{t("Thêm mục")}</Button>
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 4 }}>
        {items.map((it, i) => (
          <button key={i} type="button" title={it.path} aria-pressed={cur === i}
            onClick={() => { setCur(i); setForm(it); }} onDoubleClick={() => go(it)}
            onContextMenu={(e) => { e.preventDefault(); if (window.confirm(`${t("Xoá")} ${nameOf(it)}?`)) { keep(items.filter((_, j) => j !== i)); setCur(null); } }}
            style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 4, padding: "8px 2px", borderRadius: 4, cursor: "pointer", fontFamily: "inherit", fontSize: 12, color: tokens.colorNeutralForeground1,
              border: `1px solid ${cur === i ? tokens.colorBrandStroke1 : "transparent"}`, background: cur === i ? tokens.colorBrandBackground2 : "transparent" }}>
            <LaunchIcon it={it} />
            <span style={{ maxWidth: "100%", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{nameOf(it)}</span>
          </button>
        ))}
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 6, paddingTop: 8, borderTop: line }}>
        <span style={{ fontSize: 12, fontWeight: 600 }}>{cur === null ? t("Mục mới") : nameOf(form)}</span>
        <RadioGroup layout="horizontal" value={form.kind} onChange={(_, d) => setForm({ ...form, kind: d.value as Kind })}>
          <Radio value="web" label={t("Trang web")} /><Radio value="app" label={t("Ứng dụng")} /><Radio value="doc" label={t("Tài liệu")} />
        </RadioGroup>
        <div style={{ display: "flex", gap: 6 }}>
          <Input size="small" style={{ flex: 1 }} aria-label={t("Địa chỉ")} placeholder={form.kind === "web" ? "hoc10.vn" : form.kind === "app" ? "winword.exe" : "%USERPROFILE%\\Documents\\bai1.docx"}
            value={form.path} onChange={(_, d) => setForm({ ...form, path: d.value })} />
          {form.kind !== "web" && <Button size="small" onClick={() => void pickFile()}>{t("Chọn…")}</Button>}
        </div>
        <Input size="small" aria-label={t("Tên")} placeholder={t("Tên (không bắt buộc)")} value={form.name} onChange={(_, d) => setForm({ ...form, name: d.value })} />
        {form.kind !== "web" && <span style={{ fontSize: 11, color: mut }}>{t("Đường dẫn trên máy học sinh; tên chương trình (winword.exe) hoặc %USERPROFILE%\\… đều được")}</span>}
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 6, paddingTop: 8, borderTop: line }}>
        <span style={{ fontSize: 12, color: mut }}>{t("Mở trên")}</span>
        <Select size="small" value={to} onChange={(_, d) => setTo(d.value as "all" | "sel")}>
          <option value="all">{t("Cả lớp")}</option><option value="sel">{t("Máy đang chọn")} ({selected.size})</option>
        </Select>
        <span style={{ flex: 1 }} />
        <Button size="small" disabled={!valid} onClick={saveForm}>{t("Lưu")}</Button>
        <Button size="small" appearance="primary" disabled={!valid} onClick={() => go(form)}>{t("Mở ngay")}</Button>
      </div>
    </div>
  );
}

// ================= Send Work (design N06)
const INBOX = "Documents\\ERG\\Received"; // fixed by the student program
type F = { path: string; size: number };

export function SendWork({ onClose, say, fail }: { onClose: () => void; say: (s: string) => void; fail: (s: string) => void }) {
  const t = useT();
  const { students, selected, send } = useClass();
  const { groups } = useLayout();
  const online = Object.values(students).filter((s) => s.online);
  const names = [...new Set(Object.values(groups))].sort();
  const [name, setName] = useState("");
  const [to, setTo] = useState(selected.size ? "sel" : "all");
  const [files, setFiles] = useState<F[]>([]);
  const [cur, setCur] = useState<string | null>(null);
  const [openDir, setOpenDir] = useState(true);
  const [tell, setTell] = useState(true);
  const [busy, setBusy] = useState(false);
  const ids = to === "all" ? online.map((s) => s.id) : to === "sel" ? online.filter((s) => selected.has(s.id)).map((s) => s.id) : online.filter((s) => groups[s.hostname] === to.slice(2)).map((s) => s.id);
  const bytes = files.reduce((a, f) => a + f.size, 0);
  const dir = (p: string) => p.replace(/[\\/][^\\/]*$/, "");
  // sizes come from listing the folder the files are in (no extra backend call)
  const addPaths = async (paths: string[]) => {
    const out: F[] = [];
    for (const d of new Set(paths.map(dir))) {
      try {
        const [, l] = await invoke<[string, { name: string; is_dir: boolean; size: number }[]]>("local_dir", { path: d });
        for (const p of paths.filter((p) => dir(p) === d)) { const e = l.find((x) => x.name === p.slice(d.length + 1)); if (e && !e.is_dir) out.push({ path: p, size: e.size }); }
      } catch (e) { fail(String(e)); }
    }
    setFiles((f) => [...f, ...out.filter((o) => !f.some((x) => x.path === o.path))]);
  };
  const addFiles = async () => { const f = await open({ multiple: true }); if (f) await addPaths(Array.isArray(f) ? f : [f]); };
  const addFolder = async () => {
    const d = await open({ directory: true });
    if (!d || Array.isArray(d)) return;
    try {
      const [real, l] = await invoke<[string, { name: string; is_dir: boolean; size: number }[]]>("local_dir", { path: d });
      const out = l.filter((x) => !x.is_dir).map((x) => ({ path: real.replace(/\\$/, "") + "\\" + x.name, size: x.size }));
      setFiles((f) => [...f, ...out.filter((o) => !f.some((x) => x.path === o.path))]);
    } catch (e) { fail(String(e)); }
  };
  useEffect(() => {
    // dropping files on the window adds them; outside Tauri (browser preview) there is no drop event
    let off: (() => void) | undefined;
    import("@tauri-apps/api/webview").then(({ getCurrentWebview }) => getCurrentWebview().onDragDropEvent((e) => { if (e.payload.type === "drop") void addPaths(e.payload.paths); }))
      .then((f) => (off = f), () => { /* no drag and drop */ });
    return () => off?.();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps
  const go = async () => {
    setBusy(true);
    let ok = 0, bad = 0;
    for (const f of files) {
      try { const [a, b] = await invoke<[number, number]>("send_file", { ids, path: f.path }); ok += a; bad += b; } catch (e) { bad += ids.length; fail(String(e)); }
    }
    const cmd = (c: unknown) => invoke("send_command", { ids, cmd: c }).catch((e) => fail(String(e)));
    if (tell) await cmd({ Message: { text: `${name.trim() || t("Bài tập")}: ${files.length} ${t("tệp trong")} ${INBOX}`, seconds: 20 } });
    if (openDir) await cmd({ RunApp: { path: `%USERPROFILE%\\${INBOX}`, args: [] } });
    setBusy(false);
    say(bad ? `${name || t("Bài tập")}: ${ok} ${t("bản đã gửi")}, ${bad} ${t("lỗi")}` : `${name || t("Bài tập")}: ${files.length} ${t("tệp đã gửi tới")} ${ids.length} ${t("máy")}`);
    if (!bad) onClose();
  };
  const lab: React.CSSProperties = { display: "flex", flexDirection: "column", gap: 4, fontSize: 12, fontWeight: 600 };
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && !busy && onClose()}>
      <DialogSurface style={{ ...D.surface, maxWidth: "none", width: 760 }}>
        <DialogBody style={D.body}>
          <DlgHead title={t("Gửi bài")} onClose={onClose} off={busy} />
          <DialogContent style={{ ...D.content, height: 470 }}>
            <div style={{ display: "flex", gap: 10 }}>
              <label style={{ ...lab, flex: 1 }}>{t("Tên bài")}<Input value={name} placeholder="IC3 – Bài 7" onChange={(_, d) => setName(d.value)} /></label>
              <label style={{ ...lab, width: 250 }}>{t("Gửi tới")}
                <Select value={to} onChange={(_, d) => setTo(d.value)}>
                  <option value="all">{t("Cả lớp")} ({online.length} {t("trực tuyến")})</option>
                  <option value="sel">{t("Máy đang chọn")} ({selected.size})</option>
                  {names.map((g) => <option key={g} value={"g:" + g}>{t("Nhóm")} {g}</option>)}
                </Select>
              </label>
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 6, flex: 1, minHeight: 0 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <span style={lab}>{t("Tệp cần gửi")}</span><span style={{ flex: 1 }} />
                <Button size="small" onClick={() => void addFiles()}>{t("Thêm tệp…")}</Button>
                <Button size="small" onClick={() => void addFolder()}>{t("Thêm thư mục…")}</Button>
                <Button size="small" appearance="subtle" disabled={!cur} onClick={() => { setFiles(files.filter((f) => f.path !== cur)); setCur(null); }}>{t("Bỏ")}</Button>
              </div>
              <div style={{ ...D.box, flex: 1, minHeight: 0, display: "flex", flexDirection: "column" }}>
                <table style={{ width: "100%", borderCollapse: "collapse", tableLayout: "fixed" }}>
                  <thead><tr><th style={th}>{t("Tên")}</th><th style={{ ...th, width: 250 }}>{t("Từ")}</th><th style={{ ...th, width: 80, textAlign: "right" }}>{t("Cỡ")}</th></tr></thead>
                  <tbody>
                    {files.map((f) => (
                      <tr key={f.path} onClick={() => setCur(f.path)} style={{ cursor: "default", background: cur === f.path ? tokens.colorBrandBackground2 : undefined }}>
                        <td style={td}><span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}><Document16Regular style={{ color: mut, flex: "none" }} />{f.path.slice(dir(f.path).length + 1)}</span></td>
                        <td style={{ ...td, color: mut }} title={dir(f.path)}>{dir(f.path)}</td>
                        <td style={{ ...td, color: mut, textAlign: "right" }}>{size(f.size)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                <div style={{ flex: 1, minHeight: 40, display: "grid", placeItems: "center", margin: 8, border: `1px dashed ${tokens.colorNeutralStroke1}`, borderRadius: 4, color: mut, fontSize: 12 }}>
                  {files.length ? t("Thả thêm tệp vào đây") : t("Thả tệp vào đây hoặc bấm Thêm tệp…")}
                </div>
              </div>
            </div>
            <label style={lab}>{t("Thư mục trên máy học sinh")}
              <Input readOnly value={INBOX} contentBefore={<Folder16Filled style={{ color: "#B7791F" }} />} title={t("Máy học sinh luôn nhận tệp vào thư mục này")} />
            </label>
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "0 16px" }}>
              <Checkbox label={t("Mở thư mục trên máy học sinh sau khi gửi")} checked={openDir} onChange={(_, d) => setOpenDir(!!d.checked)} />
              <Checkbox label={t("Báo cho học sinh bằng tin nhắn")} checked={tell} onChange={(_, d) => setTell(!!d.checked)} />
            </div>
          </DialogContent>
          <DialogActions style={D.foot}>
            <span style={{ marginRight: "auto", fontSize: 12, color: mut }}>{files.length} {t("tệp")} · {size(bytes)} {t("tới")} {pcs(ids.length, t)}</span>
            <Button appearance="primary" style={{ minWidth: 96 }} disabled={!files.length || !ids.length || busy} onClick={() => void go()}>{busy ? t("Đang gửi…") : t("Gửi")}</Button>
            <Button style={{ minWidth: 80 }} disabled={busy} onClick={onClose}>{t("Huỷ")}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
