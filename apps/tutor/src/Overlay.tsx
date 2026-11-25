import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type Tool = "pen" | "highlight" | "arrow" | "rect" | "erase";
const COLORS = ["#e81123", "#0078d4", "#107c10", "#ffb900", "#000000", "#ffffff"];

/**
 * Full-screen transparent drawing layer on the teacher's own screen. Screen capture includes it, so
 * the strokes reach the students through the normal Show — no extra protocol. `whiteboard` paints
 * an opaque white page instead of a transparent one. Typed characters (Keyshow) appear bottom-right.
 */
export function OverlayView({ whiteboard }: { whiteboard: boolean }) {
  const cv = useRef<HTMLCanvasElement>(null);
  const [tool, setTool] = useState<Tool>("pen");
  const [color, setColor] = useState(COLORS[0]);
  const [keys, setKeys] = useState("");
  const start = useRef<{ x: number; y: number } | null>(null);
  const snap = useRef<ImageData | null>(null);

  useEffect(() => {
    const c = cv.current!;
    c.width = window.innerWidth;
    c.height = window.innerHeight;
    if (whiteboard) {
      const g = c.getContext("2d")!;
      g.fillStyle = "#fff";
      g.fillRect(0, 0, c.width, c.height);
    }
  }, [whiteboard]);

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout>;
    const un = listen<string>("keyshow", (e) => {
      setKeys((k) => (k + (e.payload === "\b" ? "" : e.payload)).slice(-40));
      clearTimeout(timer);
      timer = setTimeout(() => setKeys(""), 2500);
    });
    return () => { void un.then((f) => f()); clearTimeout(timer); };
  }, []);

  const ctx = () => cv.current!.getContext("2d")!;
  const style = (g: CanvasRenderingContext2D) => {
    g.lineCap = "round";
    g.lineJoin = "round";
    g.globalCompositeOperation = tool === "erase" && !whiteboard ? "destination-out" : "source-over";
    g.strokeStyle = tool === "erase" && whiteboard ? "#fff" : color;
    g.globalAlpha = tool === "highlight" ? 0.35 : 1;
    g.lineWidth = tool === "highlight" ? 22 : tool === "erase" ? 28 : 4;
  };
  const pos = (e: React.PointerEvent) => ({ x: e.clientX, y: e.clientY });
  const down = (e: React.PointerEvent) => {
    (e.target as Element).setPointerCapture(e.pointerId);
    start.current = pos(e);
    const g = ctx();
    snap.current = g.getImageData(0, 0, cv.current!.width, cv.current!.height);
    style(g);
    g.beginPath();
    g.moveTo(start.current.x, start.current.y);
  };
  const move = (e: React.PointerEvent) => {
    const s = start.current;
    if (!s) return;
    const g = ctx();
    const p = pos(e);
    if (tool === "pen" || tool === "highlight" || tool === "erase") {
      g.lineTo(p.x, p.y);
      g.stroke();
      return;
    }
    // shapes are redrawn from the snapshot while dragging
    g.putImageData(snap.current!, 0, 0);
    style(g);
    g.beginPath();
    if (tool === "rect") g.strokeRect(s.x, s.y, p.x - s.x, p.y - s.y);
    else {
      g.moveTo(s.x, s.y);
      g.lineTo(p.x, p.y);
      const a = Math.atan2(p.y - s.y, p.x - s.x);
      for (const d of [0.5, -0.5]) g.lineTo(p.x - 18 * Math.cos(a - d), p.y - 18 * Math.sin(a - d)), g.moveTo(p.x, p.y);
      g.stroke();
    }
  };
  const up = () => { start.current = null; };
  const clear = () => {
    const g = ctx();
    g.globalCompositeOperation = "source-over";
    g.globalAlpha = 1;
    if (whiteboard) { g.fillStyle = "#fff"; g.fillRect(0, 0, cv.current!.width, cv.current!.height); }
    else g.clearRect(0, 0, cv.current!.width, cv.current!.height);
  };

  const btn = (active: boolean): React.CSSProperties => ({ padding: "6px 10px", border: "1px solid #8888", borderRadius: 6, background: active ? "#0078d4" : "#fffd", color: active ? "#fff" : "#000", cursor: "pointer", font: "13px Segoe UI" });
  return (
    <div style={{ position: "fixed", inset: 0 }}>
      <canvas ref={cv} style={{ position: "absolute", inset: 0, touchAction: "none", cursor: "crosshair" }} onPointerDown={down} onPointerMove={move} onPointerUp={up} />
      <div style={{ position: "fixed", top: 10, left: "50%", transform: "translateX(-50%)", display: "flex", gap: 6, padding: 6, background: "#fffe", borderRadius: 10, boxShadow: "0 2px 12px #0006" }}>
        {(["pen", "highlight", "arrow", "rect", "erase"] as Tool[]).map((t) => (
          <button key={t} style={btn(tool === t)} onClick={() => setTool(t)}>{{ pen: "Pen", highlight: "Highlight", arrow: "Arrow", rect: "Box", erase: "Eraser" }[t]}</button>
        ))}
        {COLORS.map((c) => <button key={c} aria-label={c} style={{ width: 26, height: 26, borderRadius: 13, border: color === c ? "3px solid #0078d4" : "1px solid #888", background: c, cursor: "pointer" }} onClick={() => setColor(c)} />)}
        <button style={btn(false)} onClick={clear}>Clear all</button>
        <button style={btn(false)} onClick={() => invoke("close_overlay")}>Close</button>
      </div>
      {keys && <div style={{ position: "fixed", right: 24, bottom: 24, padding: "10px 18px", background: "#000c", color: "#fff", borderRadius: 10, font: "600 28px Segoe UI" }}>{keys}</div>}
    </div>
  );
}
