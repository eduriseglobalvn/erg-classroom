import { useEffect, useState } from "react";
import { Button, Checkbox, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, Input } from "@fluentui/react-components";
import { Dismiss16Regular } from "@fluentui/react-icons";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { D, DlgHead, mut, RED } from "./FileDist";
import { loadGuard, parseList, saveGuard, tryGuard } from "./guardlist";
import { useT } from "./i18n";
import { useClass } from "./store";

const send = (ids: string[] | null) => {
  const g = loadGuard();
  return invoke("send_command", { ids, cmd: { SetGuard: g } });
};

/** Blocked websites and keywords the teacher adds on top of the built-in adult list (design N27). A student who opens
 *  one is locked with the warning screen and the Tutor gets an alert. Stays mounted so that late joiners get the lists too. */
export function GuardDialog({ open: shown, onClose, say }: { open: boolean; onClose: () => void; say: (s: string) => void }) {
  const t = useT();
  const [lists, setLists] = useState({ sites: [] as string[], words: [] as string[] });
  const [typed, setTyped] = useState({ sites: "", words: "" });
  const [find, setFind] = useState({ sites: "", words: "" });
  const [probe, setProbe] = useState("");
  const [probed, setProbed] = useState<string | null>(null);
  useEffect(() => {
    if (!shown) return;
    setLists(loadGuard());
    setProbed(null);
  }, [shown]);
  // a student that connects later (or reconnects) has empty lists: send ours again
  useEffect(() => {
    const un = listen<{ id: string; legacy?: boolean }>("student-online", (e) => {
      const g = loadGuard();
      // a student older than 0.2.0 drops the connection on a command it does not know: it gets the lists after its update
      if (!e.payload.legacy && (g.sites.length || g.words.length)) void send([e.payload.id]).catch(() => {});
    });
    return () => void un.then((f) => f());
  }, []);
  type K = "sites" | "words";
  const kindOf = (k: K) => (k === "sites" ? "site" : "word");
  const add = (k: K, text: string) => {
    const got = parseList(text, kindOf(k));
    if (!got.length) return say(t("Dòng không hợp lệ sẽ bị bỏ qua."));
    setLists((l) => ({ ...l, [k]: [...new Set([...l[k], ...got])].slice(0, 500) }));
    setTyped((x) => ({ ...x, [k]: "" }));
  };
  const importFile = async (k: K) => {
    try {
      const f = await open({ multiple: false, filters: [{ name: "Text", extensions: ["txt", "csv"] }] });
      if (f && !Array.isArray(f)) add(k, await invoke<string>("load_text", { path: f }));
    } catch (e) { say(String(e)); }
  };
  const exportFile = async (k: K) => {
    try {
      const path = await save({ defaultPath: k === "sites" ? "blocked-sites.txt" : "blocked-words.txt" });
      if (path) await invoke("write_text", { path, text: lists[k].join("\r\n") });
    } catch (e) { say(String(e)); }
  };
  const apply = async () => {
    saveGuard(lists);
    try {
      const n = await send(Object.values(useClass.getState().students).filter((x) => x.online && !x.legacy).map((x) => x.id));
      say(`${t("Đã gửi danh sách chặn tới")} ${n} ${t("máy")}`);
      onClose();
    } catch (e) {
      say(String(e));
    }
  };
  const line = "1px solid #E5E5E5";
  const pane = (k: K, title: string, desc: string, builtin: string, builtinDesc: string, ph: string, col: string, foot: string) => {
    const rows = lists[k].filter((x) => x.includes(find[k].trim().toLowerCase()));
    return (
      <section aria-label={title} style={{ flex: "1 1 0", minWidth: 0, display: "flex", flexDirection: "column", gap: 10, padding: "14px 16px", borderRight: k === "sites" ? line : undefined }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <b style={{ fontSize: 15 }}>{title}</b><span style={{ color: mut }}>{lists[k].length} {t("của bạn")}</span>
          <span style={{ flex: 1 }} />
          <Input size="small" style={{ width: 150 }} aria-label={`${t("Tìm")} ${title}`} placeholder={t("Tìm")} value={find[k]} onChange={(_, d) => setFind({ ...find, [k]: d.value })} />
        </div>
        <span style={{ color: mut, fontSize: 12 }}>{desc}</span>
        <div style={{ display: "flex", alignItems: "center", gap: 10, padding: "9px 10px", border: "1px solid #B9CDEE", borderRadius: 4, background: "#F3F7FD" }}>
          <Checkbox checked disabled aria-label={builtin} />
          <span style={{ display: "flex", flexDirection: "column", lineHeight: "17px", minWidth: 0 }}><b>{builtin}</b><span style={{ color: mut, fontSize: 12 }}>{builtinDesc}</span></span>
        </div>
        <form style={{ display: "flex", gap: 8 }} onSubmit={(e) => { e.preventDefault(); add(k, typed[k]); }}>
          <Input style={{ flex: 1 }} aria-label={title} placeholder={ph} value={typed[k]} onChange={(_, d) => setTyped({ ...typed, [k]: d.value })} />
          <Button appearance="primary" type="submit" disabled={!typed[k].trim()}>{t("Thêm")}</Button>
        </form>
        <div style={{ ...D.box, height: 260 }}>
          <div style={{ display: "flex", alignItems: "center", height: 30, padding: "0 4px 0 10px", borderBottom: line, background: "#FAFAFA", fontSize: 12, color: mut }}>
            <span style={{ flex: 1 }}>{col}</span><span style={{ width: 140 }}>{t("Khi vi phạm")}</span><span style={{ width: 32 }} />
          </div>
          {rows.map((x) => (
            <div key={x} style={{ display: "flex", alignItems: "center", height: 32, padding: "0 4px 0 10px", borderBottom: line }}>
              <span style={{ flex: 1, minWidth: 0, fontWeight: 600, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{x}</span>
              <span style={{ width: 140, display: "flex", alignItems: "center", gap: 7 }}><span style={{ width: 8, height: 8, borderRadius: 4, background: RED }} />{t("Khoá, hiện cảnh báo")}</span>
              <Button size="small" appearance="subtle" icon={<Dismiss16Regular />} aria-label={`${t("Xoá")} ${x}`} onClick={() => setLists((l) => ({ ...l, [k]: l[k].filter((y) => y !== x) }))} />
            </div>
          ))}
          {!rows.length && <div style={{ padding: 12, color: mut }}>{lists[k].length ? t("Không có mục nào khớp") : t("Chưa có mục nào")}</div>}
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <Button size="small" appearance="subtle" onClick={() => void importFile(k)}>{t("Nhập từ tệp…")}</Button>
          <Button size="small" appearance="subtle" disabled={!lists[k].length} onClick={() => void exportFile(k)}>{t("Xuất…")}</Button>
          <span style={{ flex: 1 }} /><span style={{ color: mut, fontSize: 12 }}>{foot}</span>
        </div>
      </section>
    );
  };
  const hit = probed === null ? null : tryGuard(probed, lists.sites, lists.words);
  return (
    <Dialog open={shown} onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ ...D.surface, maxWidth: 1040 }}>
        <DialogBody style={D.body}>
          <DlgHead title={t("Web và từ khoá bị chặn")} onClose={onClose} />
          <DialogContent style={{ ...D.content, padding: 0, gap: 0 }}>
            <div style={{ display: "flex", borderTop: line }}>
              {pane("sites", t("Trang web bị chặn"), t("Học sinh mở một trong các trang này sẽ bị chặn dù đang ở chế độ web nào. Dùng *.site.com cho mọi trang con."),
                t("Danh sách web người lớn có sẵn (18+)"), t("Có sẵn trong ERG Classroom · luôn khoá và cảnh báo · không sửa được"), "example.com", t("Trang web"), "")}
              {pane("words", t("Từ khoá bị chặn"), t("So theo nguyên từ trong tiêu đề và địa chỉ trang, nên “Essex” không khớp “sex”. Không đọc hay lưu nội dung nào khác."),
                t("Từ khoá người lớn có sẵn"), t("porn, sex, xxx và các từ khác · luôn khoá và cảnh báo"), t("Một từ hoặc cụm từ ngắn"), t("Từ khoá"), t("Không phân biệt hoa thường · dùng được tiếng Việt có dấu"))}
            </div>
            <form aria-label={t("Thử")} onSubmit={(e) => { e.preventDefault(); setProbed(probe); }}
              style={{ display: "flex", alignItems: "center", gap: 10, minHeight: 56, padding: "0 16px", background: "#FAFAFA", borderTop: line }}>
              <b>{t("Thử")}</b>
              <Input style={{ width: 380 }} aria-label={t("Địa chỉ hoặc tiêu đề trang để thử")} placeholder={t("Địa chỉ hoặc tiêu đề trang để thử")} value={probe} onChange={(_, d) => { setProbe(d.value); setProbed(null); }} />
              <Button type="submit" disabled={!probe.trim()}>{t("Kiểm tra")}</Button>
              {probed !== null && (hit
                ? <><span style={{ padding: "2px 8px", borderRadius: 10, background: "#FDE7E5", color: "#8E1F15", fontSize: 12 }}>{t("Sẽ khoá và cảnh báo")}</span><span style={{ color: mut, fontSize: 12 }}>{hit.kind === "site" ? t("khớp trang") : t("khớp từ khoá")} “{hit.hit}”</span></>
                : <span style={{ color: mut, fontSize: 12 }}>{t("Không khớp danh sách của bạn (danh sách 18+ có sẵn được kiểm tra trên máy học sinh)")}</span>)}
            </form>
          </DialogContent>
          <DialogActions style={D.foot}>
            <span style={{ color: mut, fontSize: 12, marginRight: "auto" }}>{t("Trang của học sinh vẫn mở phía sau màn hình khoá, để bạn xem khi mở khoá.")}</span>
            <Button appearance="primary" onClick={() => void apply()}>{t("Lưu và gửi cho cả lớp")}</Button>
            <Button onClick={onClose}>{t("Hủy")}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
