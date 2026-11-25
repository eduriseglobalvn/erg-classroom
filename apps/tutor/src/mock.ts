// Browser-only fake backend so the UI can be developed with hot reload at http://localhost:3333
// (no Tauri, no real students). Never loaded inside the real app.
import { mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";

const COUNT = 47;
const names = ["VIEN", "LAPTOP"];

// Draw a fake desktop so the tiles look alive; returns base64 JPEG without the data: prefix.
function fakeScreen(seed: number): string {
  const c = document.createElement("canvas");
  c.width = 320;
  c.height = 180;
  const g = c.getContext("2d")!;
  const hue = (seed * 47) % 360;
  const bg = g.createLinearGradient(0, 0, 320, 180);
  bg.addColorStop(0, `hsl(${hue} 60% 45%)`);
  bg.addColorStop(1, `hsl(${(hue + 60) % 360} 70% 30%)`);
  g.fillStyle = bg;
  g.fillRect(0, 0, 320, 180);
  g.fillStyle = "rgba(255,255,255,.92)";
  g.fillRect(30 + (seed % 5) * 6, 24, 190, 112); // a window
  g.fillStyle = `hsl(${hue} 70% 50%)`;
  g.fillRect(30 + (seed % 5) * 6, 24, 190, 14);
  g.fillStyle = "rgba(20,20,20,.8)";
  g.fillRect(0, 164, 320, 16); // taskbar
  return c.toDataURL("image/jpeg", 0.6).split(",")[1];
}

export function install() {
  mockIPC(
    async (cmd, args) => {
      if (cmd === "start_roster" || cmd === "set_room") { // set_room: the class reconnects
        for (let i = 1; i <= COUNT; i++) {
          const id = `mock-${i}`;
          const hostname = i === 41 ? names[0] : `${names[1]}${String(i).padStart(2, "0")}`;
          await emit("student-online", { id, hostname, user: `hs${i}`, room: "Phòng Tin 1", online: true, ...(i % 9 === 3 ? { version: "0.1", outdated: true, legacy: true } : { version: "0.2.0" }) });
          await emit("student-thumb", { id, jpeg: fakeScreen(i) });
        }
        await emit("student-help", { id: "mock-5" });
        return null;
      }
      if (cmd === "app_version") return "0.2.0";
      if (cmd === "open_releases") return null;
      if (cmd === "send_command") return (args as { ids: string[] | null }).ids?.length ?? COUNT;
      if (cmd === "send_file") return [COUNT, 0];
      if (cmd === "local_dir") return ["C:\\Users\\Teacher\\Downloads", [{ name: "Lessons", is_dir: true, size: 0, modified_s: 1795000000 }, { name: "Bai 8 - Cai thien cau lenh.pptx", is_dir: false, size: 965905, modified_s: 1794900000 }, { name: "Bai 9.pptx", is_dir: false, size: 3728349, modified_s: 1794800000 }]];
      if (cmd === "start_show") return COUNT;
      if (cmd === "plugin:dialog|open") return "C:\\bai1.xlsx";
      return null;
    },
    { shouldMockEvents: true },
  );
}
