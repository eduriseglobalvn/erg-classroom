import { useEffect, useState } from "react";
import { Badge, Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, Input, tokens } from "@fluentui/react-components";
import { Add16Regular, ArrowExport16Regular, ArrowImport16Regular, CheckmarkCircle16Filled, Dismiss16Regular, Search16Regular, ShieldCheckmark20Filled, Warning16Filled } from "@fluentui/react-icons";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { D, DlgHead, mut, RED } from "./FileDist";
import { loadGuard, parseList, saveGuard, STARTER, tryGuard } from "./guardlist";
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
  const line = `1px solid ${tokens.colorNeutralStroke2}`;
  const pane = (k: K, title: string, desc: string, builtin: string, builtinDesc: string, ph: string, col: string, foot: string) => {
    const rows = lists[k].filter((x) => x.includes(find[k].trim().toLowerCase()));
    return (
      <section aria-label={title} style={{ flex: "1 1 0", minWidth: 0, display: "flex", flexDirection: "column", gap: 10, padding: "14px 16px", borderRight: k === "sites" ? line : undefined }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <b style={{ fontSize: 15 }}>{title}</b><Badge appearance="tint" color="informative">{lists[k].length}</Badge>
          <span style={{ flex: 1 }} />
          <Input size="small" style={{ width: 160 }} contentBefore={<Search16Regular />} aria-label={`${t("Tìm")} ${title}`} placeholder={t("Tìm")} value={find[k]} onChange={(_, d) => setFind({ ...find, [k]: d.value })} />
        </div>
        <span style={{ color: mut, fontSize: 12, lineHeight: "16px", minHeight: 32 }}>{desc}</span>
        <div style={{ display: "flex", alignItems: "center", gap: 10, padding: "8px 12px", borderRadius: 6, background: tokens.colorBrandBackground2, border: `1px solid ${tokens.colorBrandStroke2}` }}>
          <ShieldCheckmark20Filled style={{ color: tokens.colorBrandForeground1, flex: "none" }} />
          <span style={{ display: "flex", flexDirection: "column", lineHeight: "17px", minWidth: 0, flex: 1 }}><b>{builtin}</b><span style={{ color: mut, fontSize: 12 }}>{builtinDesc}</span></span>
          <Badge appearance="filled" color="brand">{t("Luôn bật")}</Badge>
        </div>
        <form style={{ display: "flex", gap: 8 }} onSubmit={(e) => { e.preventDefault(); add(k, typed[k]); }}>
          <Input style={{ flex: 1 }} aria-label={title} placeholder={ph} value={typed[k]} onChange={(_, d) => setTyped({ ...typed, [k]: d.value })} />
          <Button appearance="primary" type="submit" icon={<Add16Regular />} disabled={!typed[k].trim()}>{t("Thêm")}</Button>
        </form>
        <div style={{ ...D.box, height: 260 }}>
          <div style={{ display: "flex", alignItems: "center", height: 30, padding: "0 4px 0 10px", borderBottom: line, background: tokens.colorNeutralBackground2, fontSize: 12, color: mut, position: "sticky", top: 0, zIndex: 1 }}>
            <span style={{ flex: 1 }}>{col}</span><span style={{ width: 140 }}>{t("Khi vi phạm")}</span><span style={{ width: 32 }} />
          </div>
          {rows.map((x) => (
            <div key={x} style={{ display: "flex", alignItems: "center", height: 32, padding: "0 4px 0 10px", borderBottom: `1px solid ${tokens.colorNeutralBackground4}` }}>
              <span style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{x}</span>
              <span style={{ width: 140, display: "flex", alignItems: "center", gap: 7, fontSize: 12, color: mut }}><span style={{ width: 8, height: 8, borderRadius: 4, background: RED, flex: "none" }} />{t("Khoá, hiện cảnh báo")}</span>
              <Button size="small" appearance="subtle" icon={<Dismiss16Regular />} aria-label={`${t("Xoá")} ${x}`} onClick={() => setLists((l) => ({ ...l, [k]: l[k].filter((y) => y !== x) }))} />
            </div>
          ))}
          {!rows.length && (
            <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 8, padding: "40px 12px", color: mut }}>
              {lists[k].length ? t("Không có mục nào khớp") : t("Chưa có mục nào")}
              {!lists[k].length && <Button size="small" onClick={() => setLists((l) => ({ ...l, [k]: [...STARTER[k]] }))}>{t("Dùng danh sách mẫu")}</Button>}
            </div>
          )}
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <Button size="small" style={{ whiteSpace: "nowrap", flex: "none" }} icon={<ArrowImport16Regular />} onClick={() => void importFile(k)}>{t("Nhập từ tệp…")}</Button>
          <Button size="small" style={{ whiteSpace: "nowrap", flex: "none" }} icon={<ArrowExport16Regular />} disabled={!lists[k].length} onClick={() => void exportFile(k)}>{t("Xuất…")}</Button>
          {lists[k].length > 0 && <Button size="small" appearance="subtle" onClick={() => setLists((l) => ({ ...l, [k]: [] }))}>{t("Xoá hết")}</Button>}
          <span style={{ flex: 1 }} /><span style={{ color: mut, fontSize: 12, textAlign: "right" }}>{foot}</span>
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
              style={{ display: "flex", alignItems: "center", gap: 10, minHeight: 56, padding: "0 16px", background: tokens.colorNeutralBackground2, borderTop: line }}>
              <b style={{ whiteSpace: "nowrap" }}>{t("Thử")}</b>
              <Input style={{ width: 380 }} contentBefore={<Search16Regular />} aria-label={t("Địa chỉ hoặc tiêu đề trang để thử")} placeholder={t("Địa chỉ hoặc tiêu đề trang để thử")} value={probe} onChange={(_, d) => { setProbe(d.value); setProbed(null); }} />
              <Button type="submit" disabled={!probe.trim()}>{t("Kiểm tra")}</Button>
              {probed !== null && (hit
                ? <><Badge appearance="tint" color="danger" icon={<Warning16Filled />}>{t("Sẽ khoá và cảnh báo")}</Badge><span style={{ color: mut, fontSize: 12 }}>{hit.kind === "site" ? t("khớp trang") : t("khớp từ khoá")} “{hit.hit}”</span></>
                : <><Badge appearance="tint" color="success" icon={<CheckmarkCircle16Filled />}>{t("Không khớp")}</Badge><span style={{ color: mut, fontSize: 12 }}>{t("Không khớp danh sách của bạn (danh sách 18+ có sẵn được kiểm tra trên máy học sinh)")}</span></>)}
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
