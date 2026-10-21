//! Desktop presentation, preferences and supervised maintenance jobs.

pub mod catalog;
pub mod i18n;
pub mod job;
pub mod progress;
pub mod repository;
pub mod settings;
pub mod worker;

#[cfg(all(feature = "gui", any(target_os = "windows", target_os = "macos")))]
pub mod app;
#[cfg(all(feature = "gui", any(target_os = "windows", target_os = "macos")))]
mod pages;
#[cfg(all(feature = "gui", any(target_os = "windows", target_os = "macos")))]
mod theme;
