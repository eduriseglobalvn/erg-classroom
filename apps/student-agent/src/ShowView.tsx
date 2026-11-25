import { useEffect, useRef, useState } from "react";
import { Channel, invoke } from "@tauri-apps/api/core";
import { t } from "./i18n";

// Rust prefixes each access unit with: [keyframe: u8][frame_no: u32 LE]. Baseline, level 4.0, Annex-B.
const CODEC = "avc1.42E02A";
const HEADER = 5;

/** Full-screen view of the teacher's screen, decoded with the GPU through WebCodecs. */
export function ShowView() {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [status, setStatus] = useState(t("Đang chờ hình từ giáo viên…"));
  const [info, setInfo] = useState("");

  useEffect(() => {
    const cv = canvas.current!;
    const ctx = cv.getContext("2d")!;
    let decoder: VideoDecoder | null = null;
    let needKey = true;
    let shown = false;
    let frames = 0;

    const make = () => {
      decoder?.close();
      decoder = new VideoDecoder({
        output: (f) => {
          if (cv.width !== f.displayWidth || cv.height !== f.displayHeight) {
            cv.width = f.displayWidth;
            cv.height = f.displayHeight;
          }
          ctx.drawImage(f, 0, 0);
          f.close();
          frames++;
          if (frames % 15 === 1) setInfo(`${cv.width}×${cv.height} · ${frames} ${t("khung")}`);
          if (!shown) {
            shown = true;
            setStatus("");
          }
        },
        // A decoder error leaves it closed; restart and wait for the next keyframe (at most 2 s).
        error: () => {
          needKey = true;
          make();
        },
      });
      decoder.configure({ codec: CODEC, hardwareAcceleration: "prefer-hardware", optimizeForLatency: true });
    };
    make();

    const ch = new Channel<ArrayBuffer>();
    ch.onmessage = (buf) => {
      const b = new Uint8Array(buf);
      const key = b[0] === 1;
      if (needKey && !key) return;
      if (!decoder || decoder.state !== "configured") return;
      // Dropping when the decoder falls behind keeps latency bounded instead of growing.
      if (decoder.decodeQueueSize > 6 && !key) {
        needKey = true;
        return;
      }
      needKey = false;
      const frameNo = new DataView(buf).getUint32(1, true);
      decoder.decode(new EncodedVideoChunk({ type: key ? "key" : "delta", timestamp: frameNo * 16_667, data: b.subarray(HEADER) }));
    };
    invoke("show_subscribe", { onFrame: ch });
    return () => decoder?.close();
  }, []);

  return (
    <div style={{ position: "fixed", inset: 0, background: "#000", cursor: "none" }}>
      <canvas ref={canvas} style={{ width: "100%", height: "100%", objectFit: "contain", display: "block" }} />
      {info && <div style={{ position: "absolute", left: 12, bottom: 8, color: "#8a8", fontSize: 13, opacity: 0.8 }}>● {info}</div>}
      {status && (
        <div style={{ position: "absolute", inset: 0, display: "grid", placeItems: "center", color: "#aaa", fontSize: 24 }}>{status}</div>
      )}
    </div>
  );
}
