import { useEffect, useState } from "react";
import { Button, MessageBar, MessageBarActions, MessageBarBody } from "@fluentui/react-components";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useT } from "./i18n";
import { useClass } from "./store";

/** "1.2.3" or "v1.2.3" as one comparable number, same rule as erg_proto::version_code. */
export const versionCode = (v: string) => {
  const [a = 0, b = 0, c = 0] = v.replace(/^v/, "").split(".").map((x) => parseInt(x, 10) || 0);
  return a * 10000 + Math.min(b, 99) * 100 + Math.min(c, 99);
};

/** Newest release on GitHub when it is newer than this Tutor, checked once at start. No Internet = nothing shown. */
function useNewRelease() {
  const [tag, setTag] = useState<string | null>(null);
  useEffect(() => {
    void (async () => {
      try {
        const mine = await invoke<string>("app_version");
        const r = await fetch("https://api.github.com/repos/eduriseglobalvn/erg-classroom/releases/latest", { headers: { Accept: "application/vnd.github+json" } });
        const j = (await r.json()) as { tag_name?: unknown };
        if (typeof j.tag_name === "string" && versionCode(j.tag_name) > versionCode(mine)) setTag(j.tag_name.slice(0, 20));
      } catch { /* offline or rate limited: try again next start */ }
    })();
  }, []);
  return tag;
}

/** Shown while connected students run an older version than this Tutor: sends them the update package over the LAN
 *  (the existing signed Remote Deploy). An updated student comes back with the new version and leaves the count. */
export function UpdateBar({ say, fail }: { say: (s: string) => void; fail: (s: string) => void }) {
  const t = useT();
  const students = useClass((s) => s.students);
  const [busy, setBusy] = useState(false);
  const [later, setLater] = useState(false);
  const release = useNewRelease();
  const old = Object.values(students).filter((x) => x.online && x.outdated);
  const tutorBar = release && !later && (
    <MessageBar intent="info" shape="square">
      <MessageBarBody>{t("Có phiên bản mới")} {release}. {t("Tải ERGClassroom.msi: cùng một tệp cập nhật máy này và máy học sinh.")}</MessageBarBody>
      <MessageBarActions>
        <Button size="small" appearance="primary" onClick={() => void invoke("open_releases").catch((e) => fail(String(e)))}>{t("Tải về")}</Button>
        <Button size="small" onClick={() => setLater(true)}>{t("Để sau")}</Button>
      </MessageBarActions>
    </MessageBar>
  );
  if (!old.length) return tutorBar || null;
  const go = async () => {
    const f = await open({ multiple: false, filters: [{ name: "ERG Classroom update", extensions: ["msi"] }] });
    if (!f || Array.isArray(f)) return;
    // the student service installs whatever this Tutor signs: do not let a wrong file through by accident
    if (!/[\\/]ERGClassroom[^\\/]*\.msi$/i.test(f)) return fail(t("Hãy chọn tệp ERGClassroom.msi"));
    if (!window.confirm(`${t("Cập nhật")} ${old.length} ${t("máy")}?\n${t("Các máy này sẽ mất kết nối khoảng một phút rồi tự vào lại. Máy đang đóng băng (Deep Freeze) sẽ quay về bản cũ sau khi khởi động lại.")}\n${f}`)) return;
    setBusy(true);
    try {
      const [ok, failed] = await invoke<[number, number]>("deploy", { ids: old.map((x) => x.id), path: f, args: "", minutes: 30 });
      say(`${t("Đã gửi bản cập nhật tới")} ${ok} ${t("máy")}${failed ? `, ${failed} ${t("máy lỗi")}` : ""}`);
    } catch (e) {
      fail(String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
    {tutorBar}
    <MessageBar intent="warning" shape="square">
      <MessageBarBody>{old.length} {t("máy đang chạy bản cũ")} ({old[0].version}).</MessageBarBody>
      <MessageBarActions><Button size="small" disabled={busy} onClick={() => void go()}>{busy ? t("Đang gửi…") : t("Cập nhật máy học sinh…")}</Button></MessageBarActions>
    </MessageBar>
    </>
  );
}
