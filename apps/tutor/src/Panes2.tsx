import { useEffect, useState } from "react";
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, Input, Radio, RadioGroup, Select, tokens } from "@fluentui/react-components";
import { Dismiss16Regular } from "@fluentui/react-icons";
import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useClass } from "./store";
import { useLang, useT } from "./i18n";
import { useUi } from "./uistate";
import { D, DlgHead, mut, th, td, RED } from "./FileDist";
import { mergeSites, parseSites, pickSites, useWeb, webCmd, type Mode, type Site, type Web } from "./Panes";

const line = `1px solid ${tokens.colorNeutralStroke2}`;
const GREEN = "#107C41";
const who = (id: string) => { const s = useClass.getState().students[id]; return s?.user || s?.hostname || id; };

// ================= Alerts (design N24): unauthorised access, kept for the whole lesson
export type Alert = { key: number; id: string; kind: string; app: string; title: string; at: number; state: "open" | "kept" | "done" };
let seq = 0;
export const useAlerts = create<{ list: Alert[]; add: (a: { id: string; kind: string; app: string; title: string }) => void; mark: (key: number, state: Alert["state"]) => void; clear: () => void }>((set) => ({
  list: [],
  add: (a) => set((s) => ({ list: [...s.list.slice(-199), { ...a, key: ++seq, at: Date.now(), state: "open" }] })),
  mark: (key, state) => set((s) => ({ list: s.list.map((a) => (a.key === key ? { ...a, state } : a)) })),
  clear: () => set((s) => ({ list: s.list.filter((a) => a.state !== "done") })),
}));
/** What the student did, from the kind the student computer reports (guard.rs: adult, site, word, tool). */
export const alertWhat = (kind: string) =>
  kind === "tool" ? "Mở công cụ hệ thống được bảo vệ" : kind === "site" ? "Mở trang trong danh sách chặn" : kind === "word" ? "Gặp từ khoá bị chặn" : "Mở trang web người lớn (18+)";
export const unlockAlert = (a: Alert) => invoke("send_command", { ids: [a.id], cmd: "Unlock" }).then(() => useAlerts.getState().mark(a.key, "done"));

export function AlertsPanel({ onRules, fail }: { onRules: () => void; fail: (s: string) => void }) {
  const t = useT();
  const list = useAlerts((s) => s.list);
  const hosts = useClass((s) => s.students);
  if (!list.length) return null;
  const need = list.filter((a) => a.state !== "done").length;
  const view = (a: Alert) => { useClass.setState({ selected: new Set([a.id]) }); useUi.getState().request("remote"); };
  return (
    <aside aria-label={t("Cảnh báo")} style={{ width: 340, flex: "none", display: "flex", flexDirection: "column", minHeight: 0, background: tokens.colorNeutralBackground2, borderLeft: line }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8, height: 40, flex: "none", padding: "0 8px 0 12px", borderBottom: line }}>
        <b>{t("Cảnh báo")}</b>
        {need > 0 && <span style={{ minWidth: 18, height: 18, padding: "0 5px", borderRadius: 9, background: RED, color: "#fff", fontSize: 11, lineHeight: "18px", textAlign: "center" }}>{need}</span>}
        <span style={{ flex: 1 }} />
        <Button size="small" appearance="subtle" disabled={need === list.length} onClick={useAlerts.getState().clear}>{t("Xoá mục đã xử lý")}</Button>
      </div>
      <div style={{ flex: 1, minHeight: 0, overflow: "auto", padding: 8, display: "grid", gap: 8, alignContent: "start" }}>
        {[...list].reverse().map((a) => {
          const done = a.state === "done", c = done ? tokens.colorNeutralForeground3 : RED;
          return (
            <article key={a.key} style={{ display: "grid", gap: 5, padding: "9px 10px", border: `1px solid ${done ? tokens.colorNeutralStroke2 : "#E8B4AE"}`, borderLeft: `3px solid ${c}`, borderRadius: 4, background: tokens.colorNeutralBackground1 }}>
              <div style={{ display: "flex", gap: 6, minWidth: 0 }}>
                <b>{hosts[a.id]?.hostname ?? a.id}</b>
                <span style={{ flex: 1, minWidth: 0, color: mut, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{hosts[a.id]?.user}</span>
                <span style={{ color: mut, fontSize: 12 }}>{new Date(a.at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span>
              </div>
              <b style={{ color: c }}>{t(alertWhat(a.kind))}</b>
              <span style={{ color: mut, fontSize: 12, wordBreak: "break-word" }}>{a.app} · {a.title.slice(0, 120)}</span>
              <span style={{ justifySelf: "start", fontSize: 12, padding: "1px 6px", borderRadius: 3, background: done ? tokens.colorNeutralBackground3 : "#FDE7E5", color: done ? tokens.colorNeutralForeground1 : "#8E1F15" }}>
                {done ? t("Đã mở khoá") : t("Đã khoá, đang hiện cảnh báo")}
              </span>
              {!done && (
                <div style={{ display: "flex", gap: 6, marginTop: 2 }}>
                  <Button size="small" appearance="primary" style={{ flex: 1 }} onClick={() => unlockAlert(a).catch((e) => fail(String(e)))}>{t("Mở khoá")}</Button>
                  <Button size="small" style={{ flex: 1 }} onClick={() => view(a)}>{t("Xem")}</Button>
                  {a.state === "open" && <Button size="small" style={{ flex: 1 }} onClick={() => useAlerts.getState().mark(a.key, "kept")}>{t("Giữ khoá")}</Button>}
                </div>
              )}
            </article>
          );
        })}
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 8, height: 40, flex: "none", padding: "0 8px 0 12px", borderTop: line, fontSize: 12, color: mut }}>
        <span style={{ flex: 1 }}>{t("Mọi cảnh báo đều được ghi vào Nhật ký")}</span>
        <Button size="small" appearance="subtle" onClick={onRules}>{t("Quy tắc…")}</Button>
      </div>
    </aside>
  );
}

// ================= Web and internet rules (design N26): edits a copy, Apply sends it to the whole class
export function WebRules({ onClose, onGuard, say, fail }: { onClose: () => void; onGuard: () => void; say: (s: string) => void; fail: (s: string) => void }) {
  const t = useT();
  const [w, setW] = useState<Web>(() => useWeb.getState().w);
  const [typed, setTyped] = useState({ ok: "", no: "" });
  const add = (k: "ok" | "no", sites: Site[]) => (sites.length ? setW((x) => ({ ...x, [k]: mergeSites(x[k], sites) })) : fail(t("Không có tên trang web hợp lệ")));
  const apply = () => {
    useWeb.getState().set(w);
    invoke<number>("send_command", { ids: null, cmd: webCmd(w) }).then((n) => { say(`${t("Đã áp dụng chế độ web cho")} ${n} ${t("máy")}`); onClose(); }, (e) => fail(String(e)));
  };
  const MODES: [Mode, string, string][] = [
    ["Unrestricted", "Không hạn chế", "Mở được mọi trang, trừ web người lớn và danh sách chặn."],
    ["AllowListOnly", "Chỉ trang được duyệt", "Chỉ mở được các trang trong danh sách Được duyệt."],
    ["BlockList", "Chặn trang bị hạn chế", "Mở được mọi trang, trừ các trang trong danh sách Bị hạn chế."],
    ["BlockAll", "Chặn hết", "Không trang web nào mở được."],
  ];
  const RULES: [string, string, boolean][] = [
    ["Web người lớn (18+)", "Khoá máy, hiện cảnh báo", true],
    ["Trang và từ khoá trong danh sách chặn", "Khoá máy, hiện cảnh báo", true],
    ["Task Manager, Control Panel, Registry, Command Prompt", "Khoá máy, hiện cảnh báo", true],
    ["Trang không được duyệt hoặc bị hạn chế", "Trang không mở được", false],
  ];
  const STEPS: [string, string][] = [
    ["Học sinh mở trang hoặc công cụ", "Trang web hoặc công cụ hệ thống được bảo vệ"],
    ["Máy học sinh tự kiểm tra", "Theo danh sách của bạn và danh sách có sẵn"],
    ["Khoá máy", "Màn hình cảnh báo đỏ cho tới khi bạn mở khoá"],
    ["Bạn được báo", "Cảnh báo trên máy giáo viên, ghi vào Nhật ký"],
  ];
  const box = (k: "ok" | "no", title: string, color: string, ph: string) => (
    <>
      <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
        <b style={{ fontSize: 14 }}>{title}</b><span style={{ color: mut }}>{w[k].length}</span><span style={{ flex: 1 }} />
        <Input size="small" style={{ width: 170 }} aria-label={title} placeholder={ph} value={typed[k]} onChange={(_, d) => setTyped({ ...typed, [k]: d.value })}
          onKeyDown={(e) => { if (e.key === "Enter" && typed[k].trim()) { add(k, parseSites(typed[k])); setTyped({ ...typed, [k]: "" }); } }} />
        <Button size="small" disabled={!typed[k].trim()} onClick={() => { add(k, parseSites(typed[k])); setTyped({ ...typed, [k]: "" }); }}>{t("Thêm")}</Button>
        <Button size="small" appearance="subtle" onClick={() => pickSites().then((l) => l && add(k, l), (e) => fail(String(e)))}>{t("Nhập…")}</Button>
      </div>
      <div style={{ flex: 1, minHeight: 120, overflow: "auto", border: line, borderRadius: 4 }}>
        {w[k].map((s) => (
          <div key={s.u} style={{ display: "flex", alignItems: "center", gap: 8, height: 30, padding: "0 4px 0 10px", borderBottom: line }}>
            <span style={{ width: 8, height: 8, borderRadius: 4, background: color, flex: "none" }} />
            <span style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{s.u}{s.d && <span style={{ color: mut }}> · {s.d}</span>}</span>
            <Button size="small" appearance="subtle" icon={<Dismiss16Regular />} aria-label={`${t("Xoá")} ${s.u}`} onClick={() => setW((x) => ({ ...x, [k]: x[k].filter((y) => y.u !== s.u) }))} />
          </div>
        ))}
        {!w[k].length && <div style={{ padding: 10, color: mut }}>{t("Chưa có trang nào")}</div>}
      </div>
    </>
  );
  const col = { display: "flex", flexDirection: "column", gap: 8, padding: "14px 16px", minHeight: 0 } as const;
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ ...D.surface, maxWidth: 1240, width: "94vw" }}>
        <DialogBody style={D.body}>
          <DlgHead title={`${t("Quy tắc web và Internet")} · ${t("áp dụng cho cả lớp")}`} onClose={onClose} />
          <DialogContent style={{ ...D.content, padding: 0, gap: 0, overflow: "hidden", display: "flex", flexDirection: "column", height: "min(620px, 70vh)" }}>
            <div style={{ flex: 1, minHeight: 0, display: "grid", gridTemplateColumns: "300px minmax(0, 1fr) 380px" }}>
              <section aria-label={t("Trang web")} style={{ ...col, borderRight: line }}>
                <b style={{ fontSize: 14 }}>1 · {t("Trang web")}</b>
                <RadioGroup value={w.mode} onChange={(_, d) => setW({ ...w, mode: d.value as Mode })} aria-label={t("Trang web")} style={{ gap: 6 }}>
                  {MODES.map(([m, l, d]) => (
                    <div key={m} style={{ boxSizing: "border-box", width: "100%", border: `1px solid ${w.mode === m ? tokens.colorBrandStroke2 : tokens.colorNeutralStroke2}`, borderRadius: 4, background: w.mode === m ? tokens.colorBrandBackground2 : undefined }}>
                      <Radio value={m} style={{ width: "100%" }} label={<span style={{ display: "grid" }}><b>{t(l)}</b><span style={{ color: mut, fontSize: 12 }}>{t(d)}</span></span>} />
                    </div>
                  ))}
                </RadioGroup>
              </section>
              <section aria-label={t("Danh sách")} style={{ ...col, borderRight: line }}>
                {box("ok", t("Trang được duyệt"), GREEN, "example.edu.vn")}
                {box("no", t("Trang bị hạn chế"), RED, "example.com")}
              </section>
              <section aria-label={t("Khi học sinh vi phạm")} style={col}>
                <b style={{ fontSize: 14 }}>2 · {t("Khi học sinh vi phạm")}</b>
                <table style={{ width: "100%", borderCollapse: "collapse", tableLayout: "fixed" }}>
                  <thead><tr><th style={th}>{t("Học sinh làm gì")}</th><th style={{ ...th, width: 160 }}>{t("Điều gì xảy ra")}</th></tr></thead>
                  <tbody>{RULES.map(([a, b, lock]) => (
                    <tr key={a}><td style={{ ...td, whiteSpace: "normal", height: "auto", padding: "6px 12px" }}><span style={{ display: "inline-block", width: 8, height: 8, borderRadius: 4, marginRight: 8, background: lock ? RED : "#D9A400" }} />{t(a)}</td><td style={{ ...td, whiteSpace: "normal", height: "auto", padding: "6px 12px" }}>{t(b)}</td></tr>
                  ))}</tbody>
                </table>
                <span style={{ color: mut, fontSize: 12 }}>{t("Web người lớn luôn bị chặn ở mọi chế độ, kể cả Không hạn chế.")}</span>
                <Button style={{ alignSelf: "start" }} onClick={onGuard}>{t("Web và từ khoá bị chặn…")}</Button>
              </section>
            </div>
            <div aria-label={t("Từng bước")} style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 10, padding: "14px 16px", background: tokens.colorBrandBackground2, borderTop: `1px solid ${tokens.colorBrandStroke2}` }}>
              {STEPS.map(([l, d], i) => (
                <div key={l} style={{ display: "flex", gap: 9, alignItems: "flex-start" }}>
                  <span style={{ display: "grid", placeItems: "center", flex: "none", width: 22, height: 22, borderRadius: 11, background: tokens.colorBrandBackground, color: "#fff", fontSize: 12 }}>{i + 1}</span>
                  <span style={{ display: "grid" }}><b>{t(l)}</b><span style={{ color: mut, fontSize: 12 }}>{t(d)}</span></span>
                </div>
              ))}
            </div>
          </DialogContent>
          <DialogActions style={D.foot}>
            <Button appearance="subtle" style={{ marginRight: "auto" }} onClick={() => setW({ mode: "Unrestricted", ok: [], no: [] })}>{t("Khôi phục mặc định")}</Button>
            <Button appearance="primary" onClick={apply}>{t("Áp dụng cho cả lớp")}</Button>
            <Button onClick={onClose}>{t("Huỷ")}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

// ================= Tutor settings (design N20): only what the Tutor can really change
export function Settings({ onClose, fail }: { onClose: () => void; fail: (s: string) => void }) {
  const t = useT();
  const lang = useLang();
  const cur = useClass((s) => s.room);
  const [room, setRoom] = useState(cur);
  const [ver, setVer] = useState("");
  useEffect(() => { invoke<string>("app_version").then(setVer, () => {}); }, []);
  const ok = () => {
    if (room.trim() !== cur) useClass.getState().connect(room).catch((e) => fail(String(e)));
    onClose();
  };
  const row = (l: string, h: string, ctl: React.ReactNode) => (
    <div style={{ display: "flex", alignItems: "center", gap: 12, minHeight: 48, padding: "4px 12px", borderBottom: line }}>
      <span style={{ flex: 1, display: "grid", lineHeight: "16px" }}><span>{t(l)}</span><span style={{ color: mut, fontSize: 12 }}>{t(h)}</span></span>
      <div style={{ width: 240 }}>{ctl}</div>
    </div>
  );
  const group = (title: string, rows: React.ReactNode) => (
    <section style={{ display: "grid", gap: 8 }}><b style={{ fontSize: 14 }}>{t(title)}</b><div style={{ border: line, borderRadius: 4 }}>{rows}</div></section>
  );
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ ...D.surface, maxWidth: 680 }}>
        <DialogBody style={D.body}>
          <DlgHead title={t("Cài đặt")} onClose={onClose} />
          <DialogContent style={{ ...D.content, display: "grid", gap: 16 }}>
            {group("Phòng học", <>
              {row("Tên phòng", "Học sinh cùng tên phòng sẽ vào lớp này; để trống là mọi phòng", <Input style={{ width: "100%" }} aria-label={t("Tên phòng")} value={room} onChange={(_, d) => setRoom(d.value)} />)}
            </>)}
            {group("Chung", <>
              {row("Ngôn ngữ", "Ngôn ngữ của màn hình giáo viên", (
                <Select aria-label={t("Ngôn ngữ")} value={lang.lang} onChange={(_, d) => d.value !== lang.lang && lang.toggle()}>
                  <option value="vi">Tiếng Việt</option><option value="en">English</option>
                </Select>
              ))}
              {row("Phiên bản", "ERG Classroom Tutor", <span>{ver || "–"}</span>)}
            </>)}
          </DialogContent>
          <DialogActions style={D.foot}>
            <Button appearance="primary" onClick={ok}>OK</Button>
            <Button onClick={onClose}>{t("Huỷ")}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

// ================= Assessments (design N22): tests, the running one, and each student's result
type Score = { points: number; max: number; per_question: number[] };
type Q = { text: string; points: number; kind: Record<string, Record<string, unknown>> };
type Paper = { title: string; questions: Q[] };
type TestRef = { name: string; path: string; n: number };
type Run = { id: number; path: string; paper: string; ends: number; done: boolean; scores: Record<string, Score> };
const loadTests = (): TestRef[] => { try { const j = JSON.parse(localStorage.getItem("erg.tests") ?? "[]"); return Array.isArray(j) ? j : []; } catch { return []; } };

export const useTest = create<{
  tests: TestRef[]; run: Run | null;
  addTest: (path: string) => Promise<void>; drop: (path: string) => void;
  start: (path: string, minutes: number) => Promise<number>; handIn: (sid: string, test: number, answers: string) => Promise<void>;
  finish: () => Promise<void>; exportCsv: () => Promise<void>;
}>((set, get) => ({
  tests: loadTests(),
  run: null,
  addTest: async (path) => {
    const p = JSON.parse(await invoke<string>("load_text", { path })) as Paper;
    await invoke("make_student_paper", { paper: JSON.stringify(p), seed: 1 }); // the grader's own check
    const tests = [{ name: p.title || path, path, n: p.questions.length }, ...get().tests.filter((x) => x.path !== path)].slice(0, 50);
    localStorage.setItem("erg.tests", JSON.stringify(tests));
    set({ tests });
  },
  drop: (path) => { const tests = get().tests.filter((x) => x.path !== path); localStorage.setItem("erg.tests", JSON.stringify(tests)); set({ tests }); },
  start: async (path, minutes) => {
    const paper = await invoke<string>("load_text", { path });
    const id = Date.now() % 2 ** 31;
    const json = await invoke<string>("make_student_paper", { paper, seed: id });
    const n = await useClass.getState().send({ StartTest: { id, json, seconds: minutes * 60 } });
    set({ run: { id, path, paper, ends: minutes ? Date.now() + minutes * 60_000 : 0, done: false, scores: {} } });
    return n;
  },
  // marked as soon as it comes in, so the teacher sees scores while the others are still working
  handIn: async (sid, test, answers) => {
    const r = get().run;
    if (!r || r.id !== test) return;
    const score = await invoke<Score>("grade_test", { paper: r.paper, answers, seed: r.id });
    set((s) => (s.run && s.run.id === test ? { run: { ...s.run, scores: { ...s.run.scores, [sid]: score } } } : {}));
  },
  finish: async () => {
    if (!get().run) throw new Error("No test is running");
    await useClass.getState().send("StopTest");
    set((s) => ({ run: s.run && { ...s.run, done: true } }));
  },
  exportCsv: async () => {
    const r = get().run;
    if (!r) return;
    const csv = await invoke<string>("test_report_csv", { rows: Object.entries(r.scores).map(([sid, sc]) => [who(sid), sc]) });
    const path = await save({ defaultPath: "results.csv" });
    if (path) await invoke("write_text", { path, text: csv });
  },
}));

/** What the teacher sees under a question: the right answer, in the form the paper keeps it. */
function answerOf(q: Q): [string, boolean][] {
  const [k, v] = Object.entries(q.kind)[0] ?? ["", {}];
  const a = v as { options?: string[]; correct?: number[]; answer?: boolean; accepted?: string[][]; left?: string[]; right?: string[]; pairs?: number[]; items?: string[]; image?: string };
  switch (k) {
    case "Choice": return (a.options ?? []).map((o, i) => [o, !!a.correct?.includes(i)]);
    case "TrueFalse": return [["Đúng", a.answer === true], ["Sai", a.answer === false]];
    case "Fill": return (a.accepted ?? []).map((x) => [x.join(" | "), true]);
    case "Match": return (a.left ?? []).map((l, i) => [`${l} = ${a.right?.[a.pairs?.[i] ?? i] ?? ""}`, true]);
    case "Order": return (a.items ?? []).map((x, i) => [`${i + 1}. ${x}`, true]);
    case "Region": return [[a.image ?? "", true]];
  }
  return [];
}
const KIND: Record<string, string> = { Choice: "Trắc nghiệm", TrueFalse: "Đúng / Sai", Fill: "Điền vào chỗ trống", Match: "Nối", Order: "Sắp xếp", Region: "Chọn vùng trên ảnh" };

export function AssessPane({ onClose, say, fail }: { onClose: () => void; say: (s: string) => void; fail: (s: string) => void }) {
  const t = useT();
  const { tests, run } = useTest();
  const students = useClass((s) => s.students);
  const [cur, setCur] = useState<string | null>(run?.path ?? tests[0]?.path ?? null);
  const [paper, setPaper] = useState<Paper | null>(null);
  const [qi, setQi] = useState(0);
  const [minutes, setMinutes] = useState("15");
  const [now, setNow] = useState(Date.now());
  useEffect(() => { const iv = setInterval(() => setNow(Date.now()), 1000); return () => clearInterval(iv); }, []);
  useEffect(() => {
    setPaper(null); setQi(0);
    if (cur) invoke<string>("load_text", { path: cur }).then((s) => setPaper(JSON.parse(s)), (e) => fail(String(e)));
  }, [cur, fail]);
  const mine = run && run.path === cur ? run : null;
  const scores = Object.values(mine?.scores ?? {});
  const q = paper?.questions[qi];
  const right = scores.filter((s) => q && q.points > 0 && (s.per_question[qi] ?? 0) >= q.points).length;
  const left = mine && !mine.done && mine.ends ? Math.max(0, Math.round((mine.ends - now) / 1000)) : null;
  const go = (p: Promise<unknown>) => p.catch((e) => fail(String(e)));
  const openTest = () => go(open({ multiple: false, filters: [{ name: "Test", extensions: ["json"] }] }).then(async (f) => {
    if (!f || Array.isArray(f)) return;
    await useTest.getState().addTest(f);
    setCur(f);
  }));
  const rows = Object.values(students).filter((s) => s.online || mine?.scores[s.id]).sort((a, b) => a.hostname.localeCompare(b.hostname, undefined, { numeric: true }));
  const pct = (n: number, d: number) => `${d ? Math.round((100 * n) / d) : 0}%`;
  return (
    <div style={{ flex: 1, minWidth: 0, display: "flex", background: tokens.colorNeutralBackground1 }}>
      <aside aria-label={t("Bài kiểm tra")} style={{ width: 280, flex: "none", display: "flex", flexDirection: "column", background: tokens.colorNeutralBackground2, borderRight: line }}>
        <div style={{ display: "flex", alignItems: "center", gap: 6, height: 40, flex: "none", padding: "0 8px 0 12px", borderBottom: line }}>
          <b>{t("Bài kiểm tra")}</b><span style={{ flex: 1 }} />
          <Button size="small" onClick={openTest}>{t("Mở…")}</Button>
          <Button size="small" onClick={() => useUi.getState().request("design")}>{t("Soạn mới…")}</Button>
        </div>
        <div style={{ flex: 1, overflow: "auto", padding: 6, display: "grid", gap: 2, alignContent: "start" }}>
          {tests.map((x) => (
            <div key={x.path} role="button" tabIndex={0} aria-pressed={x.path === cur} onClick={() => setCur(x.path)} onKeyDown={(e) => e.key === "Enter" && setCur(x.path)} title={x.path}
              style={{ display: "flex", alignItems: "center", gap: 6, minHeight: 46, padding: "0 4px 0 10px", borderRadius: 4, cursor: "pointer", background: x.path === cur ? tokens.colorBrandBackground2 : undefined }}>
              <span style={{ flex: 1, minWidth: 0, display: "grid", lineHeight: "17px" }}>
                <b style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{x.name}</b>
                <span style={{ fontSize: 12, color: mut }}>{x.n} {t("câu")}{run?.path === x.path ? ` · ${run.done ? t("đã kết thúc") : t("đang làm")}` : ""}</span>
              </span>
              <Button size="small" appearance="subtle" icon={<Dismiss16Regular />} aria-label={`${t("Bỏ khỏi danh sách")} ${x.name}`} disabled={run?.path === x.path && !run.done}
                onClick={(e) => { e.stopPropagation(); useTest.getState().drop(x.path); if (cur === x.path) setCur(null); }} />
            </div>
          ))}
          {!tests.length && <span style={{ padding: 8, color: mut }}>{t("Bấm Mở… để chọn tệp đề (.json) hoặc Soạn mới…")}</span>}
        </div>
      </aside>

      <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8, height: 40, flex: "none", padding: "0 8px 0 14px", borderBottom: line }}>
          <b style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{paper?.title ?? t("Đánh giá")}</b>
          {mine && <span style={{ fontSize: 12, padding: "1px 6px", borderRadius: 3, background: mine.done ? tokens.colorNeutralBackground3 : "#E6F2EA", color: mine.done ? undefined : "#0B5C2E" }}>
            {mine.done ? t("Đã kết thúc") : left === null ? t("Đang làm") : `${t("Đang làm")} · ${t("còn")} ${Math.floor(left / 60)}:${String(left % 60).padStart(2, "0")}`}
          </span>}
          {paper && <span style={{ fontSize: 12, color: mut }}>{t("Câu")} {qi + 1} / {paper.questions.length}</span>}
          <span style={{ flex: 1 }} />
          {paper && (!run || run.done) && <>
            <Input size="small" type="number" style={{ width: 80 }} aria-label={t("Số phút (0 = không giới hạn)")} value={minutes} onChange={(_, d) => setMinutes(d.value)} contentAfter={<span style={{ fontSize: 12 }}>{t("phút")}</span>} />
            <Button size="small" appearance="primary" onClick={() => go(useTest.getState().start(cur!, Math.max(0, Math.min(600, Number(minutes) || 0))).then((n) => say(`${t("Đã giao bài cho")} ${n} ${t("máy")}`)))}>{t("Giao bài")}</Button>
          </>}
          {mine && !mine.done && <Button size="small" appearance="primary" onClick={() => go(useTest.getState().finish().then(() => say(t("Đã thu bài"))))}>{t("Kết thúc và thu bài")}</Button>}
          {mine && <Button size="small" disabled={!scores.length} onClick={() => go(useTest.getState().exportCsv())}>{t("Xuất kết quả…")}</Button>}
          <Button size="small" appearance="subtle" icon={<Dismiss16Regular />} aria-label={t("Đóng")} onClick={onClose} />
        </div>
        {q ? (
          <div style={{ flex: 1, minHeight: 0, overflow: "auto", padding: "18px 22px", display: "flex", flexDirection: "column", gap: 12 }}>
            <span style={{ color: mut, fontSize: 12 }}>{t("Câu")} {qi + 1} · {t(KIND[Object.keys(q.kind)[0]] ?? "")} · {q.points} {t("điểm")}</span>
            <span style={{ fontSize: 17, lineHeight: "25px", fontWeight: 600, whiteSpace: "pre-wrap" }}>{q.text}</span>
            <div style={{ display: "grid", gap: 8, maxWidth: 620 }}>
              {answerOf(q).map(([a, ok], i) => (
                <div key={i} style={{ display: "flex", alignItems: "center", gap: 8, minHeight: 40, padding: "0 12px", borderRadius: 4, border: `1px solid ${ok ? "#9BC7AC" : tokens.colorNeutralStroke2}`, background: ok ? "#F1F8F3" : undefined }}>
                  {"Choice" in q.kind && <b style={{ width: 18 }}>{String.fromCharCode(65 + i)}</b>}
                  <span style={{ flex: 1 }}>{t(a)}</span>
                  {ok && "Choice" in q.kind && <span style={{ fontSize: 12, color: GREEN }}>{t("Đáp án đúng")}</span>}
                </div>
              ))}
            </div>
            {mine && (
              <div style={{ display: "flex", alignItems: "center", gap: 10, fontSize: 12, color: mut }}>
                <span>{scores.length} / {rows.length} {t("đã nộp")}</span>
                {scores.length > 0 && <>
                  <span>·</span><span>{right} {t("làm đúng câu này")}</span>
                  <span style={{ width: 160, height: 6, borderRadius: 3, background: tokens.colorNeutralBackground3 }}><span style={{ display: "block", width: pct(right, scores.length), height: 6, borderRadius: 3, background: GREEN }} /></span>
                </>}
              </div>
            )}
            <div style={{ display: "flex", gap: 8, marginTop: "auto" }}>
              <Button disabled={qi === 0} onClick={() => setQi(qi - 1)}>{t("Câu trước")}</Button>
              <Button disabled={!paper || qi >= paper.questions.length - 1} onClick={() => setQi(qi + 1)}>{t("Câu sau")}</Button>
            </div>
          </div>
        ) : (
          <span style={{ padding: 22, color: mut }}>{cur ? t("Đang mở đề…") : t("Chọn một bài kiểm tra bên trái")}</span>
        )}
      </div>

      <aside aria-label={t("Học sinh")} style={{ width: 320, flex: "none", display: "flex", flexDirection: "column", borderLeft: line }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8, height: 40, flex: "none", padding: "0 12px", borderBottom: line, fontSize: 12, color: mut }}>
          <b style={{ flex: 1, fontSize: 13, color: tokens.colorNeutralForeground1 }}>{t("Học sinh")}</b><span style={{ width: 96 }}>{t("Trạng thái")}</span><span style={{ width: 60, textAlign: "right" }}>{t("Điểm")}</span>
        </div>
        <div style={{ flex: 1, overflow: "auto" }}>
          {rows.map((s) => {
            const sc = mine?.scores[s.id];
            return (
              <div key={s.id} style={{ display: "flex", alignItems: "center", gap: 8, height: 28, padding: "0 12px", borderBottom: line }}>
                <span style={{ width: 8, height: 8, borderRadius: 4, flex: "none", background: s.online ? GREEN : "#9E9E9E" }} />
                <span style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{s.user || s.hostname}</span>
                <span style={{ width: 96, whiteSpace: "nowrap", fontSize: 12, color: sc ? GREEN : mut }}>{!mine ? "" : sc ? t("Đã nộp") : mine.done ? t("Chưa nộp") : t("Đang làm")}</span>
                <span style={{ width: 60, textAlign: "right", fontVariantNumeric: "tabular-nums" }}>{sc ? `${+sc.points.toFixed(2)}/${+sc.max.toFixed(2)}` : "–"}</span>
              </div>
            );
          })}
        </div>
      </aside>
    </div>
  );
}
