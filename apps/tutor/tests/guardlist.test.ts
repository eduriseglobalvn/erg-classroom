// Run: node --experimental-strip-types tests/guardlist.test.ts
import assert from "node:assert/strict";
import { parseList, STARTER, tryGuard } from "../src/guardlist.ts";

// the starter lists survive their own parser, so Import/Export round-trips them
assert.deepEqual(parseList(STARTER.sites.join("\n"), "site"), STARTER.sites);
assert.deepEqual(parseList(STARTER.words.join("\n"), "word"), STARTER.words);

assert.deepEqual(parseList("https://www.Facebook.com/groups?x=1\n*.garena.vn, tiktok.com\nnot a site\nfacebook.com", "site"), ["facebook.com", "*.garena.vn", "tiktok.com"]);
assert.deepEqual(parseList("Casino\ncá   độ\nx\n<script>\ncasino", "word"), ["casino", "cá độ"]);
// "Try it": sites by host, words as whole words (with Vietnamese accents)
assert.deepEqual(tryGuard("https://m.facebook.com/x", ["facebook.com"], []), { kind: "site", hit: "facebook.com" });
assert.deepEqual(tryGuard("Sub - news.garena.vn", ["*.garena.vn"], []), { kind: "site", hit: "*.garena.vn" });
assert.deepEqual(tryGuard("Kèo CÁ ĐỘ hôm nay", [], ["cá độ"]), { kind: "word", hit: "cá độ" });
assert.equal(tryGuard("Essex county news", [], ["sex"]), null);
assert.equal(tryGuard("cá độc", [], ["cá độ"]), null);
assert.equal(parseList(Array.from({ length: 600 }, (_, i) => `s${i}.com`).join("\n"), "site").length, 500);
