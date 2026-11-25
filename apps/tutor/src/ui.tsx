import { ButtonHTMLAttributes, ReactElement, ReactNode, forwardRef, useState } from "react";
import { Badge, Popover, PopoverSurface, PopoverTrigger, Tooltip, makeStyles, mergeClasses, shorthands, tokens, useArrowNavigationGroup } from "@fluentui/react-components";
import { ChevronDown12Regular } from "@fluentui/react-icons";

const useStyles = makeStyles({
  items: { display: "flex", flex: "0 0 auto", alignItems: "stretch" },
  div: { width: "1px", margin: "0 4px", background: tokens.colorNeutralStroke2, flex: "0 0 auto" },
  btn: {
    position: "relative", display: "flex", flexDirection: "column", alignItems: "center", gap: "4px",
    width: "66px", flex: "0 0 auto", boxSizing: "border-box", padding: "6px 1px 4px", ...shorthands.border("1px", "solid", "transparent"), borderRadius: tokens.borderRadiusMedium,
    background: "transparent", color: tokens.colorNeutralForeground1, cursor: "pointer", fontFamily: "inherit",
    ":hover": { background: tokens.colorSubtleBackgroundHover },
    ":active": { background: tokens.colorSubtleBackgroundPressed },
    ":focus-visible": { outline: `2px solid ${tokens.colorStrokeFocus2}`, outlineOffset: "-2px" },
    ":disabled": { opacity: 0.4, cursor: "default", background: "transparent" },
  },
  active: { background: tokens.colorBrandBackground2, ...shorthands.borderColor(tokens.colorBrandStroke2), ":hover": { background: tokens.colorBrandBackground2 } },
  open: { background: tokens.colorSubtleBackgroundSelected, ...shorthands.borderColor(tokens.colorNeutralStroke2) },
  icon: { display: "grid", placeItems: "center", height: "26px", flex: "0 0 auto", color: tokens.colorBrandForeground1, "& svg": { width: "26px", height: "26px" } },
  label: { fontSize: "11.5px", lineHeight: "14px", textAlign: "center" },
  chev: { marginLeft: "2px", verticalAlign: "-1px", color: tokens.colorNeutralForeground3 },
  badge: { position: "absolute", top: "2px", right: "4px" },
  split: { display: "flex", alignItems: "stretch" },
  splitMain: { borderTopRightRadius: 0, borderBottomRightRadius: 0, width: "52px" },
  splitCh: {
    width: "14px", boxSizing: "border-box", display: "flex", alignItems: "flex-end", justifyContent: "center", padding: "0 0 22px", ...shorthands.border("1px", "solid", "transparent"),
    borderTopLeftRadius: 0, borderBottomLeftRadius: 0, borderRadius: tokens.borderRadiusMedium, background: "transparent", color: tokens.colorNeutralForeground3, cursor: "pointer",
    ":hover": { background: tokens.colorSubtleBackgroundHover },
    ":focus-visible": { outline: `2px solid ${tokens.colorStrokeFocus2}`, outlineOffset: "-2px" },
  },
  surface: { padding: "4px", minWidth: "220px", maxWidth: "380px", borderRadius: "8px", ...shorthands.border("1px", "solid", tokens.colorNeutralStroke1) },
  list: { display: "flex", flexDirection: "column", gap: "1px" },
  item: {
    display: "flex", alignItems: "center", textAlign: "left", width: "100%", minHeight: "32px", boxSizing: "border-box",
    padding: "4px 10px", ...shorthands.border("0"), borderRadius: tokens.borderRadiusMedium, background: "transparent", color: tokens.colorNeutralForeground1,
    cursor: "pointer", fontFamily: "inherit",
    ":hover": { background: tokens.colorSubtleBackgroundHover }, ":active": { background: tokens.colorSubtleBackgroundPressed },
    ":focus-visible": { outline: `2px solid ${tokens.colorStrokeFocus2}`, outlineOffset: "-2px" },
  },
  itemOff: { opacity: 0.45, cursor: "not-allowed", ":hover": { background: "transparent" }, ":active": { background: "transparent" } },
  itemIcon: { display: "grid", placeItems: "center", color: tokens.colorBrandForeground1, "& svg": { width: "24px", height: "24px" } },
  title: { fontSize: "13px", lineHeight: "18px", display: "flex", alignItems: "center", gap: "6px" },
  desc: { display: "block", fontSize: "12px", lineHeight: "16px", color: tokens.colorNeutralForeground3 },
  sep: { height: "1px", margin: "4px 0", background: tokens.colorNeutralStroke2, flex: "0 0 auto" },
  foot: { display: "flex", alignItems: "center", flexWrap: "wrap", gap: "6px", padding: "6px 8px 4px", marginTop: "2px", borderTop: `1px solid ${tokens.colorNeutralStroke2}` },
  footLbl: { fontSize: "12px", color: tokens.colorNeutralForeground3 },
  chip: {
    display: "inline-flex", alignItems: "center", gap: "4px", height: "26px", padding: "0 10px", borderRadius: tokens.borderRadiusMedium, fontFamily: "inherit", fontSize: "12px", cursor: "pointer",
    background: "transparent", color: tokens.colorNeutralForeground2, ...shorthands.border("1px", "solid", tokens.colorNeutralStroke1),
    ":hover": { background: tokens.colorSubtleBackgroundHover }, ":focus-visible": { outline: `2px solid ${tokens.colorStrokeFocus2}` },
    "& svg": { width: "16px", height: "16px" },
  },
  chipOn: { background: tokens.colorBrandBackground2, color: tokens.colorBrandForeground2, ...shorthands.borderColor(tokens.colorBrandStroke1) },
  fly: { display: "flex", gap: "2px", padding: "4px 6px" },
  flyBtn: {
    display: "flex", flexDirection: "column", alignItems: "center", gap: "3px", width: "68px", padding: "6px 2px 4px", ...shorthands.border("0"), borderRadius: tokens.borderRadiusMedium,
    background: "transparent", color: tokens.colorNeutralForeground1, cursor: "pointer", fontFamily: "inherit", fontSize: "12px", lineHeight: "14px", textAlign: "center",
    ":hover": { background: tokens.colorSubtleBackgroundHover }, ":active": { background: tokens.colorSubtleBackgroundPressed },
    ":focus-visible": { outline: `2px solid ${tokens.colorStrokeFocus2}`, outlineOffset: "-2px" },
  },
});

/** Ribbon command: big icon above a (up to two-line) label. `menu` adds the ▾ marker. */
export const RBtn = forwardRef<HTMLButtonElement, { icon: ReactNode; badge?: number; active?: boolean; menu?: boolean; open?: boolean } & ButtonHTMLAttributes<HTMLButtonElement>>(
  function RBtn({ icon, children, badge, active, menu, open, className, ...rest }, ref) {
    const s = useStyles();
    return (
      <button ref={ref} type="button" {...rest} className={mergeClasses(s.btn, active && s.active, open && s.open, className)} aria-pressed={active}>
        <span className={s.icon}>{icon}</span>
        <span className={s.label}>{children}{menu && <ChevronDown12Regular className={s.chev} />}</span>
        {!!badge && <Badge className={s.badge} color="danger" size="small" appearance="filled">{badge}</Badge>}
      </button>
    );
  });

/** v2 ribbon icon on a 24px grid: a line path `d` over an optional soft fill `f`, in the group's colour. */
export const Ico = ({ d, f, color }: { d: string; f?: string; color: string }) => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden style={{ color }}>
    {f && <path d={f} fill="currentColor" fillOpacity=".16" stroke="none" />}<path d={d} />
  </svg>
);

/** A run of related commands followed by a separator; the caption is for screen readers only. */
export function RGroup({ caption, children }: { caption: string; children: ReactNode }) {
  const s = useStyles();
  return (
    <>
      <div className={s.items} role="group" aria-label={caption}>{children}</div>
      <div className={s.div} role="separator" />
    </>
  );
}

export const Tip = ({ label, children }: { label: string; children: React.ReactElement }) => <Tooltip content={label} relationship="label" positioning="after">{children}</Tooltip>;

/** One row of a dropdown. `off` is the reason it is unavailable (shown as a tooltip); `"-"` is a separator; `sub` is a submenu (menu bar and right-click menus only). */
export type Item = "-" | { icon?: ReactElement; title: string; desc?: string; run?: () => void; off?: string; on?: boolean; sub?: Item[]; kbd?: string };
export type Chip = { icon?: ReactNode; label: string; on?: boolean; off?: string; run: () => void };

function Disabled({ off, children }: { off?: string; children: React.ReactElement }) {
  return off ? <Tooltip content={off} relationship="description" positioning="after">{children}</Tooltip> : children;
}

/** Popover with keyboard support: Esc / outside click close it, Tab stays inside, arrow keys move between rows. */
export function Pop({ label, trigger, children, width }: { label: string; trigger: (open: boolean) => React.ReactElement; children: (close: () => void) => ReactNode; width?: number }) {
  const s = useStyles();
  const [open, setOpen] = useState(false);
  return (
    <Popover open={open} onOpenChange={(_, d) => setOpen(d.open)} positioning="below-start" trapFocus withArrow={false}>
      <PopoverTrigger disableButtonEnhancement>{trigger(open)}</PopoverTrigger>
      <PopoverSurface className={s.surface} style={width !== undefined ? { minWidth: width, maxWidth: Math.max(380, width) } : undefined} aria-label={label}>{children(() => setOpen(false))}</PopoverSurface>
    </Popover>
  );
}

function Rows({ label, items, close }: { label: string; items: Item[]; close: () => void }) {
  const s = useStyles();
  const nav = useArrowNavigationGroup({ axis: "vertical", circular: true });
  return (
    <div className={s.list} role="menu" aria-label={label} {...nav}>
      {items.map((it, i) => it === "-" ? <div key={i} className={s.sep} role="separator" /> : (
        <Disabled key={i} off={it.off}>
          <button type="button" role="menuitem" className={mergeClasses(s.item, it.off && s.itemOff)} aria-disabled={!!it.off}
            onClick={() => { if (it.off) return; close(); it.run?.(); }}>
            <span>
              <span className={s.title}>{it.title}{it.on && <Badge size="tiny" appearance="filled" color="brand" />}</span>
              {it.desc && <span className={s.desc}>{it.desc}</span>}
            </span>
          </button>
        </Disabled>
      ))}
    </div>
  );
}

function Chips({ chips, label, close }: { chips: Chip[]; label?: string; close: () => void }) {
  const s = useStyles();
  return (
    <div className={s.foot} role="group" aria-label={label}>
      {label && <span className={s.footLbl}>{label}</span>}
      {chips.map((c) => (
        <Disabled key={c.label} off={c.off}>
          <button type="button" className={mergeClasses(s.chip, c.on && s.chipOn)} aria-pressed={c.on} aria-disabled={!!c.off}
            onClick={() => { if (c.off) return; close(); c.run(); }}>{c.icon}{c.label}</button>
        </Disabled>
      ))}
    </div>
  );
}

/** Ribbon button that opens a rich dropdown (title + description rows, separators, optional footer chips). */
export function RDrop({ icon, label, items, chips, chipsLabel, width }: { icon: ReactNode; label: string; items: Item[]; chips?: Chip[]; chipsLabel?: string; width?: number }) {
  return (
    <Pop label={label} width={width} trigger={(open) => <RBtn icon={icon} menu open={open} aria-haspopup="menu" aria-expanded={open}>{label}</RBtn>}>
      {(close) => <><Rows label={label} items={items} close={close} />{chips && <Chips chips={chips} label={chipsLabel} close={close} />}</>}
    </Pop>
  );
}

/** Split button: the main part runs `onClick`, the ▾ strip opens the dropdown. */
export function RSplit({ icon, label, onClick, off, active, items, chips, chipsLabel, optionsLabel, width }: {
  icon: ReactNode; label: string; onClick: () => void; off?: string; active?: boolean; items: Item[]; chips?: Chip[]; chipsLabel?: string; optionsLabel: string; width?: number;
}) {
  const s = useStyles();
  return (
    <div className={s.split}>
      <RBtn icon={icon} className={s.splitMain} active={active} disabled={!!off} title={off} onClick={onClick}>{label}</RBtn>
      <Pop label={label} width={width} trigger={(open) => (
        <button type="button" className={mergeClasses(s.splitCh, open && s.open)} aria-label={`${label}: ${optionsLabel}`} aria-haspopup="menu" aria-expanded={open}><ChevronDown12Regular /></button>
      )}>
        {(close) => <><Rows label={label} items={items} close={close} />{chips && <Chips chips={chips} label={chipsLabel} close={close} />}</>}
      </Pop>
    </div>
  );
}

/** Ribbon button that opens a horizontal flyout of big icon buttons (several quick commands in one place). */
export function RFlyout({ icon, label, items }: { icon: ReactNode; label: string; items: Exclude<Item, "-">[] }) {
  const s = useStyles();
  const nav = useArrowNavigationGroup({ axis: "horizontal", circular: true });
  return (
    <Pop label={label} width={0} trigger={(open) => <RBtn icon={icon} menu open={open} aria-haspopup="menu" aria-expanded={open}>{label}</RBtn>}>
      {(close) => (
        <div className={s.fly} role="menu" aria-label={label} {...nav}>
          {items.map((it) => (
            <Disabled key={it.title} off={it.off}>
              <button type="button" role="menuitem" className={mergeClasses(s.flyBtn, it.off && s.itemOff)} aria-disabled={!!it.off}
                onClick={() => { if (it.off) return; close(); it.run?.(); }}>
                <span className={s.itemIcon}>{it.icon}</span>{it.title}
              </button>
            </Disabled>
          ))}
        </div>
      )}
    </Pop>
  );
}
