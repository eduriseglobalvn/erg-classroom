// Run: node --experimental-strip-types tests/inventory.test.ts
import assert from "node:assert/strict";
import { cell, inventoryCsv } from "../src/inventory.ts";

assert.equal(cell('a"b'), '"a""b"');
assert.equal(cell("=1+1"), `"'=1+1"`, "formula defused");
assert.equal(cell("-2"), `"'-2"`);
assert.equal(cell(5), '"5"');

const inv = { os: "Windows 11", cpu: "i5", cores: 8, ram_total: 8 * 2 ** 30, ram_used: 2 ** 30, disks: [["C:", 100 * 2 ** 30, 20 * 2 ** 30], ["D:", 50 * 2 ** 30, 5 * 2 ** 30]] as [string, number, number][], uptime_s: 7300, processes: [] };
const csv = inventoryCsv([{ hostname: "PC-01", user: "an", inventory: inv }, { hostname: "=evil", user: "x" }]);
const lines = csv.replace("﻿", "").trim().split("\r\n");
assert.equal(lines.length, 3);
assert.equal(lines[1], '"PC-01","an","Windows 11","i5","8","8.0","1.0","5.0","2"');
assert.ok(lines[2].startsWith(`"'=evil","x","",`), lines[2]);
console.log("inventory OK");
