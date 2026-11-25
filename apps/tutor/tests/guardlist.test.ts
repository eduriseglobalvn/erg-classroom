// Run: node --experimental-strip-types tests/guardlist.test.ts
import assert from "node:assert/strict";
import { parseList } from "../src/guardlist.ts";

assert.deepEqual(parseList("https://www.Facebook.com/groups?x=1\n*.garena.vn, tiktok.com\nnot a site\nfacebook.com", "site"), ["facebook.com", "*.garena.vn", "tiktok.com"]);
assert.deepEqual(parseList("Casino\ncá   độ\nx\n<script>\ncasino", "word"), ["casino", "cá độ"]);
assert.equal(parseList(Array.from({ length: 600 }, (_, i) => `s${i}.com`).join("\n"), "site").length, 500);
