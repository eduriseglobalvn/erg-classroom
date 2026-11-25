export type SortKey = "name" | "user" | "group" | "status";
export type SortDir = "asc" | "desc";
type S = { hostname: string; user: string; online: boolean; help?: boolean };

const cmp = (a: string, b: string) => a.localeCompare(b, undefined, { numeric: true, sensitivity: "base" });
/** help first, then online, then offline */
const rank = (s: S) => (s.help ? 0 : s.online ? 1 : 2);

/** Pure: sorted copy. Empty values go last in ascending order; ties fall back to host name so the result is stable. */
export function sortStudents<T extends S>(list: T[], key: SortKey, dir: SortDir, groups: Record<string, string>): T[] {
  const val = (s: T) => (key === "name" ? s.hostname : key === "user" ? s.user : key === "group" ? groups[s.hostname] ?? "" : "");
  const sign = dir === "asc" ? 1 : -1;
  return [...list].sort((a, b) => {
    let r: number;
    if (key === "status") r = rank(a) - rank(b);
    else {
      const x = val(a), y = val(b);
      r = !x && y ? 1 : x && !y ? -1 : cmp(x, y); // empties last, independent of direction below
      if (!x !== !y) return r;
    }
    return r ? r * sign : cmp(a.hostname, b.hostname);
  });
}
