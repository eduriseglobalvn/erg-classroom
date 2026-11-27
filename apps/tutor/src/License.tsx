import { useEffect, useState } from "react";
import { Badge, Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, MessageBar, MessageBarActions, MessageBarBody, Textarea, tokens } from "@fluentui/react-components";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { D, DlgHead, mut } from "./FileDist";
import { useT } from "./i18n";

/** `seats` 0 = no limit; `waiting` = student computers left out because every seat is taken (main.rs LicenseDto). */
type Lic = { state: "open" | "ok" | "none" | "expired" | "invalid"; school: string; seats: number; expires: number; waiting: number };

/** The warning bar while students are left out, and the dialog (Help > Licence) that shows the licence and takes a key. */
export function License({ open, onOpen, onClose, say }: { open: boolean; onOpen: () => void; onClose: () => void; say: (s: string) => void }) {
  const t = useT();
  const [lic, setLic] = useState<Lic | null>(null);
  const [key, setKey] = useState("");
  const [bad, setBad] = useState("");
  useEffect(() => {
    invoke<Lic>("license_get").then(setLic, () => {});
    const un = listen<number>("license-waiting", (e) => setLic((l) => l && { ...l, waiting: e.payload }));
    return () => { void un.then((f) => f()); };
  }, []);
  useEffect(() => { if (open) { setKey(""); setBad(""); invoke<Lic>("license_get").then(setLic, () => {}); } }, [open]);
  if (!lic) return null;
  const activate = () => invoke<Lic>("license_set", { key }).then(
    (l) => { setLic(l); say(t("Đã kích hoạt bản quyền")); onClose(); },
    (e) => setBad(String(e) === "expired" ? t("Key này đã hết hạn.") : t("Key không hợp lệ. Kiểm tra lại đã chép đủ chưa.")),
  );
  const status = { open: "", ok: t("Đã kích hoạt"), none: t("Dùng thử"), expired: t("Hết hạn"), invalid: t("Key đã lưu không hợp lệ") }[lic.state];
  const rows: [string, string][] = [
    [t("Trường"), lic.school || "–"],
    [t("Số máy học sinh"), String(lic.seats)],
    [t("Hạn dùng"), lic.state === "none" || lic.state === "invalid" ? "–" : lic.expires ? new Date(lic.expires * 1000).toLocaleDateString() : t("Không thời hạn")],
  ];
  const line = `1px solid ${tokens.colorNeutralStroke2}`;
  return (
    <>
      {lic.waiting > 0 && (
        <MessageBar intent="warning" shape="square">
          <MessageBarBody>{lic.waiting} {t("máy chưa được kết nối: bản quyền hiện tại cho")} {lic.seats} {t("máy")}.</MessageBarBody>
          <MessageBarActions><Button size="small" appearance="primary" onClick={onOpen}>{t("Nhập key…")}</Button></MessageBarActions>
        </MessageBar>
      )}
      {open && (
        <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
          <DialogSurface style={{ ...D.surface, maxWidth: 480 }}>
            <DialogBody style={D.body}>
              <DlgHead title={t("Bản quyền")} onClose={onClose} />
              <DialogContent style={{ ...D.content, gap: 12 }}>
                {lic.state === "open" ? <span>{t("Bản này không cần key và không giới hạn số máy.")}</span> : <>
                  <div style={{ border: line, borderRadius: 4 }}>
                    <div style={{ display: "flex", gap: 12, minHeight: 36, alignItems: "center", padding: "0 12px", borderBottom: line }}>
                      <b style={{ flex: 1 }}>ERG Classroom</b><Badge appearance="tint" color={lic.state === "ok" ? "success" : lic.state === "none" ? "informative" : "danger"}>{status}</Badge>
                    </div>
                    {rows.map(([a, b]) => (
                      <div key={a} style={{ display: "flex", gap: 12, minHeight: 32, alignItems: "center", padding: "0 12px", borderBottom: line }}>
                        <span style={{ width: 130, color: mut }}>{a}</span><span style={{ flex: 1, minWidth: 0, wordBreak: "break-word" }}>{b}</span>
                      </div>
                    ))}
                  </div>
                  <Textarea rows={4} aria-label={t("Dán key bản quyền vào đây")} placeholder={t("Dán key bản quyền vào đây")} value={key} onChange={(_, d) => { setKey(d.value); setBad(""); }} style={{ fontFamily: "Consolas, monospace" }} />
                  {bad && <span role="alert" style={{ color: tokens.colorPaletteRedForeground1, fontSize: 12 }}>{bad}</span>}
                </>}
              </DialogContent>
              <DialogActions style={D.foot}>
                {lic.state !== "open" && <Button appearance="primary" disabled={!key.trim()} onClick={() => void activate()}>{t("Kích hoạt")}</Button>}
                <Button onClick={onClose}>{t("Đóng")}</Button>
              </DialogActions>
            </DialogBody>
          </DialogSurface>
        </Dialog>
      )}
    </>
  );
}
