use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct Emoji(String);

impl Emoji {
    pub fn new_from_string_single(emoji: impl Into<String>) -> Emoji {
        let without_variant_selector = emoji.into().trim_matches(|c| c =='\u{fe0f}' || c == '\u{fe0e}').to_string();
        Emoji(without_variant_selector)
    }
}

impl Emoji {
    pub fn to_string_without_variant(&self) -> String {
        self.0.to_string()
    }
    
    pub fn to_string_with_variant(&self) -> String {
        format!("{}\u{fe0f}", self.0)
    }
}

pub fn parse_first_emoji(input: &str) -> (Option<Emoji>, &str) {
    let mut graphemes = input.grapheme_indices(true);
    let Some((first_index, first_grapheme)) = graphemes.next() else {
        return (None, input);
    };
    if first_index != 0 {
        return (None, input);
    }
    let Some(emoji) = emojis::get(first_grapheme).or_else(|| {
        let without_variant_selector = first_grapheme.trim_matches(|c| c =='\u{fe0f}' || c == '\u{fe0e}');
        emojis::get(without_variant_selector)
    }) else {
        return (None, input);
    };
    let emoji = Some(Emoji::new_from_string_single(emoji.as_str()));
    if let Some((second_index, _)) = graphemes.next() {
        (emoji, &input[second_index..])
    } else {
        (emoji, "")
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use super::*;

    #[test]
    fn test_parse_first_0() {
        let result = parse_first_emoji("❗ ❕‼️⁉️");
        assert_eq!(result, (Some(Emoji("❗".into())), " ❕‼️⁉️"));
        let result = parse_first_emoji("‼️⁉️");
        assert_eq!(result, (Some(Emoji("‼️".into())), "⁉️"));
        let result = parse_first_emoji("⁉️");
        assert_eq!(result, (Some(Emoji("⁉️".into())), ""));
    }

    #[test]
    fn test_parse_first_1() {
        let result = parse_first_emoji(" 🐾");
        assert_eq!(result, (None, " 🐾"));
        let result = parse_first_emoji("🏳️‍🌈🐾");
        assert_eq!(result, (Some(Emoji::new_from_string_single("🏳️‍🌈")), "🐾"));
        let result = parse_first_emoji("☕️"); // with variant selector
        assert_eq!(result, (Some(Emoji::new_from_string_single("☕️")), "")); // with variant selector
        assert_eq!(result, (Some(Emoji::new_from_string_single("☕")), "")); // no variant selector
    }
}
