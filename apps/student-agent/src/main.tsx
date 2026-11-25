import { createRoot } from "react-dom/client";
import { FluentProvider, webDarkTheme, webLightTheme } from "@fluentui/react-components";
import { ShowView } from "./ShowView";
import { ConfigView } from "./ConfigView";
import { ChatView, LockView, MessageView, PollView } from "./Views";
import { initLang } from "./i18n";
import { BoardView, QuestionView, RegisterView, TestView, ToolbarView, VideoView } from "./Views2";

// One bundle, several window kinds: the Rust side opens windows with ?view=show|lock|msg.
const q = new URLSearchParams(location.search);
const view = q.get("view");
const dark = window.matchMedia("(prefers-color-scheme: dark)").matches;
// the v2 windows are drawn light with the navy accent whatever the system theme
const v2 = view === "toolbar" || view === "msg" || view === "chat";
const v2Theme = { ...webLightTheme, colorBrandBackground: "#1F4E9E", colorBrandBackgroundHover: "#1A4387", colorBrandBackgroundPressed: "#173B78", colorCompoundBrandStroke: "#1F4E9E" };

function Root() {
  if (view === "show") return <ShowView />;
  // a normal opaque window, unlike the overlays below
  if (view === "config") return <FluentProvider theme={dark ? webDarkTheme : webLightTheme} style={{ height: "100%" }}><ConfigView /></FluentProvider>;
  return (
    <FluentProvider theme={v2 ? v2Theme : view === "lock" || dark ? webDarkTheme : webLightTheme} style={{ background: "transparent", height: "100%" }}>
      {view === "lock" ? (
        <LockView blank={q.get("blank") === "1"} text={q.get("text") ?? ""} warn={q.get("warn") ?? undefined} app={q.get("app") ?? undefined} />
      ) : view === "msg" ? (
        <MessageView text={q.get("text") ?? ""} seconds={Number(q.get("s") ?? 20)} />
      ) : view === "chat" ? (
        <ChatView text={q.get("text") ?? ""} />
      ) : view === "toolbar" ? (
        <ToolbarView />
      ) : view === "register" ? (
        <RegisterView />
      ) : view === "question" ? (
        <QuestionView id={Number(q.get("id"))} text={q.get("text") ?? ""} />
      ) : view === "board" ? (
        <BoardView />
      ) : view === "video" ? (
        <VideoView path={q.get("p") ?? ""} />
      ) : view === "test" ? (
        <TestView id={Number(q.get("id"))} />
      ) : view === "poll" ? (
        <PollView id={Number(q.get("id"))} data={JSON.parse(q.get("d") ?? "{\"q\":\"\",\"o\":[]}")} />
      ) : null}
    </FluentProvider>
  );
}

// the language must be known before the first paint, or English windows would flash Vietnamese
initLang().finally(() => createRoot(document.getElementById("root")!).render(<Root />));
