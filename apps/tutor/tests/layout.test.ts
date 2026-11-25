// Run: node --experimental-strip-types tests/layout.test.ts   (pure helpers only; no Tauri needed)
import assert from "node:assert/strict";
import { balancedGroups, ordered, swapOrder } from "../src/layout.ts";

const h = (n: string) => ({ hostname: n });
const all = ["PC-01", "PC-02", "PC-10", "PC-03"];

// natural order when nothing is saved
assert.deepEqual(ordered(all.map(h), []).map((x) => x.hostname), ["PC-01", "PC-02", "PC-03", "PC-10"]);
// saved positions win, unknown hosts go after them
assert.deepEqual(ordered(all.map(h), ["PC-10", "PC-01"]).map((x) => x.hostname), ["PC-10", "PC-01", "PC-02", "PC-03"]);

// swapping two seats; hosts without a saved seat are placed first so the swap is well defined
assert.deepEqual(swapOrder([], ["a", "b", "c"], "a", "c"), ["c", "b", "a"]);
assert.deepEqual(swapOrder(["b", "a"], ["a", "b", "c"], "b", "c"), ["c", "a", "b"]);
// gone hosts are dropped, unknown targets change nothing, same seat is a no-op
assert.deepEqual(swapOrder(["gone", "a", "b"], ["a", "b"], "a", "b"), ["b", "a"]);
assert.deepEqual(swapOrder(["a", "b"], ["a", "b"], "a", "zzz"), ["a", "b"]);
const same = ["a", "b"];
assert.equal(swapOrder(same, ["a", "b"], "a", "a"), same);

// balanced groups: sizes differ by at most one; n larger than the class and n=0 are safe
const g = balancedGroups(["a", "b", "c", "d", "e"], 2);
assert.deepEqual(Object.values(g), ["Group 1", "Group 2", "Group 1", "Group 2", "Group 1"]);
assert.deepEqual(balancedGroups(["a", "b"], 9), { a: "Group 1", b: "Group 2" });
assert.deepEqual(balancedGroups(["a"], 0), { a: "Group 1" });
assert.deepEqual(balancedGroups([], 3), {});
console.log("layout helpers OK");

import { ranking } from "../src/layout.ts";
assert.deepEqual(ranking({ B: 3, A: 3, C: 5 }), [["C", 5], ["A", 3], ["B", 3]]);
console.log("ranking OK");

import { addElem, applyMsg, arrowHead, thin, toElem, toMsg, toWire, fromWire, type Elem } from "../src/board.ts";
assert.deepEqual(toWire(-5, 500, 1000, 500), [0, 65535], "clamped to the board");
const [bx, by] = fromWire(toWire(250, 125, 1000, 500), 1000, 500);
assert.ok(Math.abs(bx - 250) < 0.1 && Math.abs(by - 125) < 0.1);
const many = Array.from({ length: 5000 }, (_, i) => [i, i] as [number, number]);
const th = thin(many);
assert.ok(th.length <= 2000 && th[0][0] === 0 && th[th.length - 1][0] === 4999, "thinned but ends kept");

// elements <-> wire messages
const els: Elem[] = [
  { t: "stroke", id: 1, color: 1, width: 2, pts: [[0, 0], [5, 5]] },
  { t: "shape", id: 2, color: 2, width: 3, kind: "Ellipse", a: [1, 2], b: [3, 4] },
  { t: "text", id: 3, color: 3, size: 20, at: [9, 9], text: "xin chào" },
];
for (const e of els) assert.deepEqual(toElem(toMsg(e)), e, "round trip " + e.t);
assert.equal(toElem("Clear"), null);
assert.deepEqual(toMsg(els[1]), { Shape: { id: 2, color: 2, width: 3, kind: "Ellipse", a: [1, 2], b: [3, 4] } }, "serde externally-tagged shape");

// the relay of our own element is ignored; undo and clear remove
const m = new Map<number, Elem>();
assert.ok(addElem(m, els[0]) && !addElem(m, els[0]));
applyMsg(m, toMsg(els[1]));
applyMsg(m, toMsg(els[2]));
assert.equal(m.size, 3);
applyMsg(m, { Undo: { id: 2 } });
assert.deepEqual([...m.keys()], [1, 3]);
applyMsg(m, { Undo: { id: 99 } });
assert.equal(m.size, 2, "undo of an unknown id changes nothing");
applyMsg(m, "Clear");
assert.equal(m.size, 0);

// arrow head: two points behind the tip, symmetric about the shaft
const [h1, h2] = arrowHead([0, 0], [100, 0], 10);
assert.ok(h1[0] < 100 && h2[0] < 100 && Math.abs(h1[1] + h2[1]) < 1e-9 && h1[1] !== 0);
console.log("board helpers OK");
