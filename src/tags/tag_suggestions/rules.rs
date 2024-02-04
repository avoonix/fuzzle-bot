use std::collections::HashMap;
use std::ops::Range;

use chumsky::error::Simple;
use chumsky::prelude::*;

use itertools::Itertools;

use crate::util::Emoji;

use super::tag_suggestion::ScoredTagSuggestion;

/// Anything that is more compicated than the e621 implications can be expressed as a rule
///
/// The left hand side of the rule is a pattern that must match the sticker emoji, set title, or
/// set name. The right hand side of the rule is a list of tags to apply to the sticker.
/// If any of the left hand side patterns match, all of the right hand side tags are applied.
/// If multiple rules match, scores are added together.
#[derive(Clone, Debug)]
pub struct TagSuggestionRules {
    emoji_rules: HashMap<Emoji, Vec<String>>,
    string_rules: Vec<StringRule>,
}

#[derive(Clone, Debug)]
enum StringRule {
    AnyStr(String, Vec<String>),
    Title(String, Vec<String>),
    TitleSuffix(String, Vec<String>),
    Name(String, Vec<String>),
    NameSuffix(String, Vec<String>),
}

impl TagSuggestionRules {
    pub fn parse(input: &str) -> anyhow::Result<Self> {
        let rules: Vec<Rule> = parser().parse(input).map_err(|e| {
            let err = e.into_iter().next().unwrap();
            let span = Range {
                start: err.span().start.max(50),
                end: err.span().end,
            };
            anyhow::anyhow!("{:?} {}", err, input[span].to_string())
        })?;
        let mut emoji_rules = HashMap::new();
        let mut string_rules = Vec::new();

        for (lhs, rhs) in rules {
            let mut tags = Vec::new();
            for rhs in rhs {
                match rhs {
                    Rhs::Tag(tag) => tags.push(tag),
                }
            }

            for lhs in lhs {
                match lhs {
                    Lhs::Emoji(emojis) => {
                        for emoji in Emoji::parse(&emojis) {
                            emoji_rules
                                .entry(emoji)
                                .or_insert_with(Vec::new)
                                .extend(tags.clone());
                        }
                    }
                    Lhs::Call(function, args) => match function.as_str() {
                        "any_str" => {
                            string_rules.push(StringRule::AnyStr(args, tags.clone()));
                        }
                        "title" => {
                            string_rules.push(StringRule::Title(args, tags.clone()));
                        }
                        "title_suffix" => {
                            string_rules.push(StringRule::TitleSuffix(args, tags.clone()));
                        }
                        "name" => {
                            string_rules.push(StringRule::Name(args, tags.clone()));
                        }
                        "name_suffix" => {
                            string_rules.push(StringRule::NameSuffix(args, tags.clone()));
                        }
                        _ => {
                            return Err(anyhow::anyhow!("Unknown function {}", function));
                        }
                    },
                }
            }
        }
        Ok(Self {
            emoji_rules,
            string_rules,
        })
    }

    pub fn parse_default() -> anyhow::Result<Self> {
        let res = Self::parse(include_str!("./rules.uwu"))?;
        Ok(res)
    }

    fn apply_string_rules(&self, set_title: &str, set_name: &str) -> Vec<String> {
        let mut tags: Vec<String> = Vec::new();
        let combined = format!("{set_name} {set_title}").to_lowercase();
        for rule in &self.string_rules {
            match rule {
                StringRule::AnyStr(string, rule_tags) => {
                    if combined.contains(string) {
                        tags.extend(rule_tags.clone());
                    }
                }
                StringRule::Title(string, rule_tags) => {
                    if set_title.contains(string) {
                        tags.extend(rule_tags.clone());
                    }
                }
                StringRule::TitleSuffix(string, rule_tags) => {
                    if set_title.ends_with(string) {
                        tags.extend(rule_tags.clone());
                    }
                }
                StringRule::Name(string, rule_tags) => {
                    if set_name.contains(string) {
                        tags.extend(rule_tags.clone());
                    }
                }
                StringRule::NameSuffix(string, rule_tags) => {
                    if set_name.ends_with(string) {
                        tags.extend(rule_tags.clone());
                    }
                }
            }
        }

        tags
    }

    #[must_use]
    pub fn suggest_tags(
        &self,
        emojis: Vec<Emoji>,
        set_title: &str,
        set_name: &str,
    ) -> Vec<ScoredTagSuggestion> {
        let mut tags: Vec<String> = Vec::new();
        for emoji in emojis {
            if let Some(emoji_tags) = self.emoji_rules.get(&emoji) {
                // TODO: store
                // Emojis in
                // emoji_rules
                // directly?
                tags.extend(emoji_tags.clone());
            }
        }

        tags.into_iter()
            .chain(self.apply_string_rules(set_title, set_name))
            .sorted()
            .group_by(std::clone::Clone::clone)
            .into_iter()
            .map(|(tag, group)| ScoredTagSuggestion {
                tag,
                score: compute_score_for_suggestion_count(group.count()),
            })
            .collect_vec()
    }
}

const fn compute_score_for_suggestion_count(count: usize) -> f64 {
    match count {
        1 => 0.5,
        2 => 0.6,
        3 => 0.7,
        4 => 0.8,
        _ => 0.9,
    }
}

#[derive(Debug)]
enum Lhs {
    Call(String, String),
    Emoji(String),
}

#[derive(Debug)]
enum Rhs {
    Tag(String),
}

type Rule = (Vec<Lhs>, Vec<Rhs>);

fn parser() -> impl Parser<char, Vec<Rule>, Error = Simple<char>> {
    let string = filter::<_, _, Simple<char>>(|c: &char| c != &'"')
        .repeated()
        .at_least(1)
        .collect::<String>()
        .delimited_by(just('"'), just('"'));

    let emoji = filter::<_, _, Simple<char>>(|c: &char| c != &' ' && c != &'=' && c != &';')
        .repeated()
        .at_least(1)
        .collect::<String>();

    let ident =
        filter::<_, _, Simple<char>>(|c: &char| c != &' ' && c != &'=' && c != &';' && c != &'(')
            .repeated()
            .at_least(1)
            .collect::<String>();

    let tag = filter::<_, _, Simple<char>>(|c: &char| c != &' ' && c != &'=' && c != &';')
        .repeated()
        .at_least(1)
        .collect::<String>();

    let call = ident
        .then(string.delimited_by(just('('), just(')')))
        .map(|(f, args)| Lhs::Call(f, args));

    let lhs = choice((call, emoji.map(Lhs::Emoji)))
        .padded()
        .repeated()
        .at_least(1);

    let rhs = tag.map(Rhs::Tag).padded().repeated().at_least(1);

    let rule = lhs
        .then_ignore(just("=>"))
        .then(rhs)
        .then_ignore(just(";"))
        .map(|(lhs, rhs)| (lhs, rhs));

    rule.padded().repeated().at_least(1).then_ignore(end())
}

#[cfg(test)]
mod tests {
    use crate::tags::get_default_tag_manager;

    use super::*;

    fn assert_suggested_tags_only_contain(suggestions: Vec<ScoredTagSuggestion>, tags: &[&str]) {
        let suggestions = suggestions
            .into_iter()
            .map(|suggestion| suggestion.tag)
            .sorted()
            .collect_vec();
        let tags = tags
            .iter()
            .map(|tag| (*tag).to_string())
            .sorted()
            .collect_vec();
        assert_eq!(suggestions, tags);
    }

    #[test]
    fn test_parse() -> anyhow::Result<()> {
        let input = r#"
            ❗ ❕‼️⁉️  => exclamation_point;
            ❔ ❓ ⁉️  => question_mark;
            ⛓ 🔗 🔓 🔏 🔒 🔐 => bondage bound;
            🗝 🔑 🔓 🔏 🔒 🔐 ⛓  🔗 => chastity_cage;
            🐾 👣 🦶 🧦 🦵 any_str("paws") => foot_focus;
            👙 👗 => girly;
            👙 🩲 => underwear;
            🧦 🦵 => thigh_highs;
            🤗 🙆‍♀️ 🙆‍♂️ 🫂 => hug duo;
            any_str("friends") title(" and ") title(" & ") => duo;
            name_suffix("NL") name_suffix("NaL") => nowandlater;
            any_str("nsfw") => questionable explicit;
        "#;
        let rules = TagSuggestionRules::parse(input)?;

        let suggestions = rules.suggest_tags(
            Emoji::parse("⁉️‼️ "),
            "Furry Paws Collection (NSFW)",
            "PawsNsfw",
        );
        assert_suggested_tags_only_contain(
            suggestions,
            &[
                "exclamation_point",
                "question_mark",
                "foot_focus",
                "questionable",
                "explicit",
            ],
        );

        let suggestions = rules.suggest_tags(Emoji::parse("🔓🧦"), "My Set", "set385972");
        assert_suggested_tags_only_contain(
            suggestions,
            &[
                "bondage",
                "bound",
                "chastity_cage",
                "foot_focus",
                "thigh_highs",
            ],
        );

        let suggestions = rules.suggest_tags(Emoji::parse("😍"), "Fox Pack", "FoxByNaL");
        assert_suggested_tags_only_contain(suggestions, &["nowandlater"]);

        let suggestions = rules.suggest_tags(Emoji::parse("😍"), "Fox and Friends", "foxfriends");
        assert_suggested_tags_only_contain(suggestions, &["duo"]);

        Ok(())
    }

    #[tokio::test]
    async fn test_default_rules_parse() -> anyhow::Result<()> {
        let rules = TagSuggestionRules::parse_default()?;
        let tag_manager = get_default_tag_manager(std::env::temp_dir()).await?;
        // go through all tags and verify that they exist
        rules.emoji_rules.values().flatten().for_each(|tag| {
            assert!(
                tag_manager.get_category(tag).is_some(),
                "Tag {tag} does not exist"
            );
        });

        Ok(())
    }
}
