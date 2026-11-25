import { useState } from "react";
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, Dropdown, Input, Option, Switch, Text, Textarea } from "@fluentui/react-components";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { D, DlgHead } from "./FileDist";
import { HELP, toPaper, type Draft, type KindName } from "./paper";

const KINDS: [KindName, string][] = [["Choice", "Multiple choice"], ["TrueFalse", "True / False"], ["Fill", "Fill in the blanks"], ["Match", "Matching"], ["Order", "Ordering"], ["Region", "Pick a region on an image"]];
const blank = (): Draft => ({ kind: "Choice", text: "", points: 1, a: "", b: "" });

/** In-app test editor: builds the paper, checks it with the same validator the grader uses, and saves a .json file. */
export function Designer({ onClose, say }: { onClose: () => void; say: (s: string) => void }) {
  const [title, setTitle] = useState("");
  const [shuffle, setShuffle] = useState(true);
  const [qs, setQs] = useState<Draft[]>([blank()]);
  const [err, setErr] = useState("");
  const set = (i: number, p: Partial<Draft>) => setQs((l) => l.map((q, j) => (j === i ? { ...q, ...p } : q)));

  const build = async () => {
    const json = JSON.stringify(toPaper(title, shuffle, qs));
    await invoke("make_student_paper", { paper: json, seed: 1 }); // Rust-side validation
    return json;
  };
  const doSave = async () => {
    try {
      const json = await build();
      const path = await save({ defaultPath: "test.json", filters: [{ name: "Test", extensions: ["json"] }] });
      if (path) { await invoke("write_text", { path, text: JSON.stringify(JSON.parse(json), null, 2) }); say(`Test saved: ${path}`); onClose(); }
    } catch (e) { setErr(String((e as Error)?.message ?? e)); }
  };

  return (
    <Dialog open onOpenChange={(_, d) => !d.open && onClose()}>
      <DialogSurface style={{ ...D.surface, maxWidth: 760, width: "90vw" }}>
        <DialogBody style={D.body}>
          <DlgHead title="Test Designer" onClose={onClose} />
          <DialogContent style={{ ...D.content, display: "grid", gap: 10, maxHeight: "65vh", overflow: "auto" }}>
            <div style={{ display: "flex", gap: 12, alignItems: "center" }}>
              <Input placeholder="Test title" value={title} onChange={(_, d) => setTitle(d.value)} style={{ flex: 1 }} />
              <Switch label="Shuffle" checked={shuffle} onChange={(_, d) => setShuffle(d.checked)} />
            </div>
            {qs.map((q, i) => (
              <div key={i} style={{ border: "1px solid #8884", borderRadius: 6, padding: 8, display: "grid", gap: 6 }}>
                <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                  <Text weight="semibold">Question {i + 1}</Text>
                  <Dropdown value={KINDS.find((k) => k[0] === q.kind)![1]} selectedOptions={[q.kind]} onOptionSelect={(_, d) => set(i, { kind: d.optionValue as KindName, a: "", b: "" })}>
                    {KINDS.map(([k, l]) => <Option key={k} value={k}>{l}</Option>)}
                  </Dropdown>
                  <Input type="number" style={{ width: 90 }} value={String(q.points)} onChange={(_, d) => set(i, { points: Number(d.value) })} contentAfter={<span>points</span>} />
                  <Button size="small" onClick={() => setQs((l) => l.filter((_, j) => j !== i))} disabled={qs.length === 1}>Delete</Button>
                </div>
                <Textarea rows={2} placeholder="Question text" value={q.text} onChange={(_, d) => set(i, { text: d.value })} />
                <Text size={200}>{HELP[q.kind]}</Text>
                <Textarea rows={4} value={q.a} onChange={(_, d) => set(i, { a: d.value })} />
                {q.kind === "Region" && <Input placeholder="x,y,width,height  e.g. 0.1,0.2,0.3,0.3" value={q.b} onChange={(_, d) => set(i, { b: d.value })} />}
              </div>
            ))}
            <Button onClick={() => setQs((l) => [...l, blank()])}>+ Add question</Button>
            {err && <Text style={{ color: "#c00" }}>{err}</Text>}
          </DialogContent>
          <DialogActions style={D.foot}>
            <Button onClick={onClose}>Close</Button>
            <Button appearance="primary" onClick={doSave}>Check and save…</Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
