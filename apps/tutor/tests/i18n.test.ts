// Run: node --experimental-strip-types tests/i18n.test.ts  (the store needs zustand only; localStorage is absent in node and handled)
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { tr } from "../src/i18n.ts";
assert.equal(tr("en", "Bảng trắng"), "Whiteboard");
assert.equal(tr("vi", "Bảng trắng"), "Bảng trắng");
assert.equal(tr("en", "không có trong từ điển"), "không có trong từ điển"); // falls back to Vietnamese
// every t("...") string in the Tutor screens has an English entry
let n = 0;
for (const f of ["Extras.tsx", "App.tsx", "JournalDialog.tsx", "Wall.tsx", "LessonPanel.tsx"]) {
  const src = readFileSync(new URL("../src/" + f, import.meta.url), "utf8");
  for (const m of src.matchAll(/\{t\("([^"]*)"\)\}/g)) {
    n++;
    assert.notEqual(tr("en", m[1]), m[1], `no English for: ${m[1]}`);
  }
}
// the ribbon and menu bar build every label with t("..."), in any position
for (const f of ["Ribbon.tsx", "MenuBar.tsx", "ui.tsx"]) {
  const src = readFileSync(new URL("../src/" + f, import.meta.url), "utf8");
  for (const m of src.matchAll(/t("([^"]*)")/g)) { n++; assert.notEqual(tr("en", m[1]), m[1], `no English for: ${m[1]}`); }
}
// journal dialog strings used outside {t("…")} (attributes, calls) are translated too
for (const s of ["Nhật ký buổi học", "Lọc theo giờ, học sinh, loại, nội dung…", "Đã xuất nhật ký", "Xem nhật ký…"]) assert.notEqual(tr("en", s), s, s);
// every t(<literal or label key>) string in Wall: also check table headers and sort labels, which are not in {t("…")} form
for (const m of readFileSync(new URL("../src/Wall.tsx", import.meta.url), "utf8").matchAll(/(?:label|[a-z]+): "([^"]+)"/g)) if (/^(Theo|Tên máy|Người dùng|Nhóm|Phòng|Trạng thái|Khoá|Mất gói|Đang online)/.test(m[1])) assert.notEqual(tr("en", m[1]), m[1], m[1]);
assert.ok(n > 60, `only ${n} translated strings found`);
console.log("i18n OK", n);
