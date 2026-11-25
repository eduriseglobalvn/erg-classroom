import { useCallback, useEffect, useRef, useState } from "react";
import { Button, Input, Text, Textarea, makeStyles, tokens } from "@fluentui/react-components";
import { ChevronDown20Regular, ChevronUp20Regular } from "@fluentui/react-icons";
import { invoke } from "@tauri-apps/api/core";
import { useT } from "./i18n";
import { useUi } from "./uistate";

/** Same "lesson" document the Start-lesson wizard writes: {teacher, title, objectives[], minutes, ...}. `outcome` is added here; other fields are kept. */
type Doc = Record<string, unknown>;
const LS = "lesson-doc";

async function loadDoc(): Promise<Doc> {
  try {
    const raw = await invoke<string | null>("load_doc", { name: "lesson" });
    if (raw) return JSON.parse(raw);
  } catch { /* fall through */ }
  // browser dev / damaged file: local copy
  try { return JSON.parse(localStorage.getItem(LS) ?? "{}"); } catch { return {}; }
}

const useStyles = makeStyles({
  root: { flex: "0 0 auto", background: tokens.colorNeutralBackground1, borderTop: `1px solid ${tokens.colorNeutralStroke2}` },
  head: { display: "flex", alignItems: "center", gap: "8px", padding: "0 6px 0 12px", minHeight: "36px" },
  grid: { display: "grid", gridTemplateColumns: "minmax(0, 180px) minmax(0, 240px) minmax(0, 1fr) minmax(0, 1fr)", gap: "10px", padding: "0 12px 10px" },
  field: { display: "flex", flexDirection: "column", gap: "2px", minWidth: 0 },
});

export function LessonPanel({ onPlan, onEnd }: { onPlan: () => void; onEnd: () => void }) {
  const t = useT();
  const s = useStyles();
  const ui = useUi();
  const [doc, setDoc] = useState<Doc>({});
  const dirty = useRef(false);

  const reload = useCallback(() => { void loadDoc().then((d) => { if (!dirty.current) setDoc(d); }); }, []);
  // the wizard writes the same document from another dialog: pick its changes up unless the teacher is typing
  useEffect(() => { reload(); const h = setInterval(reload, 4000); return () => clearInterval(h); }, [reload]);

  const val = (k: string) => (typeof doc[k] === "string" ? (doc[k] as string) : "");
  const title = val("title");
  useEffect(() => { useUi.setState({ lesson: title }); }, [title]);
  const objectives = Array.isArray(doc.objectives) ? (doc.objectives as unknown[]).join("\n") : val("objectives");
  const edit = (k: string, v: string) => { dirty.current = true; setDoc((d) => ({ ...d, [k]: k === "objectives" ? v.split("\n") : v })); };
  const save = () => {
    if (!dirty.current) return;
    dirty.current = false;
    const json = JSON.stringify(doc);
    try { localStorage.setItem(LS, json); } catch { /* ignore */ }
    void invoke("save_doc", { name: "lesson", json }).catch(() => {});
  };
  const field = (id: string, k: string, label: string, multi = false) => (
    <div className={s.field}>
      <label htmlFor={id}><Text size={200}>{label}</Text></label>
      {multi
        ? <Textarea id={id} rows={1} resize="vertical" value={k === "objectives" ? objectives : val(k)} onChange={(_, d) => edit(k, d.value)} onBlur={save} />
        : <Input id={id} value={val(k)} onChange={(_, d) => edit(k, d.value)} onBlur={save} />}
    </div>
  );

  return (
    <section className={s.root} aria-label={t("Chi tiết tiết học")}>
      <div className={s.head}>
        <Text weight="semibold">{t("Chi tiết tiết học")}</Text>
        <div style={{ flex: 1 }} />
        <Button size="small" appearance="subtle" onClick={onPlan}>{t("Kế hoạch bài giảng…")}</Button>
        <Button size="small" onClick={onEnd}>{t("Kết thúc lớp")}</Button>
        <Button size="small" appearance="subtle" aria-expanded={ui.lessonOpen} aria-controls="lesson-body" aria-label={ui.lessonOpen ? t("Thu gọn") : t("Mở rộng")}
          icon={ui.lessonOpen ? <ChevronDown20Regular /> : <ChevronUp20Regular />} onClick={() => ui.set({ lessonOpen: !ui.lessonOpen })} />
      </div>
      {ui.lessonOpen && (
        <div id="lesson-body" className={s.grid}>
          {field("ls-teacher", "teacher", t("Giáo viên"))}
          {field("ls-title", "title", t("Tiết học"))}
          {field("ls-obj", "objectives", t("Mục tiêu"), true)}
          {field("ls-out", "outcome", t("Kết quả"), true)}
        </div>
      )}
    </section>
  );
}
