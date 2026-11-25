// Run: node --experimental-strip-types tests/i18n.test.ts
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { tr } from "../src/i18n.ts";

assert.equal(tr("en", "Nộp bài"), "Submit");
assert.equal(tr("vi", "Nộp bài"), "Nộp bài");
assert.equal(tr("en", "không có"), "không có");
let n = 0;
for (const f of ["Views.tsx", "Views2.tsx", "ShowView.tsx"]) {
  const src = readFileSync(new URL("../src/" + f, import.meta.url), "utf8");
  for (const m of src.matchAll(/\bt\("([^"]*)"\)/g)) {
    n++;
    assert.notEqual(tr("en", m[1]), m[1], `no English for: ${m[1]}`);
  }
}
assert.ok(n > 30, `only ${n} strings`);
console.log("agent i18n OK", n);
