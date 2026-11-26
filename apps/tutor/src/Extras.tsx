import { useEffect, useRef, useState } from "react";
import { Button, Checkbox, Field, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, Input, Skeleton, SkeletonItem, Spinner, Text, Textarea, tokens } from "@fluentui/react-components";
import { ArrowClockwise16Regular, ArrowUp16Regular, ChevronLeft16Regular, ChevronRight16Regular, Document16Regular, Folder16Filled, Info16Regular, Warning16Regular } from "@fluentui/react-icons";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useClass } from "./store";
import { ordered, ranking, useLayout } from "./layout";
import { Designer } from "./Designer";
import { BoardDialog } from "./BoardDialog";
import { JournalDialog } from "./JournalDialog";
import { Bar, BLUE, D, DlgHead, FileDist, GREEN, GREY, RED, Status, mut, td, th } from "./FileDist";
import { inventoryCsv } from "./inventory";
import { useUi } from "./uistate";
import { alertWhat, unlockAlert, useAlerts, useTest } from "./Panes2";
import { tr, useLang } from "./i18n";

type Send = (cmd: unknown) => Promise<number>;
type DirEntry = { name: string; is_dir: boolean; size: number; modified_s: number };

/** Small modal with a text area; `onOk` receives the text. */
function TextDialog({ title, hint, initial = "", rows = 6, onOk, onClose }: { title: string; hint: string; initial?: string; rows?: number; onOk: (v: string) => void; onClose: () => void }) {
  const [v, setV] = useState(initial);
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={D.surface}>
        <DialogBody style={D.body}>
          <DlgHead title={title} onClose={onClose} />
          <DialogContent style={D.content}>
            <Text size={200} block style={{ color: mut }}>{hint}</Text>
            <Textarea autoFocus rows={rows} value={v} onChange={(_, d) => setV(d.value)} style={{ width: "100%" }} />
          </DialogContent>
          <DialogActions style={D.foot}>
            <Button onClick={onClose}>Cancel</Button>
            <Button appearance="primary" onClick={() => { onOk(v); onClose(); }}>OK</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

const list = (t: string) => t.split(/[\s,;]+/).filter(Boolean);
const saveText = async (name: string, text: string) => {
  const path = await save({ defaultPath: name });
  if (path) await invoke("write_text", { path, text });
  return path;
};

/** Plan step as saved by the lesson planner. */
type Step = { at_s: number; label: string; command: unknown };

/** All class commands that need dialogs or local state. The ribbon, menu bar and context menus call these; `node` holds the dialogs they open. */
export function useExtras(send: Send, say: (s: string) => void, fail: (s: string) => void) {
  const { students, selected, startShow } = useClass();
  // the app opens on New Class, as NetSupport does
  const [dlg, setDlg] = useState<null | "wizard" | "keywords" | "plan" | "kill" | "collect" | "ask" | "design" | "teams" | "board" | "journal">(() => (localStorage.getItem("erg.autoRoom") === null ? "wizard" : null));
  // "Always start in <room>" on the start screen: connect there and skip the screen
  useEffect(() => { const r = localStorage.getItem("erg.autoRoom"); if (r !== null) useClass.getState().connect(r).catch((e) => fail(String(e))); /* eslint-disable-next-line react-hooks/exhaustive-deps */ }, []);
  const [keysOn, setKeysOn] = useState(false);
  const [scan, setScan] = useState(false);
  const [recording, setRecording] = useState(false);
  const [shell, setShell] = useState(false);
  const [fm, setFm] = useState(false);
  const [dist, setDist] = useState(false);
  const [bar, setBar] = useState(true); // students show their toolbar unless the teacher turns it off
  const [rc, setRc] = useState(false);
  const alerts = useAlerts((s) => s.list);
  const viols = alerts.filter((a) => a.state === "open").slice(-3); // unanswered alerts, newest three
  const run =(p: Promise<unknown>) => p.catch((e) => fail(String(e)));
  const hosts = () => Object.values(students).filter((s) => selected.size === 0 || selected.has(s.id)).map((s) => s.hostname);

  // ---- lesson timeline
  const planRef = useRef<{ steps: Step[]; start: number; done: Set<number> } | null>(null);
  useEffect(() => {
    const t = setInterval(() => {
      const p = planRef.current;
      if (!p) return;
      const el = (Date.now() - p.start) / 1000;
      p.steps.forEach((s, i) => {
        if (!p.done.has(i) && s.at_s <= el) { p.done.add(i); send(s.command); say(`Lesson plan: ${s.label}`); }
      });
      if (p.done.size === p.steps.length) planRef.current = null;
    }, 1000);
    return () => clearInterval(t);
  }, [send, say]);

  // ---- short notices for events that need no dialog
  useEffect(() => {
    const who = (id: string) => students[id]?.user || students[id]?.hostname || "Student";
    const subs = [
      listen<{ id: string; word: string }>("student-keyword", (e) => say(`${who(e.payload.id)} typed keyword: ${e.payload.word}`)),
      listen<{ id: string; kind: string; app: string; title: string }>("student-violation", (e) => useAlerts.getState().add(e.payload)),
      listen<{ id: string; kind: string; app: string; title: string }>("student-violation", (e) => say(`⚠ ${who(e.payload.id)} (${students[e.payload.id]?.hostname ?? ""}) – ${tr(useLang.getState().lang, alertWhat(e.payload.kind))}: ${e.payload.title.slice(0, 120)}. The computer is locked with a warning; Unlock it to see what is open.`)),
      listen<{ id: string; text: string; question: number }>("student-hand", (e) => say(`${who(e.payload.id)} ${e.payload.text ? "answered: " + e.payload.text : "raised a hand"}`)),
      listen<{ id: string; name: string; class: string }>("student-register", (e) => say(`Present: ${e.payload.name} (${e.payload.class})`)),
      listen<{ id: string; path: string }>("student-collected", (e) => say(`Collected: ${e.payload.path}`)),
      listen<{ id: string; text: string }>("student-clipboard", (e) => say(`Clipboard ${who(e.payload.id)}: ${e.payload.text.slice(0, 200)}`)),
    ];
    return () => subs.forEach((p) => void p.then((f) => f()));
  }, [say, students]);

  // ---- requests from the nav rail / tiles (App)
  const req = useUi((u) => u.req);
  useEffect(() => {
    if (!req) return;
    const one = useClass.getState().selected.size === 1;
    switch (req.k) {
      case "scan": setScan(true); break;
      case "design": setDlg("design"); break;
      case "board": setDlg("board"); break;
      case "journal": setDlg("journal"); break;
      case "collect": setDlg("collect"); break;
      case "whiteboard": run(invoke("open_overlay", { mode: "whiteboard" })); break;
      case "files": if (one) setFm(true); break;
      case "remote": if (one) setRc(true); break;
      case "exhibit": if (one) run(invoke("start_exhibit", { exhibitor: [...useClass.getState().selected][0] }).then(() => say("Showing the student's screen to the class"))); break;
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [req]);

  // ---- tests
  useEffect(() => {
    const un = listen<{ id: string; test: number; answers: string }>("student-test-done", (e) => {
      useTest.getState().handIn(e.payload.id, e.payload.test, e.payload.answers).catch((err) => fail(String(err)));
      say(`${students[e.payload.id]?.hostname ?? "Student"} handed in the test`);
    });
    return () => void un.then((f) => f());
  }, [say, fail, students]);

  const L = () => useLayout.getState();
  const byHost = () => Object.fromEntries(Object.values(students).map((s) => [s.hostname, s.id]));
  const protect = (p: Partial<{ no_control_panel: boolean; hide_drives: string[]; no_run_dialog: boolean; no_task_manager: boolean }>) =>
    send({ SetProtect: { no_control_panel: false, hide_drives: [], no_run_dialog: false, no_task_manager: false, ...p } });

  const A = {
    keysOn, recording, bar,
    studentBar: () => { send({ Toolbar: !bar }).then((n) => say(`Student toolbar ${bar ? "off" : "on"} on ${n} PC(s)`)); setBar(!bar); },
    wizard: () => setDlg("wizard"),
    refresh: () => run(useClass.getState().connect(useClass.getState().room).then(() => say("Refreshing the list of students"))),
    // drops everyone and opens New Class: the students of the room chosen there connect again
    endClass: () => run((async () => {
      const k = useClass.getState();
      if (k.showing) await k.stopShow();
      await k.connect(k.room);
      setDlg("wizard");
    })()),
    autoGroup: () => {
      const n = Number(window.prompt("How many groups?", "4"));
      if (n >= 1) L().autoGroup(ordered(Object.values(students), L().order).map((s) => s.hostname), n);
    },
    assignGroup: () => {
      const g = window.prompt("Add the selected students to group", "Group A");
      if (g?.trim()) L().assign(hosts(), g.trim());
    },
    setLeader: () => { if (!L().setLeader(hosts()[0])) fail("This student is not in a group"); },
    selectLeaders: () => {
      const ids = Object.values(students).filter((s) => Object.values(L().leaders).includes(s.hostname)).map((s) => s.id);
      if (!ids.length) return fail("No group leaders have been set");
      useClass.setState({ selected: new Set(ids) });
    },
    grantLeaders: () => {
      const minutes = Number(window.prompt("Grant group leader rights for how many minutes?", "30"));
      if (!(minutes >= 1)) return;
      const bh = byHost(), l = L();
      const ids = (g: string) => Object.entries(l.groups).filter(([, x]) => x === g).flatMap(([h]) => (bh[h] ? [bh[h]] : []));
      const leaders = Object.entries(l.leaders).filter(([, h]) => bh[h]);
      if (!leaders.length) return fail("No group leaders have been set");
      leaders.forEach(([g, h]) => run(invoke("grant_leader", { leader: bh[h], members: ids(g), minutes })));
      say(`Granted rights to ${leaders.length} group leader(s) for ${minutes} minutes`);
    },
    revokeLeaders: () => {
      const bh = byHost();
      Object.values(L().leaders).filter((h) => bh[h]).forEach((h) => run(invoke("revoke_leader", { leader: bh[h] })));
      say("Group leader rights revoked");
    },
    clearGroups: () => L().clearGroups(),
    plan: () => setDlg("plan"),
    roll: () => send("RequestRegister"),
    attendance: () => run(invoke<string>("attendance_csv").then((c) => saveText("register.csv", c))),
    inventory: () => run((async () => {
      await send("RequestInventory");
      say("Collecting inventory… (5 seconds)");
      await new Promise((r) => setTimeout(r, 5000));
      const rows = ordered(Object.values(useClass.getState().students), L().order);
      await saveText("inventory.csv", inventoryCsv(rows));
    })()),
    journal: () => setDlg("journal"),
    journalHtml: () => run(invoke<string>("journal_html", { title: "Lesson journal", objectives: [] }).then((h) => saveText("journal.html", h))),
    phoneOn: () => run(invoke<string>("assistant_start").then((u) => window.prompt("Open this address on a phone on the same Wi-Fi network (for the teacher only). Turn it off from the menu below.", u))),
    phoneOff: () => run(invoke("assistant_stop").then(() => say("Phone assistant turned off"))),
    wake: () => run(invoke<number>("wake", { hosts: hosts() }).then((n) => say(`Power-on sent to ${n} PC(s)`))),
    // presenting
    showApp: () => run(startShow(true).then(() => say("Showing the foreground window"))),
    exhibit: () => run(invoke("start_exhibit", { exhibitor: [...selected][0] }).then(() => say("Showing the student's screen to the class"))),
    exhibitOff: () => run(invoke("stop_exhibit")),
    annotate: () => run(invoke("open_overlay", { mode: "annotate" })),
    board: () => setDlg("board"),
    whiteboard: () => run(invoke("open_overlay", { mode: "whiteboard" })),
    keys: () => { const on = !keysOn; setKeysOn(on); run(invoke("keyshow", { on })); },
    playVideo: () => run(playVideo(send, say)),
    videoPlay: () => send({ Video: "Play" }),
    videoPause: () => send({ Video: "Pause" }),
    videoSeek: () => { const s = Number(window.prompt("Seek to second", "0")); if (s >= 0) send({ Video: { Seek: { secs: s } } }); },
    videoClose: () => send({ Video: "Close" }),
    scan: () => setScan(true),
    record: () => {
      if (!recording) run(invoke<string>("start_recording").then((d) => { setRecording(true); say(`Recording to ${d}`); }));
      else run(invoke<string>("stop_recording").then((p) => { setRecording(false); say(`Recording saved: ${p}`); }));
    },
    // control
    mute: (on: boolean) => send({ Mute: on }),
    beep: () => send("Beep"),
    usb: (m: "Allow" | "ReadOnly" | "NoExecute" | "BlockAll") => send({ SetUsbPolicy: m }),
    print: (m: "Allow" | "Pause" | "BlockAll") => send({ SetPrintPolicy: m }),
    protectOn: () => protect({ no_control_panel: true, no_run_dialog: true, no_task_manager: true }),
    protectDrives: () => { const d = window.prompt("Hide drives (e.g. D E)", "D E"); if (d) protect({ hide_drives: d.toUpperCase().split(/[^A-Z]+/).filter(Boolean).map((x) => x[0]) }); },
    protectOff: () => protect({}),
    keywords: () => setDlg("keywords"),
    kill: () => setDlg("kill"),
    // assessment
    design: () => setDlg("design"),
    giveTest: () => run(startTest(say)),
    finishTest: () => run(finishTest(say)),
    ask: () => setDlg("ask"),
    teams: () => setDlg("teams"),
    random: () => pickRandom(students, selected, say),
    reward: () => send({ Reward: { stars: 1 } }),
    // files
    files: () => setFm(true),
    distribute: () => setDist(true),
    shell: () => setShell(true),
    deploy: () => run((async () => {
      const f = await open({ multiple: false, filters: [{ name: "Installer package", extensions: ["msi", "exe"] }] });
      if (!f || Array.isArray(f)) return;
      const args = window.prompt("Silent install arguments (leave empty for .msi; usually /S or /VERYSILENT for .exe)", f.toLowerCase().endsWith(".exe") ? "/S" : "");
      if (args === null) return;
      if (!window.confirm("Install this package on the selected PCs (or the whole class if none are selected)?\n" + f)) return;
      const ids = [...useClass.getState().selected];
      const [ok, failed] = await invoke<[number, number]>("deploy", { ids: ids.length ? ids : null, path: f, args, minutes: 30 });
      say(`Package sent and install requested on ${ok} PC(s)${failed ? `, ${failed} failed` : ""}. Results are written to deploy.log on each student PC.`);
    })()),
    collect: () => setDlg("collect"),
    clipClear: () => send({ SetClipboard: "" }).then(() => say("Student clipboards cleared")),
    clipGet: () => send("RequestClipboard"),
    remote: () => setRc(true),
  };

  const node = (
    <>
      {dlg === "design" && <Designer say={say} onClose={() => setDlg(null)} />}
      {dlg === "board" && <BoardDialog fail={fail} onClose={() => setDlg(null)} />}
      {dlg === "journal" && <JournalDialog fail={fail} say={say} onClose={() => setDlg(null)} />}
      {dlg === "teams" && <TeamBoard onClose={() => setDlg(null)} />}
      {dlg === "wizard" && <Wizard send={send} say={say} fail={fail} onClose={() => setDlg(null)} />}
      {dlg === "keywords" && <TextDialog title="Keyword monitoring" hint="Separate words with commas. Only matching words are reported; leave empty to turn off. Students are always notified." onOk={(t) => send({ SetKeywords: list(t) })} onClose={() => setDlg(null)} rows={3} />}
      {dlg === "kill" && <TextDialog title="Close application" hint=".exe file name, e.g. game.exe" onOk={(t) => list(t).forEach((n) => send({ KillProcess: n }))} onClose={() => setDlg(null)} rows={2} />}
      {dlg === "ask" && <TextDialog title="Ask a question" hint="Students raise a hand or answer; the first one is reported in the messages." onOk={(t) => t.trim() && send({ AskQuestion: { id: Date.now() % 2 ** 31, text: t.trim() } })} onClose={() => setDlg(null)} rows={3} />}
      {dlg === "collect" && <CollectDialog send={send} say={say} fail={fail} onClose={() => setDlg(null)} />}
      {/* unauthorised access: stays on screen until the teacher answers it (the toast above is also kept) */}
      {viols.length > 0 && (
        <div style={{ position: "fixed", top: 118, right: 16, zIndex: 1000001, display: "grid", gap: 8, width: 400, maxWidth: "calc(100vw - 32px)" }}>
          {viols.map((v) => {
            const s = students[v.id], close = () => useAlerts.getState().mark(v.key, "kept");
            return (
              <div key={v.key} role="alert" style={{ borderRadius: 8, overflow: "hidden", border: `1px solid ${RED}`, background: tokens.colorNeutralBackground1, boxShadow: tokens.shadow28 }}>
                <div style={{ display: "flex", alignItems: "center", gap: 8, height: 36, padding: "0 16px", background: "#C42B1C", color: "#fff", fontWeight: 600 }}>
                  <Warning16Regular /><span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>Unauthorised access · {s?.hostname}</span>
                  <span style={{ fontSize: 12, fontWeight: 400 }}>{new Date(v.at).toLocaleTimeString()}</span>
                </div>
                <div style={{ display: "grid", gap: 6, padding: "12px 16px" }}>
                  <span><b>{s?.user || s?.hostname || "Student"}</b> – {tr(useLang.getState().lang, alertWhat(v.kind))}: <b style={{ wordBreak: "break-word" }}>{v.title.slice(0, 120)}</b></span>
                  <span style={{ fontSize: 12, color: mut }}>The computer is locked with a warning. It stays locked until you unlock it; unlock it to see what is open.</span>
                </div>
                <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, padding: "10px 16px", background: tokens.colorNeutralBackground2, borderTop: `1px solid ${tokens.colorNeutralStroke2}` }}>
                  <Button onClick={close}>Keep locked</Button>
                  <Button appearance="primary" onClick={() => run(unlockAlert(v))}>Unlock</Button>
                </div>
              </div>
            );
          })}
        </div>
      )}
      {dlg === "plan" && <TextDialog title="Lesson plan" hint={'JSON: [{"at_s":0,"label":"Lock","command":{"Lock":{"blank":false,"text":"Eyes on the teacher"}}},{"at_s":600,"label":"Unlock","command":"Unlock"}]'}
        initial={'[{"at_s":0,"label":"Reminder","command":{"Message":{"text":"The lesson is starting","seconds":10}}}]'}
        onOk={(t) => { try { planRef.current = { steps: JSON.parse(t), start: Date.now(), done: new Set() }; say("Lesson plan started"); } catch (e) { fail(String(e)); } }} onClose={() => setDlg(null)} />}
      {shell && selected.size === 1 && <RemoteShell id={[...selected][0]} send={send} onClose={() => setShell(false)} />}
      {scan && <ScanView send={send} onClose={() => setScan(false)} />}
      {dist && <FileDist onClose={() => setDist(false)} say={say} fail={fail} />}
      {fm && selected.size === 1 && <FileManager id={[...selected][0]} send={send} onClose={() => setFm(false)} fail={fail} />}
      {rc && selected.size === 1 && <RemoteView key={[...selected][0]} id={[...selected][0]} onFiles={() => setFm(true)} onClose={() => setRc(false)} />}
    </>
  );
  return { node, A };
}

function TeamBoard({ onClose }: { onClose: () => void }) {
  const { groups, scores, addScore, resetScores } = useLayout();
  const names = [...new Set(Object.values(groups))].sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));
  const rank = ranking(Object.fromEntries(names.map((g) => [g, scores[g] ?? 0])));
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={D.surface}>
        <DialogBody style={D.body}>
          <DlgHead title="Team scoreboard" onClose={onClose} />
          <DialogContent style={{ ...D.content, gap: 6 }}>
            {!names.length && <Text>No groups yet: use Class management → Auto group.</Text>}
            {rank.map(([g, p], i) => (
              <div key={g} style={{ display: "flex", gap: 8, alignItems: "center" }}>
                <Text style={{ width: 24 }}>{i + 1}.</Text><Text weight="semibold" style={{ flex: 1 }}>{g}</Text><Text style={{ width: 40, textAlign: "right" }}>{p}</Text>
                <Button size="small" onClick={() => addScore(g, -1)}>−1</Button><Button size="small" appearance="primary" onClick={() => addScore(g, 1)}>+1</Button>
              </div>
            ))}
          </DialogContent>
          <DialogActions style={D.foot}>
            <Button onClick={resetScores}>Reset scores</Button>
            <Button appearance="primary" onClick={onClose}>Close</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

/** Collect Work: which files, where to save them, and which computers have handed something in (counted from "student-collected"). */
function CollectDialog({ send, say, fail, onClose }: { send: Send; say: (s: string) => void; fail: (s: string) => void; onClose: () => void }) {
  const { students, selected } = useClass();
  const to = Object.values(students).filter((s) => s.online && (selected.size === 0 || selected.has(s.id)));
  const [folder, setFolder] = useState("Lesson1");
  const [pattern, setPattern] = useState("*");
  const [dir, setDir] = useState("");
  // files received per computer since Collect was pressed: how many, total bytes, time of the last one
  const [got, setGot] = useState<Record<string, { n: number; b: number; at: Date }> | null>(null);
  useEffect(() => {
    const un = listen<{ id: string; path: string; size?: number }>("student-collected", (e) => setGot((g) => {
      const o = g?.[e.payload.id];
      return g && { ...g, [e.payload.id]: { n: (o?.n ?? 0) + 1, b: (o?.b ?? 0) + (e.payload.size ?? 0), at: new Date() } };
    }));
    return () => void un.then((f) => f());
  }, []);
  const browse = async () => {
    const d = await open({ directory: true, title: "Save collected work to folder" });
    if (!d || Array.isArray(d)) return "";
    setDir(d);
    return d;
  };
  const collect = async () => {
    try {
      const d = dir || await browse();
      if (!d) return;
      await invoke("set_collect_dir", { dir: d });
      setGot({});
      const n = await send({ CollectFiles: { folder: folder.trim(), pattern: pattern.trim() || "*" } });
      say(`Collecting work from ${n} PC(s) into ${d}`);
    } catch (e) { fail(String(e)); }
  };
  const have = got ? to.filter((s) => got[s.id]).length : 0;
  const missing = got ? to.filter((s) => !got[s.id]) : [];
  const remind = () => invoke<number>("send_command", { ids: missing.map((s) => s.id), cmd: { Message: { text: `${tr(useLang.getState().lang, "Nhớ lưu bài vào thư mục")} Documents\\${folder.trim()}`, seconds: 20 } } })
    .then((n) => say(`Reminder sent to ${n} PC(s)`), (e) => fail(String(e)));
  const kb = (b: number) => b >= 1 << 20 ? `${(b / (1 << 20)).toFixed(1)} MB` : `${Math.ceil(b / 1024)} KB`;
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ ...D.surface, maxWidth: 900, width: "94vw" }}>
        <DialogBody style={D.body}>
          <DlgHead title="Collect Work" sub={selected.size ? `${to.length} selected` : "whole class"} onClose={onClose} />
          <DialogContent style={D.content}>
            <div style={{ display: "flex", gap: 10, alignItems: "end" }}>
              <Field label="Folder in the student's Documents" style={{ width: 240 }}><Input autoFocus value={folder} onChange={(_, d) => setFolder(d.value)} /></Field>
              <Field label="File name pattern" style={{ width: 130 }}><Input value={pattern} placeholder="*.docx" onChange={(_, d) => setPattern(d.value)} /></Field>
              <Field label="Save on this computer in" style={{ flex: 1, minWidth: 0 }}><Input value={dir} placeholder="Choose a folder" onChange={(_, d) => setDir(d.value)} /></Field>
              <Button onClick={() => void browse()}>Browse…</Button>
            </div>
            <div style={{ ...D.box, height: 330 }}>
              <table style={{ width: "100%", borderCollapse: "collapse", tableLayout: "fixed" }}>
                <thead><tr><th style={{ ...th, width: 130 }}>Computer</th><th style={th}>Student</th><th style={{ ...th, width: 70, textAlign: "right" }}>Files</th><th style={{ ...th, width: 90, textAlign: "right" }}>Size</th><th style={{ ...th, width: 220 }}>Status</th></tr></thead>
                <tbody>
                  {to.map((s) => {
                    const r = got?.[s.id], n = r?.n ?? 0;
                    return (
                      <tr key={s.id}>
                        <td style={td}>{s.hostname}</td><td style={td}>{s.user}</td>
                        <td style={{ ...td, color: mut, textAlign: "right" }}>{got ? n : ""}</td>
                        <td style={{ ...td, color: mut, textAlign: "right" }}>{r ? kb(r.b) : ""}</td>
                        <td style={td}>{!got ? <Status color={GREY}>Ready</Status> : r ? <Status color={GREEN}>Collected at {hhmm(r.at)}</Status> : <Status color={BLUE}>Nothing received yet</Status>}</td>
                      </tr>
                    );
                  })}
                  {!to.length && <tr><td colSpan={5} style={{ ...td, color: mut }}>No student computers are connected.</td></tr>}
                </tbody>
              </table>
            </div>
            {got && (
              <div style={{ display: "grid", gap: 6 }}>
                <span><b>{have} of {to.length}</b> students collected <span style={{ color: mut }}>· a student with no matching files stays at "Nothing received yet"</span></span>
                <Bar h={4} label="Students collected" v={to.length ? have / to.length : 0} color={BLUE} />
              </div>
            )}
          </DialogContent>
          <DialogActions style={D.foot}>
            <Button style={{ marginRight: "auto" }} disabled={!dir} onClick={() => invoke("open_folder", { dir }).catch((e) => fail(String(e)))}>Open folder</Button>
            <Button disabled={!missing.length} onClick={() => void remind()}>Remind {missing.length || ""}</Button>
            <Button appearance="primary" style={{ minWidth: 96 }} disabled={!to.length} onClick={() => void collect()}>Collect</Button>
            <Button style={{ minWidth: 80 }} onClick={onClose}>Close</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

const hhmm = (d: Date) => `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
/** Minutes from `now` until the next time the clock reads "HH:MM" (0 for an unreadable time; a time already past means tomorrow). */
export function minutesUntil(end: string, now: Date) {
  const [h, m] = end.split(":").map(Number);
  if (!(h >= 0 && h < 24 && m >= 0 && m < 60)) return 0;
  return ((h * 60 + m - now.getHours() * 60 - now.getMinutes()) % 1440 + 1440) % 1440;
}

/** New Class: lesson details, who is connected, and what to start with the lesson (same "lesson" document as LessonPanel). */
function Wizard({ send, say, fail, onClose }: { send: Send; say: (s: string) => void; fail: (s: string) => void; onClose: () => void }) {
  const all = Object.values(useClass((s) => s.students));
  const room = useClass((s) => s.room);
  const connect = useClass((s) => s.connect);
  const off = all.filter((s) => !s.online).map((s) => s.hostname);
  // the teacher's own list of rooms ("rooms" document), plus the connected room and any room seen on the network
  const [saved, setSaved] = useState<string[]>([]);
  useEffect(() => { invoke<string | null>("load_doc", { name: "rooms" }).then((raw) => { const d = raw ? JSON.parse(raw) : []; if (Array.isArray(d)) setSaved(d.filter((x) => typeof x === "string")); }).catch(() => { /* none yet */ }); }, []);
  const keep = (list: string[]) => { setSaved(list); invoke("save_doc", { name: "rooms", json: JSON.stringify(list) }).catch(() => { /* browser dev */ }); };
  const rooms = [...new Set([...saved, room, ...all.map((s) => s.room)].filter(Boolean))].sort();
  const [edit, setEdit] = useState<null | { old: string | null; name: string }>(null);
  const go = (r: string) => connect(r).catch((e) => fail(String(e)));
  const [last] = useState<Record<string, string>>(() => { try { return JSON.parse(localStorage.getItem("erg.roomLast") ?? "{}"); } catch { return {}; } });
  const saveEdit = () => {
    const name = edit?.name.trim();
    if (!edit || !name) return;
    keep([...new Set([...saved.filter((x) => x !== edit.old), name])]);
    setEdit(null);
    if (edit.old === null || edit.old === room) go(name); // a new room is the one to teach in; a renamed current room stays current
  };
  const [teacher, setTeacher] = useState("");
  const [title, setTitle] = useState("");
  const [obj, setObj] = useState("");
  const [outcome, setOutcome] = useState("");
  const [end, setEnd] = useState(() => hhmm(new Date(Date.now() + 45 * 60000)));
  const [openEnded, setOpenEnded] = useState(true);
  const [register, setRegister] = useState(false);
  // the teacher's name and last lesson are remembered; the end time is always asked again
  useEffect(() => {
    invoke<string | null>("load_doc", { name: "lesson" }).then((raw) => {
      const d = raw ? JSON.parse(raw) : {};
      if (typeof d.teacher === "string") setTeacher(d.teacher);
      if (typeof d.title === "string") setTitle(d.title);
      if (Array.isArray(d.objectives)) setObj(d.objectives.join("\n"));
      if (typeof d.outcome === "string") setOutcome(d.outcome);
    }).catch(() => { /* no saved lesson: start empty */ });
  }, []);
  const minutes = openEnded ? 0 : Math.min(600, minutesUntil(end, new Date()));
  const [auto, setAuto] = useState(() => localStorage.getItem("erg.autoRoom"));
  const here = room || "All rooms";
  const h = new Date().getHours();
  const hello = `${h < 12 ? "Good morning" : h < 18 ? "Good afternoon" : "Good evening"}${teacher.trim() ? `, ${teacher.trim()}` : ""}`;
  const pcs = [...all].sort((x, y) => x.hostname.localeCompare(y.hostname, undefined, { numeric: true }));
  const start = () => {
    const objectives = obj.split("\n").map((x) => x.trim()).filter(Boolean);
    invoke("save_doc", { name: "lesson", json: JSON.stringify({ teacher, title, objectives, outcome, minutes, end_class_when_timer_stops: false }) }).catch(() => { /* browser dev */ });
    localStorage.setItem("erg.roomLast", JSON.stringify({ ...last, [room]: new Date().toISOString() }));
    if (title.trim() || minutes) send({ SetLesson: { title: title.trim(), objectives, seconds_left: minutes * 60 } }).then((n) => say(`Lesson started on ${n} PC(s)`));
    if (register) send("RequestRegister");
    onClose();
  };
  const tag = (bg: string, fg: string): React.CSSProperties => ({ padding: "1px 8px", borderRadius: 10, background: bg, color: fg, fontSize: 12 });
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ ...D.surface, maxWidth: 980, width: "92vw" }}>
        <DialogBody style={D.body}>
          <DlgHead title="Start a class" onClose={onClose} />
          {/* design N13: rooms and the lesson on the left, the chosen room's computers on the right */}
          <DialogContent style={{ ...D.content, padding: 0, display: "grid", gridTemplateColumns: "minmax(360px, 440px) 1fr", height: "min(600px, 100vh - 140px)" }}>
            <section aria-label="Start a class" style={{ display: "flex", flexDirection: "column", gap: 12, padding: "20px 24px", borderRight: `1px solid ${tokens.colorNeutralStroke2}`, overflow: "auto" }}>
              <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
                <h1 style={{ margin: 0, fontSize: 20, lineHeight: "28px", fontWeight: 600 }}>{hello}</h1>
                <span style={{ color: mut }}>Choose the room you are teaching in.</span>
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 6, marginTop: 4 }}>
                <b style={{ flex: 1 }}>Rooms</b>
                <Button size="small" appearance="subtle" icon={<ArrowClockwise16Regular />} onClick={() => go(room)}>Search again</Button>
                <Button size="small" onClick={() => setEdit({ old: null, name: "" })}>New room…</Button>
              </div>
              {/* one click connects; the connected room is the highlighted one */}
              <div role="radiogroup" aria-label="Rooms" style={{ ...D.box, flex: "none", maxHeight: 260 }}>
                {[...rooms, ""].map((r) => {
                  const on = r === room;
                  const seen = all.filter((s) => !r || s.room === r);
                  return (
                    <div key={r} role="radio" aria-checked={on} tabIndex={0} onClick={() => !on && go(r)} onKeyDown={(e) => (e.key === " " || e.key === "Enter") && !on && go(r)}
                      style={{ display: "flex", alignItems: "center", gap: 10, height: 48, padding: "0 12px", borderBottom: `1px solid ${tokens.colorNeutralStroke3}`, background: on ? tokens.colorBrandBackground2 : "transparent", cursor: "pointer" }}>
                      <span style={{ width: 14, height: 14, boxSizing: "border-box", borderRadius: 7, flex: "none", border: on ? `4px solid ${BLUE}` : `1px solid ${tokens.colorNeutralStrokeAccessible}`, background: tokens.colorNeutralBackground1 }} />
                      <span style={{ display: "flex", flexDirection: "column", flex: 1, minWidth: 0, lineHeight: "17px" }}>
                        <b style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{r || "All rooms on this network"}</b>
                        <span style={{ color: mut, fontSize: 12 }}>{!r ? "Every student computer that answers" : last[r] ? `Last class ${new Date(last[r]).toLocaleDateString()}` : "No class yet"}</span>
                      </span>
                      {on && saved.includes(r) && <>
                        <Button size="small" appearance="subtle" onClick={(e) => { e.stopPropagation(); setEdit({ old: r, name: r }); }}>Rename</Button>
                        <Button size="small" appearance="subtle" onClick={(e) => { e.stopPropagation(); keep(saved.filter((x) => x !== r)); go(""); }}>Remove</Button>
                      </>}
                      {/* only rooms whose students we can see have a count */}
                      {seen.length > 0 && <span style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12 }}><span style={{ width: 8, height: 8, borderRadius: 4, background: seen.some((s) => s.online) ? GREEN : GREY }} />{seen.filter((s) => s.online).length} of {seen.length} on</span>}
                    </div>
                  );
                })}
              </div>
              {edit && (
                <div style={{ display: "flex", gap: 6 }}>
                  <Input autoFocus size="small" placeholder="Room name, as typed when the student computers were installed" style={{ flex: 1 }} value={edit.name}
                    onChange={(_, d) => setEdit({ ...edit, name: d.value })} onKeyDown={(e) => { if (e.key === "Enter") saveEdit(); }} />
                  <Button size="small" appearance="primary" disabled={!edit.name.trim()} onClick={saveEdit}>{edit.old === null ? "Add" : "Rename"}</Button>
                  <Button size="small" onClick={() => setEdit(null)}>Cancel</Button>
                </div>
              )}
              <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10, marginTop: 4, alignItems: "start" }}>
                <Field label="Lesson title (optional)" style={{ gridColumn: "1 / 3" }}><Input value={title} onChange={(_, d) => setTitle(d.value)} /></Field>
                <Field label="Teacher"><Input value={teacher} onChange={(_, d) => setTeacher(d.value)} /></Field>
                <Field label="Ends at" hint={openEnded ? undefined : `${minutes} min from now`}>
                  <Input type="time" disabled={openEnded} value={end} onChange={(_, d) => setEnd(d.value)} />
                </Field>
              </div>
              <Checkbox label="No end time" checked={openEnded} onChange={(_, d) => setOpenEnded(!!d.checked)} />
              <Checkbox label="Ask students to register their name" checked={register} onChange={(_, d) => setRegister(!!d.checked)} />
              <span style={{ flex: 1 }} />
              <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                <Button appearance="primary" style={{ minWidth: 150 }} onClick={start}>Start class in {here}</Button>
                <Button onClick={onClose}>Open without a lesson</Button>
                <span style={{ flex: 1 }} />
                <Checkbox label={`Always start in ${here}`} checked={auto === room && auto !== null} onChange={(_, d) => {
                  if (d.checked) localStorage.setItem("erg.autoRoom", room); else localStorage.removeItem("erg.autoRoom");
                  setAuto(d.checked ? room : null);
                }} />
              </div>
            </section>
            <section aria-label="Computers in the chosen room" style={{ display: "flex", flexDirection: "column", gap: 12, padding: "20px 24px", background: tokens.colorNeutralBackground2, minWidth: 0, overflow: "auto" }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                <b style={{ fontSize: 15 }}>{here}</b>
                <span style={tag(tokens.colorPaletteGreenBackground1, tokens.colorPaletteGreenForeground1)}>{all.length - off.length} of {all.length} found</span>
                {off.length > 0 && <span style={tag(tokens.colorNeutralBackground3, tokens.colorNeutralForeground2)}>{off.length} switched off</span>}
                <span style={{ flex: 1 }} />
                <Button size="small" disabled={!off.length} onClick={() => invoke<number>("wake", { hosts: off }).then((n) => say(`Power-on sent to ${n} PC(s)`), (e) => fail(String(e)))}>
                  {off.length ? `Power on the ${off.length} computer${off.length > 1 ? "s" : ""}` : "Power on"}
                </Button>
              </div>
              <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(118px, 1fr))", gap: 8, alignContent: "start" }}>
                {pcs.map((p) => (
                  <div key={p.id} title={p.user || undefined} style={{ display: "flex", alignItems: "center", gap: 6, height: 30, padding: "0 8px", border: `1px solid ${tokens.colorNeutralStroke2}`, borderRadius: 6, background: p.online ? tokens.colorNeutralBackground1 : tokens.colorNeutralBackground3, color: p.online ? tokens.colorNeutralForeground1 : tokens.colorNeutralForeground3, fontSize: 12, boxShadow: p.online ? tokens.shadow2 : "none" }}>
                    <span style={{ width: 8, height: 8, borderRadius: 4, flex: "none", background: p.online ? GREEN : GREY }} />
                    <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{p.hostname}</span>
                  </div>
                ))}
                {!pcs.length && <Spinner size="small" label="Looking for student computers…" style={{ gridColumn: "1 / -1", padding: "48px 0" }} />}
              </div>
              <span style={{ flex: 1 }} />
              <div style={{ display: "flex", alignItems: "center", gap: 8, paddingTop: 12, borderTop: `1px solid ${tokens.colorNeutralStroke2}`, color: mut, fontSize: 12 }}>
                <Info16Regular />
                <span>A computer is missing? Check that it is switched on and set to room “{room || "…"}” in the Student Configurator.</span>
              </div>
            </section>
          </DialogContent>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

// the ribbon path; the Assessments pane does the same with its own controls
async function startTest(say: (s: string) => void) {
  const file = await open({ multiple: false, filters: [{ name: "Test", extensions: ["json"] }] });
  if (!file || Array.isArray(file)) return;
  await useTest.getState().addTest(file);
  const minutes = Math.max(0, Math.min(600, Number(window.prompt("Time limit (minutes, 0 = no limit)", "15") ?? "15") || 0));
  say(`Test sent to ${await useTest.getState().start(file, minutes)} PC(s)`);
}

async function finishTest(say: (s: string) => void) {
  const t = useTest.getState();
  if (!t.run || t.run.done) return say("No test is running");
  await t.finish();
  say(`Marked ${Object.keys(t.run.scores).length} test(s)`);
  await t.exportCsv();
}

/** Send an mp4/webm to the class, then open it on every student; playback is then controlled from the menu. */
async function playVideo(send: Send, say: (s: string) => void) {
  const file = await open({ multiple: false, filters: [{ name: "Video", extensions: ["mp4", "webm"] }] });
  if (!file || Array.isArray(file)) return;
  const ids = [...useClass.getState().selected];
  const [ok, failed] = await invoke<[number, number]>("send_file", { ids: ids.length ? ids : null, path: file });
  say(`Video sent to ${ok} PC(s)${failed ? `, ${failed} failed` : ""}`);
  const name = file.split(/[\\/]/).pop()!;
  await send({ Video: { Open: { name } } });
}

/** Scan: step through the online students, a few seconds each, showing one large live picture. */
function ScanView({ send, onClose }: { send: Send; onClose: () => void }) {
  const students = useClass((s) => s.students);
  const [i, setI] = useState(0);
  const [secs, setSecs] = useState(5);
  const [paused, setPaused] = useState(false);
  const online = Object.values(students).filter((s) => s.online).sort((a, b) => a.hostname.localeCompare(b.hostname, undefined, { numeric: true }));
  const cur = online.length ? online[i % online.length] : undefined;
  useEffect(() => {
    if (paused || online.length < 2) return;
    const t = setInterval(() => setI((n) => n + 1), secs * 1000);
    return () => clearInterval(t);
  }, [secs, paused, online.length]);
  // ask only the shown student for large, fast pictures; everyone else keeps the small slow ones
  const id = cur?.id;
  useEffect(() => {
    if (!id) return;
    void invoke("send_command", { ids: [id], cmd: { RequestThumbnail: { w: 960, h: 540, interval_ms: 700 } } });
    return () => void invoke("send_command", { ids: [id], cmd: { RequestThumbnail: { w: 480, h: 270, interval_ms: 1000 } } });
  }, [id]);
  void send;
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ ...D.surface, maxWidth: 1000, width: "90vw" }}>
        <DialogBody style={D.body}>
          <DlgHead title={<>Scan: {cur ? `${cur.user || cur.hostname} (${cur.hostname})` : "no students online"}</>} onClose={onClose} />
          <DialogContent style={D.content}>
            {cur?.thumb ? <img src={cur.thumb} alt="" style={{ width: "100%" }} /> : <Text>Loading…</Text>}
          </DialogContent>
          <DialogActions style={D.foot}>
            <Input type="number" style={{ width: 90 }} value={String(secs)} onChange={(_, d) => setSecs(Math.max(2, Math.min(300, Number(d.value) || 5)))} />
            <Text>sec/student</Text>
            <Button onClick={() => setPaused(!paused)}>{paused ? "Resume" : "Pause"}</Button>
            <Button onClick={() => setI((n) => n + 1)}>Next</Button>
            <Button appearance="primary" onClick={onClose}>Close</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

function pickRandom(students: Record<string, { id: string; online: boolean; user: string; hostname: string }>, selected: Set<string>, say: (s: string) => void) {
  const pool = Object.values(students).filter((s) => s.online && (selected.size === 0 || selected.has(s.id)));
  if (!pool.length) return say("No students available");
  const p = pool[Math.floor(Math.random() * pool.length)];
  say(`Selected: ${p.user || p.hostname}`);
}

function FileManager({ id, send, onClose, fail }: { id: string; send: Send; onClose: () => void; fail: (s: string) => void }) {
  const [path, setPath] = useState("C:\\Users");
  const [rows, setRows] = useState<DirEntry[]>([]);
  const [msg, setMsg] = useState("");
  useEffect(() => {
    const a = listen<{ id: string; path: string; entries: DirEntry[] }>("student-dir", (e) => { if (e.payload.id === id) { setPath(e.payload.path); setRows(e.payload.entries); setMsg(""); } });
    const b = listen<{ id: string; ok: boolean; message: string }>("student-fs", (e) => { if (e.payload.id === id) { setMsg(e.payload.message); if (!e.payload.ok) fail(e.payload.message); else go(path); } });
    return () => { a.then((f) => f()); b.then((f) => f()); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id, path]);
  const go = (p: string) => void send({ ListDir: p });
  useEffect(() => { go(path); /* eslint-disable-next-line react-hooks/exhaustive-deps */ }, []);
  const join = (n: string) => path.replace(/\\$/, "") + "\\" + n;
  const up = () => { const i = path.replace(/\\$/, "").lastIndexOf("\\"); go(i > 2 ? path.slice(0, i) : path.slice(0, 3)); };
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ ...D.surface, maxWidth: 720 }}>
        <DialogBody style={D.body}>
          <DlgHead title="Files on the student PC" onClose={onClose} />
          <DialogContent style={D.content}>
            <div style={{ display: "flex", gap: 6 }}>
              <Button icon={<ArrowUp16Regular />} aria-label="Up one folder" onClick={up} />
              <Input style={{ flex: 1 }} value={path} onChange={(_, d) => setPath(d.value)} onKeyDown={(e) => e.key === "Enter" && go(path)} />
              <Button onClick={() => go(path)}>Open</Button>
              <Button onClick={() => { const n = window.prompt("New folder name"); if (n) send({ MakeDir: join(n) }); }}>New folder</Button>
            </div>
            <div style={{ ...D.box, height: 340 }}>
              {rows.map((r) => (
                <div key={r.name} style={{ display: "flex", gap: 8, alignItems: "center", minHeight: 30, padding: "0 12px", borderBottom: `1px solid ${tokens.colorNeutralBackground4}` }}>
                  <Text style={{ flex: 1, cursor: r.is_dir ? "pointer" : "default" }} onDoubleClick={() => r.is_dir && go(join(r.name))}><span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>{r.is_dir ? <Folder16Filled style={{ color: "#B7791F" }} /> : <Document16Regular style={{ color: mut }} />}{r.name}</span></Text>
                  <Text size={200} style={{ color: mut }}>{r.is_dir ? "" : `${(r.size / 1024).toFixed(1)} KB`}</Text>
                  <Button size="small" onClick={() => { const n = window.prompt("New name", r.name); if (n && n !== r.name) send({ RenamePath: { from: join(r.name), to: join(n) } }); }}>Rename</Button>
                  <Button size="small" onClick={() => window.confirm(`Delete ${r.name}?`) && send({ DeletePath: join(r.name) })}>Delete</Button>
                </div>
              ))}
            </div>
            <Text size={200} style={{ color: tokens.colorNeutralForeground3 }}>{msg}</Text>
          </DialogContent>
          <DialogActions style={D.foot}><Button onClick={onClose}>Close</Button></DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

/** Watch (zoomed thumbnails ~2 fps) and optionally Control (mouse + keyboard injection). */
function RemoteView({ id, onClose, onFiles }: { id: string; onClose: () => void; onFiles: () => void }) {
  // always this student, even after Next has moved the selection (the clean-up below runs after that)
  const send = (cmd: unknown) => invoke<number>("send_command", { ids: [id], cmd });
  const img = useClass((s) => s.students[id]?.thumb);
  const name = useClass((s) => s.students[id]?.hostname ?? "");
  const user = useClass((s) => s.students[id]?.user ?? "");
  const online = useClass((s) => !!s.students[id]?.online);
  const [control, setControl] = useState(false);
  const canvas = useRef<HTMLCanvasElement>(null);
  const [live, setLive] = useState(false);
  const [ar, setAr] = useState(16 / 9);
  useEffect(() => {
    // H.264 through WebCodecs; without it (or on a decode problem) the slow JPEG picture stays
    const hasCodec = typeof VideoDecoder !== "undefined";
    if (!hasCodec) send({ RequestThumbnail: { w: 960, h: 540, interval_ms: 500 } });
    let dec: VideoDecoder | null = null;
    let needKey = true;
    const make = () => {
      dec?.close();
      dec = new VideoDecoder({
        output: (f) => {
          const cv = canvas.current;
          if (cv) { if (cv.width !== f.displayWidth || cv.height !== f.displayHeight) { cv.width = f.displayWidth; cv.height = f.displayHeight; setAr(f.displayWidth / f.displayHeight); } cv.getContext("2d")?.drawImage(f, 0, 0); setLive(true); }
          f.close();
        },
        error: () => { needKey = true; make(); },
      });
      dec.configure({ codec: "avc1.42E028", optimizeForLatency: true });
    };
    let un: Promise<() => void> | null = null;
    if (hasCodec) {
      make();
      un = listen<{ id: string; key: boolean; data: string }>("student-video", (e) => {
        if (e.payload.id !== id || !dec || dec.state !== "configured") return;
        if (needKey && !e.payload.key) return;
        if (dec.decodeQueueSize > 6 && !e.payload.key) { needKey = true; return; }
        needKey = false;
        const bin = atob(e.payload.data);
        const b = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) b[i] = bin.charCodeAt(i);
        dec.decode(new EncodedVideoChunk({ type: e.payload.key ? "key" : "delta", timestamp: performance.now() * 1000, data: b }));
      });
      send({ WatchScreen: { on: true } });
    }
    return () => {
      if (hasCodec) { send({ WatchScreen: { on: false } }); void un?.then((f) => f()); dec?.close(); }
      // back to the small grid size when closed
      send({ RequestThumbnail: { w: 480, h: 270, interval_ms: 1000 } });
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const pos = (e: React.MouseEvent) => { const r = e.currentTarget.getBoundingClientRect(); return { x: Math.min(Math.max((e.clientX - r.left) / r.width, 0), 1), y: Math.min(Math.max((e.clientY - r.top) / r.height, 0), 1) }; };
  // previous / next student (design N18), in the order of the computer names
  const others = Object.values(useClass((s) => s.students)).filter((x) => x.online).map((x) => [x.id, x.hostname] as const).sort((a, b) => a[1].localeCompare(b[1], undefined, { numeric: true }));
  const at = others.findIndex(([x]) => x === id);
  const step = (d: number) => others.length && useClass.setState({ selected: new Set([others[(at + d + others.length) % others.length][0]]) });
  const [held, setHeld] = useState<null | "blank" | "lock">(null);
  const hold = (k: "blank" | "lock") => { send(held === k ? "Unlock" : { Lock: { blank: k === "blank", text: k === "blank" ? "" : null } }); setHeld(held === k ? null : k); };
  const stage = useRef<HTMLDivElement>(null);
  const lastEsc = useRef(0);
  const [fit, setFit] = useState(true); // off: the student's screen pixel for pixel, with scroll bars
  const [sharing, setSharing] = useState(false);
  const [chat, setChat] = useState(false);
  const share = () => { const on = !sharing; invoke(on ? "start_exhibit" : "stop_exhibit", on ? { exhibitor: id } : {}).then(() => setSharing(on), () => { /* stays as it was */ }); };
  const shot = () => { const d = live ? canvas.current?.toDataURL("image/png") : img; if (d) void invoke("save_snapshot", { dataUrl: d }); };
  const tool = (label: string, run: () => void, on = false, off = false) => (
    <Button size="small" appearance="subtle" disabled={off} onClick={run} style={on ? { background: "#E8EFFA", border: "1px solid #B9CDEE" } : undefined}>{label}</Button>
  );
  const sep = <span style={{ width: 1, height: 22, background: "#E0E0E0", flex: "none" }} />;
  const cv = canvas.current;
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      {/* as large as the window allows, keeping the student's screen proportions (the pointer maths needs the box to match the picture) */}
      <DialogSurface style={{ ...D.surface, maxWidth: "none", width: "fit-content" }}>
        <DialogBody style={D.body}>
          <DlgHead title={`${name}${user ? ` · ${user}` : ""} – View`} onClose={onClose} />
          <div role="toolbar" aria-label="View" style={{ gridColumn: "1 / 4", gridRow: 2, display: "flex", alignItems: "center", gap: 6, height: 44, padding: "0 10px", background: "#FBFBFB", borderTop: "1px solid #E5E5E5", borderBottom: "1px solid #D6D6D6" }}>
            <div role="radiogroup" aria-label="Mode" style={{ display: "flex", padding: 2, border: "1px solid #D1D1D1", borderRadius: 5, background: "#F0F0F0" }}>
              {(["Watch", "Control"] as const).map((m) => {
                const on = (m === "Control") === control;
                return <Button key={m} size="small" appearance="subtle" role="radio" aria-checked={on} onClick={() => setControl(m === "Control")} style={{ minWidth: 0, fontWeight: on ? 600 : 400, background: on ? "#FFFFFF" : undefined }}>{m}</Button>;
              })}
            </div>
            {sep}
            {tool("Full screen", () => void stage.current?.requestFullscreen().catch(() => { /* not allowed here */ }))}
            {tool("Fit", () => setFit(!fit), fit)}
            {sep}
            {tool("Share", share, sharing, !online)}
            {tool("Chat", () => setChat(true), false, !online)}
            {sep}
            {tool("Clipboard", () => void send("RequestClipboard"))}
            {tool("Screenshot", shot, false, !live && !img)}
            {sep}
            {tool("File Transfer", onFiles)}
            {sep}
            {tool("Blank their screen", () => hold("blank"), held === "blank")}
            {tool("Lock keyboard and mouse", () => hold("lock"), held === "lock")}
            <span style={{ flex: 1 }} />
            <Button size="small" appearance="subtle" aria-label="Previous student" icon={<ChevronLeft16Regular />} disabled={others.length < 2} onClick={() => step(-1)} />
            <span style={{ fontSize: 12, color: mut }}>{at + 1} of {others.length}</span>
            <Button size="small" appearance="subtle" aria-label="Next student" icon={<ChevronRight16Regular />} disabled={others.length < 2} onClick={() => step(1)} />
          </div>
          <DialogContent style={{ gridColumn: "1 / 4", gridRow: 3, padding: 0, overflow: fit ? "hidden" : "auto", maxWidth: "calc(96vw - 24px)", maxHeight: "calc(100vh - 220px)", background: "#1E1E1E" }}>
            <div ref={stage} tabIndex={0} style={{ position: "relative", outline: control ? "2px solid #1F4E9E" : "none", outlineOffset: -2, lineHeight: 0, width: fit || !cv ? `min(96vw - 24px, (100vh - 220px) * ${ar})` : cv.width, aspectRatio: ar, background: "#000" }}
              onMouseMove={(e) => control && send({ Input: { MouseMove: pos(e) } })}
              onMouseDown={(e) => control && send({ Input: { MouseButton: { button: Math.min(e.button, 2), down: true } } })}
              onMouseUp={(e) => control && send({ Input: { MouseButton: { button: Math.min(e.button, 2), down: false } } })}
              onWheel={(e) => control && send({ Input: { Wheel: Math.max(-32768, Math.min(32767, Math.round(-e.deltaY))) } })}
              onKeyDown={(e) => {
                if (!control) return;
                e.preventDefault(); e.stopPropagation(); // the keys belong to the student, not to this dialog
                if (e.key === "Escape") { const n = Date.now(); if (n - lastEsc.current < 600) { setControl(false); return; } lastEsc.current = n; }
                send({ Input: { Key: { vk: e.keyCode, down: true } } });
              }}
              onKeyUp={(e) => { if (control) { e.preventDefault(); e.stopPropagation(); send({ Input: { Key: { vk: e.keyCode, down: false } } }); } }}>
              <canvas ref={canvas} style={{ width: "100%", display: live ? "block" : "none" }} />
              {!live && (img ? <img src={img} alt="" style={{ width: "100%", height: "100%", userSelect: "none" }} draggable={false} /> : <Skeleton style={{ height: "100%" }}><SkeletonItem style={{ height: "100%" }} /></Skeleton>)}
              {control && <span style={{ position: "absolute", left: 12, top: 12, padding: "2px 8px", borderRadius: 10, background: "#1F4E9E", color: "#fff", fontSize: 12, lineHeight: "18px" }}>You are controlling this computer · Esc Esc to let go</span>}
            </div>
          </DialogContent>
          <div style={{ gridColumn: "1 / 4", gridRow: 4, display: "flex", alignItems: "center", gap: 16, height: 24, padding: "0 10px", fontSize: 12, color: mut, background: "#F3F3F3", borderTop: "1px solid #D6D6D6" }}>
            <span style={{ display: "flex", alignItems: "center", gap: 6 }}><span style={{ width: 8, height: 8, borderRadius: 4, background: online ? GREEN : GREY }} />{online ? "Connected" : "Not connected"}</span>
            {live && cv && <span>{cv.width} × {cv.height}</span>}
            <span>{live ? "Live video" : "Pictures (slower)"}</span>
          </div>
        </DialogBody>
      {chat && <TextDialog title={`Chat with ${name}`} hint="Message" rows={3} onOk={(v) => { if (v.trim()) void send({ Chat: v.trim() }); setChat(false); }} onClose={() => setChat(false)} />}
      </DialogSurface>
    </Dialog>
  );
}

/** Tech Console command line for one student (works only where the administrator enabled it). */
function RemoteShell({ id, send, onClose }: { id: string; send: Send; onClose: () => void }) {
  const [cmd, setCmd] = useState("");
  const [out, setOut] = useState("");
  useEffect(() => {
    const un = listen<{ id: string; ok: boolean; message: string }>("student-fs", (e) => { if (e.payload.id === id) setOut((o) => o + "\n" + e.payload.message); });
    return () => void un.then((f) => f());
  }, [id]);
  const run = () => { if (cmd.trim()) { setOut((o) => `${o}\n> ${cmd}`); send({ Shell: cmd }); setCmd(""); } };
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ ...D.surface, maxWidth: 760 }}>
        <DialogBody style={D.body}>
          <DlgHead title="Remote command prompt" onClose={onClose} />
          <DialogContent style={D.content}>
            <pre style={{ height: 300, overflow: "auto", background: "#000", color: "#ddd", padding: 8, margin: 0, whiteSpace: "pre-wrap" }}>{out || "Commands run with the student's rights. An administrator must enable ERG_ALLOW_SHELL=1 on the PC."}</pre>
            <Input style={{ width: "100%", marginTop: 6 }} value={cmd} placeholder="Type a command and press Enter" onChange={(_, d) => setCmd(d.value)} onKeyDown={(e) => e.key === "Enter" && run()} />
          </DialogContent>
          <DialogActions style={D.foot}><Button onClick={onClose}>Close</Button></DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
