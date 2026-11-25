// Run: node --experimental-strip-types tests/paper.test.ts
import assert from "node:assert/strict";
import { toPaper, type Draft } from "../src/paper.ts";

const d = (kind: Draft["kind"], a: string, b = ""): Draft => ({ kind, text: "Q", points: 1, a, b });
const q = (x: Draft) => toPaper("t", false, [x]).questions[0].kind;

assert.deepEqual(q(d("Choice", "a\n*b\n*c")), { Choice: { options: ["a", "b", "c"], correct: [1, 2] } });
assert.deepEqual(q(d("TrueFalse", "Đúng")), { TrueFalse: { answer: true } });
assert.deepEqual(q(d("Fill", "Hà Nội|Ha Noi\n5")), { Fill: { accepted: [["Hà Nội", "Ha Noi"], ["5"]] } });
assert.deepEqual(q(d("Match", "1 = một\n2 = hai")), { Match: { left: ["1", "2"], right: ["một", "hai"], pairs: [0, 1] } });
assert.deepEqual(q(d("Order", "x\ny\nz")), { Order: { items: ["x", "y", "z"] } });
assert.deepEqual(q(d("Region", "map.png", "0.1,0.2,0.3,0.4")), { Region: { image: "map.png", x: 0.1, y: 0.2, w: 0.3, h: 0.4 } });

for (const bad of [d("Choice", "a\nb"), d("Choice", "*a"), d("TrueFalse", "có"), d("Fill", ""), d("Match", "a=b"), d("Order", "x"), d("Region", "m.png", "0,0,2,1"), { ...d("Order", "x\ny"), points: -1 }, { ...d("Order", "x\ny"), text: " " }])
  assert.throws(() => toPaper("t", false, [bad]));
assert.throws(() => toPaper("t", false, []));
console.log("paper helpers OK");
