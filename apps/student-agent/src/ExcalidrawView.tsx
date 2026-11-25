import { useEffect, useRef } from "react";
import { Excalidraw } from "@excalidraw/excalidraw";
import "@excalidraw/excalidraw/index.css";
import { listen } from "@tauri-apps/api/event";

type Api = { updateScene: (s: { elements: unknown[] }) => void };
const parse = (json: string) => { try { const v = JSON.parse(json); return Array.isArray(v) ? v : []; } catch { return []; } };

/** Read-only Excalidraw showing the tutor's scene; follows later "board" Scene messages. */
export default function ExcalidrawView({ json }: { json: string }) {
  const api = useRef<Api | null>(null);
  useEffect(() => {
    const un = listen<{ Scene?: { json: string } } | string>("board", (e) => {
      const m = e.payload;
      if (typeof m === "object" && m.Scene) api.current?.updateScene({ elements: parse(m.Scene.json) });
      else if (m === "Clear") api.current?.updateScene({ elements: [] });
    });
    return () => void un.then((f) => f());
  }, []);
  return (
    <div style={{ height: "calc(100vh - 56px)" }}>
      {/* eslint-disable-next-line @typescript-eslint/no-explicit-any */}
      <Excalidraw viewModeEnabled initialData={{ elements: parse(json) as any }} excalidrawAPI={(a) => (api.current = a as unknown as Api)} />
    </div>
  );
}
