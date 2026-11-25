/** Test Designer model: one text-based draft per question, converted to the JSON `Paper` the Rust side grades. */
export type KindName = "Choice" | "TrueFalse" | "Fill" | "Match" | "Order" | "Region";
export type Draft = { kind: KindName; text: string; points: number; a: string; b: string };

export const HELP: Record<KindName, string> = {
  Choice: "One option per line; start a line with * to mark a correct answer (more than one allowed).",
  TrueFalse: "Type: true or false.",
  Fill: "One blank per line; separate accepted spellings with |.",
  Match: "One pair per line: left = right.",
  Order: "One item per line, in the CORRECT order; students see them shuffled.",
  Region: "Line 1: image file name (already sent to students). Box below: x,y,width,height (each 0..1).",
};

const lines = (s: string) => s.split("\n").map((x) => x.trim()).filter(Boolean);

export function toQuestion(d: Draft, n: number) {
  const bad = (m: string): never => { throw new Error(`Question ${n}: ${m}`); };
  if (!d.text.trim()) bad("the question text is empty");
  if (!(d.points >= 0 && d.points <= 1000)) bad("points must be between 0 and 1000");
  const L = lines(d.a);
  let kind: unknown;
  switch (d.kind) {
    case "Choice": {
      const options = L.map((x) => x.replace(/^\*\s*/, ""));
      const correct = L.flatMap((x, i) => (x.startsWith("*") ? [i] : []));
      if (options.length < 2) bad("at least 2 options are needed");
      if (!correct.length) bad("no correct answer is marked with *");
      kind = { Choice: { options, correct } };
      break;
    }
    case "TrueFalse": {
      const v = d.a.trim().toLowerCase();
      if (!["đúng", "sai", "true", "false"].includes(v)) bad("type true or false");
      kind = { TrueFalse: { answer: v === "đúng" || v === "true" } };
      break;
    }
    case "Fill": {
      if (!L.length) bad("at least one blank is needed");
      kind = { Fill: { accepted: L.map((x) => x.split("|").map((y) => y.trim()).filter(Boolean)) } };
      if ((kind as { Fill: { accepted: string[][] } }).Fill.accepted.some((a) => !a.length)) bad("a blank has no accepted answer");
      break;
    }
    case "Match": {
      const pairs = L.map((x) => x.split("=").map((y) => y.trim()));
      if (pairs.length < 2 || pairs.some((p) => p.length !== 2 || !p[0] || !p[1])) bad("at least 2 lines of the form 'left = right' are needed");
      kind = { Match: { left: pairs.map((p) => p[0]), right: pairs.map((p) => p[1]), pairs: pairs.map((_, i) => i) } };
      break;
    }
    case "Order":
      if (L.length < 2) bad("at least 2 items are needed");
      kind = { Order: { items: L } };
      break;
    case "Region": {
      const r = d.b.split(",").map((x) => Number(x.trim()));
      if (!L[0]) bad("the image name is missing");
      if (r.length !== 4 || r.some((x) => !(x >= 0 && x <= 1))) bad("the region needs 4 numbers in 0..1");
      kind = { Region: { image: L[0], x: r[0], y: r[1], w: r[2], h: r[3] } };
      break;
    }
  }
  return { text: d.text.trim(), points: d.points, kind };
}

export function toPaper(title: string, shuffle: boolean, drafts: Draft[]) {
  if (!drafts.length) throw new Error("The test has no questions");
  return { title: title.trim() || "Test", shuffle, questions: drafts.map((d, i) => toQuestion(d, i + 1)) };
}
