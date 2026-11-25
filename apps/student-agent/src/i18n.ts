import { invoke } from "@tauri-apps/api/core";

/** Vietnamese is the source language; `lang=en` in config.txt switches the student windows to English. */
const EN: Record<string, string> = { "Màn hình đang bị khoá": "Screen is locked", "Tin nhắn từ giáo viên": "Message from your teacher", "Tự đóng sau": "Closes in", "giây": "s", "Đã hiểu": "Got it", "Giáo viên": "Teacher", "Trả lời…": "Reply…", "Gửi": "Send", "Tiết học": "Lesson", "Giáo viên đang theo dõi một số từ khoá khi bạn gõ phím": "Your teacher is watching for some words while you type", "👁 Theo dõi từ khoá": "👁 Keyword watch", "Mục tiêu": "Goals", "Giáo viên giao quyền trưởng nhóm cho bạn": "Your teacher made you a group leader", "★ Trưởng nhóm": "★ Group leader", "Nhắn cho nhóm": "Message your group", "Nhắn nhóm": "Message group", "Khoá nhóm": "Lock group", "Mở nhóm": "Unlock group", "Trợ giúp": "Help", "Xin trợ giúp": "Ask for help", "Còn lại": "Time left", "Phần thưởng": "Rewards", "Ghim thanh công cụ": "Keep the toolbar open", "phòng": "room","Điểm danh": "Roll call", "Họ và tên": "Full name", "Lớp": "Class", "Có mặt": "Present", "Câu trả lời (không bắt buộc)": "Answer (optional)", "Giơ tay": "Raise hand", "Gửi trả lời": "Send answer", "Đúng": "True", "Sai": "False", "Chỗ trống": "Blank", "Đang tải đề…": "Loading test…", "Câu": "Question", "điểm": "points", "Nộp bài": "Submit", "Đang chờ hình từ giáo viên…": "Waiting for the teacher's picture…", "khung": "frames", "Bảng chung": "Shared board", "Chỉ xem": "View only", "Hoàn tác": "Undo", "Nội dung chữ": "Text to write" };
export let lang: "vi" | "en" = "vi";

export const tr = (l: "vi" | "en", vi: string) => (l === "en" ? EN[vi] ?? vi : vi);
export const t = (vi: string) => tr(lang, vi);
export async function initLang() {
  try { lang = (await invoke<string>("ui_lang")) === "en" ? "en" : "vi"; } catch { /* keep Vietnamese */ }
}
