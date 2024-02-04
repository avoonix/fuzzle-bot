use std::sync::Arc;

use crate::{tags::{tag_suggestions::defaults::suggest_default_tags, TagManager}, util::Emoji};

use self::{implied::suggest_tags_by_reverse_implication, rules::TagSuggestionRules};

mod defaults;
mod implied;
mod rules;
mod suggest_tags;
mod tag_suggestion;

pub use suggest_tags::suggest_tags;
pub use tag_suggestion::ScoredTagSuggestion;

pub fn suggest_tags_2(
    known_good_tags: &[String],
    tag_manager: Arc<TagManager>,
    emojis: Vec<Emoji>,
    set_title: &str,
    set_name: &str,
) -> anyhow::Result<Vec<ScoredTagSuggestion>> {
    // TODO: do not always re-parse
    // TODO: add rules for other emojis from other stickers in the set -> eg if many hugs, suggest
    // hug even thoug the current sticker does not have a hug emoji
    let rules = TagSuggestionRules::parse_default()?;
    dbg!("TODO: do not parse this every time!!");
    // TODO: use image based suggestions
    // , dynamic_image: &DynamicImage
    // .chain(suggest_tags_by_counting_pixel_colors(dynamic_image))

    Ok(ScoredTagSuggestion::merge(
        ScoredTagSuggestion::merge(
            suggest_tags_by_reverse_implication(known_good_tags, tag_manager),
            rules.suggest_tags(emojis, set_title, set_name),
        ),
        suggest_default_tags(),
    ))
}
