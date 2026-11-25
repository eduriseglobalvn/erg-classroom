import { type ReactNode, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Button, Checkbox, Input, MessageBar, MessageBarBody, Select, Text, makeStyles, mergeClasses, tokens } from "@fluentui/react-components";
import { Globe20Regular, Door20Regular, Shield20Regular, Window20Regular } from "@fluentui/react-icons";

/** Student Configurator (`erg-student-agent.exe --config`, as administrator): edits %ProgramData%\ERGClassroom\config.txt.
 *  Only settings the agent really reads are here: room, gateway, lang. English only: it is an IT tool. */
type Settings = Record<string, string>;
const PAGES = [
  { k: "network", label: "Network Settings", icon: <Globe20Regular /> },
  { k: "room", label: "Room", icon: <Door20Regular /> },
  { k: "security", label: "Security", icon: <Shield20Regular /> },
  { k: "ui", label: "User Interface", icon: <Window20Regular /> },
] as const;

const stroke = `1px solid ${tokens.colorNeutralStroke2}`;
const useStyles = makeStyles({
  root: { display: "flex", flexDirection: "column", height: "100%", fontSize: "13px", color: tokens.colorNeutralForeground1, background: tokens.colorNeutralBackground3 },
  body: { flex: 1, minHeight: 0, display: "flex" },
  nav: { width: "200px", flex: "0 0 auto", display: "flex", flexDirection: "column", gap: "2px", padding: "8px 4px", borderRight: stroke },
  navBtn: {
    position: "relative", display: "flex", alignItems: "center", gap: "10px", height: "36px", padding: "0 12px", border: "0", borderRadius: tokens.borderRadiusMedium,
    background: "transparent", color: "inherit", font: "inherit", textAlign: "left", cursor: "pointer",
    ":hover": { background: tokens.colorSubtleBackgroundHover }, ":focus-visible": { outline: `2px solid ${tokens.colorStrokeFocus2}`, outlineOffset: "-2px" },
  },
  navOn: {
    background: tokens.colorSubtleBackgroundSelected, fontWeight: 600,
    "::before": { content: '""', position: "absolute", left: 0, top: "10px", bottom: "10px", width: "3px", borderRadius: "2px", background: tokens.colorBrandBackground },
  },
  page: { flex: 1, minWidth: 0, overflow: "auto", padding: "16px 24px", display: "flex", flexDirection: "column", gap: "4px" },
  h: { fontSize: "20px", fontWeight: 600, lineHeight: "28px", margin: "0 0 12px" },
  // SettingsCard-style row: label and description on the left, the control on the right
  row: { display: "flex", alignItems: "center", gap: "16px", minHeight: "60px", padding: "8px 16px", boxSizing: "border-box", background: tokens.colorNeutralBackground1, border: stroke, borderRadius: tokens.borderRadiusMedium },
  rowText: { flex: 1, minWidth: 0, display: "flex", flexDirection: "column" },
  desc: { fontSize: "12px", color: tokens.colorNeutralForeground3 },
  foot: { display: "flex", alignItems: "center", gap: "8px", padding: "12px 24px", borderTop: stroke, background: tokens.colorNeutralBackground2 },
});

export function ConfigView() {
  const s = useStyles();
  const [page, setPage] = useState<(typeof PAGES)[number]["k"]>("network");
  const [cfg, setCfg] = useState<Settings>({});
  const [file, setFile] = useState("");
  const [keyOk, setKeyOk] = useState(true);
  const [msg, setMsg] = useState<{ err: boolean; text: string } | null>(null);
  useEffect(() => {
    invoke<[Settings, string, boolean]>("config_load").then(([c, f, k]) => { setCfg(c); setFile(f); setKeyOk(k); }, (e) => setMsg({ err: true, text: String(e) }));
  }, []);
  const set = (k: string, v: string) => { setCfg((c) => ({ ...c, [k]: v })); setMsg(null); };
  const save = () => invoke("config_save", { settings: cfg }).then(
    () => { setMsg({ err: false, text: "Saved. The student program picks the changes up by itself within a few seconds." }); return true; },
    (e) => { setMsg({ err: true, text: String(e) }); return false; });
  const close = () => void getCurrentWindow().close();

  const row = (id: string, label: string, desc: string, control: ReactNode) => (
    <div className={s.row}>
      <div className={s.rowText}><label htmlFor={id}>{label}</label><span className={s.desc}>{desc}</span></div>
      {control}
    </div>
  );
  const gatewayOn = cfg.gateway !== undefined && cfg.gateway !== "";

  return (
    <div className={s.root}>
      <div className={s.body}>
        <nav className={s.nav} aria-label="Settings pages">
          {PAGES.map((p) => (
            <button key={p.k} type="button" className={mergeClasses(s.navBtn, page === p.k && s.navOn)} aria-current={page === p.k ? "page" : undefined} onClick={() => setPage(p.k)}>{p.icon}{p.label}</button>
          ))}
        </nav>
        <main className={s.page}>
          <h1 className={s.h}>{PAGES.find((p) => p.k === page)!.label}</h1>
          {page === "network" && <>
            {row("cf-bc", "Find the Tutor on the local network", "Always on. Works when Tutor and students are on the same subnet.", <Text className={s.desc}>Broadcast</Text>)}
            {row("cf-gw-on", "Use Name & Connectivity Server", "Needed when the Tutor is on another subnet or VLAN.",
              <Checkbox id="cf-gw-on" checked={gatewayOn} onChange={(_, d) => set("gateway", d.checked ? "10.0.0.1:47810" : "")} />)}
            {row("cf-gw", "Server address", "IP address and port, for example 10.0.0.5:47810.",
              <Input id="cf-gw" disabled={!gatewayOn} value={cfg.gateway ?? ""} onChange={(_, d) => set("gateway", d.value)} style={{ width: 200 }} />)}
          </>}
          {page === "room" && row("cf-room", "This computer is located in room", "The Tutor connects to every student computer with the same room name.",
            <Input id="cf-room" placeholder="default" value={cfg.room ?? ""} onChange={(_, d) => set("room", d.value)} style={{ width: 200 }} />)}
          {page === "security" && <>
            {row("cf-key", "Room Key", "Only a Tutor with the same key can control this computer. It is set by the installer and cannot be changed here.",
              <Text id="cf-key" style={{ color: keyOk ? tokens.colorPaletteGreenForeground1 : tokens.colorPaletteRedForeground1 }}>{keyOk ? "Installed" : "Missing"}</Text>)}
          </>}
          {page === "ui" && row("cf-lang", "Language of student windows", "Lock screen, messages, tests and the student toolbar.",
            <Select id="cf-lang" value={cfg.lang === "en" ? "en" : "vi"} onChange={(_, d) => set("lang", d.value)}>
              <option value="vi">Tiếng Việt</option>
              <option value="en">English</option>
            </Select>)}
          <div style={{ flex: 1 }} />
          {msg && <MessageBar intent={msg.err ? "error" : "success"}><MessageBarBody>{msg.text}</MessageBarBody></MessageBar>}
        </main>
      </div>
      <div className={s.foot}>
        <Text className={s.desc} style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{file}</Text>
        <Button appearance="primary" onClick={() => void save().then((ok) => ok && close())}>OK</Button>
        <Button onClick={close}>Cancel</Button>
        <Button onClick={() => void save()}>Apply</Button>
      </div>
    </div>
  );
}
