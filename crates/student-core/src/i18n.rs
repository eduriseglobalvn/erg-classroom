//! Tiny vi/en lookup for strings Rust shows to the student (window titles, notices).
//! Language comes from `lang=en` in config.txt, same as the student UI; anything else is Vietnamese.

const TABLE: &[(&str, &str, &str)] = &[
    ("poll", "Khảo sát", "Poll"),
    ("chat", "Trò chuyện với giáo viên", "Chat with the teacher"),
    ("register", "Điểm danh", "Roll call"),
    ("question", "Câu hỏi của giáo viên", "Question from the teacher"),
    ("board", "Bảng", "Board"),
    ("teacher", "Giáo viên", "Teacher"),
    (
        "keywords",
        "Giáo viên đang theo dõi một số từ khoá khi bạn gõ phím. Chỉ những từ này được báo, không ghi lại nội dung khác.",
        "Your teacher is watching for a few keywords as you type. Only those words are reported; nothing else is recorded.",
    ),
];

/// `lang` is "en" or anything else (= Vietnamese). An unknown key returns itself so a typo is visible, not fatal.
pub fn tr(lang: &str, key: &'static str) -> &'static str {
    TABLE.iter().find(|r| r.0 == key).map_or(key, |r| if lang.eq_ignore_ascii_case("en") { r.2 } else { r.1 })
}

/// "en" when config.txt says `lang=en`, otherwise "vi".
pub fn ui_lang() -> &'static str {
    if crate::load_settings().get("lang").is_some_and(|l| l.eq_ignore_ascii_case("en")) { "en" } else { "vi" }
}

/// Translate using the configured language.
pub fn t(key: &'static str) -> &'static str {
    tr(ui_lang(), key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vi_and_en() {
        assert_eq!(tr("vi", "poll"), "Khảo sát");
        assert_eq!(tr("en", "poll"), "Poll");
        assert_eq!(tr("EN", "teacher"), "Teacher");
        assert_eq!(tr("fr", "teacher"), "Giáo viên"); // unknown language -> Vietnamese
        assert_eq!(tr("en", "nope"), "nope");
    }

    #[test]
    fn every_key_has_both_languages() {
        for (k, vi, en) in TABLE {
            assert!(!vi.is_empty() && !en.is_empty() && vi != en, "{k}");
        }
    }
}
