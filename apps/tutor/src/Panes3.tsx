import { useEffect, useRef, useState } from "react";
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, Field, Input, Select, Textarea, tokens } from "@fluentui/react-components";
import { invoke } from "@tauri-apps/api/core";
import { useClass } from "./store";
import { tr, useLang, useT } from "./i18n";
import { D, DlgHead, mut } from "./FileDist";

const line = `1px solid ${tokens.colorNeutralStroke2}`;

// ================= Keyboard shortcuts (design N21): the list App's key handler uses, so the dialog never lies
/** [what it does, key, ribbon command it runs] */
export const KEYS: [string, string, string][] = [
  ["Chọn hết", "Ctrl+A", "selectAll"],
  ["Bỏ chọn", "Esc", "selectNone"],
  ["Khoá", "Ctrl+L", "lock"],
  ["Mở khoá", "Ctrl+U", "unlock"],
  ["Gửi tin nhắn", "Ctrl+M", "msg"],
  ["Nhóm mới từ máy đang chọn", "Ctrl+G", "assignGroup"],
  ["Tiết học mới", "Ctrl+N", "wizard"],
  ["Làm mới danh sách", "F5", "refresh"],
  ["Phím tắt", "F1", "shortcuts"],
];
/** "Ctrl+L" for a key event, matching the KEYS spelling. */
export const keyName = (e: KeyboardEvent) => (e.ctrlKey || e.metaKey ? "Ctrl+" : "") + (e.key === "Escape" ? "Esc" : e.key.length === 1 ? e.key.toUpperCase() : e.key);

export function Shortcuts({ onClose }: { onClose: () => void }) {
  const t = useT();
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ ...D.surface, maxWidth: 460 }}>
        <DialogBody style={D.body}>
          <DlgHead title={t("Phím tắt")} onClose={onClose} />
          <DialogContent style={{ ...D.content, gap: 0 }}>
            {KEYS.map(([l, k]) => (
              <div key={k} style={{ display: "flex", alignItems: "center", height: 34, borderBottom: line }}>
                <span style={{ flex: 1 }}>{t(l)}</span>
                <kbd style={{ fontFamily: "inherit", fontSize: 12, padding: "1px 7px", border: `1px solid ${tokens.colorNeutralStroke1}`, borderBottomWidth: 2, borderRadius: 4, background: tokens.colorNeutralBackground2 }}>{k}</kbd>
              </div>
            ))}
            <span style={{ marginTop: 10, fontSize: 12, color: mut }}>{t("Phím tắt không chạy khi đang gõ chữ hoặc đang mở một hộp thoại.")}</span>
          </DialogContent>
          <DialogActions style={D.foot}><Button appearance="primary" onClick={onClose}>{t("Đóng")}</Button></DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

// ================= Timetable (design N21): when a period starts, its lesson becomes the lesson title
type TT = { times: string[]; cells: string[][] }; // cells[period][weekday 0 = Monday]
const DAYS = ["Thứ Hai", "Thứ Ba", "Thứ Tư", "Thứ Năm", "Thứ Sáu", "Thứ Bảy"];
const EMPTY: TT = { times: ["07:15", "08:05", "09:10", "10:00", "13:30"], cells: Array.from({ length: 5 }, () => Array(6).fill("")) };
async function loadTT(): Promise<TT> {
  try { const s = await invoke<string | null>("load_doc", { name: "timetable" }); if (s) return JSON.parse(s); } catch { /* browser dev */ }
  try { return JSON.parse(localStorage.getItem("erg.timetable") ?? "") as TT; } catch { return EMPTY; }
}
const saveTT = (tt: TT) => { const json = JSON.stringify(tt); localStorage.setItem("erg.timetable", json); return invoke("save_doc", { name: "timetable", json }).catch(() => {}); };
const hhmm = (s: string) => /^([01]?\d|2[0-3]):[0-5]\d$/.test(s.trim());

/** Runs in App: at the start of a period with a lesson, the lesson panel takes that title and the teacher is told. */
export function useTimetable(say: (s: string) => void) {
  const sayRef = useRef(say);
  sayRef.current = say;
  useEffect(() => { // empty deps: App re-renders constantly, and a restarted interval would never reach its first tick
    const t = (vi: string) => tr(useLang.getState().lang, vi);
    let fired = "";
    const tick = async () => {
      const now = new Date(), day = (now.getDay() + 6) % 7; // 0 = Monday
      const at = `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;
      const tt = await loadTT();
      const p = tt.times.findIndex((x) => x.trim().padStart(5, "0") === at);
      const lesson = p >= 0 ? tt.cells[p]?.[day]?.trim() : "";
      const key = `${now.toDateString()} ${at}`;
      if (!lesson || fired === key) return;
      fired = key;
      let doc: Record<string, unknown> = {};
      try { doc = JSON.parse((await invoke<string | null>("load_doc", { name: "lesson" })) ?? "{}"); } catch { /* new document */ }
      await invoke("save_doc", { name: "lesson", json: JSON.stringify({ ...doc, title: lesson }) }).catch(() => {});
      sayRef.current(`${t("Tiết")} ${p + 1} ${t("bắt đầu")}: ${lesson}`);
    };
    const iv = setInterval(() => void tick(), 20_000);
    return () => clearInterval(iv);
  }, []);
}

export function Timetable({ onClose, say }: { onClose: () => void; say: (s: string) => void }) {
  const t = useT();
  const [tt, setTt] = useState<TT | null>(null);
  useEffect(() => { void loadTT().then(setTt); }, []);
  if (!tt) return null;
  const setCell = (p: number, d: number, v: string) => setTt({ ...tt, cells: tt.cells.map((r, i) => (i === p ? r.map((c, j) => (j === d ? v : c)) : r)) });
  const bad = tt.times.some((x) => !hhmm(x));
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ ...D.surface, maxWidth: 1000, width: "92vw" }}>
        <DialogBody style={D.body}>
          <DlgHead title={t("Thời khoá biểu")} sub={t("Khi tới giờ một tiết, tên bài của tiết đó tự điền vào Chi tiết tiết học")} onClose={onClose} />
          <DialogContent style={{ ...D.content, overflow: "auto" }}>
            <table style={{ borderCollapse: "collapse", width: "100%", tableLayout: "fixed" }}>
              <thead><tr>
                <th style={{ width: 120, textAlign: "left", fontWeight: 400, fontSize: 12, color: mut, padding: "0 6px 6px" }}>{t("Tiết · giờ bắt đầu")}</th>
                {DAYS.map((d) => <th key={d} style={{ textAlign: "left", fontWeight: 600, padding: "0 6px 6px" }}>{t(d)}</th>)}
              </tr></thead>
              <tbody>{tt.cells.map((row, p) => (
                <tr key={p}>
                  <td style={{ padding: 4, display: "flex", alignItems: "center", gap: 6 }}>
                    <b style={{ width: 14 }}>{p + 1}</b>
                    <Input size="small" style={{ width: 80 }} aria-label={`${t("Giờ bắt đầu tiết")} ${p + 1}`} value={tt.times[p]} placeholder="07:15"
                      appearance={hhmm(tt.times[p]) ? "outline" : "underline"} onChange={(_, d) => setTt({ ...tt, times: tt.times.map((x, i) => (i === p ? d.value : x)) })} />
                  </td>
                  {row.map((c, d) => (
                    <td key={d} style={{ padding: 4 }}>
                      <Input size="small" style={{ width: "100%", background: c ? tokens.colorBrandBackground2 : undefined }} aria-label={`${t(DAYS[d])}, ${t("tiết")} ${p + 1}`} value={c} onChange={(_, v) => setCell(p, d, v.value)} />
                    </td>
                  ))}
                </tr>
              ))}</tbody>
            </table>
            <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <Button size="small" disabled={tt.cells.length >= 12} onClick={() => setTt({ times: [...tt.times, ""], cells: [...tt.cells, Array(6).fill("")] })}>{t("Thêm tiết")}</Button>
              <Button size="small" appearance="subtle" disabled={tt.cells.length <= 1} onClick={() => setTt({ times: tt.times.slice(0, -1), cells: tt.cells.slice(0, -1) })}>{t("Bỏ tiết cuối")}</Button>
              {bad && <span style={{ fontSize: 12, color: tokens.colorPaletteRedForeground1 }}>{t("Giờ bắt đầu phải có dạng 07:15")}</span>}
            </div>
          </DialogContent>
          <DialogActions style={D.foot}>
            <Button appearance="primary" disabled={bad} onClick={() => void saveTT(tt).then(() => { say(t("Đã lưu thời khoá biểu")); onClose(); })}>{t("Lưu")}</Button>
            <Button onClick={onClose}>{t("Huỷ")}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

// ================= About (design N21)
export function About({ onClose, say }: { onClose: () => void; say: (s: string) => void }) {
  const t = useT();
  const [ver, setVer] = useState("");
  const room = useClass((s) => s.room);
  const students = useClass((s) => s.students);
  useEffect(() => { invoke<string>("app_version").then(setVer, () => {}); }, []);
  const online = Object.values(students).filter((s) => s.online).length;
  const rows: [string, string][] = [
    [t("Phiên bản"), `Tutor ${ver || "–"}`],
    [t("Phòng"), `${room || Object.values(students).find((s) => s.room)?.room || t("mọi phòng")} · ${online} ${t("máy trực tuyến")}`],
    [t("Giấy phép"), "Apache-2.0"],
    [t("Mã nguồn"), "github.com/eduriseglobalvn/erg-classroom"],
  ];
  const copy = () => navigator.clipboard.writeText(["ERG Classroom", ...rows.map(([a, b]) => `${a}: ${b}`)].join("\n")).then(() => say(t("Đã chép thông tin")), () => {});
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ ...D.surface, maxWidth: 480 }}>
        <DialogBody style={D.body}>
          <DlgHead title={t("Giới thiệu ERG Classroom")} onClose={onClose} />
          <DialogContent style={{ ...D.content, gap: 14 }}>
            <div style={{ display: "flex", gap: 14, alignItems: "center" }}>
              <img src="/erg.png" alt="" width={48} height={48} style={{ borderRadius: 10 }} />
              <span style={{ display: "grid" }}><b style={{ fontSize: 18 }}>ERG Classroom</b><span style={{ color: mut }}>{t("Quản lý lớp học cho phòng máy trường học. Miễn phí, mã nguồn mở.")}</span></span>
            </div>
            <div style={{ border: line, borderRadius: 4 }}>
              {rows.map(([a, b]) => (
                <div key={a} style={{ display: "flex", gap: 12, minHeight: 32, alignItems: "center", padding: "0 12px", borderBottom: line }}>
                  <span style={{ width: 110, color: mut }}>{a}</span><span style={{ flex: 1, minWidth: 0, wordBreak: "break-word" }}>{b}</span>
                </div>
              ))}
            </div>
          </DialogContent>
          <DialogActions style={D.foot}>
            <Button appearance="subtle" style={{ marginRight: "auto" }} onClick={copy}>{t("Chép thông tin")}</Button>
            <Button appearance="primary" onClick={onClose}>OK</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

// ================= Send a message (design N10): saved templates and how long it stays on screen
const TPL = ["Cả lớp nhìn lên bảng.", "Còn 5 phút, các em lưu bài.", "Lưu bài và đóng các cửa sổ khác.", "Đeo tai nghe vào."];
const SECS = [10, 20, 60, 300, 600]; // the student clamps to 1..600 s
const loadTpl = (): string[] => { try { const v = JSON.parse(localStorage.getItem("erg.msgTemplates") ?? ""); if (Array.isArray(v)) return v.filter((x) => typeof x === "string"); } catch { /* first run */ } return TPL; };

export function SendMessage({ onClose, say, fail }: { onClose: () => void; say: (s: string) => void; fail: (s: string) => void }) {
  const t = useT();
  const send = useClass((s) => s.send);
  const picked = useClass((s) => s.selected.size);
  const [text, setText] = useState("");
  const [secs, setSecs] = useState(20);
  const [tpl, setTpl] = useState(loadTpl);
  const keep = (l: string[]) => { setTpl(l); localStorage.setItem("erg.msgTemplates", JSON.stringify(l)); };
  const msg = text.trim();
  const go = () => send({ Message: { text: msg, seconds: secs } }).then((n) => { say(`${t("Đã gửi tin nhắn tới")} ${n} ${t("máy")}`); onClose(); }, (e) => fail(String(e)));
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ ...D.surface, maxWidth: 560 }}>
        <DialogBody style={D.body}>
          <DlgHead title={t("Gửi tin nhắn")} sub={picked ? `${picked} ${t("máy đang chọn")}` : t("Cả lớp")} onClose={onClose} />
          <DialogContent style={{ ...D.content, gap: 12 }}>
            <Field label={t("Nội dung")}>
              <Textarea autoFocus rows={3} maxLength={500} value={text} onChange={(_, d) => setText(d.value)}
                onKeyDown={(e) => { if (e.key === "Enter" && e.ctrlKey && msg) void go(); }} />
            </Field>
            <div style={{ display: "grid", gap: 4 }}>
              <span style={{ fontSize: 12, color: mut }}>{t("Mẫu soạn sẵn")}</span>
              <div style={{ border: line, borderRadius: 4, maxHeight: 160, overflowY: "auto" }}>
                {tpl.map((x) => (
                  <div key={x} style={{ display: "flex", alignItems: "center", borderBottom: line }}>
                    <Button appearance="subtle" style={{ flex: 1, justifyContent: "flex-start", fontWeight: 400, minWidth: 0 }} onClick={() => setText(t(x))}>{t(x)}</Button>
                    <Button appearance="subtle" size="small" aria-label={`${t("Xoá mẫu")}: ${t(x)}`} onClick={() => keep(tpl.filter((y) => y !== x))}>×</Button>
                  </div>
                ))}
              </div>
            </div>
            <div style={{ display: "flex", alignItems: "end", gap: 12 }}>
              <Field label={t("Hiện trên màn hình")}>
                <Select value={String(secs)} onChange={(_, d) => setSecs(Number(d.value))}>
                  {SECS.map((s) => <option key={s} value={s}>{s < 60 ? `${s} ${t("giây")}` : `${s / 60} ${t("phút")}`}</option>)}
                </Select>
              </Field>
              <Button disabled={!msg || tpl.includes(msg)} onClick={() => keep([...tpl, msg])}>{t("Lưu làm mẫu")}</Button>
            </div>
          </DialogContent>
          <DialogActions style={D.foot}>
            <Button appearance="primary" disabled={!msg} onClick={() => void go()}>{t("Gửi")}</Button>
            <Button onClick={onClose}>{t("Huỷ")}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
