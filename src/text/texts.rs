use std::iter::once;

use crate::{
    callback::TagOperation,
    database::{PopularTag, SavedStickerSet, Stats},
    message::{
        admin_command_description, escape_sticker_unique_id_for_command, user_command_description,
    },
    util::Emoji,
};
use itertools::Itertools;
use log::warn;
use teloxide::{types::Message, utils::markdown::escape};

use super::Markdown;

/*
 * All text here uses markdown v2 syntax
 *
*/

pub struct Text;

impl Text {
    #[must_use]
    pub fn get_help_text(admin: bool) -> Markdown {
        Markdown::new(if admin {
            format!(
                "{}\n\n{}",
                escape(&user_command_description()),
                escape(&admin_command_description())
            )
        } else {
            user_command_description()
        })
    }
    
    #[must_use]
    pub fn sticker_not_found() -> Markdown {
        Markdown::new(
            "Sticker not found",
        )
    }

    #[must_use]
    pub fn get_settings_text() -> Markdown {
        Markdown::new(
            "Settings", // TODO: settings info
        )
    }

    #[must_use]
    pub fn get_popular_tag_text(tags: Vec<PopularTag>) -> Markdown {
        let message = tags
            .into_iter()
            .map(|tag| format!("{}: {}", tag.name, tag.count))
            .collect_vec()
            .join("\n");
        Markdown::new(message)
    }

    #[must_use]
    pub fn get_find_set_messages(
        sets: Vec<SavedStickerSet>,
        sticker_unique_id: &str,
    ) -> Vec<Markdown> {
        let escaped_sticker_id =
            &escape_sticker_unique_id_for_command(sticker_unique_id);
        let lines = sets
            .into_iter()
            // TODO: separate function for generating message
            .enumerate()
            .map(|(id, set)| {
                let link =
                    format_set_as_markdown_link(&set.id.clone(), &set.title.unwrap_or(set.id));
                format!("Set {}: {link}", id + 1)
            })
            .chain(once(escape(
                format!("\nFind Similar Stickers: /similar_{escaped_sticker_id}").as_str(),
            )))
            .collect_vec();
        lines
            .chunks(32)
            .map(|line| Markdown::new(line.join("\n")))
            .collect_vec()
    }

    #[must_use]
    pub fn get_stats_text(stats: Stats) -> Markdown {
        Markdown::new(format!(
            "Sets: {}\nStickers: {}\nTaggings: {}",
            stats.sets, stats.stickers, stats.taggings
        ))
    }

    #[must_use]
    pub fn get_info_text() -> Markdown {
        Markdown::new(
        "*Tagging:*
Tag what you see\\. This is the same policy as e621\\.
Infos \\(e621 wiki\\): [twys](https://e621.net/wiki_pages/1684), [genders](https://e621.net/wiki_pages/3294), [checklist](https://e621.net/wiki_pages/310)",
    )
    }

    #[must_use]
    pub fn get_blacklist_text() -> Markdown {
        Markdown::new(
        "*Blacklist Info:*
The blacklist is not very useful as of now because the majority of stickers are not properly tagged yet\\.

*Emoji Search:*
If you search stickers by emojis instead of tags, the blacklist is not in effect\\.",
    )
    }

    #[must_use]
    pub fn get_set_operations_text(set_id: &str, set_title: &str) -> Markdown {
        Markdown::new(format!(
            "Tag or untag all stickers in the set {}",
            format_set_as_markdown_link(set_id, set_title)
        ))
    }

    #[must_use]
    pub fn get_processed_sticker_sets_text(queued_set_names: Vec<String>) -> Markdown {
        let mut message = String::new();
        message.push_str("Queued sticker sets:\n");
        for (i, set_name) in queued_set_names.iter().enumerate() {
            message.push_str(&format_set_as_markdown_link(set_name, set_name));
            message.push('\n');
        }

        Markdown::new(message)
    }

    #[must_use]
    pub fn get_sticker_text(
        set_name: &str,
        set_title: &str,
        sticker_unique_id: &str,
        emojis: Vec<Emoji>,
    ) -> Markdown {
        let escaped_sticker_id =
            &escape_sticker_unique_id_for_command(sticker_unique_id);
        Markdown::new(
    format!(
        "UwU you sent some stickers :3\nSet: {}\nSticker ID: {}\nEmojis: {}\nFind Sets: {}\nSet Operations: {}",
        format_set_as_markdown_link(set_name, set_title),
        escape(sticker_unique_id),
        escape(&emojis.iter().join(", ")),
        escape(format!("/findsets_{escaped_sticker_id}").as_str()),
        escape(format!("/setops_{escaped_sticker_id}").as_str()),
    ))
    }

    #[must_use]
    pub fn get_start_text() -> Markdown {
        Markdown::new(
    "Beep\\!

I'm a bot that can help you find stickers by tags\\. 

If you send me some stickers in this chat, I will add them to the database\\! It would also be awesome if you help tagging :3".to_string()
    )
    }

    #[must_use]
    pub fn get_main_text() -> Markdown {
        Markdown::new(
    "Below are some things you can explore\\. You will also find some commands in the bot menu\\."
        .to_string()
    )
    }

    #[must_use]
    pub fn get_continuous_tag_mode_text(tag: TagOperation, message: String) -> Markdown {
        match tag {
            TagOperation::Tag(tag) => Markdown::new(format!(
                "{message}
You are in Continuous Tag Mode\\.
Send a sticker to apply the 
```Add_Tag
{tag}
```
tag and wait for my reply before sending the next one\\.
/cancel to stop\\."
            )),
            TagOperation::Untag(tag) => Markdown::new(format!(
                "{message}
You are in Continuous Untag Mode\\.
Send a sticker to remove the
```Remove_Tag
{tag}
```
tag and wait for my reply before sending the next one\\.
/cancel to stop\\."
            )),
        }
    }

    #[must_use]
    pub fn parse_continuous_tag_mode_message(message: &Message) -> Option<TagOperation> {
        message.parse_entities().and_then(|entities| {
            entities
                .into_iter()
                .filter_map(|entity| match entity.kind() {
                    teloxide::types::MessageEntityKind::Pre { language } => {
                        Some((entity.text().trim(), language))
                    }
                    _ => None,
                })
                .find_map(|(codeblock_content, codeblock_language)| {
                    let Some(language) = codeblock_language else {
                        return None;
                    };
                    match language.as_str() {
                        "Add_Tag" => Some(TagOperation::Tag(codeblock_content.to_string())),
                        "Remove_Tag" => Some(TagOperation::Untag(codeblock_content.to_string())),
                        &_ => {
                            warn!("unknown language");
                            None
                        }
                    }
                })
        })
    }
}

#[must_use]
fn format_set_as_markdown_link(name: &str, title: &str) -> String {
    format!("[{}](https://t.me/addstickers/{})", escape(title), name)
}