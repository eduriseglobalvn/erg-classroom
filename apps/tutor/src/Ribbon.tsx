import { makeStyles, tokens } from "@fluentui/react-components";
import {
  Globe24Regular, GlobeProhibited24Regular, Settings24Regular, ShieldTask24Regular, ClipboardTaskListLtr24Regular, AppsList24Regular, Play24Filled, Stop24Filled, LockClosed24Regular, LockOpen24Regular, Square24Filled, Chat24Regular,
  ChatMultiple24Regular, Comment24Regular, Power24Regular, Link24Regular, Send24Regular, ArrowDownload24Regular, HandRight24Regular, Poll24Regular, Info24Regular,
  PeopleTeam24Regular, PeopleCommunity24Regular, PersonAdd24Regular, PersonStar24Regular, Key24Regular, KeyReset24Regular, Dismiss24Regular, Clock24Regular,
  ClipboardCheckmark24Regular, DocumentTable24Regular, HardDrive24Regular, ArrowShuffle24Regular, History24Regular, DocumentArrowDown24Regular, Window24Regular,
  PersonBoard24Regular, Pen24Regular, Board24Regular, Edit24Regular, Video24Regular, Play24Regular, Pause24Regular, FastForward24Regular, ScanCamera24Regular,
  Record24Regular, Keyboard24Regular, Folder24Regular, AppsAddIn24Regular, Clipboard24Regular, ClipboardPaste24Regular, WindowConsole20Regular, SpeakerMute24Regular,
  Speaker224Regular, Alert24Regular, Timer24Regular, CheckmarkCircle24Regular, Prohibited24Regular, DismissCircle24Regular, UsbStick24Regular, Print24Regular,
  Shield24Regular, Search24Regular, PlugConnected24Regular, ArrowSync24Regular, SignOut24Regular, QuestionCircle24Regular, Trophy24Regular, Star24Regular,
  DocumentEdit24Regular, Calendar24Regular, PhoneDesktop24Regular, Eye24Regular, Checkmark24Regular, Desktop24Regular, Rocket24Regular, Translate24Regular,
  PanelRight24Regular, ZoomIn24Regular, SelectAllOn24Regular, SelectAllOff24Regular, PersonQuestionMark24Regular,
} from "@fluentui/react-icons";
import { invoke } from "@tauri-apps/api/core";
import { useClass } from "./store";
import { useExtras } from "./Extras";
import { useLang, useT } from "./i18n";
import { useUi } from "./uistate";
import { type Chip, Ico, type Item, Pop, RBtn, RDrop, RGroup, RSplit } from "./ui";
import { QuickLaunch } from "./Panes";
import { KEYS } from "./Panes3";

type Cmd = Exclude<Item, "-">;
export type Menu = { label: string; items: Item[] };

/** What App owns: the small prompt dialogs, the machine-info dialog and the help filter. */
export type AppCtl = {
  say: (s: string) => void; fail: (s: string) => void; run: (p: Promise<unknown>) => void;
  ask: { link: () => void; msg: () => void; chat: () => void; poll: () => void; webAllow: () => void; webBlock: () => void; guard: () => void; appBlock: () => void; appAllow: () => void; work: () => void };
  info: () => void;
  /** show the Web access pane in place of the wall */
  openWeb: () => void;
  /** show the Assessments pane; open the Settings / Web rules dialogs */
  openAssess: () => void; settings: () => void; rules: () => void;
  shortcuts: () => void; timetable: () => void; about: () => void;
  help: { count: number; on: boolean; toggle: () => void };
};

const useStyles = makeStyles({
  ribbon: {
    display: "flex", alignItems: "stretch", height: "78px", boxSizing: "border-box", padding: "2px 6px", overflowX: "auto", overflowY: "hidden", flex: "0 0 auto", scrollbarWidth: "thin",
    background: tokens.colorNeutralBackground1, borderTop: `1px solid ${tokens.colorNeutralStroke3}`, borderBottom: `1px solid ${tokens.colorNeutralStroke2}`,
  },
});

// v2 ribbon icons (design NRibbon): [colour, line path, soft fill path]
const SCR = "M3 5h18v11H3z", CIRC = "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z", LOCK = "M6 11h12v9H6z";
const BLUE = "#1F5FBF", GREEN = "#107C41", AMBER = "#B7791F", RED = "#C42B1C", PURPLE = "#6B4FBB", TEAL = "#0B7285", SLATE = "#4A5568";
const ICONS = {
  class: [BLUE, "M8 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM16 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM3 19c0-3 2.2-5 5-5s5 2 5 5M14.5 14.2c.5-.1 1-.2 1.5-.2 2.8 0 5 2 5 5", "M8 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM16 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6z"],
  register: [BLUE, "M9 5H6v15h12V5h-3M9 3.5h6v3H9zM9 13l2 2 4-4", "M6 5h12v15H6z"],
  random: [PURPLE, "M3 7h3l9 10h5M3 17h3l3-3.3M12.3 10.3 15 7h5M17.5 4.5 20 7l-2.5 2.5M17.5 14.5 20 17l-2.5 2.5", ""],
  journal: [BLUE, "M5 4h11a2 2 0 0 1 2 2v14H7a2 2 0 0 1-2-2zM5 18a2 2 0 0 1 2-2h11M9 8h5", "M5 4h11a2 2 0 0 1 2 2v10H7a2 2 0 0 0-2 2z"],
  show: [GREEN, SCR + "M9 20h6M12 16v4M10.5 8.3v4.4l3.8-2.2z", SCR],
  view: [TEAL, "M2.5 12s3.5-6 9.5-6 9.5 6 9.5 6-3.5 6-9.5 6-9.5-6-9.5-6zM12 14.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5z", ""],
  files: [AMBER, "M3 7V5h6l2 2h10v12H3zM8.5 13h7M13 10.5l2.5 2.5-2.5 2.5", "M3 7h18v12H3z"],
  work: [BLUE, "M4 4l17 8-17 8 3-8zM7 12h8", "M4 4l17 8-17 8 3-8z"],
  lock: [AMBER, LOCK + "M8.5 11V8a3.5 3.5 0 0 1 7 0v3M12 14.5v2", LOCK],
  unlock: [AMBER, LOCK + "M8.5 11V8a3.5 3.5 0 0 1 6.7-1.4M12 14.5v2", ""],
  blank: [SLATE, SCR + "M9 20h6M12 16v4", SCR + SCR],
  web: [TEAL, CIRC + "M3 12h18M12 3c2.5 2.6 3.8 5.6 3.8 9s-1.3 6.4-3.8 9c-2.5-2.6-3.8-5.6-3.8-9S9.5 5.6 12 3z", CIRC],
  manage: [SLATE, "M4 7h9M17 7h3M4 17h3M11 17h9M15 9a2 2 0 1 0 0-4 2 2 0 0 0 0 4zM9 19a2 2 0 1 0 0-4 2 2 0 0 0 0 4z", ""],
  apps: [PURPLE, "M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z", "M13 4h7v7h-7zM4 13h7v7H4z"],
  talk: [BLUE, "M4 5h16v11H10l-4 4v-4H4zM8 9h8M8 12h5", "M4 5h16v11H10l-4 4v-4H4z"],
  help: [RED, CIRC + "M12 16a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM5.6 5.6l3.6 3.6M14.8 14.8l3.6 3.6M18.4 5.6l-3.6 3.6M9.2 14.8l-3.6 3.6", CIRC],
  bar: [BLUE, "M3 5h18v14H3zM3 9.5h18M6 7.3h.01M8.6 7.3h.01", "M3 5h18v4.5H3z"],
  test: [GREEN, "M6 3h9l4 4v14H6zM14 3v5h5M9 14l2 2 4-4", "M6 3h9l4 4v14H6z"],
  launch: [AMBER, "M13 3 5 13h6l-1 8 8-10h-6z", "M13 3 5 13h6l-1 8 8-10h-6z"],
  plans: [TEAL, "M4 6h16v14H4zM4 10h16M8 4v4M16 4v4M8 14h3", "M4 6h16v4H4z"],
} satisfies Record<string, [string, string, string]>;
const ico = (k: keyof typeof ICONS) => <Ico color={ICONS[k][0]} d={ICONS[k][1]} f={ICONS[k][2]} />;

/** One registry of every class command; the ribbon and the menu bar both pick from it, so each command has a single implementation. */
export function useCommands(app: AppCtl) {
  const t = useT();
  const { students, selected, showing, startShow, stopShow, sendFile, send, selectAll, clear } = useClass();
  const { lang, toggle: toggleLang } = useLang();
  const ui = useUi();
  const { node, A } = useExtras(send, app.say, app.fail);
  const n1 = selected.size === 1;
  const need1 = n1 ? undefined : t("Chọn đúng một máy học sinh");
  const need0 = selected.size ? undefined : t("Chọn ít nhất một máy");
  // showing to an empty room would look like it works while nobody sees it
  const needAny = Object.values(students).some((s) => s.online) ? undefined : "No student computers are connected";
  const lockText = "Please look at the teacher";
  const web = (mode: string) => () => send({ SetWebPolicy: { mode, list: [] } });
  // shutting down a room by a slip of the mouse loses every student's unsaved work
  const askPower = (p: string, what: string) => () => {
    const n = selected.size || Object.values(students).filter((s) => s.online).length;
    if (window.confirm(`${what}: ${n} ${t("máy")}?\n${t("Bài học sinh chưa lưu sẽ bị mất.")}`)) void send({ Power: p });
  };
  const c = {
    // class / groups
    wizard: { icon: <Clock24Regular />, title: t("Bắt đầu tiết học…"), desc: t("Đặt tên bài, mục tiêu và hẹn giờ trên máy học sinh"), run: A.wizard },
    studentBar: { icon: <Window24Regular />, title: "Student Toolbar", desc: "Show or hide the toolbar at the top of the student screens", run: A.studentBar, on: A.bar },
    refresh: { icon: <ArrowSync24Regular />, title: "Refresh", desc: "Refresh the list of students", run: A.refresh },
    endClass: { icon: <SignOut24Regular />, title: "End Class", desc: "Disconnect the current class and choose a new one", run: A.endClass },
    info: { icon: <Info24Regular />, title: t("Thông tin máy"), desc: t("Hệ điều hành, RAM, ổ đĩa, tiến trình của máy đang chọn"), run: app.info, off: need1 },
    autoGroup: { icon: <PeopleCommunity24Regular />, title: t("Chia nhóm tự động…"), desc: t("Chia đều học sinh vào các nhóm"), run: A.autoGroup },
    assignGroup: { icon: <PersonAdd24Regular />, title: t("Đưa máy đang chọn vào nhóm…"), run: A.assignGroup, off: need0 },
    setLeader: { icon: <PersonStar24Regular />, title: t("Đặt máy đang chọn làm trưởng nhóm"), run: A.setLeader, off: need1 },
    selectLeaders: { icon: <PersonStar24Regular />, title: t("Chọn tất cả trưởng nhóm"), run: A.selectLeaders },
    grantLeaders: { icon: <Key24Regular />, title: t("Cấp quyền cho các trưởng nhóm…"), desc: t("Trưởng nhóm điều khiển được máy trong nhóm, có hẹn giờ"), run: A.grantLeaders },
    revokeLeaders: { icon: <KeyReset24Regular />, title: t("Thu hồi quyền trưởng nhóm"), run: A.revokeLeaders },
    clearGroups: { icon: <Dismiss24Regular />, title: t("Bỏ chia nhóm"), run: A.clearGroups },
    teams: { icon: <Trophy24Regular />, title: t("Bảng điểm theo đội…"), desc: t("Cộng, trừ điểm cho từng nhóm"), run: A.teams },
    // roll call / journal
    roll: { icon: <ClipboardCheckmark24Regular />, title: t("Điểm danh"), desc: t("Yêu cầu học sinh nhập tên và lớp"), run: A.roll },
    attendance: { icon: <DocumentTable24Regular />, title: t("Xuất điểm danh (CSV)…"), desc: t("Lưu danh sách có mặt thành file CSV"), run: A.attendance },
    inventory: { icon: <HardDrive24Regular />, title: t("Kiểm kê phần cứng cả lớp (CSV)…"), desc: t("Thu cấu hình từng máy và lưu CSV"), run: A.inventory },
    random: { icon: <ArrowShuffle24Regular />, title: t("Chọn học sinh ngẫu nhiên"), run: A.random },
    journal: { icon: <History24Regular />, title: t("Xem nhật ký…"), desc: t("Sự kiện của buổi học, có bộ lọc"), run: A.journal },
    journalHtml: { icon: <DocumentArrowDown24Regular />, title: t("Xuất nhật ký (HTML → in PDF)…"), desc: t("Lưu nhật ký để in hoặc nộp"), run: A.journalHtml },
    // presenting
    show: showing
      ? { icon: <Stop24Filled />, title: t("Dừng chiếu"), desc: t("Ngừng chiếu màn hình giáo viên"), run: () => app.run(stopShow()), on: true }
      : { icon: <Play24Filled />, title: t("Chiếu màn hình giáo viên"), desc: t("Cả lớp (hoặc máy đang chọn) xem màn hình của thầy cô"), run: () => app.run(startShow()), off: needAny },
    showApp: { icon: <Window24Regular />, title: t("Chiếu một ứng dụng (chọn cửa sổ trong 3 giây)"), desc: t("Chỉ chiếu một cửa sổ, không lộ phần còn lại"), run: A.showApp, off: needAny },
    exhibit: { icon: <PersonBoard24Regular />, title: t("Chiếu màn hình học sinh đang chọn"), desc: t("Cả lớp xem màn hình của một học sinh"), run: A.exhibit, off: need1 },
    exhibitOff: { icon: <Stop24Filled />, title: t("Dừng chiếu màn hình học sinh"), run: A.exhibitOff },
    annotate: { icon: <Pen24Regular />, title: t("Vẽ lên màn hình (chú thích)"), desc: t("Vẽ, đánh dấu trực tiếp lên màn hình đang chiếu"), run: A.annotate },
    board: { icon: <Board24Regular />, title: t("Bảng tương tác (cả lớp cùng thấy / vẽ)…"), run: A.board },
    whiteboard: { icon: <Edit24Regular />, title: t("Bảng trắng"), run: A.whiteboard },
    playVideo: { icon: <Video24Regular />, title: t("Phát video cho cả lớp…"), desc: t("Gửi video rồi mở trên máy học sinh"), run: A.playVideo },
    videoPlay: { icon: <Play24Regular />, title: t("Video: phát"), run: A.videoPlay },
    videoPause: { icon: <Pause24Regular />, title: t("Video: tạm dừng"), run: A.videoPause },
    videoSeek: { icon: <FastForward24Regular />, title: t("Video: tua…"), run: A.videoSeek },
    videoClose: { icon: <Dismiss24Regular />, title: t("Video: đóng"), run: A.videoClose },
    scan: { icon: <ScanCamera24Regular />, title: t("Quét lần lượt từng máy…"), desc: t("Xem lần lượt từng màn hình học sinh, cỡ lớn"), run: A.scan },
    record: { icon: <Record24Regular />, title: A.recording ? t("Dừng ghi và tạo bản xem lại") : t("Ghi lại màn hình lớp (Replay)"), run: A.record, on: A.recording },
    keys: { icon: <Keyboard24Regular />, title: A.keysOn ? t("Tắt hiện phím đang gõ") : t("Bật hiện phím đang gõ"), run: A.keys, on: A.keysOn },
    remote: { icon: <Eye24Regular />, title: t("Xem / điều khiển"), desc: t("Xem và điều khiển máy học sinh đang chọn"), run: A.remote, off: need1 },
    // files
    sendFile: { icon: <Send24Regular />, title: t("Gửi file"), desc: t("Gửi một tệp tới máy đang chọn hoặc cả lớp"), run: () => sendFile().then((r) => r && app.say(r), (e) => app.fail(String(e))) },
    sendWork: { icon: <Send24Regular />, title: t("Gửi bài…"), desc: t("Gửi một bộ tệp bài tập tới cả lớp, nhóm hoặc máy đang chọn"), run: app.ask.work, off: needAny },
    collect: { icon: <ArrowDownload24Regular />, title: t("Thu bài"), desc: t("Thu tệp bài làm từ máy học sinh"), run: A.collect },
    distribute: { icon: <Folder24Regular />, title: "File Distribution…", desc: "Copy files from this computer to the selected PCs or the whole class", run: A.distribute, off: needAny },
    files: { icon: <Folder24Regular />, title: t("Quản lý tệp máy đang chọn…"), desc: t("Duyệt, đổi tên, xoá tệp trên máy học sinh"), run: A.files, off: need1 },
    deploy: { icon: <AppsAddIn24Regular />, title: t("Cài phần mềm từ xa (.msi/.exe)…"), desc: t("Gửi gói cài và chạy cài im lặng"), run: A.deploy },
    clipClear: { icon: <Clipboard24Regular />, title: t("Xoá clipboard học sinh"), run: A.clipClear },
    clipGet: { icon: <ClipboardPaste24Regular />, title: t("Lấy clipboard máy đang chọn"), run: A.clipGet, off: need1 },
    shell: { icon: <WindowConsole20Regular />, title: t("Dòng lệnh từ xa…"), desc: t("Chạy lệnh trên máy đang chọn (cần quản trị viên bật)"), run: A.shell, off: need1 },
    // student screen
    lock: { icon: <LockClosed24Regular />, title: t("Khoá"), desc: t("Khoá màn hình, học sinh nhìn lên bảng"), run: () => send({ Lock: { blank: false, text: lockText } }) },
    unlock: { icon: <LockOpen24Regular />, title: t("Mở khoá"), run: () => send("Unlock") },
    blank: { icon: <Square24Filled />, title: t("Làm đen"), desc: t("Làm đen màn hình học sinh"), run: () => send({ Lock: { blank: true, text: "" } }) },
    mute: { icon: <SpeakerMute24Regular />, title: t("Tắt tiếng"), run: () => A.mute(true) },
    unmute: { icon: <Speaker224Regular />, title: t("Bật tiếng"), run: () => A.mute(false) },
    beep: { icon: <Alert24Regular />, title: t("Phát tiếng bíp"), run: A.beep },
    // web / apps
    webOpen: { icon: <Globe24Regular />, title: t("Web: không hạn chế"), desc: t("Cho phép mọi trang web"), run: web("Unrestricted") },
    webAll: { icon: <GlobeProhibited24Regular />, title: t("Chặn hết Internet"), run: web("BlockAll") },
    webTemp: {
      icon: <Timer24Regular />, title: t("Cho phép Internet 10 phút rồi chặn lại"), desc: t("Tự chặn lại khi hết giờ"),
      run: () => {
        // The target machines are fixed now, not when the timer fires.
        const ids = [...useClass.getState().selected];
        const cmd = (mode: string) => invoke("send_command", { ids: ids.length ? ids : null, cmd: { SetWebPolicy: { mode, list: [] } } });
        app.run(cmd("Unrestricted").then(() => app.say("Internet allowed for 10 minutes")));
        setTimeout(() => { cmd("BlockAll").then(() => app.say("10 minutes are up: Internet blocked again"), (e) => app.fail(String(e))); }, 10 * 60 * 1000);
      },
    },
    webPane: { icon: <Globe24Regular />, title: t("Truy cập web…"), desc: t("Xem trang mỗi học sinh đang mở, sửa danh sách duyệt / hạn chế"), run: app.openWeb },
    webRules: { icon: <ShieldTask24Regular />, title: t("Quy tắc web và Internet…"), desc: t("Chế độ web, danh sách trang và điều gì xảy ra khi vi phạm"), run: app.rules },
    webAllow: { icon: <CheckmarkCircle24Regular />, title: t("Chỉ cho phép các trang…"), run: app.ask.webAllow },
    webBlock: { icon: <Prohibited24Regular />, title: t("Chặn các trang…"), run: app.ask.webBlock },
    webGuard: { icon: <Prohibited24Regular />, title: t("Web và từ khoá bị chặn…"), desc: t("Mở phải là khoá máy và báo giáo viên"), run: app.ask.guard },
    appOpen: { icon: <AppsList24Regular />, title: t("Ứng dụng: không hạn chế"), desc: t("Cho phép mọi ứng dụng"), run: () => send({ SetAppPolicy: { mode: "Unrestricted", list: [] } }) },
    appBlock: { icon: <Prohibited24Regular />, title: t("Chặn ứng dụng…"), run: app.ask.appBlock },
    appAllow: { icon: <CheckmarkCircle24Regular />, title: t("Chỉ cho phép ứng dụng…"), run: app.ask.appAllow },
    kill: { icon: <DismissCircle24Regular />, title: t("Đóng chương trình…"), desc: t("Đóng chương trình đang chạy theo tên file .exe"), run: A.kill },
    // restrictions
    usbAllow: { icon: <UsbStick24Regular />, title: t("USB: cho phép"), run: () => A.usb("Allow") },
    usbRead: { icon: <UsbStick24Regular />, title: t("USB: chỉ đọc"), run: () => A.usb("ReadOnly") },
    usbNoExec: { icon: <UsbStick24Regular />, title: t("USB: cấm chạy file"), run: () => A.usb("NoExecute") },
    usbBlock: { icon: <UsbStick24Regular />, title: t("USB: chặn hết"), run: () => A.usb("BlockAll") },
    printAllow: { icon: <Print24Regular />, title: t("In ấn: cho phép"), run: () => A.print("Allow") },
    printPause: { icon: <Print24Regular />, title: t("In ấn: tạm dừng"), run: () => A.print("Pause") },
    printBlock: { icon: <Print24Regular />, title: t("In ấn: chặn và xoá hàng đợi"), run: () => A.print("BlockAll") },
    protectOn: { icon: <Shield24Regular />, title: t("Bảo vệ: chặn Control Panel, Run, Task Manager"), run: A.protectOn },
    protectDrives: { icon: <Shield24Regular />, title: t("Bảo vệ: ẩn ổ đĩa…"), run: A.protectDrives },
    protectOff: { icon: <Shield24Regular />, title: t("Bảo vệ: gỡ hết hạn chế"), run: A.protectOff },
    keywords: { icon: <Search24Regular />, title: t("Theo dõi từ khoá gõ phím…"), desc: t("Báo khi học sinh gõ từ khoá cấm"), run: A.keywords },
    // power
    wake: { icon: <PlugConnected24Regular />, title: t("Bật máy (Wake-on-LAN)"), desc: t("Đánh thức máy đang tắt qua mạng"), run: A.wake },
    shutdown: { icon: <Power24Regular />, title: t("Tắt máy"), desc: t("Tắt máy đang chọn hoặc cả lớp"), run: askPower("Shutdown", t("Tắt máy")) },
    reboot: { icon: <ArrowSync24Regular />, title: t("Khởi động lại"), desc: t("Khởi động lại máy đang chọn hoặc cả lớp"), run: askPower("Reboot", t("Khởi động lại")) },
    logoff: { icon: <SignOut24Regular />, title: t("Đăng xuất"), desc: t("Đăng xuất tài khoản học sinh"), run: askPower("Logoff", t("Đăng xuất")) },
    // talk
    msg: { icon: <Comment24Regular />, title: t("Tin nhắn"), desc: t("Hiện một thông báo ngắn trên màn hình học sinh"), run: app.ask.msg },
    chat: { icon: <Chat24Regular />, title: t("Chat"), desc: t("Trò chuyện hai chiều với học sinh"), run: app.ask.chat },
    poll: { icon: <Poll24Regular />, title: t("Khảo sát"), desc: t("Câu hỏi trắc nghiệm nhanh, xem kết quả ngay"), run: app.ask.poll },
    ask: { icon: <QuestionCircle24Regular />, title: t("Hỏi miệng…"), desc: t("Học sinh giơ tay hoặc trả lời"), run: A.ask },
    link: { icon: <Link24Regular />, title: t("Mở link"), desc: t("Mở một trang web trên máy học sinh"), run: app.ask.link },
    helpFilter: { icon: <HandRight24Regular />, title: `${t("Xin trợ giúp")} (${app.help.count})`, run: app.help.toggle, on: app.help.on },
    // assessment / planning
    assess: { icon: <ClipboardTaskListLtr24Regular />, title: t("Đánh giá…"), desc: t("Bài kiểm tra, bài đang làm và điểm từng học sinh"), run: app.openAssess },
    settings: { icon: <Settings24Regular />, title: t("Cài đặt…"), run: app.settings },
    shortcuts: { title: t("Phím tắt"), run: app.shortcuts },
    timetable: { icon: <Calendar24Regular />, title: t("Thời khoá biểu…"), desc: t("Tên bài tự điền khi tới giờ mỗi tiết"), run: app.timetable },
    about: { title: t("Giới thiệu ERG Classroom"), run: app.about },
    design: { icon: <DocumentEdit24Regular />, title: t("Soạn đề kiểm tra…"), desc: t("Tạo đề trắc nghiệm, có trộn câu"), run: A.design },
    giveTest: { icon: <Send24Regular />, title: t("Giao bài kiểm tra (file JSON)…"), run: A.giveTest },
    finishTest: { icon: <Checkmark24Regular />, title: t("Kết thúc và chấm điểm…"), desc: t("Thu bài, chấm điểm và lưu kết quả CSV"), run: A.finishTest },
    reward: { icon: <Star24Regular />, title: t("Thưởng 1 sao"), run: A.reward },
    plan: { icon: <Calendar24Regular />, title: t("Kế hoạch bài giảng…"), desc: t("Tự chạy các lệnh theo mốc thời gian"), run: A.plan },
    phoneOn: { icon: <PhoneDesktop24Regular />, title: t("Trợ lý trên điện thoại…"), desc: t("Điều khiển lớp từ điện thoại cùng mạng Wi-Fi"), run: A.phoneOn },
    phoneOff: { icon: <Dismiss24Regular />, title: t("Tắt trợ lý điện thoại"), run: A.phoneOff },
    // selection / view
    selectAll: { icon: <SelectAllOn24Regular />, title: t("Chọn hết"), run: selectAll },
    selectNone: { icon: <SelectAllOff24Regular />, title: t("Bỏ chọn"), run: clear, off: need0 },
    zoom: { icon: <ZoomIn24Regular />, title: t("Phóng to"), run: () => ui.request("remote"), off: need1 },
    thumbs: { title: t("Hình thu nhỏ"), run: () => ui.set({ view: "thumbs" }), on: ui.view === "thumbs" },
    icons: { title: t("Biểu tượng lớn"), run: () => ui.set({ view: "icons" }), on: ui.view === "icons" },
    details: { title: t("Chi tiết"), run: () => ui.set({ view: "details" }), on: ui.view === "details" },
    lesson: { title: t("Chi tiết tiết học"), run: () => ui.set({ lessonOpen: !ui.lessonOpen }), on: ui.lessonOpen },
    autoArrange: { title: t("Tự sắp xếp"), run: () => ui.set({ autoArrange: !ui.autoArrange }), on: ui.autoArrange },
    detail: { icon: <PanelRight24Regular />, title: t("Khung chi tiết"), run: () => ui.set({ detail: !ui.detail }), on: ui.detail },
    autoFit: { icon: <Desktop24Regular />, title: t("Tự khớp"), run: () => ui.set({ autoFit: !ui.autoFit }), on: ui.autoFit },
    showUser: { icon: <PersonQuestionMark24Regular />, title: t("Tên người dùng"), run: () => ui.set({ showUser: !ui.showUser }), on: ui.showUser },
    showGroup: { icon: <PeopleTeam24Regular />, title: t("Nhóm"), run: () => ui.set({ showGroup: !ui.showGroup }), on: ui.showGroup },
    lang: { icon: <Translate24Regular />, title: lang === "vi" ? "English" : "Tiếng Việt", run: toggleLang },
  } satisfies Record<string, Cmd>;
  for (const [, k, name] of KEYS) (c as Record<string, Cmd>)[name].kbd = k;

  const webItems: Item[] = [c.webPane, c.webRules, "-", c.webOpen, c.webAll, c.webTemp, "-", c.webAllow, c.webBlock, "-", c.webGuard];
  const appItems: Item[] = [c.appOpen, c.appBlock, c.appAllow, "-", c.kill];
  const power: Item[] = [c.wake, c.shutdown, c.reboot, c.logoff];
  // submenus of the menu bar and of the student right-click menu
  const sub = {
    web: { title: t("Truy cập web"), sub: webItems },
    apps: { title: t("Ứng dụng"), sub: appItems },
    print: { title: t("In ấn"), sub: [c.printAllow, c.printPause, c.printBlock] },
    usb: { title: t("USB và thiết bị"), sub: [c.usbAllow, c.usbRead, c.usbNoExec, c.usbBlock] },
    sound: { title: t("Âm thanh"), sub: [c.mute, c.unmute, c.beep] },
    protect: { title: t("Bảo vệ"), sub: [c.protectOn, c.protectDrives, c.protectOff] },
    power: { title: t("Nguồn"), sub: power },
  } satisfies Record<string, Cmd>;

  // the eleven menus of the v2 title bar; only commands that exist are listed
  const menus: Menu[] = [
    { label: t("Phòng học"), items: [c.wizard, c.refresh, c.endClass, "-", c.wake, "-", c.phoneOn, c.phoneOff, "-", c.settings] },
    { label: t("Lớp học"), items: [c.selectAll, c.selectNone, "-", c.lock, c.unlock, c.blank, "-", c.show, c.msg, c.sendWork, c.sendFile, c.collect, "-", c.shutdown, c.reboot, c.logoff] },
    { label: t("Học sinh"), items: [c.remote, c.chat, c.msg, "-", c.files, c.info, c.shell, "-", c.reward, c.random] },
    { label: t("Nhóm"), items: [c.assignGroup, c.autoGroup, c.clearGroups, "-", c.setLeader, c.selectLeaders, c.grantLeaders, c.revokeLeaders, "-", c.showGroup, c.teams] },
    { label: t("Xem"), items: [c.thumbs, c.icons, c.details, "-", c.lesson, c.detail, "-", c.zoom, c.helpFilter] },
    { label: t("Giám sát"), items: [sub.web, sub.apps, c.keywords, sub.print, sub.usb, sub.sound, sub.protect, "-", c.studentBar, c.scan, c.record, c.keys] },
    { label: t("Sơ đồ"), items: [c.autoFit, c.autoArrange, c.showUser] },
    { label: t("Kế hoạch"), items: [c.plan, c.timetable, c.wizard, "-", c.assess, c.design, c.giveTest, c.finishTest] },
    { label: t("Nhật ký"), items: [c.journal, c.journalHtml, c.attendance] },
    { label: t("Cửa sổ"), items: [c.detail, "-", c.whiteboard, c.board, c.annotate] },
    { label: t("Trợ giúp"), items: [c.shortcuts, c.lang, c.helpFilter, "-", c.settings, c.about] },
  ];
  return { c, app, node, menus, webItems, appItems, power, sub, A, t };
}

export type Commands = ReturnType<typeof useCommands>;

export function Ribbon({ cmds }: { cmds: Commands }) {
  const { c, app, t, webItems, appItems, power } = cmds;
  const s = useStyles();
  const { showing } = useClass();
  const btn = (x: Cmd, label: string, icon: React.ReactNode) => <RBtn icon={icon} disabled={!!x.off} title={x.off} active={x.on} onClick={x.run}>{label}</RBtn>;
  const chips: Chip[] = [
    { icon: c.record.icon, label: t("Tạo bản ghi"), on: c.record.on, run: c.record.run },
    { icon: c.keys.icon, label: t("Hiện phím gõ"), on: c.keys.on, run: c.keys.run },
  ];
  return (
    <div className={s.ribbon} role="toolbar" aria-label={t("Lệnh lớp học")}>
      {/* groups and order follow the v2 design; every command sits in exactly one place here (the menu bar repeats them) */}
      <RGroup caption={t("Lớp học")}>
        <RDrop icon={ico("class")} label={t("Quản lý lớp")} width={320} items={[c.refresh, c.endClass, c.wizard]} />
        <RDrop icon={ico("register")} label="Register" items={[c.roll, c.attendance, c.inventory]} />
        {btn(c.random, "Random", ico("random"))}
        <RDrop icon={ico("journal")} label={t("Nhật ký")} items={[c.journal, c.journalHtml]} />
      </RGroup>
      <RGroup caption={t("Trình chiếu")}>
        {/* one click starts or stops Show; the strip beside it opens the other ways to present */}
        <RSplit icon={showing ? <Stop24Filled style={{ color: RED }} /> : ico("show")} label={showing ? t("Dừng chiếu") : t("Chiếu")} onClick={c.show.run} off={c.show.off} active={showing} optionsLabel={t("Tuỳ chọn")} width={360}
          items={[c.show, c.showApp, c.exhibit, c.exhibitOff, "-", c.annotate, c.board, c.whiteboard, "-", c.playVideo, c.videoPlay, c.videoPause, c.videoSeek, c.videoClose, "-", c.scan]}
          chips={chips} chipsLabel={t("Tuỳ chọn")} />
        {btn(c.remote, "View Client", ico("view"))}
      </RGroup>
      <RGroup caption={t("Tệp")}>
        <RDrop icon={ico("files")} label={t("Truyền tệp")} width={340} items={[c.distribute, c.files, c.deploy, "-", c.clipClear, c.clipGet]} />
        <RDrop icon={ico("work")} label="Send / Collect" items={[c.sendWork, c.sendFile, c.collect]} />
      </RGroup>
      <RGroup caption={t("Màn hình")}>
        {btn(c.lock, c.lock.title, ico("lock"))}{btn(c.unlock, c.unlock.title, ico("unlock"))}{btn(c.blank, "Blank All", ico("blank"))}
      </RGroup>
      <RGroup caption={t("Truy cập")}>
        <RDrop icon={ico("web")} label="Web" items={webItems} />
        <RDrop icon={ico("manage")} label="Manage" width={320} items={[...power, "-", c.info, c.shell]} />
        <RDrop icon={ico("apps")} label={t("Ứng dụng")} items={appItems} />
      </RGroup>
      <RGroup caption={t("Giao tiếp")}>
        <RDrop icon={ico("talk")} label={t("Giao tiếp")} items={[c.msg, c.chat, c.link, "-", c.poll, c.ask]} />
        <RBtn icon={ico("help")} badge={app.help.count} active={c.helpFilter.on} onClick={c.helpFilter.run}>Help Requests</RBtn>
      </RGroup>
      <RGroup caption="Student">
        {btn(c.studentBar, "Student Toolbar", ico("bar"))}
        <Pop label="Quick Launch" width={432} trigger={(open) => <RBtn icon={ico("launch")} menu open={open} aria-haspopup="dialog" aria-expanded={open}>Quick Launch</RBtn>}>
          {(close) => <QuickLaunch close={close} say={app.say} fail={app.fail} />}
        </Pop>
      </RGroup>
      <RGroup caption={t("Đánh giá")}>
        <RDrop icon={ico("test")} label="Assessments" items={[c.design, c.giveTest, c.finishTest, "-", c.teams, c.reward]} />
        {btn(c.plan, "Lesson Plans", ico("plans"))}
      </RGroup>
    </div>
  );
}
