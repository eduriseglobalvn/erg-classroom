//! Assessment logic with no I/O: test papers and auto-grading, survey tallies, oral Q&A
//! (random pick, first hand, team points) and reward stars.
pub mod qa;
pub mod testing;
pub use testing::{grade, Answer, Paper, Question, Score};
