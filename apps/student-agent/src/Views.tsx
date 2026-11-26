import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, Card, Input, Text, tokens } from "@fluentui/react-components";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { lang, t } from "./i18n";

const w = (vi: string, en: string) => (lang === "en" ? en : vi);

/** The app icon (screen + rising arrow), same drawing as the program icon. */
export const Mark = ({ s }: { s: number }) => (
  <svg viewBox="0 0 256 256" width={s} height={s} role="img" aria-label="ERG Classroom" style={{ display: "block", flex: "none" }}>
    <defs><linearGradient id="nmk-erg" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stopColor="#2B2BD0" /><stop offset="1" stopColor="#000066" /></linearGradient></defs>
    <rect x="4" y="4" width="248" height="248" rx="56" fill="url(#nmk-erg)" />
    <rect x="42" y="66" width="172" height="114" rx="14" fill="none" stroke="#FFFFFF" strokeWidth="13" />
    <path d="M128 182 V208 M98 210 H158" fill="none" stroke="#FFFFFF" strokeWidth="13" strokeLinecap="round" />
    <path d="M52 206 C78 150 128 150 168 104 L150 92 L214 60 L208 132 L190 118 C150 170 96 170 52 206 Z" transform="translate(26 -22) scale(0.94)" fill="#FF3B36" stroke="#0A0A8C" strokeWidth="9" strokeLinejoin="round" paintOrder="stroke" />
  </svg>
);

/** Full-screen lock. `blank` shows plain black with no text. */
export function LockView({ blank, text, warn, app }: { blank: boolean; text: string; warn?: string; app?: string }) {
  // normal lock only: lesson title, (computer, room) and a clock
  const [les, setLes] = useState("");
  const [info, setInfo] = useState<[string, string, string]>(["", "", ""]);
  const [now, setNow] = useState(() => new Date());
  const [at] = useState(() => new Date()); // when the warning lock came up
  useEffect(() => {
    invoke<{ title: string }>("lesson_state").then((l) => setLes(l.title)).catch(() => {});
    invoke<[string, string, string]>("toolbar_info").then(setInfo).catch(() => {});
    const i = setInterval(() => setNow(new Date()), 10000);
    return () => clearInterval(i);
  }, []);
  // Warning lock: the student opened a forbidden website or system tool. `text` is the window title that caused it.
  // A system tool gets the yellow screen, a website the red one.
  if (warn) {
    const tool = warn === "tool";
    const fg = tool ? "#1B1B1B" : "#fff", soft = tool ? "#3A2A00" : "#F3D4D0", label = tool ? "#3A2A00" : "#E0B3AD";
    // website: yellow/black stripes; program: solid black bars
    const bar = tool ? "#1B1B1B" : "repeating-linear-gradient(135deg, #F2B600 0 24px, #1B1B1B 24px 48px)";
    const what = warn === "adult" ? w("Trang web người lớn (18+)", "A website in the group Adult content (18+)") : warn === "word" ? w("Trang có từ khoá bị chặn", "A page with a blocked keyword") : w("Trang web bị chặn", "A blocked website");
    const when = `${at.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" })} · ${at.toLocaleDateString()}`;
    const student = info[2] ? `${info[2]} · ${info[0]}` : info[0];
    const rows: [string, string][] = tool
      ? [[w("Chương trình", "Program"), `${text || app} (${app})`], [w("Lý do", "Reason"), w("Công cụ hệ thống có thể dừng hoặc thay đổi phần mềm lớp học, nên bị tắt trong giờ học", "System tools can stop or change the classroom software, so they are off during a lesson")], [w("Lúc", "When"), when], [w("Học sinh", "Student"), student]]
      : [[w("Nội dung", "What"), `${what}${text ? ` · ${text}` : ""}`], [w("Chương trình", "Program"), app ?? ""], [w("Lúc", "When"), when], [w("Học sinh", "Student"), student]];
    return (
      <div role="alertdialog" aria-label={tool ? w("Chương trình không được phép", "Program not allowed") : w("Truy cập trái phép", "Unauthorised access")} style={{ position: "relative", height: "100%", display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", gap: 16, padding: 24, boxSizing: "border-box", background: tool ? "#F2B600" : "#4A120D", color: fg, userSelect: "none", cursor: "none", textAlign: "center" }}>
        <div style={{ position: "absolute", left: 0, right: 0, top: 0, height: 8, background: bar }} />
        <div style={{ position: "absolute", left: 0, right: 0, bottom: 0, height: 8, background: bar }} />
        <svg viewBox="0 0 64 64" width="84" height="84" aria-hidden="true"><path d="M32 6l28 50H4z" fill={tool ? "#1B1B1B" : "#F2B600"} /><path d="M32 24v16M32 47h.01" fill="none" stroke={tool ? "#F2B600" : "#1B1B1B"} strokeWidth="5" strokeLinecap="round" /></svg>
        <span style={{ fontSize: 34, fontWeight: 600, lineHeight: "42px" }}>{tool ? w("Chương trình không được phép", "Program not allowed") : w("Truy cập trái phép", "Unauthorised access")}</span>
        <span style={{ fontSize: 17, lineHeight: "26px", maxWidth: 760, color: soft }}>
          {tool ? w("Em đã mở một chương trình không được phép trong giờ học. Máy đã bị khoá và giáo viên đã được báo.", "You opened a program that is not allowed in this lesson. This computer is locked and your teacher has been told.")
            : w("Em đã mở nội dung không được phép trong giờ học. Nội dung đó đã bị đóng, máy bị khoá và giáo viên đã được báo.", "You opened something that is not allowed in this lesson. It was closed, this computer is locked, and your teacher has been told.")}
        </span>
        <div style={{ width: tool ? 640 : 560, maxWidth: "100%", marginTop: 8, border: `1px solid ${tool ? "rgba(0,0,0,.35)" : "rgba(255,255,255,.22)"}`, borderRadius: 6, background: tool ? "rgba(255,255,255,.35)" : "rgba(0,0,0,.22)", textAlign: "left" }}>
          {rows.map(([l, v], i) => (
            <div key={l} style={{ display: "flex", gap: 12, minHeight: 36, alignItems: "center", padding: "6px 16px", boxSizing: "border-box", borderTop: i ? `1px solid ${tool ? "rgba(0,0,0,.12)" : "rgba(255,255,255,.1)"}` : undefined }}>
              <span style={{ width: 120, flex: "none", color: label }}>{l}</span><span style={{ fontWeight: 600, overflowWrap: "anywhere" }}>{v}</span>
            </div>
          ))}
        </div>
        <span style={{ marginTop: 6, fontSize: 15, color: soft }}>{w("Chỉ giáo viên mới mở khoá được máy này. Hãy giơ tay và chờ.", "Only your teacher can unlock this computer. Raise your hand and wait.")}</span>
        <div style={{ position: "absolute", left: 24, right: 24, bottom: 24, display: "flex", alignItems: "center", gap: 8, fontSize: 12, color: tool ? "#3A2A00" : "#D9A7A0" }}>
          <Mark s={18} /><span>ERG Classroom{les ? ` · ${les}` : ""}</span><span style={{ flex: 1 }} /><span>{w("Sự việc này được lưu trong nhật ký buổi học", "This event is saved in the lesson journal")}</span>
        </div>
      </div>
    );
  }
  return (
    <div
      style={{
        position: "relative", height: "100%", display: "grid", placeItems: "center",
        background: blank ? "#000" : "#101B33", color: "#fff", userSelect: "none", cursor: "none",
      }}
    >
      {!blank && (
        <>
          <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 14, textAlign: "center", maxWidth: 760, padding: 24 }}>
            <Mark s={56} />
            <span style={{ fontSize: 26, fontWeight: 600, lineHeight: "34px", overflowWrap: "anywhere" }}>{text || w("Giáo viên đã khoá máy này", "Your teacher has locked this computer")}</span>
            <span style={{ fontSize: 15, color: "#C9D3E0" }}>{w("Em hãy nhìn lên giáo viên. Bài làm của em vẫn còn nguyên khi tiết học tiếp tục.", "Please look at the teacher. Your work is safe and will be here when the lesson goes on.")}</span>
            {les && (
              <div style={{ marginTop: 18, padding: "12px 20px", border: "1px solid rgba(255,255,255,.18)", borderRadius: 6, fontSize: 13 }}>
                <div style={{ fontWeight: 600 }}>{les}</div>
                <div style={{ fontSize: 12, color: "#C9D3E0" }}>{w("phòng", "room")} {info[1]}</div>
              </div>
            )}
          </div>
          <div style={{ position: "absolute", left: 20, right: 20, bottom: 16, display: "flex", justifyContent: "space-between", fontSize: 12, color: "#9FB0C8", fontVariantNumeric: "tabular-nums" }}>
            <span>{info[0]}</span>
            <span>{now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })} · {now.toLocaleDateString()}</span>
          </div>
        </>
      )}
    </div>
  );
}

/** Small always-on-top message from the teacher; closes itself after `seconds`. */
export function MessageView({ text, seconds }: { text: string; seconds: number }) {
  const [left, setLeft] = useState(seconds);
  useEffect(() => {
    const t = setInterval(() => setLeft((n) => n - 1), 1000);
    return () => clearInterval(t);
  }, []);
  useEffect(() => {
    if (left <= 0) getCurrentWindow().close();
  }, [left]);
  return (
    <div role="alertdialog" aria-label={t("Tin nhắn từ giáo viên")} style={{ height: "100%", boxSizing: "border-box", display: "flex", flexDirection: "column", background: "#fff", color: "#1B1B1B", border: "1px solid #C8C8C8", fontSize: 13 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8, height: 36, flex: "none", padding: "0 16px", background: "#1F4E9E", color: "#fff", fontWeight: 600 }}>
        <span>{t("Tin nhắn từ giáo viên")}</span>
        <span style={{ flex: 1 }} />
        <span style={{ fontSize: 12, fontWeight: 400 }}>{t("Tự đóng sau")} {Math.max(left, 0)} {t("giây")}</span>
      </div>
      <div style={{ flex: 1, minHeight: 0, overflow: "auto", padding: 16, fontSize: 16, lineHeight: "23px", wordBreak: "break-word" }}>{text}</div>
      <div style={{ display: "flex", gap: 8, flex: "none", padding: "12px 16px", background: "#F7F7F7", borderTop: "1px solid #E5E5E5" }}>
        <Button style={{ marginRight: "auto" }} onClick={() => invoke("open_chat").finally(() => getCurrentWindow().close())}>{w("Trả lời trong chat", "Reply in chat")}</Button>
        <Button appearance="primary" style={{ minWidth: 80 }} onClick={() => getCurrentWindow().close()}>{t("Đã hiểu")}</Button>
      </div>
    </div>
  );
}

/** Teacher's chat line with a reply box. Stays open until the student closes it. */
export function ChatView({ text }: { text: string }) {
  const [reply, setReply] = useState("");
  // me: the student's own line; the teacher's later lines arrive as "chat" events on this one window
  const [log, setLog] = useState<{ me: boolean; text: string }[]>(() => (text ? [{ me: false, text }] : []));
  useEffect(() => {
    const un = listen<string>("chat", (e) => setLog((l) => [...l, { me: false, text: e.payload }]));
    return () => void un.then((f) => f());
  }, []);
  const send = async () => {
    const t = reply.trim();
    if (t && (await invoke<boolean>("chat_say", { text: t }))) {
      setLog((l) => [...l, { me: true, text: t }]);
      setReply("");
    }
  };
  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column", background: "#fff", color: "#1B1B1B", fontSize: 13 }}>
      <div style={{ flex: 1, minHeight: 0, overflow: "auto", display: "flex", flexDirection: "column", gap: 10, padding: 12, background: "#F7F7F7", borderBottom: "1px solid #E5E5E5" }}>
        {log.map((l, i) => l.me
          ? <div key={i} style={{ ...bubble, alignSelf: "flex-end", maxWidth: 250, borderRadius: "8px 8px 2px 8px", background: "#1F4E9E", color: "#fff" }}>{l.text}</div>
          : <div key={i} style={{ alignSelf: "flex-start", maxWidth: 250 }}>
              <div style={{ fontSize: 12, color: "#5F5F5F", marginBottom: 2 }}>{t("Giáo viên")}</div>
              <div style={{ ...bubble, borderRadius: "8px 8px 8px 2px", background: "#fff", border: "1px solid #E0E0E0" }}>{l.text}</div>
            </div>)}
      </div>
      <div style={{ display: "flex", gap: 8, flex: "none", padding: "10px 12px" }}>
        <Input style={{ flex: 1, minWidth: 0 }} value={reply} aria-label={t("Trả lời…")} placeholder={t("Trả lời…")} onChange={(_, d) => setReply(d.value)} onKeyDown={(e) => e.key === "Enter" && send()} />
        <Button appearance="primary" disabled={!reply.trim()} onClick={send}>{t("Gửi")}</Button>
      </div>
    </div>
  );
}
const bubble: React.CSSProperties = { padding: "7px 10px", wordBreak: "break-word" };

/** Survey: one button per option; answering closes the window. */
export function PollView({ id, data }: { id: number; data: { q: string; o: string[] } }) {
  const pick = async (i: number) => {
    await invoke("poll_answer", { id, choice: i });
    getCurrentWindow().close();
  };
  return (
    <Card style={{ height: "100%", boxSizing: "border-box", borderRadius: 0, background: tokens.colorNeutralBackground1 }}>
      <Text weight="semibold" block style={{ wordBreak: "break-word" }}>{data.q}</Text>
      {data.o.map((o, i) => <Button key={i} onClick={() => pick(i)} style={{ justifyContent: "flex-start" }}>{o}</Button>)}
    </Card>
  );
}
