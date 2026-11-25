/** Shared whiteboard helpers (the student app has an identical copy). Coordinates on the wire are 0..65535 of the board. */
export type Pt = [number, number];
export type ShapeKind = "Line" | "Rect" | "Ellipse" | "Arrow";
export type Elem =
  | { t: "stroke"; id: number; color: number; width: number; pts: Pt[] }
  | { t: "shape"; id: number; color: number; width: number; kind: ShapeKind; a: Pt; b: Pt }
  | { t: "text"; id: number; color: number; size: number; at: Pt; text: string };

type StrokeW = { id: number; color: number; width: number; pts: Pt[] };
type ShapeW = { id: number; color: number; width: number; kind: ShapeKind; a: Pt; b: Pt };
type TextW = { id: number; color: number; size: number; at: Pt; text: string };
export type BoardMsg = "Close" | "Clear" | { Open: { draw: boolean } } | { Stroke: StrokeW } | { Shape: ShapeW } | { Text: TextW } | { Undo: { id: number } } | { Scene: { version: number; json: string } };

/** Wire message -> drawable element (null for control messages). */
export function toElem(m: BoardMsg): Elem | null {
  if (typeof m !== "object") return null;
  if ("Stroke" in m) return { t: "stroke", ...m.Stroke };
  if ("Shape" in m) return { t: "shape", ...m.Shape };
  if ("Text" in m) return { t: "text", ...m.Text };
  return null;
}

export function toMsg(e: Elem): BoardMsg {
  const { t, ...rest } = e;
  if (t === "stroke") return { Stroke: rest as StrokeW };
  if (t === "shape") return { Shape: rest as ShapeW };
  return { Text: rest as TextW };
}

export const toWire = (x: number, y: number, w: number, h: number): Pt => [
  Math.round(Math.min(Math.max(x / w, 0), 1) * 65535),
  Math.round(Math.min(Math.max(y / h, 0), 1) * 65535),
];
export const fromWire = (p: Pt, w: number, h: number): Pt => [(p[0] / 65535) * w, (p[1] / 65535) * h];
export const css = (c: number) => "#" + (c & 0xffffff).toString(16).padStart(6, "0");
export const newId = () => Date.now() * 1000 + Math.floor(Math.random() * 1000);

/** Keep at most 2000 points (the receiver refuses longer strokes) by dropping every other point of long ones. */
export function thin(pts: Pt[], max = 2000): Pt[] {
  let p = pts;
  while (p.length > max) p = p.filter((_, i) => i % 2 === 0 || i === p.length - 1);
  return p;
}

/** Arrow head: two short lines back from `b` at ±25° from the shaft. */
export function arrowHead(a: Pt, b: Pt, len: number): [Pt, Pt] {
  const ang = Math.atan2(b[1] - a[1], b[0] - a[0]);
  const d = (s: number): Pt => [b[0] - len * Math.cos(ang + s), b[1] - len * Math.sin(ang + s)];
  return [d(Math.PI / 7), d(-Math.PI / 7)];
}

export function redraw(ctx: CanvasRenderingContext2D, elems: Iterable<Elem>, w: number, h: number) {
  ctx.clearRect(0, 0, w, h);
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  for (const e of elems) {
    ctx.strokeStyle = ctx.fillStyle = css(e.color);
    if (e.t === "text") {
      const [x, y] = fromWire(e.at, w, h);
      ctx.font = `${e.size}px sans-serif`;
      ctx.textBaseline = "top";
      e.text.split("\n").forEach((line, i) => ctx.fillText(line, x, y + i * e.size * 1.2));
      continue;
    }
    ctx.lineWidth = e.width;
    ctx.beginPath();
    if (e.t === "stroke") {
      e.pts.forEach((p, i) => {
        const [x, y] = fromWire(p, w, h);
        if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
      });
      if (e.pts.length === 1) { const [x, y] = fromWire(e.pts[0], w, h); ctx.lineTo(x + 0.1, y); }
    } else {
      const a = fromWire(e.a, w, h), b = fromWire(e.b, w, h);
      if (e.kind === "Rect") ctx.rect(Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1]));
      else if (e.kind === "Ellipse") ctx.ellipse((a[0] + b[0]) / 2, (a[1] + b[1]) / 2, Math.abs(b[0] - a[0]) / 2, Math.abs(b[1] - a[1]) / 2, 0, 0, Math.PI * 2);
      else {
        ctx.moveTo(a[0], a[1]);
        ctx.lineTo(b[0], b[1]);
        if (e.kind === "Arrow") arrowHead(a, b, 10 + e.width * 2).forEach((p) => { ctx.moveTo(b[0], b[1]); ctx.lineTo(p[0], p[1]); });
      }
    }
    ctx.stroke();
  }
}

/** Add an element unless that id is already there (our own element comes back as the tutor's relay). */
export function addElem(map: Map<number, Elem>, e: Elem): boolean {
  if (map.has(e.id)) return false;
  map.set(e.id, e);
  return true;
}

/** Apply a relayed message to the element map. */
export function applyMsg(map: Map<number, Elem>, m: BoardMsg) {
  if (m === "Clear") map.clear();
  else if (typeof m === "object" && "Undo" in m) map.delete(m.Undo.id);
  else { const e = toElem(m); if (e) addElem(map, e); }
}
