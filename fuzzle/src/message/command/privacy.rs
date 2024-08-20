use enum_primitive_derive::Primitive;

#[derive(PartialEq, Clone, Copy, Debug, Eq, Primitive, Default)]
pub enum PrivacyPolicy {
    #[default]
    Introduction = 0,
    License = 1,
    DataCollection = 2,
    DataUsage = 3,
}

impl PrivacyPolicy {
    pub fn text(&self) -> &'static str {
        match self {
            PrivacyPolicy::Introduction => r#"*Terms of Service & Privacy Policy*

FuzzleBot is a personal project\. As such, we try to protect your privacy to an extent reasonable for a personal project\.
"#,
            PrivacyPolicy::License => r#"*License*

FuzzleBot, a Telegram bot for organizing furry sticker sets
Copyright \(C\) 2024 Avoonix

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU Affero General Public License as published
by the Free Software Foundation, either version 3 of the License, or
\(at your option\) any later version\.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE\.  See the
GNU Affero General Public License for more details\.

You should have received a copy of the GNU Affero General Public License
along with this program\.  If not, see [gnu\.org/licenses](https://www.gnu.org/licenses/)\.
"#,
            PrivacyPolicy::DataCollection => r#"*Data Collection*

The bot collects:

Public information about sticker sets shared by users including, but not limited to: id, title, name, sticker files, and thumbnails\.

Public information about users interacting directly with the bot or indirectly by other users \(for examlpe, by forwarding someone else\'s messages to the bot\) including, but not limited to: id, and username\.

Other information shared with the bot directly, such as settings, blacklist, tags, favorites, and other information for enhancing the user experience\.

Other data, such as usage information to prevent misuse\.

You can also inspect the [source code](https://github.com/avoonix/fuzzle-bot/blob/213692c7ec070f372175bd7bd8352ec884606171/fuzzle/src/database/schema.rs#L92) for details on which data is stored \(make sure to switch to the latest version\)\.
"#,
            PrivacyPolicy::DataUsage => r#"*Data Usage*

The collected data is needed for the general operation of the bot \(such as finding stickers\) and user experience enhancement \(such as linking sticker packs to telegram users\)\.

Additionally, we may temporarily store parts of messages for debugging purposes in memory \(not persisted to disk\)\.

We do not share any information with advertisers\.
"#,
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            PrivacyPolicy::Introduction => "Introduction",
            PrivacyPolicy::License => "License",
            PrivacyPolicy::DataCollection => "Data Collection",
            PrivacyPolicy::DataUsage => "Data Usage",
        }
    }
}