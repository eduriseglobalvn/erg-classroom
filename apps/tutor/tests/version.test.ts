// Run: node --experimental-strip-types tests/version.test.ts
import assert from "node:assert/strict";
const versionCode = (v: string) => {
  const [a = 0, b = 0, c = 0] = v.replace(/^v/, "").split(".").map((x) => parseInt(x, 10) || 0);
  return a * 10000 + Math.min(b, 99) * 100 + Math.min(c, 99);
};
assert.equal(versionCode("v0.2.0"), 200); // same numbers as erg_proto::version_code
assert.ok(versionCode("v0.10.0") > versionCode("0.9.9"));
assert.ok(versionCode("1.0.0") > versionCode("v0.99.1"));
