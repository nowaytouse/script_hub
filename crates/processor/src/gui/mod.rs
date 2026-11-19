//! Native desktop presentation and transient interactive sessions.

pub mod app;
mod app_history;
mod file_dialog;
pub mod i18n;
mod pages;
mod relay;
pub(crate) use relay::operation_failure_message;
mod subscriptions;
pub(crate) mod templates;
mod theme;

pub(crate) use pages::node_operations::{
    new_operations, next_step_id, operation_group, operation_label,
};
pub(crate) use pages::support as support_resources;
