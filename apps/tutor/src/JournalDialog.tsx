import { useEffect, useMemo, useState } from "react";
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, Input, Switch, Text } from "@fluentui/react-components";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { clockOf, filterEntries, isToday, type JEntry } from "./journal";
import { useT } from "./i18n";
import { D, DlgHead, mut, td, th } from "./FileDist";

/** Lesson journal viewer: recent events, filter box, HTML export via the Rust report code. */
export function JournalDialog({ fail, say, onClose }: { fail: (s: string) => void; say: (s: string) => void; onClose: () => void }) {
  const t = useT();
  const [all, setAll] = useState<JEntry[]>([]);
  const [q, setQ] = useState("");
  const [today, setToday] = useState(true);
  const load = () => invoke<JEntry[]>("journal_list", { limit: 1000 }).then(setAll).catch((e) => fail(String(e)));
  useEffect(() => { load(); }, []);
  const rows = useMemo(() => filterEntries(today ? all.filter((e) => isToday(e.ts)) : all, q), [all, q, today]);
  const exportHtml = async () => {
    try {
      const html = await invoke<string>("journal_html", { title: t("Nhật ký buổi học"), objectives: [] });
      const path = await save({ defaultPath: "nhat-ky.html" });
      if (path) { await invoke("write_text", { path, text: html }); say(t("Đã xuất nhật ký")); }
    } catch (e) { fail(String(e)); }
  };
  // PDF: print the same report from a hidden frame; the Windows print dialog offers "Microsoft Print to PDF"
  const exportPdf = async () => {
    try {
      const html = await invoke<string>("journal_html", { title: t("Nhật ký buổi học"), objectives: [] });
      const f = document.createElement("iframe");
      f.style.cssText = "position:fixed;width:0;height:0;border:0";
      f.onload = () => { f.contentWindow?.print(); setTimeout(() => f.remove(), 1000); };
      f.srcdoc = html;
      document.body.appendChild(f);
    } catch (e) { fail(String(e)); }
  };
  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ ...D.surface, maxWidth: 760 }}>
        <DialogBody style={D.body}>
          <DlgHead title={t("Nhật ký buổi học")} onClose={onClose} />
          <DialogContent style={D.content}>
            <div style={{ display: "flex", gap: 12, alignItems: "center" }}>
              <Input style={{ flex: 1 }} placeholder={t("Lọc theo giờ, học sinh, loại, nội dung…")} value={q} onChange={(_, d) => setQ(d.value)} />
              <Switch label={t("Chỉ hôm nay")} checked={today} onChange={(_, d) => setToday(d.checked)} />
            </div>
            <div style={{ ...D.box, height: 360 }}>
              <table style={{ width: "100%", borderCollapse: "collapse" }}>
                <thead><tr><th style={{ ...th, width: 56 }}>{t("Giờ")}</th><th style={th}>{t("Học sinh")}</th><th style={th}>{t("Loại")}</th><th style={th}>{t("Nội dung")}</th></tr></thead>
                <tbody>{rows.map((e, i) => (
                  <tr key={i}><td style={{ ...td, color: mut, fontSize: 12 }}>{clockOf(e.ts)}</td><td style={td}>{e.who}</td><td style={td}>{e.kind}</td><td style={{ ...td, whiteSpace: "normal", wordBreak: "break-word" }}>{e.detail}</td></tr>
                ))}</tbody>
              </table>
              {!rows.length && <Text block style={{ padding: 12, color: mut }}>{t("Chưa có sự kiện nào")}</Text>}
            </div>
          </DialogContent>
          <DialogActions style={D.foot}>
            <Button onClick={load}>{t("Tải lại")}</Button>
            <Button onClick={exportHtml}>{t("Xuất HTML")}</Button>
            <Button onClick={exportPdf}>{t("Xuất PDF")}</Button>
            <Button appearance="primary" onClick={onClose}>{t("Đóng")}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
