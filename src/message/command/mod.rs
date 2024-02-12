mod admin;
mod hidden;
mod start_parameter;
mod user;
mod util;

pub use start_parameter::StartParameter;

pub use admin::{send_database_export_to_chat, AdminCommand};
pub use hidden::HiddenCommand;
use teloxide::{types::BotCommand, utils::command::BotCommands};
pub use user::RegularCommand;
pub use util::*;

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
