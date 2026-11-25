/** The teacher's own blocked websites and keywords, kept on this Tutor and sent to students as Command::SetGuard. */
export type GuardLists = { sites: string[]; words: string[] };

const KEY = "erg.guard";

/** One entry per line. A site is reduced to its host name ("https://www.a.com/x" -> "a.com"); words are lowercased. */
export function parseList(text: string, kind: "site" | "word"): string[] {
  const out = text.split(/[\n,;]+/).map((s) => s.trim().toLowerCase()).map((s) => (kind === "site" ? s.replace(/^[a-z]+:\/\//, "").replace(/[/?#].*$/, "").replace(/^www\./, "") : s.replace(/\s+/g, " ")));
  const ok = kind === "site" ? /^(\*\.)?[a-z0-9-]+(\.[a-z0-9-]+)+$/ : /^[\p{L}\p{N} ]{2,100}$/u;
  return [...new Set(out.filter((s) => ok.test(s)))].slice(0, 500);
}

export function loadGuard(): GuardLists {
  try {
    const g = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    return { sites: Array.isArray(g.sites) ? g.sites : [], words: Array.isArray(g.words) ? g.words : [] };
  } catch {
    return { sites: [], words: [] };
  }
}

export const saveGuard = (g: GuardLists) => localStorage.setItem(KEY, JSON.stringify(g));
