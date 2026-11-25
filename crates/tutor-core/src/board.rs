//! Shared whiteboard state kept by the tutor: validates elements, remembers them (and who drew them) for
//! late joiners and for undo, and decides what to relay. Elements are append-only apart from undo, so
//! concurrent drawing needs no merge logic.
use erg_proto::BoardMsg;
use uuid::Uuid;

pub const MAX_ELEMENTS: usize = 5000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Who {
    Tutor,
    Student(Uuid),
}

#[derive(Default)]
pub struct Board {
    open: Option<bool>, // Some(students may draw)
    elements: Vec<(Who, BoardMsg)>,
    scene_version: u64,
}

impl Board {
    /// Returns what to send to the class, or None when the message is refused (bad data, board closed,
    /// student drawing not allowed, undo of someone else's element, a student trying to control the board).
    pub fn apply(&mut self, m: BoardMsg, who: Who) -> Option<BoardMsg> {
        let tutor = who == Who::Tutor;
        match &m {
            BoardMsg::Open { draw } if tutor => {
                self.open = Some(*draw);
                Some(m)
            }
            BoardMsg::Close if tutor => {
                self.open = None;
                self.scene_version = 0;
                self.elements.clear();
                Some(m)
            }
            BoardMsg::Clear if tutor => {
                self.elements.clear();
                Some(m)
            }
            BoardMsg::Undo { id } => {
                let allowed = self.open.is_some_and(|draw| tutor || draw);
                let pos = self.elements.iter().position(|(_, e)| e.element_id() == Some(*id))?;
                if !allowed || (!tutor && self.elements[pos].0 != who) {
                    return None;
                }
                self.elements.remove(pos);
                Some(m)
            }
            BoardMsg::Scene { version, .. } => {
                // only the tutor sends scenes; stale/duplicate versions and oversize scenes are refused
                if !tutor || self.open.is_none() || !m.valid() || *version <= self.scene_version {
                    return None;
                }
                self.scene_version = *version;
                self.elements.retain(|(_, e)| !matches!(e, BoardMsg::Scene { .. }));
                self.elements.push((who, m.clone()));
                Some(m)
            }
            BoardMsg::Stroke { .. } | BoardMsg::Shape { .. } | BoardMsg::Text { .. } => {
                let allowed = self.open.is_some_and(|draw| tutor || draw);
                if !allowed || !m.valid() {
                    return None;
                }
                if self.elements.iter().any(|(_, e)| e.element_id() == m.element_id()) {
                    return None; // same id twice (an echo or a replay attempt)
                }
                if self.elements.len() >= MAX_ELEMENTS {
                    self.elements.remove(0); // oldest drops off; the board never grows without bound
                }
                self.elements.push((who, m.clone()));
                Some(m)
            }
            _ => None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// What a student who joins late needs to see the same board.
    pub fn replay(&self) -> Vec<BoardMsg> {
        match self.open {
            None => vec![],
            Some(draw) => std::iter::once(BoardMsg::Open { draw }).chain(self.elements.iter().map(|(_, e)| e.clone())).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use erg_proto::ShapeKind;

    fn stroke(id: u64, n: usize) -> BoardMsg {
        BoardMsg::Stroke { id, color: 0xff0000, width: 3, pts: vec![(1, 2); n] }
    }
    fn rect(id: u64) -> BoardMsg {
        BoardMsg::Shape { id, color: 0, width: 2, kind: ShapeKind::Rect, a: (0, 0), b: (100, 100) }
    }
    fn text(id: u64, t: &str, size: u8) -> BoardMsg {
        BoardMsg::Text { id, color: 0, size, at: (5, 5), text: t.into() }
    }
    fn open(b: &mut Board, draw: bool) {
        b.apply(BoardMsg::Open { draw }, Who::Tutor).unwrap();
    }

    #[test]
    fn elements_need_an_open_board_and_students_need_permission() {
        let s = Who::Student(Uuid::new_v4());
        let mut b = Board::default();
        assert!(b.apply(stroke(1, 2), Who::Tutor).is_none(), "closed");
        open(&mut b, false);
        assert!(b.apply(stroke(1, 2), s).is_none(), "students may not draw yet");
        assert!(b.apply(stroke(2, 2), Who::Tutor).is_some());
        open(&mut b, true);
        assert!(b.apply(stroke(3, 2), s).is_some());
        assert!(b.apply(rect(4), s).is_some());
        assert!(b.apply(text(5, "xin chào", 24), s).is_some());
        assert_eq!(b.replay().len(), 5, "open + four elements");
    }

    #[test]
    fn students_cannot_control_the_board_and_bad_elements_are_refused() {
        let s = Who::Student(Uuid::new_v4());
        let mut b = Board::default();
        open(&mut b, true);
        for m in [BoardMsg::Clear, BoardMsg::Close, BoardMsg::Open { draw: true }] {
            assert!(b.apply(m, s).is_none());
        }
        assert!(b.apply(stroke(1, 0), s).is_none(), "empty");
        assert!(b.apply(stroke(1, erg_proto::BOARD_MAX_POINTS + 1), s).is_none(), "too long");
        assert!(b.apply(BoardMsg::Stroke { id: 1, color: 0, width: 0, pts: vec![(0, 0)] }, s).is_none());
        assert!(b.apply(BoardMsg::Shape { id: 1, color: 0, width: 200, kind: ShapeKind::Line, a: (0, 0), b: (1, 1) }, s).is_none());
        assert!(b.apply(text(1, "  ", 24), s).is_none(), "blank text");
        assert!(b.apply(text(1, "x", 4), s).is_none(), "tiny font");
        assert!(b.apply(text(1, "x", 200), s).is_none(), "huge font");
        assert!(b.apply(text(1, &"x".repeat(201), 24), s).is_none(), "long text");
        assert!(b.apply(stroke(7, 1), s).is_some());
        assert!(b.apply(stroke(7, 1), s).is_none(), "an id can be used once");
        assert!(b.is_open());
    }

    #[test]
    fn undo_removes_own_elements_only_and_the_tutor_can_remove_any() {
        let (a, c) = (Who::Student(Uuid::new_v4()), Who::Student(Uuid::new_v4()));
        let mut b = Board::default();
        open(&mut b, true);
        b.apply(stroke(1, 1), a).unwrap();
        b.apply(rect(2), c).unwrap();
        b.apply(text(3, "t", 20), Who::Tutor).unwrap();
        assert!(b.apply(BoardMsg::Undo { id: 2 }, a).is_none(), "not a's element");
        assert!(b.apply(BoardMsg::Undo { id: 3 }, a).is_none(), "not a's element");
        assert!(b.apply(BoardMsg::Undo { id: 1 }, a).is_some());
        assert!(b.apply(BoardMsg::Undo { id: 1 }, a).is_none(), "already gone");
        assert!(b.apply(BoardMsg::Undo { id: 2 }, Who::Tutor).is_some(), "tutor removes anything");
        assert_eq!(b.replay().len(), 2, "open + the tutor's text");
        open(&mut b, false);
        assert!(b.apply(BoardMsg::Undo { id: 3 }, a).is_none(), "students cannot undo while drawing is off");
    }

    #[test]
    fn scenes_are_tutor_only_versioned_and_size_capped() {
        let s = Who::Student(Uuid::new_v4());
        let scene = |v: u64, n: usize| BoardMsg::Scene { version: v, json: "x".repeat(n) };
        let mut b = Board::default();
        assert!(b.apply(scene(1, 10), Who::Tutor).is_none(), "closed");
        open(&mut b, true);
        assert!(b.apply(scene(1, 10), s).is_none(), "students cannot send scenes");
        assert!(b.apply(scene(1, 0), Who::Tutor).is_none(), "empty");
        assert!(b.apply(scene(1, erg_proto::BOARD_MAX_SCENE + 1), Who::Tutor).is_none(), "too big");
        assert!(b.apply(scene(2, erg_proto::BOARD_MAX_SCENE), Who::Tutor).is_some());
        assert!(b.apply(scene(2, 10), Who::Tutor).is_none(), "same version");
        assert!(b.apply(scene(3, 10), Who::Tutor).is_some());
        assert_eq!(b.replay().len(), 2, "open + only the newest scene");
        b.apply(BoardMsg::Close, Who::Tutor).unwrap();
        open(&mut b, false);
        assert!(b.apply(scene(1, 10), Who::Tutor).is_some(), "version counter resets on close");
    }

    #[test]
    fn clear_and_close_forget_elements_and_the_log_is_bounded() {
        let mut b = Board::default();
        open(&mut b, false);
        for i in 0..(MAX_ELEMENTS as u64 + 10) {
            b.apply(stroke(i, 1), Who::Tutor).unwrap();
        }
        assert_eq!(b.replay().len(), 1 + MAX_ELEMENTS);
        b.apply(BoardMsg::Clear, Who::Tutor).unwrap();
        assert_eq!(b.replay().len(), 1);
        b.apply(BoardMsg::Close, Who::Tutor).unwrap();
        assert!(b.replay().is_empty() && !b.is_open());
    }
}
