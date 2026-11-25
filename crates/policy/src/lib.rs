//! Restrictions the teacher can apply on a student machine: which programs may run and which
//! web sites browsers may open.
pub mod apps;
#[cfg(windows)]
pub mod print;
#[cfg(windows)]
pub mod protect;
#[cfg(windows)]
pub mod runner;
#[cfg(windows)]
pub mod usb;
#[cfg(windows)]
pub mod web;
