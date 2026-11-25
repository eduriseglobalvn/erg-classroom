import { GuardDialog } from "./Guard";
import { UpdateBar } from "./Update";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import {
  Button, Checkbox, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle, Input, Menu,
  MessageBar, MessageBarActions, MessageBarBody, Select, Slider, Text, makeStyles, mergeClasses, shorthands, tokens,
} from "@fluentui/react-components";
import {
  Desktop24Regular, Desktop20Regular, Globe20Regular, Add20Regular, ScanCamera20Regular, DocumentEdit20Regular, Board20Regular, Edit20Regular,
  History20Regular, Folder20Regular, PanelRight20Regular, Dismiss16Regular,
} from "@fluentui/react-icons";
import { listen } from "@tauri-apps/api/event";
import { type Student, useClass } from "./store";
import { Ribbon, useCommands } from "./Ribbon";
import { SendWork, WebPane } from "./Panes";
import { AlertsPanel, AssessPane, Settings, WebRules } from "./Panes2";
import { About, KEYS, Shortcuts, Timetable, keyName, useTimetable } from "./Panes3";
import { MenuBar, MenuPop } from "./MenuBar";
import { ordered, useLayout } from "./layout";
import { Wall } from "./Wall";
import { LessonPanel } from "./LessonPanel";
import { sortStudents } from "./sort";
import { useLang, useT } from "./i18n";
import { type Req, useUi } from "./uistate";
import { type Item, Tip } from "./ui";

const GAP = 10;
const PAD = 16; // wall padding, same value as Wall.tsx
const MAXW = 460; // widest tile Auto-fit will make
const CAPTION = 24; // px of name tag under a 16:9 picture, used by Auto-fit

// Flat Windows layout: tonal layers and 1px strokes, no cards or shadows.
const stroke = `1px solid ${tokens.colorNeutralStroke2}`;

const useStyles = makeStyles({
  root: {
    display: "flex", flexDirection: "column", height: "100%", color: tokens.colorNeutralForeground1, fontSize: "13px",
    background: `color-mix(in srgb, ${tokens.colorNeutralBackground3} 88%, transparent)`, overflow: "hidden",
    // v2 accent (navy) for everything inside the window; the dark theme keeps Fluent's own brand colours
    "@media (prefers-color-scheme: light)": {
      "--colorBrandBackground": "#1F4E9E", "--colorBrandBackgroundHover": "#1A4387", "--colorBrandBackgroundPressed": "#173B78",
      "--colorBrandStroke1": "#1F4E9E", "--colorBrandStroke2": "#B9CDEE", "--colorBrandBackground2": "#E8EFFA",
      "--colorBrandForeground1": "#1F4E9E", "--colorBrandForeground2": "#173B78",
      "--colorCompoundBrandBackground": "#1F4E9E", "--colorCompoundBrandBackgroundHover": "#1A4387", "--colorCompoundBrandBackgroundPressed": "#173B78",
      "--colorCompoundBrandStroke": "#1F4E9E", "--colorCompoundBrandStrokeHover": "#1A4387",
    },
  },
  title: { height: "32px", flex: "0 0 auto", display: "flex", alignItems: "center", padding: "0 8px 0 10px" },
  // logo + menus on the left, language on the right: equal sides keep the class title centred until the menus need the room
  titleSide: { flex: "1 1 0", minWidth: "max-content", display: "flex", alignItems: "center", gap: "8px" },
  room: { flex: "0 1 auto", padding: "0 16px", textAlign: "center", fontSize: "12px", color: tokens.colorNeutralForeground3 },
  spacer: { flex: 1 },
  body: { flex: 1, minHeight: 0, display: "flex" },
  rail: { width: "48px", flex: "0 0 auto", display: "flex", flexDirection: "column", alignItems: "center", gap: "2px", padding: "6px 0", borderRight: stroke },
  railBtn: {
    position: "relative", width: "40px", height: "40px", display: "grid", placeItems: "center", ...shorthands.border("0"), borderRadius: tokens.borderRadiusMedium,
    background: "transparent", color: tokens.colorNeutralForeground2, cursor: "pointer",
    ":hover": { background: tokens.colorSubtleBackgroundHover }, ":active": { background: tokens.colorSubtleBackgroundPressed },
    ":focus-visible": { outline: `2px solid ${tokens.colorStrokeFocus2}`, outlineOffset: "-2px" },
    ":disabled": { opacity: 0.35, cursor: "default", background: "transparent" },
  },
  railOn: {
    background: tokens.colorSubtleBackgroundSelected, color: tokens.colorBrandForeground1,
    "::before": { content: '""', position: "absolute", left: 0, top: "11px", bottom: "11px", width: "3px", borderRadius: "2px", background: tokens.colorBrandBackground },
  },
  main: { flex: 1, minWidth: 0, display: "flex", flexDirection: "column" },
  tabs: { display: "flex", alignItems: "center", gap: "8px", height: "40px", boxSizing: "border-box", padding: "0 12px", flex: "0 0 auto", borderBottom: stroke, background: tokens.colorNeutralBackground2 },
  tabScroll: { display: "flex", alignItems: "center", gap: "2px", overflowX: "auto", scrollbarWidth: "none", minWidth: 0 },
  tab: {
    display: "inline-flex", alignItems: "center", gap: "6px", height: "28px", flex: "0 0 auto", padding: "0 12px", cursor: "pointer", whiteSpace: "nowrap",
    background: "transparent", color: tokens.colorNeutralForeground1, fontFamily: "inherit", fontSize: "13px",
    ...shorthands.border("1px", "solid", "transparent"), borderRadius: "4px",
    ":hover": { background: tokens.colorSubtleBackgroundHover }, ":focus-visible": { outline: `2px solid ${tokens.colorStrokeFocus2}`, outlineOffset: "-2px" },
  },
  tabOn: {
    background: tokens.colorNeutralBackground1, color: tokens.colorNeutralForeground1, fontWeight: 600, ...shorthands.borderColor(tokens.colorNeutralStroke2),
    ":hover": { background: tokens.colorNeutralBackground1 },
  },
  count: { fontWeight: 400, color: tokens.colorNeutralForeground3 },
  tabSide: { display: "flex", alignItems: "center", gap: "4px", whiteSpace: "nowrap", color: tokens.colorNeutralForeground3, fontSize: "12px" },
  center: { flex: 1, minHeight: 0, display: "flex" },
  detail: { width: "300px", flex: "0 0 auto", display: "flex", flexDirection: "column", background: tokens.colorNeutralBackground2, borderLeft: stroke },
  detailHead: { display: "flex", alignItems: "center", gap: "8px", height: "40px", boxSizing: "border-box", flex: "0 0 auto", padding: "0 6px 0 12px", borderBottom: stroke },
  detailBody: { flex: 1, minHeight: 0, overflow: "auto", padding: "12px", display: "flex", flexDirection: "column", gap: "12px" },
  btnRow: { display: "flex", gap: "6px", "& > button": { flex: "1 1 0", minWidth: 0 } },
  kvBox: { borderRadius: "4px", background: tokens.colorNeutralBackground1, ...shorthands.border("1px", "solid", tokens.colorNeutralStroke2) },
  dot: { width: "8px", height: "8px", borderRadius: "999px", flex: "0 0 auto" },
  pic: { position: "relative", aspectRatio: "16 / 9", background: tokens.colorNeutralBackground3, display: "grid", placeItems: "center", color: tokens.colorNeutralForeground3, overflow: "hidden", outline: `1px solid ${tokens.colorNeutralStroke1}` },
  img: { position: "absolute", inset: 0, width: "100%", height: "100%", objectFit: "cover", pointerEvents: "none" },
  ell: { overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", minWidth: 0 },
  kv: { display: "flex", alignItems: "center", gap: "8px", height: "30px", padding: "0 10px", borderBottom: `1px solid ${tokens.colorNeutralStroke3}`, ":last-child": { borderBottomStyle: "none" } },
  kvKey: { width: "86px", flex: "0 0 auto", color: tokens.colorNeutralForeground3 },
  bar: { display: "flex", alignItems: "center", gap: "14px", height: "40px", boxSizing: "border-box", padding: "0 12px", flex: "0 0 auto", background: tokens.colorNeutralBackground2, borderTop: stroke, overflow: "hidden", "& > .fui-Checkbox": { flex: "none" } },
  hint: { fontSize: "12px", color: tokens.colorNeutralForeground3, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", minWidth: 0 },
  barItem: { display: "flex", alignItems: "center", gap: "8px", whiteSpace: "nowrap" },
  status: { display: "flex", alignItems: "center", gap: "16px", height: "24px", padding: "0 12px", fontSize: "12px", flex: "0 0 auto", whiteSpace: "nowrap", color: tokens.colorNeutralForegroundOnBrand, background: tokens.colorBrandBackground },
  statusErr: { background: tokens.colorPaletteRedBackground3 },
  sep: { width: "1px", height: "12px", background: "currentColor", opacity: 0.4 },
});

function useAsk(title: string, label: string, onOk: (v: string) => void) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const [v, setV] = useState("");
  const dlg = (
    <Dialog open={open} onOpenChange={(_, d) => setOpen(d.open)}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>{title}</DialogTitle>
          <DialogContent>
            <Input autoFocus placeholder={label} value={v} onChange={(_, d) => setV(d.value)} style={{ width: "100%" }} />
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={() => setOpen(false)}>{t("Hủy")}</Button>
            <Button appearance="primary" disabled={!v.trim()} onClick={() => { onOk(v.trim()); setV(""); setOpen(false); }}>{t("Gửi")}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
  return [dlg, () => setOpen(true)] as const;
}

export function App() {
  const t = useT();
  const s = useStyles();
  const { students, selected, toggle, selectAll, clear, send, init, showing } = useClass();
  const { lang, toggle: toggleLang } = useLang();
  const ui = useUi();
  const [err, setErr] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const run = (p: Promise<unknown>) => p.then(() => setErr(null), (e) => setErr(String(e)));
  // notices fade from the status bar after a while
  useEffect(() => {
    if (!note && !err) return;
    const h = setTimeout(() => { setNote(null); setErr(null); }, 9000);
    return () => clearTimeout(h);
  }, [note, err]);
  useEffect(() => {
    const un = listen<{ id: string; text: string }>("student-chat", (e) => {
      const st = useClass.getState().students[e.payload.id];
      setNote(`${st?.hostname || "Student"}: ${e.payload.text}`);
    });
    return () => void un.then((f) => f());
  }, []);
  const [poll, setPoll] = useState<{ id: number; q: string; o: string[]; ans: Record<string, number> } | null>(null);
  useEffect(() => {
    const un = listen<{ id: string; poll: number; choice: number }>("student-poll-answer", (e) =>
      setPoll((p) => (p && p.id === e.payload.poll ? { ...p, ans: { ...p.ans, [e.payload.id]: e.payload.choice } } : p)));
    return () => void un.then((f) => f());
  }, []);
  useEffect(() => {
    let off: (() => void) | undefined;
    init().then((f) => (off = f));
    return () => off?.();
  }, [init]);

  const lay = useLayout();
  useEffect(() => { void lay.load(); /* eslint-disable-next-line react-hooks/exhaustive-deps */ }, []);
  const [helpTab, setHelpTab] = useState(false);
  const all = Object.values(students);
  const allHosts = all.map((x) => x.hostname);
  const groupNames = [...new Set(Object.values(lay.groups))].sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));
  const helpCount = all.filter((x) => x.help).length;
  // saved seating first; then the group / help filter narrows what is shown
  // auto-arrange = sorted order; otherwise the saved (manual) seating
  const base = ui.autoArrange ? sortStudents(all, ui.sortKey, ui.sortDir, lay.groups) : ordered(all, lay.order);
  const list = base.filter((x) => (helpTab ? x.help : !lay.filter || lay.groups[x.hostname] === lay.filter));
  // dragging a tile leaves auto-arrange: the order on screen becomes the saved seating, then the swap applies
  const swapSeats = (from: string, to: string) => {
    if (ui.autoArrange) { useLayout.setState({ order: base.map((x) => x.hostname) }); ui.set({ autoArrange: false }); }
    lay.swap(from, to, allHosts);
  };
  const online = list.filter((x) => x.online).length;
  const room = all.find((x) => x.room)?.room ?? "";

  const [linkDlg, askLink] = useAsk("Open a link on student PCs", "https://…", (url) => {
    if (/^https?:\/\//i.test(url)) send({ OpenUrl: url });
  });
  const one = selected.size === 1 ? students[[...selected][0]] : undefined;
  const [infoOpen, setInfoOpen] = useState(false);
  const showInfo = () => { setInfoOpen(true); send("RequestInventory"); };
  const gb = (n: number) => (n / 2 ** 30).toFixed(1) + " GB";
  const [chatDlg, askChat] = useAsk("Chat with students", "Message", (text) => send({ Chat: text }));
  // "Question | option 1, option 2, ..."
  const [pollDlg, askPoll] = useAsk("Quick survey: Question | A, B, C", "2+2 = ? | 3, 4, 5", (t) => {
    const [q, rest = ""] = t.split("|");
    const o = rest.split(",").map((x) => x.trim()).filter(Boolean);
    if (!q.trim() || o.length < 2 || o.length > 8) return setErr("A question and 2–8 options are needed");
    const id = Date.now() % 2 ** 31;
    setPoll({ id, q: q.trim(), o, ans: {} });
    send({ Poll: { id, question: q.trim(), options: o } });
  });
  const [msgDlg, askMsg] = useAsk("Send a message", "Message text", (text) => send({ Message: { text, seconds: 20 } }));
  // details pane: the same Message command, typed in place
  const [quick, setQuick] = useState("");
  const sendQuick = () => { if (quick.trim()) { send({ Message: { text: quick.trim(), seconds: 20 } }); setQuick(""); } };

  const items = (t: string) => t.split(/[\s,;]+/).filter(Boolean);
  const [guardOpen, setGuardOpen] = useState(false);
  const [workOpen, setWorkOpen] = useState(false);
  const [pane, setPane] = useState<"class" | "web" | "assess">("class");
  const [rulesOpen, setRulesOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [extra, setExtra] = useState<null | "shortcuts" | "timetable" | "about">(null);
  const [webAllowDlg, askWebAllow] = useAsk("Allow only these websites (comma separated)", "school.edu.vn, wikipedia.org", (t) => send({ SetWebPolicy: { mode: "AllowListOnly", list: items(t) } }));
  const [webBlockDlg, askWebBlock] = useAsk("Block these websites (comma separated)", "facebook.com, youtube.com", (t) => send({ SetWebPolicy: { mode: "BlockList", list: items(t) } }));
  const [appBlockDlg, askAppBlock] = useAsk("Block applications (.exe file names)", "game.exe, chrome.exe", (t) => send({ SetAppPolicy: { mode: "BlockList", list: items(t) } }));
  const [appAllowDlg, askAppAllow] = useAsk("Allow only these applications (.exe file names)", "winword.exe, excel.exe", (t) => send({ SetAppPolicy: { mode: "AllowListOnly", list: items(t) } }));
  const cmds = useCommands({
    say: setNote, fail: setErr, run, info: showInfo,
    ask: { link: askLink, msg: askMsg, chat: askChat, poll: askPoll, webAllow: askWebAllow, webBlock: askWebBlock, guard: () => setGuardOpen(true), appBlock: askAppBlock, appAllow: askAppAllow, work: () => setWorkOpen(true) },
    openWeb: () => setPane("web"),
    openAssess: () => setPane("assess"), settings: () => setSettingsOpen(true), rules: () => setRulesOpen(true),
    shortcuts: () => setExtra("shortcuts"), timetable: () => setExtra("timetable"), about: () => setExtra("about"),
    help: { count: helpCount, on: helpTab, toggle: () => { setHelpTab(!helpTab); lay.setFilter(null); } },
  });

  useTimetable(setNote);
  // ---- keyboard shortcuts (the Shortcuts dialog lists the same KEYS); not while typing or in a dialog
  const keyCmds = useRef(cmds.c);
  keyCmds.current = cmds.c;
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement;
      if (e.altKey || el.closest("input, textarea, select, [contenteditable=true], [role=dialog], [role=menu]")) return;
      const k = KEYS.find(([, name]) => name === keyName(e));
      const c = k && (keyCmds.current as Record<string, { run: () => unknown; off?: boolean }>)[k[2]];
      if (!c || c.off) return;
      e.preventDefault();
      void c.run();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // ---- thumbnail wall sizing: fixed slider size, or Auto-fit so every tile is visible without scrolling
  const wallRef = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState({ w: 0, h: 0 });
  useLayoutEffect(() => {
    const el = wallRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setBox({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const n = Math.max(list.length, 1);
  let cols = 0;
  if (ui.autoFit && ui.view === "thumbs" && box.w > 0) {
    const W = box.w - PAD * 2, H = box.h - PAD * 2;
    // from one column up (a class of two must fit too); a tile never grows past MAXW
    for (let c = 1; c <= n; c++) {
      const w = Math.min(MAXW, (W - (c - 1) * GAP) / c);
      if (w < 80) { cols = c - 1 || 1; break; }
      cols = c;
      if (Math.ceil(n / c) * ((w * 9) / 16 + CAPTION + GAP) - GAP <= H) break;
    }
  }
  const gridCols = cols ? `repeat(${cols}, minmax(0, ${MAXW}px))` : `repeat(auto-fill, minmax(${ui.size}px, 1fr))`;

  const [ctx, setCtx] = useState<{ x: number; y: number } | null>(null);
  const pick = (st: Student, e: React.MouseEvent) => toggle(st.id, e.ctrlKey || e.metaKey);
  const zoom = (st: Student) => { toggle(st.id, false); ui.request("remote"); };
  const nav: { k: Req | "class" | "web" | "assess"; icon: React.ReactNode; label: string; off?: boolean }[] = [
    { k: "class", icon: <Desktop20Regular />, label: t("Màn hình học sinh") },
    { k: "web", icon: <Globe20Regular />, label: t("Truy cập web") },
    { k: "scan", icon: <ScanCamera20Regular />, label: t("Giám sát") },
    { k: "assess", icon: <DocumentEdit20Regular />, label: t("Đánh giá") },
    { k: "board", icon: <Board20Regular />, label: t("Bảng tương tác") },
    { k: "whiteboard", icon: <Edit20Regular />, label: t("Bảng trắng") },
    { k: "journal", icon: <History20Regular />, label: t("Nhật ký") },
    { k: "files", icon: <Folder20Regular />, label: t("Tệp máy học sinh"), off: selected.size !== 1 },
  ];
  const tab = (on: boolean, label: string, count: number, onClick: () => void) => (
    <button key={label} type="button" className={mergeClasses(s.tab, on && s.tabOn)} aria-pressed={on} onClick={onClick}>{label}<span className={s.count}>{count}</span></button>
  );
  // a command with nothing selected goes to every student, so say so before the teacher clicks
  const target = one ? one.hostname : selected.size ? `${selected.size} ${t("đã chọn")}` : t("cả lớp");
  const group = one && lay.groups[one.hostname];
  const green = tokens.colorPaletteGreenBackground3, grey = tokens.colorNeutralForegroundDisabled;

  // Right-click on a student. Several selected: what only works on one computer goes, the group commands come in.
  const c = cmds.c, many = selected.size > 1;
  const ctxItems: Item[] = many
    ? [c.chat, c.msg, "-", c.lock, c.unlock, c.blank, c.show, "-", c.distribute, c.sendFile, c.collect, c.link, "-",
      cmds.sub.web, c.mute, c.unmute, cmds.sub.usb, "-", c.assignGroup, c.random, c.reward, "-", cmds.sub.power, c.selectNone]
    : [c.remote, c.chat, c.msg, "-", one?.locked ? c.unlock : c.lock, c.blank, c.show, c.exhibit, "-", c.files, c.sendFile, c.collect, c.link, "-",
      cmds.sub.web, c.mute, c.unmute, cmds.sub.usb, "-", c.reward, c.assignGroup, c.setLeader, "-", cmds.sub.power, c.info];
  const ctxHead = one ? [one.hostname, one.user].filter(Boolean).join(" · ") : `${selected.size} ${t("máy đang chọn")}`;

  return (
    <div className={s.root}>
      <Dialog open={infoOpen} onOpenChange={(_, d) => setInfoOpen(d.open)}>
        <DialogSurface>
          <DialogBody>
            <DialogTitle>{one?.hostname ?? t("Thông tin máy")}</DialogTitle>
            <DialogContent>
              {one?.inventory ? (
                <div>
                  <Text block>{one.inventory.os}</Text>
                  <Text block>{one.inventory.cpu} · {one.inventory.cores} threads</Text>
                  <Text block>RAM {gb(one.inventory.ram_used)} / {gb(one.inventory.ram_total)}</Text>
                  {one.inventory.disks.map(([m, t, f]) => <Text block key={m}>Drive {m}: {gb(f)} free of {gb(t)}</Text>)}
                  <Text block>Up {Math.floor(one.inventory.uptime_s / 3600)} h</Text>
                  <Text block weight="semibold" style={{ marginTop: 8 }}>Running ({one.inventory.processes.length})</Text>
                  <div style={{ maxHeight: 160, overflow: "auto" }}><Text size={200}>{one.inventory.processes.join(", ")}</Text></div>
                </div>
              ) : <Text>{t("Đang tải…")}</Text>}
            </DialogContent>
            <DialogActions><Button onClick={() => setInfoOpen(false)}>{t("Đóng")}</Button></DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>
      {chatDlg}{pollDlg}
      <Dialog open={!!poll} onOpenChange={(_, d) => !d.open && setPoll(null)}>
        <DialogSurface>
          <DialogBody>
            <DialogTitle>{poll?.q}</DialogTitle>
            <DialogContent>
              {poll?.o.map((o, i) => {
                const c = Object.values(poll.ans).filter((x) => x === i).length;
                return <Text block key={i}>{o}: {c}</Text>;
              })}
              <Text block size={200} style={{ marginTop: 8 }}>{Object.keys(poll?.ans ?? {}).length}/{online} answered</Text>
            </DialogContent>
            <DialogActions><Button onClick={() => setPoll(null)}>{t("Đóng")}</Button></DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>
      <GuardDialog open={guardOpen} onClose={() => setGuardOpen(false)} say={setNote} />
      {webAllowDlg}{webBlockDlg}{appBlockDlg}{appAllowDlg}{linkDlg}{msgDlg}{cmds.node}
      {workOpen && <SendWork onClose={() => setWorkOpen(false)} say={setNote} fail={setErr} />}
      {rulesOpen && <WebRules onClose={() => setRulesOpen(false)} onGuard={() => setGuardOpen(true)} say={setNote} fail={setErr} />}
      {settingsOpen && <Settings onClose={() => setSettingsOpen(false)} fail={setErr} />}
      {extra === "shortcuts" && <Shortcuts onClose={() => setExtra(null)} />}
      {extra === "timetable" && <Timetable onClose={() => setExtra(null)} say={setNote} />}
      {extra === "about" && <About onClose={() => setExtra(null)} say={setNote} />}

      <div className={s.title}>
        <div className={s.titleSide}>
          {/* the app icon (screen + rising arrow), as on the taskbar */}
          <svg viewBox="0 0 256 256" width="18" height="18" role="img" aria-label="ERG Classroom" style={{ flex: "none" }}>
            <defs><linearGradient id="erg-mark" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stopColor="#2B2BD0" /><stop offset="1" stopColor="#000066" /></linearGradient></defs>
            <rect x="4" y="4" width="248" height="248" rx="56" fill="url(#erg-mark)" />
            <rect x="42" y="66" width="172" height="114" rx="14" fill="none" stroke="#FFFFFF" strokeWidth="13" />
            <path d="M128 182 V208 M98 210 H158" fill="none" stroke="#FFFFFF" strokeWidth="13" strokeLinecap="round" />
            <path d="M52 206 C78 150 128 150 168 104 L150 92 L214 60 L208 132 L190 118 C150 170 96 170 52 206 Z" transform="translate(26 -22) scale(0.94)" fill="#FF3B36" stroke="#0A0A8C" strokeWidth="9" strokeLinejoin="round" paintOrder="stroke" />
          </svg>
          <MenuBar menus={cmds.menus} label={t("Thanh menu")} />
        </div>
        <span className={mergeClasses(s.ell, s.room)}>{[room, ui.lesson, "ERG Classroom Tutor"].filter(Boolean).join(" · ")}</span>
        <div className={s.titleSide} style={{ justifyContent: "flex-end" }}>
          {/* shows the language in use; a click switches to the other one */}
          <Tip label={t("Ngôn ngữ")}>
            <Button size="small" appearance="subtle" style={{ minWidth: 0 }} onClick={toggleLang}>{lang === "vi" ? "VI" : "EN"}</Button>
          </Tip>
        </div>
      </div>

      <Ribbon cmds={cmds} />

      <div className={s.body}>
        <nav className={s.rail} aria-label="Views">
          {nav.map((it) => {
            const on = it.k === pane;
            return (
              <Tip key={it.k} label={it.label}>
                <button type="button" aria-label={it.label} aria-current={on ? "page" : undefined} disabled={it.off}
                  className={mergeClasses(s.railBtn, on && s.railOn)}
                  onClick={() => { if (it.k === "class") { setPane("class"); lay.setFilter(null); setHelpTab(false); } else if (it.k === "web" || it.k === "assess") setPane(it.k); else ui.request(it.k); }}>{it.icon}</button>
              </Tip>
            );
          })}
          <div className={s.spacer} />
          <Tip label={t("Chi tiết")}>
            <button type="button" aria-label={t("Chi tiết")} aria-pressed={ui.detail} className={mergeClasses(s.railBtn, ui.detail && s.railOn)} onClick={() => ui.set({ detail: !ui.detail })}><PanelRight20Regular /></button>
          </Tip>
        </nav>

        {pane === "web" && <WebPane onClose={() => setPane("class")} onLink={askLink} onRules={() => setRulesOpen(true)} say={setNote} fail={setErr} />}
        {pane === "assess" && <AssessPane onClose={() => setPane("class")} say={setNote} fail={setErr} />}
        {/* hidden, not unmounted: the wall keeps its size and scroll for when the teacher comes back */}
        <div className={s.main} style={pane === "class" ? undefined : { display: "none" }}>
          <div className={s.tabs}>
            <div className={s.tabScroll} role="group" aria-label={t("Nhóm")}>
              {tab(!helpTab && lay.filter === null, t("Tất cả"), all.length, () => { lay.setFilter(null); setHelpTab(false); })}
              {groupNames.map((g) => tab(!helpTab && lay.filter === g, g, all.filter((x) => lay.groups[x.hostname] === g).length, () => { lay.setFilter(g); setHelpTab(false); }))}
              {helpCount > 0 && tab(helpTab, t("Xin trợ giúp"), helpCount, () => { setHelpTab(true); lay.setFilter(null); })}
            </div>
            <div className={s.tabSide}>
              <Tip label={t("Thêm nhóm từ máy đang chọn")}>
                <Button size="small" appearance="subtle" icon={<Add20Regular />} aria-label={t("Thêm nhóm từ máy đang chọn")} disabled={selected.size === 0} onClick={cmds.c.assignGroup.run} />
              </Tip>
            </div>
            <div className={s.spacer} />
            <div className={s.tabSide}>
              <span>{t("Lệnh áp dụng cho")}: <b style={{ color: tokens.colorNeutralForeground1 }}>{target}</b></span>
              {lay.filter && <Button size="small" appearance="subtle" onClick={() => useClass.setState({ selected: new Set(list.map((x) => x.id)) })}>{t("Chọn cả nhóm")}</Button>}
              <Button size="small" appearance="subtle" onClick={selectAll}>{t("Chọn hết")}</Button>
              <Button size="small" appearance="subtle" disabled={selected.size === 0} onClick={clear}>{t("Bỏ chọn")}</Button>
            </div>
          </div>

          <UpdateBar say={setNote} fail={setErr} />
          {showing && (
            <MessageBar intent="info" shape="square">
              <MessageBarBody>Showing the tutor screen to {target}.</MessageBarBody>
              <MessageBarActions><Button size="small" onClick={cmds.c.show.run}>Stop Showing</Button></MessageBarActions>
            </MessageBar>
          )}

          <div className={s.center}>
            <Wall wallRef={wallRef} list={list} gridCols={gridCols} selected={selected} groups={lay.groups} leaders={lay.leaders} showing={showing}
              onPick={pick} onZoom={zoom} onClear={clear} onSwap={swapSeats}
              onKey={(st, e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); toggle(st.id, e.ctrlKey || e.metaKey); } }}
              onTileMenu={(st, e) => { if (!selected.has(st.id)) toggle(st.id, false); setCtx({ x: e.clientX, y: e.clientY }); }} />
            <AlertsPanel onRules={() => setRulesOpen(true)} fail={setErr} />

            {ui.detail && (
              <aside className={s.detail} aria-label={t("Chi tiết")}>
                <div className={s.detailHead}>
                  {one && <span className={s.dot} style={{ background: one.online ? green : grey }} />}
                  <Text weight="semibold" className={s.ell}>{one ? one.hostname : t("Chi tiết")}</Text>
                  <div className={s.spacer} />
                  <Button size="small" appearance="subtle" icon={<Dismiss16Regular />} aria-label={t("Đóng")} onClick={() => ui.set({ detail: false })} />
                </div>
                <div className={s.detailBody}>
                  {one ? (
                    <>
                      <div className={s.pic}>
                        {one.thumb ? <img className={s.img} src={one.thumb} alt="" /> : <Desktop24Regular />}
                      </div>
                      <div className={s.btnRow}>
                        <Button appearance="primary" onClick={() => ui.request("remote")}>{t("Xem")}</Button>
                        <Button onClick={askChat}>{t("Chat")}</Button>
                        <Button onClick={showInfo}>{t("Chi tiết")}</Button>
                      </div>
                      <div className={s.kvBox}>
                        {([
                          [t("Trạng thái"), one.online ? t("Đang trực tuyến") : t("Ngoại tuyến")], [t("Người dùng"), one.user], [t("Phòng"), one.room],
                          [t("Nhóm"), group && group + (lay.leaders[group] === one.hostname ? " ★" : "")],
                          ["OS", one.inventory?.os], ["CPU", one.inventory?.cpu], ["RAM", one.inventory && `${gb(one.inventory.ram_used)} / ${gb(one.inventory.ram_total)}`],
                        ] as const).map(([k, v]) => v && <div key={k} className={s.kv}><span className={s.kvKey}>{k}</span><span className={s.ell} title={v}>{v}</span></div>)}
                      </div>
                      <div>
                        <label htmlFor="quick-msg"><Text size={200} weight="semibold" style={{ color: tokens.colorNeutralForeground3 }}>{t("Tin nhắn nhanh")}</Text></label>
                        <div className={s.btnRow} style={{ marginTop: 6 }}>
                          <Input id="quick-msg" style={{ flex: 1, minWidth: 0 }} placeholder={t("Gõ rồi nhấn Enter")} value={quick} onChange={(_, d) => setQuick(d.value)} onKeyDown={(e) => e.key === "Enter" && sendQuick()} />
                          <Button style={{ flex: "none" }} disabled={!quick.trim()} onClick={sendQuick}>{t("Gửi")}</Button>
                        </div>
                      </div>
                      <div className={s.btnRow}>
                        <Button onClick={(one.locked ? c.unlock : c.lock).run}>{one.locked ? t("Mở khoá") : t("Khoá")}</Button>
                        <Button onClick={c.sendFile.run}>{t("Gửi file")}</Button>
                        <Button onClick={c.reward.run}>{t("Thưởng")}</Button>
                      </div>
                    </>
                  ) : (
                    <Text style={{ color: tokens.colorNeutralForeground3 }}>{selected.size > 1 ? `${selected.size} ${t("máy đang chọn")}` : t("Chọn một máy để xem chi tiết")}</Text>
                  )}
                </div>
              </aside>
            )}
          </div>

          <div className={s.bar} role="toolbar" aria-label={t("Kích thước hình")}>
            <div className={s.barItem}>
              <span className={s.hint}>{t("Cỡ")}</span>
              <Slider size="small" min={120} max={360} step={10} disabled={(ui.autoFit && ui.view === "thumbs") || ui.view === "details"} value={ui.size} onChange={(_, d) => ui.set({ size: d.value })} aria-label={t("Kích thước hình")} style={{ width: 130, minWidth: 100 }} />
            </div>
            <Checkbox label={t("Tự khớp")} checked={ui.autoFit} onChange={(_, d) => ui.set({ autoFit: !!d.checked })} />
            <Checkbox label={t("Hiện nhóm")} checked={ui.showGroup} onChange={(_, d) => ui.set({ showGroup: !!d.checked })} />
            <div className={s.barItem}>
              <label htmlFor="show-name" className={s.hint}>{t("Tên hiển thị")}</label>
              <Select id="show-name" size="small" value={ui.showUser ? "user" : "pc"} onChange={(_, d) => ui.set({ showUser: d.value === "user" })}>
                <option value="pc">{t("Tên máy tính")}</option>
                <option value="user">{t("Tên người dùng")}</option>
              </Select>
            </div>
            <div className={s.spacer} />
            <span className={s.hint}>{t("Kéo trên vùng trống để chọn nhiều học sinh")}</span>
            <Button size="small" style={{ flex: "none" }} disabled={selected.size !== 1} onClick={() => ui.request("remote")}>{t("Xem")}</Button>
          </div>

          <LessonPanel onPlan={c.plan.run} onEnd={c.endClass.run} />
        </div>
      </div>

      <div className={mergeClasses(s.status, !!err && s.statusErr)} role="status">
        <span className={s.ell}>{err ?? note ?? t("Sẵn sàng")}</span>
        <div className={s.spacer} />
        {room && <><span>{t("Phòng")}: {room}</span><span className={s.sep} /></>}
        <span>{all.length} {t("học sinh")} · {all.filter((x) => x.online).length} {t("trực tuyến")}</span>
        <span className={s.sep} />
        <span>{selected.size} {t("đã chọn")}</span>
        <span className={s.sep} />
        <span>Show: {showing ? t("bật") : t("tắt")}</span>
      </div>

      <Menu open={!!ctx} onOpenChange={(_, d) => !d.open && setCtx(null)} positioning={{ target: { getBoundingClientRect: () => new DOMRect(ctx?.x ?? 0, ctx?.y ?? 0, 0, 0) }, shiftToCoverTarget: true }}>
        <MenuPop items={ctxItems} head={ctxHead} />
      </Menu>
    </div>
  );
}
