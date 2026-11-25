import type { Inventory } from "./store";

/** One CSV cell: quotes doubled, and a leading = + - @ made harmless so Excel never runs it as a formula. */
export function cell(v: string | number): string {
  let s = String(v);
  if (/^[=+\-@\t\r]/.test(s)) s = "'" + s;
  return `"${s.replace(/"/g, '""')}"`;
}

const gb = (b: number) => (b / 2 ** 30).toFixed(1);

/** Class-wide hardware list for the technician. Machines that did not answer are listed without details. */
export function inventoryCsv(rows: { hostname: string; user: string; inventory?: Inventory }[]): string {
  const head = ["PC", "User", "Operating system", "CPU", "Threads", "RAM (GB)", "RAM in use (GB)", "Least free disk (GB)", "Uptime (hours)"];
  const lines = rows.map((r) => {
    const i = r.inventory;
    const freeMin = i && i.disks.length ? Math.min(...i.disks.map((d) => d[2])) : null;
    return [r.hostname, r.user, i?.os ?? "", i?.cpu ?? "", i?.cores ?? "", i ? gb(i.ram_total) : "", i ? gb(i.ram_used) : "", freeMin === null ? "" : gb(freeMin), i ? Math.floor(i.uptime_s / 3600) : ""];
  });
  return "﻿" + [head, ...lines].map((l) => l.map(cell).join(",")).join("\r\n") + "\r\n";
}
