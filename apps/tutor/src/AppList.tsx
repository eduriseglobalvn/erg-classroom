import { useEffect, useState } from "react";
import { Button, Checkbox, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, Input, tokens } from "@fluentui/react-components";
import { D, DlgHead, mut } from "./FileDist";
import { useT } from "./i18n";

type Mode = "BlockList" | "AllowListOnly";
/** Ready-made choices. Block: Windows' own tools a student uses to get around the class rules (checked by default).
 *  Allow: the usual lesson programs; Windows' own programs are never ended in allow-only mode anyway. */
export const PRESETS: Record<Mode, { name: string; exe: string[]; on: boolean }[]> = {
  BlockList: [
    { name: "Cài đặt Windows", exe: ["systemsettings.exe"], on: true },
    { name: "Control Panel", exe: ["control.exe"], on: true },
    { name: "Task Manager", exe: ["taskmgr.exe"], on: true },
    { name: "Command Prompt", exe: ["cmd.exe"], on: true },
    { name: "PowerShell", exe: ["powershell.exe", "pwsh.exe", "powershell_ise.exe"], on: true },
    { name: "Windows Terminal", exe: ["windowsterminal.exe"], on: true },
    { name: "Registry Editor", exe: ["regedit.exe"], on: true },
    { name: "Công cụ quản trị (MMC)", exe: ["mmc.exe"], on: true },
    { name: "Microsoft Store", exe: ["winstore.app.exe"], on: true },
    { name: "Xbox / Game Bar", exe: ["xboxpcapp.exe", "gamebar.exe"], on: false },
    { name: "Steam", exe: ["steam.exe"], on: false },
    { name: "Roblox", exe: ["robloxplayerbeta.exe"], on: false },
    { name: "Garena", exe: ["garena.exe"], on: false },
    { name: "Zalo", exe: ["zalo.exe"], on: false },
    { name: "Messenger", exe: ["messenger.exe"], on: false },
  ],
  AllowListOnly: [
    { name: "Word", exe: ["winword.exe"], on: true },
    { name: "Excel", exe: ["excel.exe"], on: true },
    { name: "PowerPoint", exe: ["powerpnt.exe"], on: true },
    { name: "Microsoft Edge", exe: ["msedge.exe"], on: false },
    { name: "Google Chrome", exe: ["chrome.exe"], on: false },
    { name: "Scratch", exe: ["scratch 3.exe"], on: false },
    { name: "Visual Studio Code", exe: ["code.exe"], on: false },
    { name: "Python", exe: ["python.exe", "pythonw.exe"], on: false },
  ],
};
const key = (m: Mode) => `erg.apps.${m}`;
/** The list last sent in this mode, or the presets that are on by default. */
export function loadApps(m: Mode): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(key(m)) ?? "null");
    if (Array.isArray(v)) return v;
  } catch { /* fall through */ }
  return PRESETS[m].filter((p) => p.on).flatMap((p) => p.exe);
}
const parse = (t: string) => t.split(/[\s,;]+/).map((s) => s.trim().toLowerCase()).filter((s) => /^[^\\/:*?"<>|]+\.exe$/.test(s));

/** Block or allow-only applications: ready-made rows plus the teacher's own .exe names (design N26). */
export function AppListDialog({ mode, onClose, onSend }: { mode: Mode | null; onClose: () => void; onSend: (m: Mode, list: string[]) => void }) {
  const t = useT();
  const [list, setList] = useState<string[]>([]);
  const [typed, setTyped] = useState("");
  useEffect(() => { if (mode) { setList(loadApps(mode)); setTyped(""); } }, [mode]);
  if (!mode) return null;
  const presets = PRESETS[mode];
  const preset = new Set(presets.flatMap((p) => p.exe));
  const own = list.filter((x) => !preset.has(x));
  const toggle = (exe: string[], on: boolean) => setList((l) => on ? [...new Set([...l, ...exe])] : l.filter((x) => !exe.includes(x)));
  const add = () => { const got = parse(typed); if (got.length) { setList((l) => [...new Set([...l, ...got])]); setTyped(""); } };
  const final = [...new Set([...list, ...parse(typed)])];
  const block = mode === "BlockList";
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ ...D.surface, maxWidth: 640 }}>
        <DialogBody style={D.body}>
          <DlgHead title={block ? t("Chặn ứng dụng") : t("Chỉ cho phép ứng dụng")} onClose={onClose} />
          <DialogContent style={D.content}>
            <span style={{ color: mut, fontSize: 12 }}>
              {block ? t("Ứng dụng được chọn sẽ bị đóng ngay khi học sinh mở.") : t("Mọi ứng dụng khác sẽ bị đóng. Các chương trình của Windows (Explorer, bộ gõ…) vẫn chạy.")}
            </span>
            <div style={{ ...D.box, display: "grid", gridTemplateColumns: "1fr 1fr", padding: "4px 0", maxHeight: 300 }}>
              {presets.map((p) => (
                <Checkbox key={p.name} checked={p.exe.every((x) => list.includes(x))} onChange={(_, d) => toggle(p.exe, !!d.checked)}
                  label={<span style={{ display: "flex", flexDirection: "column", lineHeight: "16px" }}>{t(p.name)}<span style={{ color: mut, fontSize: 11 }}>{p.exe.join(", ")}</span></span>} />
              ))}
            </div>
            <form style={{ display: "flex", gap: 8 }} onSubmit={(e) => { e.preventDefault(); add(); }}>
              <Input style={{ flex: 1 }} aria-label={t("Thêm tên tệp .exe")} placeholder={block ? "game.exe, chrome.exe" : "winword.exe, excel.exe"} value={typed} onChange={(_, d) => setTyped(d.value)} />
              <Button type="submit" disabled={!parse(typed).length}>{t("Thêm")}</Button>
            </form>
            {own.length > 0 && (
              <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
                {own.map((x) => (
                  <Button key={x} size="small" shape="circular" style={{ background: tokens.colorNeutralBackground3 }} aria-label={`${t("Xoá")} ${x}`} onClick={() => setList((l) => l.filter((y) => y !== x))}>{x} ✕</Button>
                ))}
              </div>
            )}
          </DialogContent>
          <DialogActions style={D.foot}>
            <span style={{ color: mut, fontSize: 12, marginRight: "auto" }}>{final.length} {t("ứng dụng")}</span>
            <Button appearance="primary" disabled={!final.length} onClick={() => { localStorage.setItem(key(mode), JSON.stringify(final)); onSend(mode, final); onClose(); }}>{t("Gửi")}</Button>
            <Button onClick={onClose}>{t("Hủy")}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
