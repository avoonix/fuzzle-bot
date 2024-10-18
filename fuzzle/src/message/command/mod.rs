mod admin;
mod hidden;
mod start_parameter;
mod user;
mod util;
mod privacy;

pub use start_parameter::StartParameter;

pub use admin::{send_database_export_to_chat, AdminCommand, send_merge_queue};
pub use hidden::{HiddenCommand, set_tag_id};
use teloxide::{types::BotCommand, utils::command::BotCommands};
pub use user::{RegularCommand, send_sticker_with_tag_input};
pub use util::*;
pub use privacy::*;

pub fn list_visible_admin_commands() -> Vec<BotCommand> {
    AdminCommand::list_visible()
}

pub fn list_visible_user_commands() -> Vec<BotCommand> {
    RegularCommand::list_visible()
}

pub fn admin_command_description() -> String {
    AdminCommand::descriptions().to_string()
}

pub fn user_command_description() -> String {
    RegularCommand::descriptions().to_string()
}
