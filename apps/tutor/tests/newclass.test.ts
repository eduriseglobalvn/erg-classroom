// Run: node --experimental-strip-types tests/newclass.test.ts
// minutesUntil is copied by regex because Extras.tsx (JSX + Tauri imports) cannot be loaded in node.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
const src = readFileSync(new URL("../src/Extras.tsx", import.meta.url), "utf8");
const body = src.match(/export function minutesUntil\(end: string, now: Date\) \{([\s\S]*?)\n\}/)![1];
const minutesUntil = new Function("end", "now", body) as (end: string, now: Date) => number;
const at = (h: number, m: number) => new Date(2026, 0, 5, h, m);
assert.equal(minutesUntil("09:30", at(8, 45)), 45);
assert.equal(minutesUntil("08:45", at(8, 45)), 0);
assert.equal(minutesUntil("00:10", at(23, 50)), 20); // past midnight
assert.equal(minutesUntil("", at(8, 0)), 0);
assert.equal(minutesUntil("25:00", at(8, 0)), 0);
console.log("new class OK");
