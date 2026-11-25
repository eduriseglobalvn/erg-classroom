// Run: node --experimental-strip-types tests/showstats.test.ts
import assert from "node:assert/strict";
import { lossLevel, lossPct } from "../src/showstats.ts";
import { tr } from "../src/i18n.ts";
assert.equal(lossPct(0, 0), null);
assert.equal(lossPct(100, 0), 0);
assert.equal(lossPct(90, 10), 10);
assert.equal(lossPct(0, 5), 100);
assert.equal(lossLevel(0), "success");
assert.equal(lossLevel(1), "success");
assert.equal(lossLevel(2), "warning");
assert.equal(lossLevel(9), "warning");
assert.equal(lossLevel(10), "danger");
assert.equal(lossLevel(100), "danger");
assert.notEqual(tr("en", "Tỷ lệ khung hình Show bị mất"), "Tỷ lệ khung hình Show bị mất");
console.log("showstats OK");
