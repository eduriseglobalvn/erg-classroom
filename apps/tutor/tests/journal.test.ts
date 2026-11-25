// Run: node --experimental-strip-types tests/journal.test.ts
import assert from "node:assert/strict";
import { clockOf, filterEntries, isToday } from "../src/journal.ts";

const at = (h: number, m: number, s = 0) => new Date(2026, 0, 5, h, m, s).getTime();
const E = [
  { ts: at(9, 0), kind: "online", who: "An (PC-1)", detail: "id1" },
  { ts: at(9, 5, 7), kind: "chat", who: "id1", detail: "Em cần GIÚP bài 2" },
  { ts: at(9, 3), kind: "help", who: "id2", detail: "" },
];
assert.equal(clockOf(at(9, 5, 7)), "09:05:07");
assert.deepEqual(filterEntries(E, "").map((e) => e.kind), ["chat", "help", "online"]); // newest first
assert.deepEqual(filterEntries(E, "giúp bài").map((e) => e.kind), ["chat"]); // all words, any case
assert.deepEqual(filterEntries(E, "09:03").map((e) => e.kind), ["help"]); // time is searchable
assert.deepEqual(filterEntries(E, "", new Set(["online", "help"])).map((e) => e.kind), ["help", "online"]);
assert.deepEqual(filterEntries(E, "zzz"), []);
assert.equal(isToday(at(1, 0), at(23, 0)), true);
assert.equal(isToday(at(1, 0), at(23, 0) + 86_400_000), false);
console.log("journal helpers OK");
