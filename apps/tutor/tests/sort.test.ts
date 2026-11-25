// Run: node --experimental-strip-types tests/sort.test.ts
import assert from "node:assert/strict";
import { sortStudents } from "../src/sort.ts";

const s = (hostname: string, user = "", online = true, help = false) => ({ hostname, user, online, help });
const l = [s("PC-10", "an"), s("PC-2", "", true), s("PC-1", "binh", false), s("PC-3", "cuong", true, true)];
const names = (x: { hostname: string }[]) => x.map((i) => i.hostname);
assert.deepEqual(names(sortStudents(l, "name", "asc", {})), ["PC-1", "PC-2", "PC-3", "PC-10"]); // natural numbers
assert.deepEqual(names(sortStudents(l, "name", "desc", {})), ["PC-10", "PC-3", "PC-2", "PC-1"]);
assert.deepEqual(names(sortStudents(l, "user", "asc", {})), ["PC-10", "PC-1", "PC-3", "PC-2"]); // empty user last
assert.deepEqual(names(sortStudents(l, "user", "desc", {})), ["PC-3", "PC-1", "PC-10", "PC-2"]); // still last
assert.deepEqual(names(sortStudents(l, "status", "asc", {})), ["PC-3", "PC-2", "PC-10", "PC-1"]); // help, online, offline
assert.deepEqual(names(sortStudents(l, "group", "asc", { "PC-2": "B", "PC-10": "A" })), ["PC-10", "PC-2", "PC-1", "PC-3"]);
const orig = [...l];
sortStudents(l, "name", "asc", {});
assert.deepEqual(l, orig); // input untouched
console.log("sort OK");
