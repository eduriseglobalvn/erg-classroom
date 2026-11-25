import { Menu, MenuDivider, MenuItem, MenuList, MenuPopover, MenuTrigger, makeStyles, mergeClasses, shorthands, tokens } from "@fluentui/react-components";
import { Checkmark20Regular } from "@fluentui/react-icons";
import { useState } from "react";
import type { Menu as MenuDef } from "./Ribbon";
import type { Item } from "./ui";

const useStyles = makeStyles({
  bar: { display: "flex", alignItems: "center", flex: "0 1 auto", minWidth: 0, overflowX: "auto", scrollbarWidth: "none" },
  top: {
    height: "24px", padding: "0 8px", borderRadius: tokens.borderRadiusMedium, fontFamily: "inherit", fontSize: "13px", cursor: "pointer", whiteSpace: "nowrap",
    background: "transparent", color: tokens.colorNeutralForeground1, ...shorthands.border("0"),
    ":hover": { background: tokens.colorSubtleBackgroundHover }, ":focus-visible": { outline: `2px solid ${tokens.colorStrokeFocus2}`, outlineOffset: "-2px" },
  },
  on: { background: tokens.colorSubtleBackgroundSelected },
  popover: {
    minWidth: "220px", maxHeight: "calc(100vh - 48px)", overflowY: "auto", borderRadius: "8px",
    "& .fui-MenuItem": { minHeight: "28px", paddingTop: "3px", paddingBottom: "3px", fontSize: "13px" },
  },
  head: { padding: "6px 10px", fontSize: "12px", fontWeight: 600, color: tokens.colorNeutralForeground3, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
});

/** The flyout of a menu: text rows, separators, a check mark for items with a state (`on`), `sub` opens a submenu. Also used for the student right-click menu (`head` names the target). */
export function MenuPop({ items, head }: { items: Item[]; head?: string }) {
  const s = useStyles();
  return (
    <MenuPopover className={s.popover}>
      {head && <div className={s.head}>{head}</div>}
      <MenuList hasIcons={items.some((it) => it !== "-" && it.on !== undefined)}>
        {items.map((it, i) => it === "-" ? <MenuDivider key={i} /> : it.sub ? (
          <Menu key={i}>
            <MenuTrigger disableButtonEnhancement><MenuItem>{it.title}</MenuItem></MenuTrigger>
            <MenuPop items={it.sub} />
          </Menu>
        ) : (
          <MenuItem key={i} icon={it.on ? <Checkmark20Regular /> : undefined} disabled={!!it.off} title={it.off} onClick={it.run} secondaryContent={it.kbd}>{it.title}</MenuItem>
        ))}
      </MenuList>
    </MenuPopover>
  );
}

/** Classic menu bar: secondary access to the same commands the ribbon runs. */
export function MenuBar({ menus, label }: { menus: MenuDef[]; label: string }) {
  const s = useStyles();
  const [open, setOpen] = useState<string | null>(null);
  return (
    <div className={s.bar} role="toolbar" aria-label={label}>
      {menus.map((m) => (
        <Menu key={m.label} open={open === m.label} onOpenChange={(_, d) => setOpen(d.open ? m.label : null)}>
          <MenuTrigger disableButtonEnhancement>
            <button type="button" className={mergeClasses(s.top, open === m.label && s.on)}
              onMouseEnter={() => open && setOpen(m.label)}>{m.label}</button>
          </MenuTrigger>
          <MenuPop items={m.items} />
        </Menu>
      ))}
    </div>
  );
}
