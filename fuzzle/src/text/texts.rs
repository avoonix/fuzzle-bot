use std::collections::HashMap;

use crate::{
    callback::TagOperation,
    database::{
        AddedRemoved, AdminStats, AggregatedUserStats, FullUserStats, PersonalStats, PopularTag,
        Stats, StickerChange, StickerSet, Tag, UserSettings, UserStats, UserStickerStat,
    },
    message::{
        admin_command_description, escape_sticker_unique_id_for_command, user_command_description,
        PrivacyPolicy,
    },
    tags::Category,
    util::{format_relative_time, Emoji},
};
use itertools::Itertools;
use teloxide::{
    types::{Message, UserId},
    utils::markdown::escape,
};
use tracing::warn;

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
    pub fn removed_set() -> Markdown {
        Markdown::new("I can't add this sticker set\\.")
    }

    #[must_use]
    pub fn get_settings_text(settings: &UserSettings) -> Markdown {
        let order = match settings.order() {
            crate::database::StickerOrder::LatestFirst => "🆕 Latest First",
            crate::database::StickerOrder::Random => "🔀 Random",
        };

        Markdown::new(format!(
            "*Settings:*
Current Order: {order}
"
        ))
    }

    #[must_use]
    pub fn switch_pm_text() -> String {
        "Beep".to_string()
    }

    #[must_use]
    pub fn popular_tags(tags: Vec<(PopularTag, Category)>) -> Markdown {
        let tags_str = tags
            .into_iter()
            .map(|(tag, category)| {
                format!(
                    "{} {}: {} Stickers",
                    category.to_emoji(),
                    escape(&tag.name),
                    tag.count
                )
            })
            .collect_vec()
            .join("\n");
        Markdown::new(format!("🏷 *Most Used Tags*\n\n{tags_str}"))
    }

    #[must_use]
    pub fn general_stats(stats: Stats) -> Markdown {
        Markdown::new(format!(
            "🌐 *General Stats*\n\nSets: {}\nStickers: {}\nTagged Stickers: {}\nTaggings: {}",
            stats.sets, stats.stickers, stats.tagged_stickers, stats.taggings
        ))
    }

    #[must_use]
    pub fn personal_stats(stats: PersonalStats, set_count: i64) -> Markdown {
        Markdown::new(format!(
            "👤 *Personal Stats*\n\nFavorites: {}\nOwned Sets: {set_count}",
            stats.favorites
        ))
    }

    #[must_use]
    pub fn latest_sets(sets: Vec<StickerSet>) -> Markdown {
        let sets_str = sets
            .into_iter()
            .map(|set| {
                let link = format_set_as_markdown_link(
                    &set.id,
                    set.title.unwrap_or(set.id.clone()).as_ref(),
                );
                let relative_time = escape(&format_relative_time(set.created_at));

                format!("{link} \\({relative_time}\\)")
            })
            .collect_vec()
            .join("\n");
        Markdown::new(format!("🗂️ *New Sets*\n\n{sets_str}"))
    }

    #[must_use]
    pub fn general_user_stats(stats: AggregatedUserStats) -> Markdown {
        // TODO: add aggregate stats (eg total number of unique users)
        Markdown::new(format!(
            "👥 *User Stats*\n\nUnique sticker owners: {}",
            stats.unique_sticker_owners
        ))
    }

    #[must_use]
    pub fn latest_stickers(changes: Vec<StickerChange>) -> Markdown {
        let sets_str = changes
            .into_iter()
            .map(|change| {
                let link =
                    format_set_as_markdown_link(&change.sticker_set_id, &change.sticker_set_id);
                format!(
                    "{link}: {} added today, {} added this week",
                    change.today, change.this_week
                )
            })
            .collect_vec()
            .join("\n");
        Markdown::new(format!("✨️ *New Stickers*\n\n{sets_str}"))
    }

    #[must_use]
    pub fn infos() -> Markdown {
        Markdown::new(
        "*Tagging:*
Tag what you see\\. This is the same policy as e621\\. Tags are saved immediately\\.
Infos \\(e621 wiki\\): [twys](https://e621.net/wiki_pages/1684), [genders](https://e621.net/wiki_pages/3294), [checklist](https://e621.net/wiki_pages/310)

*Tag Locking:*
Tag locking prevents adding or removing tags via set operations\\. This is useful for adding e\\.g\\. species or fur color tags to a whole set witout messing up the tags on attribution stickers\\.

*Problems or Suggestions:*
Create an issue on [GitHub](https://github.com/avoonix/fuzzle-bot/issues)\\.",
    )
    }

    #[must_use]
    pub fn blacklist() -> Markdown {
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
    pub fn get_sticker_text(emoji: Option<Emoji>, set_is_new: bool, is_admin: bool, set_id: Option<String>) -> Markdown {
        let value = if let Some(emoji) = emoji {
            if let Some(name) = emoji.name() {
                format!(" {} {} ", name, emoji.to_string_with_variant())
            } else {
                format!(" {} ", emoji.to_string_with_variant())
            }
        } else {
            " ".to_string()
        };
        let new_set_text = if set_is_new {
            "\n\n✨ New Set ✨\nIt can take a few minutes until I'm done processing all stickers"
        } else {
            ""
        };
        let admin_text = if is_admin && let Some(set_id) = set_id {
            format!("\n\n/banset\\_{} /unbanset\\_{}", escape(&set_id), escape(&set_id))
        } else {"".to_string()};

        Markdown::new(format!(
            "UwU you sent a{}sticker :3{}{}",
            escape(&value),
            new_set_text,
            admin_text
        ))
    }

    #[must_use]
    pub fn get_start_text() -> Markdown {
        Markdown::new(
    "I'm a bot that can help you find stickers\\. I am using many of the e621 tags, but with telegram stickers\\. To use me, press the \"Use me in a chat\" button below\\.

If you send me some stickers in this chat, I will add them to the database\\. Help with tagging is appreciated :3".to_string()
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
    pub fn create_tag_task(
        tag_id: &str,
        category: Category,
        aliases: &[String],
        implications: &[String],
        db_tag: Option<Tag>,
    ) -> Markdown {
        Markdown::new(format!(
            "*Create Tag*
ID: {}
Category: {}
Aliases: {}
Implications: {}

DB: {}
",
            escape(tag_id),
            escape(category.to_human_name()),
            escape(&aliases.join(", ")),
            escape(&implications.join(", ")),
            db_tag_text(db_tag)
        ))
    }

    #[must_use]
    pub fn report_sticker_set_task() -> Markdown {
        Markdown::new("*Sticker Set Reported*\n".to_string())
    }

    #[must_use]
    pub fn review_new_sets_task() -> Markdown {
        Markdown::new("*New Sets*\n".to_string())
    }

    #[must_use]
    pub fn daily_report(
        counts: Stats,
        stats: AdminStats,
        taggings: HashMap<Option<i64>, UserStats>,
    ) -> Markdown {
        let age = stats
            .least_recently_fetched_set_age
            .map_or("never".to_string(), |age| {
                format!("{} hours", age.num_hours())
            });
        let text = escape(&format!(
            "Daily Report:
- {} stickers ({} tagged, {} sets) with {} taggings
- {} sets fetched within 24 hours
- least recently fetched set age: {}
- {} pending sets
- merge queue: /mergequeue

user taggings (24 hours):",
            counts.stickers,
            counts.tagged_stickers,
            counts.sets,
            counts.taggings,
            stats.number_of_sets_fetched_in_24_hours,
            age,
            stats.pending_set_count,
        ));

        let user_taggings = taggings
            .into_iter()
            .map(|(user_id, stats)| {
                let user = match user_id {
                    Some(user_id) => format!("user {user_id}"),
                    None => "no user".to_string(),
                };
                escape(&format!(
                    "- {user} (+{} -{})",
                    stats.added_tags, stats.removed_tags
                ))
            })
            .join("\n");

        Markdown::new(format!("{text}\n{user_taggings}"))
    }

    #[must_use]
    pub fn user_stats(user_stats: FullUserStats, user_id: u64) -> Markdown {
        let mut set_str = String::new();
        for (set_id, AddedRemoved { added, removed }) in user_stats.sets {
            set_str.push_str(
                format!(
                    "{}: \\+{added} \\-{removed}\n",
                    format_set_as_markdown_link(&set_id, &set_id)
                )
                .as_str(),
            );
        }

        Markdown::new(format!(
            "User: {user_id}
Total taggings: \\+{} \\-{}
Taggings \\(24 hours\\): \\+{} \\-{}
Taggings per set \\(24 hours\\):
{}",
            user_stats.total_tagged,
            user_stats.total_untagged,
            user_stats.tagged_24hrs,
            user_stats.untagged_24hrs,
            set_str
        ))
    }

    #[must_use]
    pub fn continuous_tag_success() -> Markdown {
        Markdown::new("Successfully tagged")
    }

    #[must_use]
    pub fn sticker_recommender_text(similar: usize, dissimilar: usize) -> Markdown {
        if similar == 0 && dissimilar == 0 {
            Markdown::escaped("Send stickers to get recommendations!")
        } else {
            Markdown::escaped(format!("Send more stickers to improve the recommendations!\nPositive Examples: {similar}\nNegative Examples: {dissimilar}"))
        }
    }

    // #[must_use]
    // pub fn continuous_tag_fail() -> Markdown {
    //     Markdown::new("Could not tag ._.")
    // }

    #[must_use]
    pub fn get_continuous_tag_mode_text(add_tags: &[String], remove_tags: &[String]) -> Markdown {
        let add_tags = if add_tags.len() > 0 {
            add_tags
                .iter()
                .map(|tag| format!("`{}`", escape(tag)))
                .join(", ")
        } else {
            "none".to_string()
        }; // TODO: better join with ` and `
        let remove_tags = if remove_tags.len() > 0 {
            remove_tags
                .iter()
                .map(|tag| format!("`{}`", escape(tag)))
                .join(", ")
        } else {
            "none".to_string()
        }; // TODO: better join with ` and `
        Markdown::new(format!(
            "You are in Continuous Tag Mode\\.
Tags that will be added:
{add_tags}
Tags that will be removed: 
{remove_tags}
Send stickers to apply these changes to them\\."
        ))
    }

    #[must_use]
    pub fn new_set(set_names: &[String]) -> Markdown {
        Markdown::new(format!(
            "Someone added the sets {}",
            set_names
                .into_iter()
                .map(|set_name| format_set_as_markdown_link(set_name, set_name))
                .join(", ")
        ))
    }

    #[must_use]
    pub fn untagged_set(set_name: &str, tags: &[String], count: usize) -> Markdown {
        let tags = tags.into_iter().map(|t| escape(t)).join(", ");
        let set = format_set_as_markdown_link(set_name, set_name);
        Markdown::new(format!(
            "Removed {tags} from set {set} \\({count} taggings changed\\)"
        ))
    }

    #[must_use]
    pub fn tagged_set(set_name: &str, tags: &[String], count: usize) -> Markdown {
        let set = format_set_as_markdown_link(set_name, set_name);
        let tags = tags
            .iter()
            .with_position()
            .map(|(position, tag)| {
                let tag = format!("\"{}\"", escape(tag));
                match position {
                    itertools::Position::Only => format!("tag {tag}"),
                    itertools::Position::First => format!("tags {tag}"),
                    itertools::Position::Middle => tag,
                    itertools::Position::Last => format!("and {tag}"),
                }
            })
            .join(", ");
        Markdown::new(format!(
            "Added {tags} for set {set} \\({count} taggings changed\\)"
        ))
    }

    #[must_use]
    pub fn get_set_article_link(set_id: &str, set_title: &str) -> Markdown {
        Markdown::new(format_set_as_markdown_link(set_id, set_title))
    }

    #[must_use]
    pub fn privacy(section: PrivacyPolicy) -> Markdown {
        Markdown::new(section.text())
    }
}

#[must_use]
fn format_set_as_markdown_link(name: &str, title: &str) -> String {
    format!("[{}](https://t.me/addstickers/{})", escape(title), name)
}

#[must_use]
fn format_user_id_as_markdown_link(user_id: UserId) -> String {
    format!("[{user_id}](tg://user?id={user_id})")
}

fn db_tag_text(tag: Option<Tag>) -> String {
    let Some(tag) = tag else {
        return "not present".to_string();
    };
    format!(
        "{}\ncreated {} \\(status updated {}\\)",
        tag.category.to_human_name(),
        escape(&format_relative_time(tag.created_at)),
        escape(&chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string())
    ) // TODO: more values
}
