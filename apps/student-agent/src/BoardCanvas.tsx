import { useEffect, useRef, useState } from "react";
import { Button } from "@fluentui/react-components";
import { listen } from "@tauri-apps/api/event";
import { addElem, applyMsg, css, newId, redraw, thin, toMsg, toWire, type BoardMsg, type Elem, type ShapeKind } from "./board";

const W = 960, H = 540;
const COLORS = [0x000000, 0xd13438, 0x0f6cbd, 0x107c10, 0xff8c00];
type Tool = "pen" | ShapeKind | "text";
const TOOLS: [Tool, string][] = [["pen", "✎"], ["Line", "／"], ["Rect", "▭"], ["Ellipse", "◯"], ["Arrow", "➜"], ["text", "T"]];

/** The drawing surface used by the tutor dialog and the student window (same file in both apps).
 *  `send` ships a message to the class; `initial` is what was already on the board; `label` translates button text. */
export function BoardCanvas({ canDraw, send, initial = [], label }: { canDraw: boolean; send: (m: BoardMsg) => void; initial?: BoardMsg[]; label: (vi: string) => string }) {
  const cv = useRef<HTMLCanvasElement>(null);
  const elems = useRef(new Map<number, Elem>());
  const mine = useRef<number[]>([]); // my elements, newest last, for Undo
  const cur = useRef<Elem | null>(null);
  const [tool, setTool] = useState<Tool>("pen");
  const [color, setColor] = useState(COLORS[0]);
  const [width, setWidth] = useState(3);
  const paint = () => { const c = cv.current?.getContext("2d"); if (c) redraw(c, [...elems.current.values(), ...(cur.current ? [cur.current] : [])], W, H); };

  useEffect(() => {
    initial.forEach((m) => applyMsg(elems.current, m));
    paint();
    const un = listen<BoardMsg>("board", (e) => {
      const m = e.payload;
      if (typeof m === "object" && "Open" in m) elems.current.clear(); // reopened: start from what follows
      else applyMsg(elems.current, m);
      paint();
    });
    return () => void un.then((f) => f());
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const pos = (e: React.PointerEvent) => { const r = e.currentTarget.getBoundingClientRect(); return toWire(e.clientX - r.left, e.clientY - r.top, r.width, r.height); };
  const commit = (e: Elem) => { addElem(elems.current, e); mine.current.push(e.id); paint(); send(toMsg(e)); };

  return (
    <div>
      {canDraw && (
        <div style={{ display: "flex", gap: 6, alignItems: "center", marginBottom: 6, flexWrap: "wrap" }}>
          {TOOLS.map(([k, g]) => <Button key={k} size="small" appearance={tool === k ? "primary" : "secondary"} onClick={() => setTool(k)} aria-label={k}>{g}</Button>)}
          {COLORS.map((c) => <button key={c} aria-label={css(c)} onClick={() => setColor(c)} style={{ width: 24, height: 24, borderRadius: 12, background: css(c), border: c === color ? "3px solid #888" : "1px solid #888" }} />)}
          <input type="range" min={1} max={20} value={width} onChange={(e) => setWidth(Number(e.target.value))} aria-label="width" />
          <Button size="small" disabled={mine.current.length === 0} onClick={() => { const id = mine.current.pop(); if (id !== undefined) { elems.current.delete(id); paint(); send({ Undo: { id } }); } }}>{label("Hoàn tác")}</Button>
        </div>
      )}
      <canvas ref={cv} width={W} height={H} style={{ width: "100%", border: "1px solid #888", background: "#fff", touchAction: "none", borderRadius: 4, cursor: canDraw ? "crosshair" : "default" }}
        onPointerDown={(e) => {
          if (!canDraw) return;
          const p = pos(e);
          if (tool === "text") {
            const text = window.prompt(label("Nội dung chữ"));
            if (text?.trim()) commit({ t: "text", id: newId(), color, size: 12 + width * 2, at: p, text: text.slice(0, 200) });
            return;
          }
          e.currentTarget.setPointerCapture(e.pointerId);
          cur.current = tool === "pen" ? { t: "stroke", id: newId(), color, width, pts: [p] } : { t: "shape", id: newId(), color, width, kind: tool, a: p, b: p };
          paint();
        }}
        onPointerMove={(e) => {
          const c = cur.current;
          if (!c) return;
          if (c.t === "stroke") c.pts.push(pos(e)); else if (c.t === "shape") c.b = pos(e);
          paint();
        }}
        onPointerUp={() => {
          const c = cur.current; cur.current = null;
          if (!c) return;
          commit(c.t === "stroke" ? { ...c, pts: thin(c.pts) } : c);
        }} />
    </div>
  );
}
