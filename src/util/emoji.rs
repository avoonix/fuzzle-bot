use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct Emoji(String);

impl Emoji {
    #[must_use]
    pub fn parse(emojis: &str) -> Vec<Self> {
        emojis
            .graphemes(true)
            .filter_map(emojis::get)
            .map(|emoji| Self(emoji.as_str().into()))
            .collect()
    }
}

impl Display for Emoji {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
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
    let Some(emoji) = emojis::get(first_grapheme) else {
        return (None, input);
    };
    let emoji = Some(Emoji(emoji.as_str().to_string()));
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
    fn test_parse_first() {
        let result = parse_first_emoji(" 🐾");
        assert_eq!(result, (None, " 🐾"));
        let result = parse_first_emoji("🏳️‍🌈🐾");
        assert_eq!(result, (Some(Emoji("🏳️‍🌈".into())), "🐾"));
        let result = parse_first_emoji("🐾");
        assert_eq!(result, (Some(Emoji("🐾".into())), ""));
    }

    #[test]
    fn test_emoji_parse() {
        assert_eq!(
            Emoji::parse("🌶️  👅"),
            vec![Emoji("🌶️".into()), Emoji("👅".into())]
        );
    }

    #[test]
    fn test_emoji_display() {
        assert_eq!(Emoji::parse("🌶️some text👅").iter().join(""), "🌶️👅");
    }
}
