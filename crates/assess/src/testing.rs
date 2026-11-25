//! Test paper model, validation, shuffling, grading and CSV report.
use serde::{Deserialize, Serialize};

pub const MAX_QUESTIONS: usize = 500;
pub const MAX_PAPER_JSON: usize = 2 << 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Kind {
    /// One or several correct options (indexes into `options`).
    Choice { options: Vec<String>, correct: Vec<usize> },
    TrueFalse { answer: bool },
    /// One list of accepted spellings per blank.
    Fill { accepted: Vec<Vec<String>> },
    /// Left item i belongs with right item `pairs[i]`.
    Match { left: Vec<String>, right: Vec<String>, pairs: Vec<usize> },
    /// Items are listed in their correct order; the student sees them shuffled.
    Order { items: Vec<String> },
    /// Click inside the rectangle (all values 0..=1 of the image size).
    Region { image: String, x: f32, y: f32, w: f32, h: f32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Question {
    pub text: String,
    pub points: f32,
    pub kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Paper {
    pub title: String,
    pub shuffle: bool,
    pub questions: Vec<Question>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Answer {
    Choice(Vec<usize>),
    Bool(bool),
    Fill(Vec<String>),
    /// For each left item, the chosen right index.
    Match(Vec<usize>),
    /// Item indexes (original order) in the order the student put them.
    Order(Vec<usize>),
    Point(f32, f32),
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Score {
    pub points: f32,
    pub max: f32,
    pub per_question: Vec<f32>,
}

impl Score {
    pub fn percent(&self) -> f32 {
        if self.max > 0.0 { 100.0 * self.points / self.max } else { 0.0 }
    }
}

impl Paper {
    pub fn from_json(s: &str) -> Result<Paper, String> {
        if s.len() > MAX_PAPER_JSON {
            return Err("paper too large".into());
        }
        let p: Paper = serde_json::from_str(s).map_err(|e| e.to_string())?;
        p.validate()?;
        Ok(p)
    }

    /// Rejects papers whose answer keys point outside their own lists, so grading never panics.
    pub fn validate(&self) -> Result<(), String> {
        if self.questions.is_empty() || self.questions.len() > MAX_QUESTIONS {
            return Err("1..=500 questions required".into());
        }
        for (i, q) in self.questions.iter().enumerate() {
            let bad = |m: &str| Err(format!("question {}: {m}", i + 1));
            if !(q.points.is_finite() && (0.0..=1000.0).contains(&q.points)) {
                return bad("points out of range");
            }
            match &q.kind {
                Kind::Choice { options, correct } => {
                    if options.len() < 2 || options.len() > 12 {
                        return bad("2..=12 options");
                    }
                    if correct.is_empty() || correct.iter().any(|&c| c >= options.len()) {
                        return bad("correct index out of range");
                    }
                }
                Kind::TrueFalse { .. } => {}
                Kind::Fill { accepted } => {
                    if accepted.is_empty() || accepted.iter().any(|a| a.is_empty()) {
                        return bad("each blank needs an accepted answer");
                    }
                }
                Kind::Match { left, right, pairs } => {
                    if left.len() < 2 || pairs.len() != left.len() || pairs.iter().any(|&p| p >= right.len()) {
                        return bad("bad matching key");
                    }
                }
                Kind::Order { items } => {
                    if items.len() < 2 {
                        return bad("need 2+ items");
                    }
                }
                Kind::Region { x, y, w, h, .. } => {
                    let ok = |v: &f32| v.is_finite() && (0.0..=1.0).contains(v);
                    if !(ok(x) && ok(y) && ok(w) && ok(h)) || *w <= 0.0 || *h <= 0.0 {
                        return bad("region outside image");
                    }
                }
            }
        }
        Ok(())
    }

    pub fn max_points(&self) -> f32 {
        self.questions.iter().map(|q| q.points).sum()
    }

    /// The paper that goes to a student: answer keys are replaced by neutral placeholders (so the
    /// key cannot be read from the window's data) and ordered lists are shuffled by `seed`,
    /// because their array order would reveal the answer. Grade the result with [`unmap`].
    pub fn student_view(&self, seed: u64) -> Paper {
        let mut p = self.clone();
        for (qi, q) in p.questions.iter_mut().enumerate() {
            let s = seed.wrapping_add(qi as u64);
            q.kind = match &q.kind {
                Kind::Choice { options, .. } => Kind::Choice { options: options.clone(), correct: vec![0] },
                Kind::TrueFalse { .. } => Kind::TrueFalse { answer: false },
                Kind::Fill { accepted } => Kind::Fill { accepted: vec![vec![String::new()]; accepted.len()] },
                Kind::Match { left, right, .. } => {
                    let perm = shuffled(right.len(), s);
                    Kind::Match { left: left.clone(), right: perm.iter().map(|&i| right[i].clone()).collect(), pairs: vec![0; left.len()] }
                }
                Kind::Order { items } => Kind::Order { items: shuffled(items.len(), s).iter().map(|&i| items[i].clone()).collect() },
                Kind::Region { image, .. } => Kind::Region { image: image.clone(), x: 0.0, y: 0.0, w: 1.0, h: 1.0 },
            };
        }
        p
    }

    /// Turn answers given against `student_view(seed)` into answers against this paper.
    /// Indexes that are out of range become `Skipped` instead of panicking.
    pub fn unmap(&self, seed: u64, answers: &[Answer]) -> Vec<Answer> {
        self.questions
            .iter()
            .enumerate()
            .map(|(qi, q)| {
                let Some(a) = answers.get(qi) else { return Answer::Skipped };
                let perm = |n: usize| shuffled(n, seed.wrapping_add(qi as u64));
                match (&q.kind, a) {
                    (Kind::Match { right, .. }, Answer::Match(got)) => {
                        let p = perm(right.len());
                        got.iter().map(|&g| p.get(g).copied()).collect::<Option<Vec<_>>>().map_or(Answer::Skipped, Answer::Match)
                    }
                    (Kind::Order { items }, Answer::Order(got)) => {
                        let p = perm(items.len());
                        got.iter().map(|&g| p.get(g).copied()).collect::<Option<Vec<_>>>().map_or(Answer::Skipped, Answer::Order)
                    }
                    _ => a.clone(),
                }
            })
            .collect()
    }
}

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Answers shorter or longer than the paper are tolerated: missing ones score 0, extras are ignored.
pub fn grade(paper: &Paper, answers: &[Answer]) -> Score {
    let per_question: Vec<f32> = paper
        .questions
        .iter()
        .enumerate()
        .map(|(i, q)| {
            let Some(a) = answers.get(i) else { return 0.0 };
            let right = match (&q.kind, a) {
                (Kind::Choice { correct, .. }, Answer::Choice(got)) => {
                    let (mut c, mut g) = (correct.clone(), got.clone());
                    c.sort_unstable();
                    c.dedup();
                    g.sort_unstable();
                    g.dedup();
                    c == g
                }
                (Kind::TrueFalse { answer }, Answer::Bool(b)) => answer == b,
                (Kind::Fill { accepted }, Answer::Fill(got)) => {
                    got.len() == accepted.len() && accepted.iter().zip(got).all(|(ok, g)| ok.iter().any(|o| norm(o) == norm(g)))
                }
                (Kind::Match { pairs, .. }, Answer::Match(got)) => got == pairs,
                (Kind::Order { items }, Answer::Order(got)) => got.len() == items.len() && got.iter().enumerate().all(|(pos, &idx)| pos == idx),
                (Kind::Region { x, y, w, h, .. }, Answer::Point(px, py)) => *px >= *x && *px <= x + w && *py >= *y && *py <= y + h,
                _ => false,
            };
            if right { q.points } else { 0.0 }
        })
        .collect();
    Score { points: per_question.iter().sum(), max: paper.max_points(), per_question }
}

/// Deterministic Fisher-Yates so a student keeps the same order after reconnecting.
pub fn shuffled(n: usize, seed: u64) -> Vec<usize> {
    let mut v: Vec<usize> = (0..n).collect();
    let mut s = seed ^ 0x9E37_79B9_7F4A_7C15;
    for i in (1..n).rev() {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        v.swap(i, (s % (i as u64 + 1)) as usize);
    }
    v
}

fn csv_cell(s: &str) -> String {
    // Quote, and defuse spreadsheet formulas typed by students (=, +, -, @).
    let s = if s.starts_with(['=', '+', '-', '@']) { format!("'{s}") } else { s.to_string() };
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// One row per student: name, points, max, percent.
pub fn report_csv(rows: &[(String, Score)]) -> String {
    let mut o = String::from("Student,Points,Max,Percent\n");
    for (n, s) in rows {
        o.push_str(&format!("{},{:.2},{:.2},{:.1}\n", csv_cell(n), s.points, s.max, s.percent()));
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paper() -> Paper {
        let q = |text: &str, points: f32, kind: Kind| Question { text: text.into(), points, kind };
        let s = |x: &str| x.to_string();
        Paper {
            title: "t".into(),
            shuffle: true,
            questions: vec![
                q("multi", 2.0, Kind::Choice { options: vec![s("a"), s("b"), s("c")], correct: vec![0, 2] }),
                q("tf", 1.0, Kind::TrueFalse { answer: true }),
                q("fill", 2.0, Kind::Fill { accepted: vec![vec![s("Hà Nội"), s("Ha Noi")], vec![s("10")]] }),
                q("match", 3.0, Kind::Match { left: vec![s("1"), s("2")], right: vec![s("x"), s("y")], pairs: vec![1, 0] }),
                q("order", 2.0, Kind::Order { items: vec![s("a"), s("b"), s("c")] }),
                q("region", 1.0, Kind::Region { image: s("m.png"), x: 0.2, y: 0.2, w: 0.2, h: 0.2 }),
            ],
        }
    }

    #[test]
    fn every_question_kind_is_graded() {
        let p = paper();
        let all = vec![
            Answer::Choice(vec![2, 0]),
            Answer::Bool(true),
            Answer::Fill(vec!["  ha   NOI ".into(), "10".into()]),
            Answer::Match(vec![1, 0]),
            Answer::Order(vec![0, 1, 2]),
            Answer::Point(0.3, 0.3),
        ];
        let s = grade(&p, &all);
        assert_eq!((s.points, s.max), (11.0, 11.0));
        let wrong = vec![
            Answer::Choice(vec![0]),
            Answer::Bool(false),
            Answer::Fill(vec!["x".into(), "10".into()]),
            Answer::Match(vec![0, 1]),
            Answer::Order(vec![1, 0, 2]),
            Answer::Point(0.9, 0.9),
        ];
        assert_eq!(grade(&p, &wrong).points, 0.0);
    }

    #[test]
    fn missing_mismatched_and_extra_answers_never_panic_or_score() {
        let p = paper();
        assert_eq!(grade(&p, &[]).points, 0.0);
        assert_eq!(grade(&p, &[Answer::Bool(true)]).points, 0.0); // wrong type for question 1
        assert_eq!(grade(&p, &vec![Answer::Skipped; 20]).points, 0.0);
        assert_eq!(grade(&p, &[Answer::Choice(vec![99])]).points, 0.0);
        assert_eq!(grade(&p, &[Answer::Choice(vec![0, 0, 2, 2])]).points, 2.0); // duplicates collapse
    }

    #[test]
    fn validation_rejects_keys_that_point_outside_their_lists() {
        let mut p = paper();
        assert!(p.validate().is_ok());
        p.questions[0].kind = Kind::Choice { options: vec!["a".into(), "b".into()], correct: vec![5] };
        assert!(p.validate().unwrap_err().contains("question 1"));
        p = paper();
        p.questions[3].kind = Kind::Match { left: vec!["1".into(), "2".into()], right: vec!["x".into()], pairs: vec![0, 1] };
        assert!(p.validate().is_err());
        p = paper();
        p.questions[5].kind = Kind::Region { image: "".into(), x: 0.9, y: 0.0, w: -1.0, h: 0.1 };
        assert!(p.validate().is_err());
        p = paper();
        p.questions[1].points = f32::NAN;
        assert!(p.validate().is_err());
        assert!(Paper::from_json("{").is_err());
        assert!(Paper::from_json(&serde_json::to_string(&paper()).unwrap()).is_ok());
    }

    #[test]
    fn shuffle_is_a_stable_permutation() {
        let a = shuffled(30, 7);
        assert_eq!(a, shuffled(30, 7));
        assert_ne!(a, shuffled(30, 8));
        let mut s = a.clone();
        s.sort_unstable();
        assert_eq!(s, (0..30).collect::<Vec<_>>());
        assert!(shuffled(0, 1).is_empty());
    }

    #[test]
    fn csv_quotes_and_defuses_formulas() {
        let s = Score { points: 5.0, max: 10.0, per_question: vec![] };
        let c = report_csv(&[("=SUM(A1)".into(), s.clone()), ("An, B".into(), s)]);
        assert!(c.contains("\"'=SUM(A1)\",5.00,10.00,50.0"));
        assert!(c.contains("\"An, B\",5.00"));
    }
}

#[cfg(test)]
mod student_view_tests {
    use super::*;

    fn paper() -> Paper {
        let s = |x: &str| x.to_string();
        Paper {
            title: "t".into(),
            shuffle: false,
            questions: vec![
                Question { text: "c".into(), points: 1.0, kind: Kind::Choice { options: vec![s("a"), s("b")], correct: vec![1] } },
                Question { text: "m".into(), points: 2.0, kind: Kind::Match { left: vec![s("1"), s("2"), s("3")], right: vec![s("x"), s("y"), s("z")], pairs: vec![0, 1, 2] } },
                Question { text: "o".into(), points: 2.0, kind: Kind::Order { items: vec![s("a"), s("b"), s("c"), s("d")] } },
                Question { text: "r".into(), points: 1.0, kind: Kind::Region { image: s("i.png"), x: 0.4, y: 0.4, w: 0.1, h: 0.1 } },
                Question { text: "f".into(), points: 1.0, kind: Kind::Fill { accepted: vec![vec![s("secret")]] } },
            ],
        }
    }

    #[test]
    fn student_copy_has_no_answer_keys_but_is_still_a_valid_paper() {
        let p = paper();
        let v = p.student_view(42);
        assert!(v.validate().is_ok());
        let json = serde_json::to_string(&v).unwrap();
        assert!(!json.contains("secret"), "fill answers leaked");
        assert_ne!(v.questions[2].kind, p.questions[2].kind, "order list must be shuffled");
        assert_eq!(Paper::from_json(&json).unwrap(), v);
    }

    #[test]
    fn answers_given_on_the_shuffled_copy_grade_correctly_on_the_original() {
        let p = paper();
        let v = p.student_view(42);
        // a clever student re-sorts the shuffled list alphabetically: that is the right order
        let Kind::Order { items: shown } = &v.questions[2].kind else { panic!() };
        let mut idx: Vec<usize> = (0..shown.len()).collect();
        idx.sort_by_key(|&i| shown[i].clone());
        // match: pair each left item with the displayed right item of the same letter position
        let Kind::Match { right: shown_right, .. } = &v.questions[1].kind else { panic!() };
        let want = ["x", "y", "z"];
        let m: Vec<usize> = want.iter().map(|w| shown_right.iter().position(|r| r == w).unwrap()).collect();
        let answers = vec![Answer::Choice(vec![1]), Answer::Match(m), Answer::Order(idx), Answer::Point(0.45, 0.45), Answer::Fill(vec!["secret".into()])];
        let s = grade(&p, &p.unmap(42, &answers));
        assert_eq!(s.points, s.max, "{s:?}");
        // the same answers graded WITHOUT unmapping are wrong for the shuffled questions
        assert!(grade(&p, &answers).points < s.max);
    }

    #[test]
    fn hostile_indexes_are_skipped_not_panicked() {
        let p = paper();
        let a = vec![Answer::Skipped, Answer::Match(vec![99, 0, 0]), Answer::Order(vec![7, 8, 9, 10]), Answer::Skipped, Answer::Skipped];
        assert_eq!(p.unmap(1, &a), vec![Answer::Skipped; 5]);
        assert_eq!(p.unmap(1, &[]), vec![Answer::Skipped; 5]);
    }
}

#[cfg(test)]
mod example_tests {
    use super::*;

    /// The sample paper shipped in examples/ must stay valid and gradable.
    #[test]
    fn shipped_example_paper_is_valid_and_a_perfect_run_scores_full() {
        let p = Paper::from_json(include_str!("../../../examples/bai-kiem-tra.json")).expect("example paper");
        assert_eq!(p.questions.len(), 5);
        let a = vec![
            Answer::Choice(vec![0, 2]),
            Answer::Bool(true),
            Answer::Fill(vec!["ha noi".into(), "10".into()]),
            Answer::Match(vec![2, 0, 1]),
            Answer::Order(vec![0, 1, 2, 3]),
        ];
        let s = grade(&p, &a);
        assert_eq!((s.points, s.max), (10.0, 10.0));
    }
}
