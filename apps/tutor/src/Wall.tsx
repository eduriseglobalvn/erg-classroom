import { useEffect, useRef, useState } from "react";
import { Badge, Menu, MenuDivider, MenuItem, MenuItemCheckbox, MenuItemRadio, MenuList, MenuPopover, MenuTrigger, Skeleton, SkeletonItem, Text, makeStyles, mergeClasses, shorthands, tokens } from "@fluentui/react-components";
import {
  Desktop24Regular, HandRight20Filled, LockClosed16Filled, PlugDisconnected20Regular, Person24Filled, ArrowSortUp20Regular, ArrowSortDown20Regular,
} from "@fluentui/react-icons";
import { type Student, useClass } from "./store";
import { lossLevel } from "./showstats";
import { useT } from "./i18n";
import { type View, useUi } from "./uistate";
import type { SortKey } from "./sort";

const BG_KEY = "wall-bg";
const readBg = () => { try { return localStorage.getItem(BG_KEY); } catch { return null; } };

/** Downscale to <=1920px JPEG so it fits localStorage; resolves null for anything that is not a decodable image. */
function loadImage(file: File): Promise<string | null> {
  return new Promise((ok) => {
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => {
      const k = Math.min(1, 1920 / Math.max(img.width, img.height));
      const c = document.createElement("canvas");
      c.width = Math.max(1, Math.round(img.width * k)); c.height = Math.max(1, Math.round(img.height * k));
      c.getContext("2d")?.drawImage(img, 0, 0, c.width, c.height);
      URL.revokeObjectURL(url);
      ok(c.toDataURL("image/jpeg", 0.8));
    };
    img.onerror = () => { URL.revokeObjectURL(url); ok(null); };
    img.src = url;
  });
}

const useStyles = makeStyles({
  wall: {
    flex: 1, minWidth: 0, overflow: "auto", padding: "14px 16px", gap: "10px", alignContent: "start", gridAutoRows: "max-content",
    background: `color-mix(in srgb, ${tokens.colorNeutralBackground1} 86%, transparent)`,
    backgroundRepeat: "no-repeat", backgroundPosition: "center",
  },
  // flat tile: the screen with a hairline, the computer name on a tag underneath (as in NetSupport)
  tile: {
    position: "relative", display: "flex", flexDirection: "column", gap: "3px", minWidth: 0, userSelect: "none", cursor: "default",
    ":focus-visible": { outline: `2px solid ${tokens.colorStrokeFocus2}`, outlineOffset: "2px" },
    "&:hover > :first-child": { outlineColor: tokens.colorNeutralStroke1Hover },
  },
  sel: {
    "& > :first-child": { outline: `2px solid ${tokens.colorBrandStroke1}`, outlineOffset: "1px" },
    "&:hover > :first-child": { outlineColor: tokens.colorBrandStroke1 },
    "& > :last-child": { background: tokens.colorBrandBackground, color: tokens.colorNeutralForegroundOnBrand },
  },
  off: { opacity: 0.6, filter: "grayscale(1)" },
  // thumbnail states of the v2 design: a locked screen is a dark panel, a computer that is not connected a grey one
  tileOff: { "& > :last-child": { color: tokens.colorNeutralForeground3 } },
  pic: { position: "relative", aspectRatio: "16 / 9", background: tokens.colorNeutralBackground3, display: "grid", placeItems: "center", color: tokens.colorNeutralForeground3, fontSize: "11px", overflow: "hidden", outline: `1px solid ${tokens.colorNeutralStroke1}` },
  picLock: { background: "#16202E", color: "#C9D3E0" },
  picOff: { background: tokens.colorNeutralBackground5 },
  help: { position: "absolute", top: "4px", right: "4px", height: "18px", padding: "0 7px", lineHeight: "18px", borderRadius: "9px", background: "#C42B1C", color: "#fff", fontSize: "11px" },
  img: { position: "absolute", inset: 0, width: "100%", height: "100%", objectFit: "cover", pointerEvents: "none" },
  topL: { position: "absolute", top: "4px", left: "4px", display: "flex", gap: "4px", alignItems: "center" },
  chip: { display: "grid", placeItems: "center", width: "20px", height: "20px", borderRadius: "4px", background: "rgba(0,0,0,.6)", color: "#fff" },
  hand: { display: "grid", placeItems: "center", width: "20px", height: "20px", borderRadius: "4px", background: tokens.colorPaletteRedBackground3, color: "#fff", "& svg": { width: "14px", height: "14px" } },
  cap: { display: "flex", alignItems: "center", justifyContent: "center", gap: "5px", height: "20px", padding: "0 6px", boxSizing: "border-box", borderRadius: "2px", fontSize: "12px", lineHeight: "20px" },
  dot: { width: "8px", height: "8px", borderRadius: "999px", flex: "0 0 auto" },
  ell: { overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", minWidth: 0 },
  icon: {
    position: "relative", display: "flex", flexDirection: "column", alignItems: "center", gap: "4px", padding: "8px 4px", borderRadius: tokens.borderRadiusMedium, userSelect: "none", cursor: "default",
    ...shorthands.border("1px", "solid", "transparent"),
    ":hover": { background: tokens.colorSubtleBackgroundHover }, ":focus-visible": { outline: `2px solid ${tokens.colorStrokeFocus2}` },
  },
  iconSel: { background: tokens.colorBrandBackground2, ...shorthands.borderColor(tokens.colorBrandStroke1) },
  avatar: { position: "relative", display: "grid", placeItems: "center", borderRadius: "999px", background: tokens.colorBrandBackground2, color: tokens.colorBrandForeground2 },
  badgeTR: { position: "absolute", top: "-2px", right: "-4px", display: "flex", gap: "2px" },
  badgeTL: { position: "absolute", top: "-2px", left: "-4px", display: "flex", gap: "2px" },
  tableWrap: { minWidth: 0, background: tokens.colorNeutralBackground1, ...shorthands.border("1px", "solid", tokens.colorNeutralStroke2) },
  table: { width: "100%", borderCollapse: "collapse", fontSize: "13px" },
  th: {
    position: "sticky", top: 0, background: tokens.colorNeutralBackground3, textAlign: "left", padding: "0", fontWeight: 600, whiteSpace: "nowrap",
    borderBottom: `1px solid ${tokens.colorNeutralStroke2}`, zIndex: 1,
  },
  thBtn: {
    display: "flex", alignItems: "center", gap: "4px", width: "100%", padding: "8px 10px", background: "transparent", color: "inherit", font: "inherit", cursor: "pointer", ...shorthands.border("0"),
    ":hover": { background: tokens.colorSubtleBackgroundHover }, ":focus-visible": { outline: `2px solid ${tokens.colorStrokeFocus2}`, outlineOffset: "-2px" },
  },
  thPlain: { display: "block", padding: "8px 10px" },
  row: {
    cursor: "default", userSelect: "none", borderBottom: `1px solid ${tokens.colorNeutralStroke2}`,
    ":hover": { background: tokens.colorSubtleBackgroundHover }, ":focus-visible": { outline: `2px solid ${tokens.colorStrokeFocus2}`, outlineOffset: "-2px" },
  },
  rowSel: { background: tokens.colorBrandBackground2 },
  td: { padding: "6px 10px", whiteSpace: "nowrap" },
  empty: { color: tokens.colorNeutralForegroundDisabled },
});

export interface WallProps {
  list: Student[];
  gridCols: string;
  wallRef: React.RefObject<HTMLDivElement | null>;
  selected: Set<string>;
  groups: Record<string, string>;
  leaders: Record<string, string>;
  showing: boolean;
  onPick: (st: Student, e: React.MouseEvent) => void;
  onKey: (st: Student, e: React.KeyboardEvent) => void;
  onZoom: (st: Student) => void;
  /** right-click on a student (selection + menu are App's) */
  onTileMenu: (st: Student, e: React.MouseEvent) => void;
  onClear: () => void;
  onSwap: (from: string, to: string) => void;
}

const COLS: { k: SortKey | null; label: string }[] = [
  { k: "name", label: "Tên máy" }, { k: "user", label: "Người dùng" }, { k: "group", label: "Nhóm" }, { k: null, label: "Phòng" },
  { k: "status", label: "Trạng thái" }, { k: null, label: "Khoá" }, { k: null, label: "Mất gói" }, { k: null, label: "Đang online" },
];

export function Wall(p: WallProps) {
  const t = useT();
  const s = useStyles();
  const ui = useUi();
  const loading = useClass((k) => k.loading);
  const [dragging, setDragging] = useState<string | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [bg, setBg] = useState<string | null>(readBg);
  const fileRef = useRef<HTMLInputElement>(null);
  const view: View = ui.view;

  // a stored image that no longer decodes must not leave a broken wall: drop it
  useEffect(() => {
    if (!bg) return;
    const i = new Image();
    i.onerror = () => { setBg(null); try { localStorage.removeItem(BG_KEY); } catch { /* ignore */ } };
    i.src = bg;
  }, [bg]);

  const setBackground = async (f: File | undefined) => {
    if (!f) return;
    const d = await loadImage(f);
    if (!d) return;
    setBg(d);
    try { localStorage.setItem(BG_KEY, d); } catch { /* too big to remember: kept for this session only */ }
  };
  const clearBg = () => { setBg(null); try { localStorage.removeItem(BG_KEY); } catch { /* ignore */ } };

  const sortBy = (k: SortKey) => ui.set({ sortKey: k, autoArrange: true, sortDir: ui.sortKey === k && ui.autoArrange && ui.sortDir === "asc" ? "desc" : "asc" });

  const label = (st: Student) => `${st.hostname}${st.user ? " " + st.user : ""}${st.online ? "" : " — " + t("Ngoại tuyến")}${st.help ? " — " + t("Cần giúp đỡ") : ""}`;
  const common = (st: Student) => ({
    "data-st": st.id, tabIndex: 0, "aria-label": label(st),
    onClick: (e: React.MouseEvent) => p.onPick(st, e),
    onDoubleClick: () => p.onZoom(st),
    onKeyDown: (e: React.KeyboardEvent) => p.onKey(st, e),
    onContextMenu: (e: React.MouseEvent) => { e.preventDefault(); e.stopPropagation(); p.onTileMenu(st, e); },
    draggable: true,
    onDragStart: () => setDragging(st.hostname),
    onDragOver: (e: React.DragEvent) => e.preventDefault(),
    onDrop: () => { if (dragging) p.onSwap(dragging, st.hostname); setDragging(null); },
  });
  const lossBadge = (st: Student) => p.showing && st.showLoss !== undefined
    && <Badge appearance="filled" color={lossLevel(st.showLoss)} size="small" title={t("Tỷ lệ khung hình Show bị mất")}>{st.showLoss}%</Badge>;

  const checked = { view: [ui.view], key: [ui.sortKey], dir: [ui.sortDir], auto: ui.autoArrange ? ["on"] : [], center: ui.bgCenter ? ["on"] : [] };
  const onChecked = (_: unknown, d: { name: string; checkedItems: string[] }) => {
    const v = d.checkedItems[0], on = v === "on";
    if (d.name === "view") ui.set({ view: v as View });
    else if (d.name === "key") ui.set({ sortKey: v as SortKey, autoArrange: true });
    else if (d.name === "dir") ui.set({ sortDir: v as "asc" | "desc", autoArrange: true });
    else if (d.name === "auto") ui.set({ autoArrange: on });
    else if (d.name === "center") ui.set({ bgCenter: on });
  };

  const bgStyle: React.CSSProperties = bg ? { backgroundImage: `url(${bg})`, backgroundSize: ui.bgCenter ? "contain" : "cover" } : {};
  const empty = (e: React.MouseEvent) => !(e.target as HTMLElement).closest("[data-st]");

  // Drag a box over empty space to select every student it touches (Ctrl keeps the current selection).
  const [box, setBox] = useState<{ x: number; y: number; w: number; h: number } | null>(null);
  const dragged = useRef(false);
  const startBox = (e: React.MouseEvent) => {
    const el = p.wallRef.current;
    if (e.button !== 0 || !empty(e) || !el || e.clientX > el.getBoundingClientRect().left + el.clientWidth) return; // not on the scrollbar
    const x0 = e.clientX, y0 = e.clientY, keep = e.ctrlKey || e.metaKey ? [...p.selected] : [];
    dragged.current = false;
    const move = (m: MouseEvent) => {
      const r = { x: Math.min(x0, m.clientX), y: Math.min(y0, m.clientY), w: Math.abs(m.clientX - x0), h: Math.abs(m.clientY - y0) };
      if (!dragged.current && r.w < 4 && r.h < 4) return; // a plain click
      dragged.current = true;
      setBox(r);
      const sel = new Set(keep);
      el.querySelectorAll<HTMLElement>("[data-st]").forEach((tile) => {
        const b = tile.getBoundingClientRect();
        if (b.left < r.x + r.w && b.right > r.x && b.top < r.y + r.h && b.bottom > r.y) sel.add(tile.dataset.st!);
      });
      useClass.setState({ selected: sel });
    };
    const up = () => { window.removeEventListener("mousemove", move); window.removeEventListener("mouseup", up); setBox(null); };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
    e.preventDefault(); // no text selection while dragging
  };

  let body: React.ReactNode;
  if (view === "details") {
    body = (
      <div className={s.tableWrap}>
        <table className={s.table} role="grid" aria-label={t("Máy học sinh")} aria-rowcount={p.list.length + 1}>
          <thead>
            <tr>
              {COLS.map((c) => {
                const on = c.k && ui.autoArrange && ui.sortKey === c.k;
                return (
                  <th key={c.label} scope="col" className={s.th} aria-sort={on ? (ui.sortDir === "asc" ? "ascending" : "descending") : c.k ? "none" : undefined}>
                    {c.k ? (
                      <button type="button" className={s.thBtn} onClick={() => sortBy(c.k!)}>
                        {t(c.label)}{on && (ui.sortDir === "asc" ? <ArrowSortUp20Regular /> : <ArrowSortDown20Regular />)}
                      </button>
                    ) : <span className={s.thPlain}>{t(c.label)}</span>}
                  </th>
                );
              })}
            </tr>
          </thead>
          <tbody>
            {p.list.map((st) => {
              const sel = p.selected.has(st.id);
              const g = p.groups[st.hostname];
              return (
                <tr key={st.id} aria-selected={sel} className={mergeClasses(s.row, sel && s.rowSel, !st.online && s.off)} {...common(st)}>
                  <td className={s.td}><Text weight="semibold">{st.hostname}</Text></td>
                  <td className={s.td}>{st.user || <span className={s.empty}>—</span>}</td>
                  <td className={s.td}>{g ? g + (p.leaders[g] === st.hostname ? " ★" : "") : <span className={s.empty}>—</span>}</td>
                  <td className={s.td}>{st.room || <span className={s.empty}>—</span>}</td>
                  <td className={s.td}>{st.help ? <Badge appearance="filled" color="warning" icon={<HandRight20Filled />}>{t("Cần giúp đỡ")}</Badge> : st.online ? t("Sẵn sàng") : t("Ngoại tuyến")}</td>
                  <td className={s.td}>{st.locked ? <span style={{ display: "inline-flex", gap: 4, alignItems: "center" }}><LockClosed16Filled />{t("Đã khoá")}</span> : <span className={s.empty}>—</span>}</td>
                  <td className={s.td}>{p.showing && st.showLoss !== undefined ? lossBadge(st) : <span className={s.empty}>—</span>}</td>
                  <td className={s.td}>
                    <span className={s.dot} style={{ display: "inline-block", marginRight: 6, background: st.online ? tokens.colorPaletteGreenBackground3 : tokens.colorNeutralForegroundDisabled }} />
                    {st.online ? t("Có") : t("Không")}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    );
  } else if (view === "icons") {
    const d = Math.round(ui.size * 0.3);
    body = p.list.map((st) => {
      const sel = p.selected.has(st.id);
      const g = p.groups[st.hostname];
      const name = (ui.showUser && st.user) || st.hostname;
      return (
        <div key={st.id} role="button" aria-pressed={sel} className={mergeClasses(s.icon, sel && s.iconSel, !st.online && s.off)} {...common(st)}>
          <div className={s.avatar} style={{ width: d, height: d }}>
            <Person24Filled style={{ width: d * 0.6, height: d * 0.6 }} />
            <div className={s.badgeTL}>
              {st.locked && <span className={s.chip} title={t("Đã khoá")}><LockClosed16Filled /></span>}
              {lossBadge(st)}
            </div>
            <div className={s.badgeTR}>
              {st.help && <span className={s.hand} title={t("Cần giúp đỡ")}><HandRight20Filled /></span>}
              {!st.online && <span className={s.chip} title={t("Ngoại tuyến")}><PlugDisconnected20Regular style={{ width: 14, height: 14 }} /></span>}
            </div>
          </div>
          <Text size={200} weight="semibold" className={s.ell} style={{ maxWidth: "100%" }}>{name}</Text>
          {ui.showGroup && g && <Text size={100} className={s.ell} style={{ maxWidth: "100%", color: tokens.colorNeutralForeground3 }}>{g}{p.leaders[g] === st.hostname ? " ★" : ""}</Text>}
        </div>
      );
    });
  } else {
    body = p.list.map((st) => {
      const sel = p.selected.has(st.id);
      const g = p.groups[st.hostname];
      const name = (ui.showUser && st.user) || st.hostname;
      const grp = ui.showGroup && g ? ` · ${g}${p.leaders[g] === st.hostname ? " ★" : ""}` : "";
      return (
        <div key={st.id} role="button" aria-pressed={sel} title={st.user ? `${st.hostname} · ${st.user}` : st.hostname} className={mergeClasses(s.tile, sel && s.sel, !st.online && s.tileOff)} {...common(st)}>
          <div className={mergeClasses(s.pic, !st.online ? s.picOff : st.locked && s.picLock)}>
            {!st.online ? t("Chưa kết nối") : st.locked ? t("Đã khoá") : st.thumb ? <img className={s.img} src={st.thumb} alt="" draggable={false} /> : <Desktop24Regular />}
            <div className={s.topL}>{lossBadge(st)}</div>
            {st.help && <span className={s.help} title={t("Cần giúp đỡ")}>{t("Trợ giúp")}</span>}
          </div>
          <div className={s.cap}>
            <span className={s.dot} style={{ background: st.online ? tokens.colorPaletteGreenBackground3 : tokens.colorNeutralForegroundDisabled }} />
            <span className={s.ell}>{name}{grp}</span>
          </div>
        </div>
      );
    });
  }

  // class still connecting: grey tiles instead of an empty wall
  if (!p.list.length && loading) {
    body = Array.from({ length: 8 }, (_, i) => (
      <Skeleton key={i} className={s.tile} aria-label="Loading"><SkeletonItem style={{ aspectRatio: "16 / 9", height: "auto", borderRadius: 0 }} /><SkeletonItem size={16} style={{ width: "60%", margin: "4px auto 0" }} /></Skeleton>
    ));
  }

  const cols = view === "details" ? "minmax(0, 1fr)" : view === "icons" ? `repeat(auto-fill, minmax(${Math.round(ui.size * 0.55)}px, 1fr))` : p.gridCols;
  const sortLabel: Record<SortKey, string> = { name: "Theo tên máy", user: "Theo người dùng", group: "Theo nhóm", status: "Theo trạng thái" };
  const sortMenu = (
    <MenuList>
      {(Object.keys(sortLabel) as SortKey[]).map((k) => <MenuItemRadio key={k} name="key" value={k}>{t(sortLabel[k])}</MenuItemRadio>)}
      <MenuDivider />
      <MenuItemRadio name="dir" value="asc">{t("Tăng dần")}</MenuItemRadio>
      <MenuItemRadio name="dir" value="desc">{t("Giảm dần")}</MenuItemRadio>
      <MenuDivider />
      <MenuItemCheckbox name="auto" value="on">{t("Tự sắp xếp")}</MenuItemCheckbox>
    </MenuList>
  );

  return (
    <div ref={p.wallRef} className={s.wall} style={{ display: "grid", gridTemplateColumns: cols, ...bgStyle }} role="group" aria-label={t("Máy học sinh")}
      onMouseDown={startBox}
      onClick={(e) => e.target === e.currentTarget && !dragged.current && p.onClear()}
      onContextMenu={(e) => { if (empty(e)) { e.preventDefault(); setMenu({ x: e.clientX, y: e.clientY }); } }}>
      {body}
      {box && <div aria-hidden style={{ position: "fixed", left: box.x, top: box.y, width: box.w, height: box.h, zIndex: 10, pointerEvents: "none", border: `1px solid ${tokens.colorBrandStroke1}`, background: `color-mix(in srgb, ${tokens.colorBrandBackground} 18%, transparent)` }} />}
      <input ref={fileRef} type="file" accept="image/*" hidden aria-label={t("Đặt hình nền…")} onChange={(e) => { void setBackground(e.target.files?.[0]); e.target.value = ""; }} />
      <Menu open={!!menu} onOpenChange={(_, d) => !d.open && setMenu(null)} checkedValues={checked} onCheckedValueChange={onChecked}
        positioning={{ target: { getBoundingClientRect: () => new DOMRect(menu?.x ?? 0, menu?.y ?? 0, 0, 0) } }}>
        <MenuPopover>
          <MenuList>
            <MenuItemRadio name="view" value="thumbs">{t("Hình thu nhỏ")}</MenuItemRadio>
            <MenuItemRadio name="view" value="icons">{t("Biểu tượng lớn")}</MenuItemRadio>
            <MenuItemRadio name="view" value="details">{t("Chi tiết")}</MenuItemRadio>
            <MenuDivider />
            <MenuItem onClick={() => fileRef.current?.click()}>{t("Đặt hình nền…")}</MenuItem>
            <MenuItem disabled={!bg} onClick={clearBg}>{t("Xoá hình nền")}</MenuItem>
            <Menu checkedValues={checked} onCheckedValueChange={onChecked}>
              <MenuTrigger disableButtonEnhancement><MenuItem>{t("Sắp xếp")}</MenuItem></MenuTrigger>
              <MenuPopover>{sortMenu}</MenuPopover>
            </Menu>
            <MenuItemCheckbox name="center" value="on" disabled={!bg}>{t("Căn giữa ảnh")}</MenuItemCheckbox>
          </MenuList>
        </MenuPopover>
      </Menu>
    </div>
  );
}
