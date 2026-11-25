import { lazy, Suspense, useEffect, useRef } from "react";
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface } from "@fluentui/react-components";
import { invoke } from "@tauri-apps/api/core";
import type { BoardMsg } from "./board";
import { useT } from "./i18n";
import { D, DlgHead } from "./FileDist";

// Excalidraw is big: only fetched when the board is opened
const ExcalidrawBoard = lazy(() => import("./ExcalidrawBoard"));

/** Shared whiteboard for the tutor: opens the board on every student, closes it again when the dialog closes. */
export function BoardDialog({ onClose, fail }: { onClose: () => void; fail: (s: string) => void }) {
  const t = useT();
  const version = useRef(0);
  const send = (msg: BoardMsg) => void invoke<boolean>("board_send", { msg }).catch((e) => fail(String(e)));

  useEffect(() => {
    send({ Open: { draw: false } });
    return () => send("Close");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ ...D.surface, maxWidth: 1040, width: "95vw" }}>
        <DialogBody style={D.body}>
          <DlgHead title={t("Bảng tương tác")} onClose={onClose} />
          <DialogContent style={D.content}>
            <Suspense fallback={null}>
              <ExcalidrawBoard onScene={(json) => { version.current = Math.max(version.current + 1, Date.now()); send({ Scene: { version: version.current, json } }); }} />
            </Suspense>
          </DialogContent>
          <DialogActions style={D.foot}><Button appearance="primary" onClick={onClose}>{t("Đóng bảng")}</Button></DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
