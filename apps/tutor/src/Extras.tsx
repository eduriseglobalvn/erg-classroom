import { useEffect, useRef, useState } from "react";
import { Button, Checkbox, Field, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, Input, Skeleton, SkeletonItem, Text, Textarea, tokens } from "@fluentui/react-components";
import { ArrowUp16Regular, Desktop16Regular, Document16Regular, DocumentArrowDown20Regular, Folder16Filled, Globe16Regular, Person20Regular, Warning16Regular } from "@fluentui/react-icons";
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
  const [dlg, setDlg] = useState<null | "wizard" | "keywords" | "plan" | "kill" | "collect" | "ask" | "design" | "teams" | "board" | "journal">("wizard");
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
      {rc && selected.size === 1 && <RemoteView id={[...selected][0]} send={send} onClose={() => setRc(false)} />}
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
  const [got, setGot] = useState<Record<string, number> | null>(null); // files received per computer since Collect was pressed
  useEffect(() => {
    const un = listen<{ id: string; path: string }>("student-collected", (e) => setGot((g) => g && { ...g, [e.payload.id]: (g[e.payload.id] ?? 0) + 1 }));
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
                <thead><tr><th style={{ ...th, width: 130 }}>Computer</th><th style={th}>Student</th><th style={{ ...th, width: 70, textAlign: "right" }}>Files</th><th style={{ ...th, width: 220 }}>Status</th></tr></thead>
                <tbody>
                  {to.map((s) => {
                    const n = got?.[s.id] ?? 0;
                    return (
                      <tr key={s.id}>
                        <td style={td}>{s.hostname}</td><td style={td}>{s.user}</td>
                        <td style={{ ...td, color: mut, textAlign: "right" }}>{got ? n : ""}</td>
                        <td style={td}>{!got ? <Status color={GREY}>Ready</Status> : n ? <Status color={GREEN}>Collected</Status> : <Status color={BLUE}>Nothing received yet</Status>}</td>
                      </tr>
                    );
                  })}
                  {!to.length && <tr><td colSpan={4} style={{ ...td, color: mut }}>No student computers are connected.</td></tr>}
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
  const [openEnded, setOpenEnded] = useState(false);
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
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ ...D.surface, maxWidth: 780, width: "94vw" }}>
        <DialogBody style={D.body}>
          <DlgHead title="New class" onClose={onClose} />
          <DialogContent style={{ ...D.content, flexDirection: "row", gap: 20 }}>
            <div style={{ display: "grid", gap: 10, width: 300, flex: "none", alignContent: "start" }}>
              <Text weight="semibold">Lesson details</Text>
              <Field label="Teacher"><Input value={teacher} onChange={(_, d) => setTeacher(d.value)} /></Field>
              <Field label="Lesson title" required><Input autoFocus value={title} onChange={(_, d) => setTitle(d.value)} /></Field>
              <Field label="Objectives (one per line, shown to students)"><Textarea rows={3} value={obj} onChange={(_, d) => setObj(d.value)} /></Field>
              <Field label="Outcome"><Textarea rows={2} value={outcome} onChange={(_, d) => setOutcome(d.value)} /></Field>
              <div style={{ display: "flex", alignItems: "start", gap: 10 }}>
                <Field label="Ends at" hint={openEnded ? undefined : `${minutes} min from now`}>
                  <Input type="time" disabled={openEnded} value={end} onChange={(_, d) => setEnd(d.value)} style={{ width: 120 }} />
                </Field>
                <Checkbox label="Open lesson (no end time)" checked={openEnded} onChange={(_, d) => setOpenEnded(!!d.checked)} style={{ marginTop: 22 }} />
              </div>
              <Checkbox label="Ask students to register their name and class" checked={register} onChange={(_, d) => setRegister(!!d.checked)} />
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 8, flex: 1, minWidth: 0 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <Text weight="semibold" style={{ flex: 1 }}>Connect to</Text>
                <Button size="small" onClick={() => setEdit({ old: null, name: "" })}>New…</Button>
                <Button size="small" disabled={!saved.includes(room)} onClick={() => setEdit({ old: room, name: room })}>Edit</Button>
                <Button size="small" disabled={!saved.includes(room)} onClick={() => { keep(saved.filter((x) => x !== room)); go(""); }}>Remove</Button>
              </div>
              {/* the room list: one click connects, the connected room is the highlighted one */}
              <div role="radiogroup" aria-label="Room" style={{ ...D.box, flex: 1, minHeight: 150, display: "flex", flexDirection: "column" }}>
                <div style={{ ...th, display: "flex", alignItems: "center", flex: "none", position: "static" }}>Room</div>
                {["", ...rooms].map((r) => (
                  <button key={r} type="button" role="radio" aria-checked={r === room} onClick={() => r !== room && go(r)} title={r || "Every room on the network"}
                    style={{ display: "flex", alignItems: "center", gap: 10, height: 30, flex: "none", padding: "0 12px", border: 0, borderBottom: `1px solid ${tokens.colorNeutralBackground4}`, textAlign: "left",
                      background: r === room ? tokens.colorBrandBackground2 : "transparent", color: tokens.colorNeutralForeground1, font: "inherit", cursor: "pointer" }}>
                    <span style={{ width: 14, height: 14, boxSizing: "border-box", borderRadius: 7, flex: "none", border: r === room ? `4px solid ${BLUE}` : `1px solid ${tokens.colorNeutralStrokeAccessible}`, background: tokens.colorNeutralBackground1 }} />
                    {r ? <Desktop16Regular /> : <Globe16Regular />}
                    <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{r || "All rooms"}</span>
                  </button>
                ))}
              </div>
              {edit && (
                <div style={{ display: "flex", gap: 6 }}>
                  <Input autoFocus size="small" placeholder="Room name, as typed when the student computers were installed" style={{ flex: 1 }} value={edit.name}
                    onChange={(_, d) => setEdit({ ...edit, name: d.value })} onKeyDown={(e) => { if (e.key === "Enter") saveEdit(); }} />
                  <Button size="small" appearance="primary" disabled={!edit.name.trim()} onClick={saveEdit}>{edit.old === null ? "Add" : "Rename"}</Button>
                  <Button size="small" onClick={() => setEdit(null)}>Cancel</Button>
                </div>
              )}
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <span style={{ flex: 1 }}><Status color={all.length - off.length ? GREEN : GREY}>{room || "All rooms"}: <b>{all.length - off.length} of {all.length}</b> computers connected</Status>{off.length > 0 && <span style={{ color: mut }}> · {off.length} switched off</span>}</span>
                <Button size="small" disabled={!off.length} onClick={() => invoke<number>("wake", { hosts: off }).then((n) => say(`Power-on sent to ${n} PC(s)`), (e) => fail(String(e)))}>Power on the room</Button>
              </div>
            </div>
          </DialogContent>
          <DialogActions style={D.foot}>
            <Button onClick={onClose}>Cancel</Button>
            <Button appearance="primary" disabled={!title.trim()} onClick={() => {
              const objectives = obj.split("\n").map((x) => x.trim()).filter(Boolean);
              invoke("save_doc", { name: "lesson", json: JSON.stringify({ teacher, title, objectives, outcome, minutes, end_class_when_timer_stops: false }) }).catch(() => { /* browser dev */ });
              send({ SetLesson: { title: title.trim(), objectives, seconds_left: minutes * 60 } }).then((n) => say(`Lesson started on ${n} PC(s)`));
              if (register) send("RequestRegister");
              onClose();
            }}>Start Lesson</Button>
          </DialogActions>
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
function RemoteView({ id, send, onClose }: { id: string; send: Send; onClose: () => void }) {
  const img = useClass((s) => s.students[id]?.thumb);
  const name = useClass((s) => s.students[id]?.hostname ?? "");
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
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      {/* as large as the window allows, keeping the student's screen proportions (the pointer maths needs the box to match the picture) */}
      <DialogSurface style={{ ...D.surface, maxWidth: "none", width: "fit-content" }}>
        <DialogBody style={D.body}>
          <DlgHead title={name} sub={control && "controlling"} onClose={onClose} />
          <DialogContent style={{ padding: 0, overflow: "hidden" }}>
            <div tabIndex={0} style={{ outline: control ? `2px solid ${tokens.colorBrandStroke1}` : "none", outlineOffset: -2, lineHeight: 0, width: `min(96vw - 24px, (100vh - 150px) * ${ar})`, aspectRatio: ar, background: "#000" }}
              onMouseMove={(e) => control && send({ Input: { MouseMove: pos(e) } })}
              onMouseDown={(e) => control && send({ Input: { MouseButton: { button: Math.min(e.button, 2), down: true } } })}
              onMouseUp={(e) => control && send({ Input: { MouseButton: { button: Math.min(e.button, 2), down: false } } })}
              onWheel={(e) => control && send({ Input: { Wheel: Math.max(-32768, Math.min(32767, Math.round(-e.deltaY))) } })}
              onKeyDown={(e) => { if (control) { e.preventDefault(); send({ Input: { Key: { vk: e.keyCode, down: true } } }); } }}
              onKeyUp={(e) => { if (control) { e.preventDefault(); send({ Input: { Key: { vk: e.keyCode, down: false } } }); } }}>
              <canvas ref={canvas} style={{ width: "100%", display: live ? "block" : "none" }} />
              {!live && (img ? <img src={img} alt="" style={{ width: "100%", height: "100%", userSelect: "none" }} draggable={false} /> : <Skeleton style={{ height: "100%" }}><SkeletonItem style={{ height: "100%" }} /></Skeleton>)}
            </div>
          </DialogContent>
          <DialogActions style={D.foot}>
            <Button appearance={control ? "primary" : "secondary"} onClick={() => setControl(!control)}>{control ? "View only" : "Control"}</Button>
            <Button icon={<DocumentArrowDown20Regular />} disabled={!img} onClick={() => img && invoke("save_snapshot", { dataUrl: img })}>Snapshot</Button>
            <Button icon={<Person20Regular />} onClick={onClose}>Close</Button>
          </DialogActions>
        </DialogBody>
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
