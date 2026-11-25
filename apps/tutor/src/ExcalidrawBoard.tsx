import { useRef } from "react";
import { Excalidraw } from "@excalidraw/excalidraw";
import "@excalidraw/excalidraw/index.css";

/** Tutor whiteboard: Excalidraw; every change (debounced 300 ms) is handed to `onScene` as elements JSON. */
export default function ExcalidrawBoard({ onScene }: { onScene: (json: string) => void }) {
  const timer = useRef<number>(0);
  const last = useRef("");
  return (
    <div style={{ height: "65vh" }}>
      <Excalidraw
        langCode="vi-VN"
        onChange={(els) => {
          window.clearTimeout(timer.current);
          timer.current = window.setTimeout(() => {
            const json = JSON.stringify(els.filter((e) => !e.isDeleted));
            if (json !== last.current) { last.current = json; onScene(json); }
          }, 300);
        }}
      />
    </div>
  );
}
