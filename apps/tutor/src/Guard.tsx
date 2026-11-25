import { useEffect, useState } from "react";
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, Textarea } from "@fluentui/react-components";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { D, DlgHead, mut } from "./FileDist";
import { loadGuard, parseList, saveGuard } from "./guardlist";
import { useT } from "./i18n";
import { useClass } from "./store";

const send = (ids: string[] | null) => {
  const g = loadGuard();
  return invoke("send_command", { ids, cmd: { SetGuard: g } });
};

/** Blocked websites and keywords the teacher adds on top of the built-in adult list. A student who opens one is locked
 *  with the warning screen and the Tutor gets an alert. Stays mounted so that late joiners get the lists too. */
export function GuardDialog({ open, onClose, say }: { open: boolean; onClose: () => void; say: (s: string) => void }) {
  const t = useT();
  const [sites, setSites] = useState("");
  const [words, setWords] = useState("");
  useEffect(() => {
    if (!open) return;
    const g = loadGuard();
    setSites(g.sites.join("\n"));
    setWords(g.words.join("\n"));
  }, [open]);
  // a student that connects later (or reconnects) has empty lists: send ours again
  useEffect(() => {
    const un = listen<{ id: string; legacy?: boolean }>("student-online", (e) => {
      const g = loadGuard();
      // a student older than 0.2.0 drops the connection on a command it does not know: it gets the lists after its update
      if (!e.payload.legacy && (g.sites.length || g.words.length)) void send([e.payload.id]).catch(() => {});
    });
    return () => void un.then((f) => f());
  }, []);
  const s = parseList(sites, "site"), w = parseList(words, "word");
  const apply = async () => {
    saveGuard({ sites: s, words: w });
    try {
      const n = await send(Object.values(useClass.getState().students).filter((x) => x.online && !x.legacy).map((x) => x.id));
      say(`${t("Đã gửi danh sách chặn tới")} ${n} ${t("máy")}`);
      onClose();
    } catch (e) {
      say(String(e));
    }
  };
  const col = { flex: "1 1 0", minWidth: 0, display: "flex", flexDirection: "column", gap: 6 } as const;
  return (
    <Dialog open={open} onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ ...D.surface, maxWidth: 760 }}>
        <DialogBody style={D.body}>
          <DlgHead title={t("Web và từ khoá bị chặn")} onClose={onClose} />
          <DialogContent style={D.content}>
            <span style={{ color: mut }}>{t("Web người lớn (18+) luôn bị chặn sẵn. Thêm trang và từ khoá của riêng bạn ở đây, mỗi dòng một mục. Học sinh mở phải sẽ bị khoá máy với màn hình cảnh báo và bạn nhận được thông báo; trang vẫn mở để bạn xem khi mở khoá.")}</span>
            <div style={{ display: "flex", gap: 16 }}>
              <label style={col}>
                <b>{t("Trang web bị chặn")} ({s.length})</b>
                <Textarea value={sites} onChange={(_, d) => setSites(d.value)} placeholder={"facebook.com\n*.garena.vn"} rows={12} resize="none" />
              </label>
              <label style={col}>
                <b>{t("Từ khoá bị chặn")} ({w.length})</b>
                <Textarea value={words} onChange={(_, d) => setWords(d.value)} placeholder={"casino\ncá độ"} rows={12} resize="none" />
              </label>
            </div>
            <span style={{ color: mut, fontSize: 12 }}>{t("Từ khoá được so theo nguyên từ trong tiêu đề và địa chỉ trang. Dòng không hợp lệ sẽ bị bỏ qua.")}</span>
          </DialogContent>
          <DialogActions style={D.foot}>
            <Button appearance="primary" onClick={() => void apply()}>{t("Lưu và gửi cho cả lớp")}</Button>
            <Button onClick={onClose}>{t("Hủy")}</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
