import { useEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Button, Card, Checkbox, Input, Radio, RadioGroup, Select, Text, Textarea, tokens } from "@fluentui/react-components";
import { Chat24Regular, ChevronDown16Regular, Folder24Regular, HandRight24Regular, Pin20Filled, Pin20Regular, QuestionCircle24Regular, Star24Filled } from "@fluentui/react-icons";
import { t } from "./i18n";
import { Mark } from "./Views";
import { BoardCanvas } from "./BoardCanvas";
import type { BoardMsg } from "./board";
import { lazy, Suspense } from "react";
const ExcalidrawView = lazy(() => import("./ExcalidrawView"));

type Lesson = { title: string; objectives: string[]; ends_at: number; stars: number; monitoring: boolean; leader: boolean };

const fmt = (s: number) => `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;

/** Thin always-on-top bar: lesson title, time left, stars, help button. */
export function ToolbarView() {
  const [l, setL] = useState<Lesson>({ title: "", objectives: [], ends_at: 0, stars: 0, monitoring: false, leader: false });
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));
  const [open, setOpen] = useState(false);
  useEffect(() => {
    invoke<Lesson>("lesson_state").then(setL);
    const un = listen<Lesson>("lesson", (e) => setL(e.payload));
    const t = setInterval(() => setNow(Math.floor(Date.now() / 1000)), 1000);
    return () => { un.then((f) => f()); clearInterval(t); };
  }, []);
  const left = l.ends_at ? Math.max(l.ends_at - now, 0) : null;
  // Auto-hide: a thin strip at the top edge; the bar drops down while the mouse is on it (and for the first seconds).
  const [info, setInfo] = useState<[string, string, string]>(["", "", ""]);
  const [folded, setFolded] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const [pin, setPin] = useState(false); // pinned: the bar stays open
  const foldSoon = (ms: number) => { clearTimeout(timer.current); if (!pin) timer.current = setTimeout(() => { setFolded(true); setOpen(false); }, ms); };
  useEffect(() => { invoke<[string, string, string]>("toolbar_info").then(setInfo); foldSoon(5000); return () => clearTimeout(timer.current); /* eslint-disable-next-line react-hooks/exhaustive-deps */ }, []);
  useEffect(() => { void invoke("toolbar_height", folded ? { height: 10, width: 140 } : { height: open ? 250 : 46 }); }, [folded, open]);
  // the teacher changing the lesson (new title, stars, leader) shows the bar for a moment
  useEffect(() => { setFolded(false); foldSoon(5000); /* eslint-disable-next-line react-hooks/exhaustive-deps */ }, [l]);
  // shortcut: the lesson start time is not sent, so the bar is full at the moment this end time was first seen; send the start time to make it exact
  const span = useRef<[number, number]>([0, 1]);
  if (left !== null && span.current[0] !== l.ends_at) span.current = [l.ends_at, Math.max(left, 1)];
  // folded: a small tab centred at the top edge
  if (folded) return <div title={t("Thanh công cụ ERG")} style={{ height: "100%", background: "#1F4E9E", borderRadius: "0 0 6px 6px" }} onMouseEnter={() => { clearTimeout(timer.current); setFolded(false); }} />;
  const d = new Date(now * 1000);
  const two: React.CSSProperties = { display: "flex", flexDirection: "column", lineHeight: "15px", whiteSpace: "nowrap", minWidth: 0 };
  const mut: React.CSSProperties = { color: "#5F5F5F", fontSize: 12 };
  const tag: React.CSSProperties = { display: "inline-flex", alignItems: "center", height: 20, padding: "0 8px", borderRadius: 10, background: "#EDEDED", fontSize: 12, whiteSpace: "nowrap" };
  const sep = <span style={{ width: 1, height: 24, background: "#E0E0E0", flex: "none" }} />;
  const title = <span style={{ fontWeight: 600, overflow: "hidden", textOverflow: "ellipsis", maxWidth: 320 }} title={l.title}>{l.title || t("Tiết học")}</span>;
  return (
    <header onMouseEnter={() => clearTimeout(timer.current)} onMouseLeave={() => foldSoon(1200)}
      style={{ background: "#FBFBFB", color: "#1B1B1B", height: 46, boxSizing: "border-box", display: "flex", alignItems: "center", gap: 10, padding: "0 8px 0 12px", borderBottom: "1px solid #C8C8C8", fontSize: 13 }}>
      <Mark s={22} />
      <div style={two}><span style={{ fontWeight: 600 }}>{info[2] || info[0]}</span><span style={mut}>{info[2] ? `${info[0]} · ` : ""}{t("phòng")} {info[1]}</span></div>
      {sep}
      {l.objectives.length > 0 ? (
        <span style={{ position: "relative", display: "flex", alignItems: "center", height: "100%", minWidth: 0 }}>
          <Button appearance="subtle" aria-expanded={open} icon={<ChevronDown16Regular />} iconPosition="after" onClick={() => setOpen(!open)} style={{ height: 36, minWidth: 0, background: open ? "#EFEFEF" : undefined }}>
            <span style={{ ...two, alignItems: "flex-start", fontWeight: 400 }}>{title}<span style={mut}>{t("Mục tiêu")}</span></span>
          </Button>
          {open && (
            <div style={{ position: "absolute", left: 0, top: "calc(100% + 5px)", width: 340, maxHeight: 190, overflow: "auto", boxSizing: "border-box", padding: "12px 14px", background: "#fff", border: "1px solid #D6D6D6", borderRadius: 8, boxShadow: "0 8px 20px rgba(0,0,0,.14)" }}>
              <Text weight="semibold" block>{t("Mục tiêu")}</Text>
              {l.objectives.map((o, i) => <Text key={i} block style={{ marginTop: 4 }}>• {o}</Text>)}
            </div>
          )}
        </span>
      ) : title}
      {left !== null && (
        <div style={{ display: "flex", flexDirection: "column", width: 150, gap: 4, flex: "none" }}>
          <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12 }}>
            <span style={mut}>{t("Còn lại")}</span>
            <span style={{ fontWeight: 600, fontVariantNumeric: "tabular-nums", color: left < 60 ? "#C42B1C" : undefined }}>{fmt(left)}</span>
          </div>
          <div style={{ height: 4, borderRadius: 2, background: "#DADADA" }}>
            <div style={{ width: `${Math.min(100, (left / span.current[1]) * 100)}%`, height: 4, borderRadius: 2, background: left < 60 ? "#C42B1C" : "#0E6F78" }} />
          </div>
        </div>
      )}
      <span style={{ flex: 1 }} />
      {l.monitoring && <span title={t("Giáo viên đang theo dõi một số từ khoá khi bạn gõ phím")} style={{ ...tag, background: "#FFF4CE", color: "#6B4A00" }}>{t("👁 Theo dõi từ khoá")}</span>}
      {l.leader && (
        <>
          <span style={tag} title={t("Giáo viên giao quyền trưởng nhóm cho bạn")}>{t("★ Trưởng nhóm")}</span>
          <Button size="small" onClick={() => { const msg = window.prompt(t("Nhắn cho nhóm")); if (msg?.trim()) invoke("leader_action", { kind: "message", text: msg }); }}>{t("Nhắn nhóm")}</Button>
          <Button size="small" onClick={() => invoke("leader_action", { kind: "lock", text: "" })}>{t("Khoá nhóm")}</Button>
          <Button size="small" onClick={() => invoke("leader_action", { kind: "unlock", text: "" })}>{t("Mở nhóm")}</Button>
        </>
      )}
      {sep}
      <Button icon={<HandRight24Regular />} style={{ borderColor: "#D9A29C", color: "#A32418" }} onClick={() => invoke("help_request", { text: "Em cần giúp đỡ" })}>{t("Xin trợ giúp")}</Button>
      <Button icon={<Chat24Regular />} onClick={() => invoke("open_chat")}>Chat</Button>
      <Button icon={<Folder24Regular />} title={t("Tệp giáo viên gửi cho bạn")} onClick={() => invoke("open_inbox")}>{t("Bài của tôi")}</Button>
      {l.stars > 0 && <span title={t("Phần thưởng")} style={{ display: "flex", alignItems: "center", gap: 4, fontWeight: 600, fontVariantNumeric: "tabular-nums" }}><Star24Filled style={{ color: "#B7791F", width: 18, height: 18 }} />{l.stars}</span>}
      {sep}
      <div style={{ ...two, alignItems: "flex-end", fontVariantNumeric: "tabular-nums" }}>
        <span style={{ fontWeight: 600 }}>{d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span><span style={mut}>{d.toLocaleDateString()}</span>
      </div>
      <Button appearance="subtle" aria-pressed={pin} aria-label={t("Ghim thanh công cụ")} title={t("Ghim thanh công cụ")} icon={pin ? <Pin20Filled /> : <Pin20Regular />} onClick={() => { clearTimeout(timer.current); setPin(!pin); }} />
    </header>
  );
}

/** Attendance: name and class. */
export function RegisterView() {
  const [name, setName] = useState("");
  const [cls, setCls] = useState("");
  const ok = name.trim().length > 0;
  const send = async () => { if (ok && (await invoke<boolean>("register", { name, class: cls }))) getCurrentWindow().close(); };
  return (
    <Card style={{ height: "100%", boxSizing: "border-box", borderRadius: 0 }}>
      <Text weight="semibold" block>{t("Điểm danh")}</Text>
      <Input value={name} placeholder={t("Họ và tên")} onChange={(_, d) => setName(d.value)} />
      <Input value={cls} placeholder={t("Lớp")} onChange={(_, d) => setCls(d.value)} onKeyDown={(e) => e.key === "Enter" && send()} />
      <Button appearance="primary" disabled={!ok} onClick={send} style={{ marginTop: "auto" }}>{t("Có mặt")}</Button>
    </Card>
  );
}

/** Oral question: the student can answer in text or only raise a hand. */
export function QuestionView({ id, text }: { id: number; text: string }) {
  const [a, setA] = useState("");
  const done = async (t: string) => { await invoke("hand", { question: id, text: t }); getCurrentWindow().close(); };
  return (
    <Card style={{ height: "100%", boxSizing: "border-box", borderRadius: 0 }}>
      <div style={{ display: "flex", gap: 8 }}><QuestionCircle24Regular /><Text weight="semibold" style={{ wordBreak: "break-word" }}>{text}</Text></div>
      <Textarea value={a} placeholder={t("Câu trả lời (không bắt buộc)")} onChange={(_, d) => setA(d.value)} />
      <div style={{ marginTop: "auto", display: "flex", gap: 8, justifyContent: "flex-end" }}>
        <Button icon={<HandRight24Regular />} onClick={() => done("")}>{t("Giơ tay")}</Button>
        <Button appearance="primary" disabled={!a.trim()} onClick={() => done(a.trim())}>{t("Gửi trả lời")}</Button>
      </div>
    </Card>
  );
}

// ---------------------------------------------------------------- test taking

type Kind =
  | { Choice: { options: string[] } }
  | { TrueFalse: object }
  | { Fill: { accepted: string[][] } }
  | { Match: { left: string[]; right: string[] } }
  | { Order: { items: string[] } }
  | { Region: { image: string } };
type Q = { text: string; points: number; kind: Kind };
type Paper = { title: string; questions: Q[] };
// Same externally tagged shapes as erg_assess::Answer.
type Ans = "Skipped" | { Choice: number[] } | { Bool: boolean } | { Fill: string[] } | { Match: number[] } | { Order: number[] } | { Point: [number, number] };

function QuestionBody({ q, a, set }: { q: Q; a: Ans; set: (a: Ans) => void }) {
  const k = q.kind;
  if ("Choice" in k) {
    const cur = typeof a === "object" && "Choice" in a ? a.Choice : [];
    return <div>{k.Choice.options.map((o, i) => (
      <Checkbox key={i} label={o} checked={cur.includes(i)} onChange={(_, d) => set({ Choice: d.checked ? [...cur, i] : cur.filter((x) => x !== i) })} />
    ))}</div>;
  }
  if ("TrueFalse" in k) {
    const v = typeof a === "object" && "Bool" in a ? (a.Bool ? "t" : "f") : "";
    return <RadioGroup layout="horizontal" value={v} onChange={(_, d) => set({ Bool: d.value === "t" })}><Radio value="t" label={t("Đúng")} /><Radio value="f" label={t("Sai")} /></RadioGroup>;
  }
  if ("Fill" in k) {
    const n = k.Fill.accepted.length;
    const cur = typeof a === "object" && "Fill" in a ? a.Fill : Array(n).fill("");
    return <div style={{ display: "grid", gap: 6 }}>{cur.map((v, i) => (
      <Input key={i} value={v} placeholder={`${t("Chỗ trống")} ${i + 1}`} onChange={(_, d) => set({ Fill: cur.map((x, j) => (j === i ? d.value : x)) })} />
    ))}</div>;
  }
  if ("Match" in k) {
    const cur = typeof a === "object" && "Match" in a ? a.Match : k.Match.left.map(() => -1);
    return <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 6 }}>{k.Match.left.map((l, i) => (
      <div key={i} style={{ display: "contents" }}>
        <Text>{l}</Text>
        <Select value={String(cur[i])} onChange={(_, d) => set({ Match: cur.map((x, j) => (j === i ? Number(d.value) : x)) })}>
          <option value="-1">—</option>
          {k.Match.right.map((r, j) => <option key={j} value={j}>{r}</option>)}
        </Select>
      </div>
    ))}</div>;
  }
  if ("Order" in k) {
    const items = k.Order.items;
    const cur = typeof a === "object" && "Order" in a ? a.Order : items.map((_, i) => i);
    const move = (p: number, d: number) => {
      const n = [...cur];
      [n[p], n[p + d]] = [n[p + d], n[p]];
      set({ Order: n });
    };
    return <div>{cur.map((idx, p) => (
      <div key={idx} style={{ display: "flex", gap: 6, alignItems: "center", marginBottom: 4 }}>
        <Button size="small" disabled={p === 0} onClick={() => move(p, -1)}>↑</Button>
        <Button size="small" disabled={p === cur.length - 1} onClick={() => move(p, 1)}>↓</Button>
        <Text>{items[idx]}</Text>
      </div>
    ))}</div>;
  }
  const pt = typeof a === "object" && "Point" in a ? a.Point : null;
  return (
    <div style={{ position: "relative", display: "inline-block", cursor: "crosshair" }}
      onClick={(e) => { const r = e.currentTarget.getBoundingClientRect(); set({ Point: [(e.clientX - r.left) / r.width, (e.clientY - r.top) / r.height] }); }}>
      <img src={k.Region.image} style={{ maxWidth: "100%", display: "block" }} draggable={false} />
      {pt && <div style={{ position: "absolute", left: `${pt[0] * 100}%`, top: `${pt[1] * 100}%`, width: 14, height: 14, margin: -7, borderRadius: 7, background: "red" }} />}
    </div>
  );
}

/** Full-screen test. Hands the answers in when the time ends or the student presses Nộp bài. */
export function TestView({ id }: { id: number }) {
  const [data, setData] = useState<{ paper: Paper; seconds: number } | null>(null);
  const [ans, setAns] = useState<Ans[]>([]);
  const [left, setLeft] = useState<number | null>(null);
  const sent = useRef(false);
  useEffect(() => {
    invoke<string | null>("test_data", { id }).then((s) => {
      if (!s) return;
      const d = JSON.parse(s);
      setData(d);
      setAns(d.paper.questions.map(() => "Skipped"));
      setLeft(d.seconds || null);
    });
  }, [id]);
  const submit = useMemo(() => () => {
    if (sent.current) return;
    sent.current = true;
    invoke("test_done", { id, answers: JSON.stringify(ans) });
  }, [id, ans]);
  useEffect(() => {
    if (left === null) return;
    if (left <= 0) { submit(); return; }
    const t = setTimeout(() => setLeft(left - 1), 1000);
    return () => clearTimeout(t);
  }, [left, submit]);
  if (!data) return <Text>{t("Đang tải đề…")}</Text>;
  return (
    <div style={{ height: "100%", overflow: "auto", background: tokens.colorNeutralBackground2, padding: 24, boxSizing: "border-box" }}>
      <div style={{ maxWidth: 820, margin: "0 auto", display: "grid", gap: 12 }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <Text size={600} weight="semibold">{data.paper.title}</Text>
          {left !== null && <Text size={500} style={{ fontVariantNumeric: "tabular-nums" }}>⏱ {fmt(left)}</Text>}
        </div>
        {data.paper.questions.map((q, i) => (
          <Card key={i}>
            <Text weight="semibold">{t("Câu")} {i + 1} ({q.points} {t("điểm")}). {q.text}</Text>
            <QuestionBody q={q} a={ans[i] ?? "Skipped"} set={(a) => setAns(ans.map((x, j) => (j === i ? a : x)))} />
          </Card>
        ))}
        <Button appearance="primary" size="large" onClick={submit}>{t("Nộp bài")}</Button>
      </div>
    </div>
  );
}

/** Teacher-controlled video lesson: the student cannot pause or seek. */
export function VideoView({ path }: { path: string }) {
  const ref = useRef<HTMLVideoElement>(null);
  useEffect(() => {
    const un = listen<{ do: string; secs?: number }>("video", (e) => {
      const v = ref.current;
      if (!v) return;
      if (e.payload.do === "play") void v.play();
      else if (e.payload.do === "pause") v.pause();
      else if (e.payload.do === "seek") v.currentTime = e.payload.secs ?? 0;
    });
    return () => void un.then((f) => f());
  }, []);
  return <video ref={ref} src={convertFileSrc(path)} style={{ width: "100%", height: "100%", background: "#000", objectFit: "contain" }} onContextMenu={(e) => e.preventDefault()} />;
}

/** Shared whiteboard: shows what the teacher draws; draws too only while the teacher allows it. */
export function BoardView() {
  const [state, setState] = useState<{ draw: boolean; log: BoardMsg[] } | null>(null);
  const [scene, setScene] = useState<string | null>(null); // tutor uses Excalidraw: show it read-only
  useEffect(() => {
    invoke<[boolean, BoardMsg[]]>("board_state").then(([draw, log]) => {
      setState({ draw, log });
      const sc = log.filter((m) => typeof m === "object" && "Scene" in m).pop();
      if (sc && typeof sc === "object" && "Scene" in sc) setScene(sc.Scene.json);
    });
    const un = listen<BoardMsg>("board", (e) => {
      const m = e.payload;
      if (typeof m === "object" && "Scene" in m) setScene(m.Scene.json);
      if (typeof m === "object" && "Open" in m) setState((s) => ({ draw: m.Open.draw, log: s?.log ?? [] }));
    });
    return () => void un.then((f) => f());
  }, []);
  if (!state) return null;
  return (
    <div style={{ height: "100%", boxSizing: "border-box", padding: 8, background: "#f3f3f3", overflow: "auto" }}>
      <Text weight="semibold" block>{t("Bảng chung")} {!state.draw && <Text size={200}>· {t("Chỉ xem")}</Text>}</Text>
      {scene !== null
        ? <Suspense fallback={null}><ExcalidrawView json={scene} /></Suspense>
        : <BoardCanvas canDraw={state.draw} initial={state.log} label={t} send={(msg) => void invoke("board_emit", { msg })} />}
    </div>
  );
}
