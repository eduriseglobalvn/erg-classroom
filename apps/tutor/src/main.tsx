import React from "react";
import { createRoot } from "react-dom/client";
import { FluentProvider, webDarkTheme, webLightTheme } from "@fluentui/react-components";
import { App } from "./App";
import { OverlayView } from "./Overlay";

const q = new URLSearchParams(location.search);

// Follow the Windows light/dark setting live.
function Root() {
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  const [dark, setDark] = React.useState(mq.matches);
  React.useEffect(() => {
    const f = (e: MediaQueryListEvent) => setDark(e.matches);
    mq.addEventListener("change", f);
    return () => mq.removeEventListener("change", f);
  }, []);
  // The drawing layer is a separate transparent window using the same bundle.
  if (q.get("view") === "overlay") return <OverlayView whiteboard={q.get("mode") === "whiteboard"} />;
  const theme = dark ? webDarkTheme : webLightTheme;
  return (
    // transparent backdrop lets the Mica window effect show through
    <FluentProvider theme={{ ...theme, fontFamilyBase: `'Segoe UI Variable Text', 'Segoe UI Variable', ${theme.fontFamilyBase}` }} style={{ background: "__TAURI_INTERNALS__" in window ? "transparent" : undefined, height: "100%" }}>
      <App />
    </FluentProvider>
  );
}

// Plain browser (npm run dev, port 3333): fake backend instead of Tauri.
if (!("__TAURI_INTERNALS__" in window)) (await import("./mock")).install();

createRoot(document.getElementById("root")!).render(<Root />);
