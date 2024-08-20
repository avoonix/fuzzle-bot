use diesel::{deserialize::FromSqlRow, expression::AsExpression, Queryable, Selectable};
use enum_primitive_derive::Primitive;
use itertools::Itertools;

use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    time::SystemTime,
};
use strsim::sorensen_dice;
use tracing::{info, warn};

use super::csv::{TagAliasCsv, TagCsv, TagImplicationCsv};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagManager {
    tags: HashMap<String, Category>,
    aliases: HashMap<String, String>,
    implications: HashMap<String, Vec<String>>,
    match_distance: f64,
    allowed_statuses: HashSet<String>,
    inverse_implications: HashMap<String, Vec<String>>,
    banned_tags: HashSet<String>,
}

impl Default for TagManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TagManager {
    #[must_use]

    pub fn new() -> Self {
        Self {
            tags: HashMap::new(),
            aliases: HashMap::new(),
            implications: HashMap::new(),
            match_distance: 0.5,
            allowed_statuses: HashSet::from(["active".to_string()]),
            inverse_implications: HashMap::new(),
            banned_tags: HashSet::from([
                "sticker".to_string(),
                "telegram_sticker".to_string(),
                "sticker_pack".to_string(),
            ]),
        }
    }

    #[must_use]

    pub const fn set_match_distance(mut self, distance: f64) -> Self {
        self.match_distance = distance;
        self
    }

    #[must_use]

    pub fn set_allowed_statuses(mut self, statuses: HashSet<String>) -> Self {
        self.allowed_statuses = statuses;
        self
    }

    #[must_use]

    pub fn add_default_tags(mut self) -> Self {
        let mut tags = self.tags.clone();
        tags.insert("safe".to_string(), Category::Rating);
        tags.insert("questionable".to_string(), Category::Rating);
        tags.insert("explicit".to_string(), Category::Rating);

        tags.insert("meta_sticker".to_string(), Category::Meta);
        tags.insert("attribution".to_string(), Category::Meta);
        tags.insert("segmented_sticker".to_string(), Category::Meta);

        tags.insert("nowandlater".to_string(), Category::Artist);
        tags.insert("keavemind".to_string(), Category::Artist);
        tags.insert("yuniwusky".to_string(), Category::Artist);
        tags.insert("pulex".to_string(), Category::Artist);
        tags.insert("firelex".to_string(), Category::Artist);
        tags.insert("felisrandomis".to_string(), Category::Artist);
        tags.insert("stumblinbear".to_string(), Category::Artist);
        tags.insert("spookyfoxinc".to_string(), Category::Artist);
        tags.insert("niuka_folfsky".to_string(), Category::Artist);
        tags.insert("rainyote".to_string(), Category::Artist);
        tags.insert("dlw".to_string(), Category::Artist);
        tags.insert("kwik".to_string(), Category::Artist);
        tags.insert("rustledfluff".to_string(), Category::Artist);

        tags.insert("yes".to_string(), Category::General);
        tags.insert("no".to_string(), Category::General);
        tags.insert("unsure".to_string(), Category::General);
        tags.insert("thumbs_down".to_string(), Category::General);
        tags.insert("ill".to_string(), Category::General);
        tags.insert("a".to_string(), Category::General);
        tags.insert("yeet".to_string(), Category::General);
        tags.insert("holding_heart".to_string(), Category::General);
        tags.insert("segufix".to_string(), Category::General);
        tags.insert("hiding_behind_tail".to_string(), Category::General);
        tags.insert("diaper_creature".to_string(), Category::Species);

        self.tags = tags;
        self
    }

    #[must_use]

    pub fn add_tags_from_csv(
        mut self,
        csv: Vec<TagCsv>,
        min_post_count: i64,
        min_post_count_character: i64,
        ignore_meta_tags: bool,
        ignore_artist_tags: bool,
    ) -> Self {
        let mut tags = self.tags.clone();
        let mut counts: HashMap<Category, u64> = HashMap::new();
        for tag in csv {
            if tag.post_count < min_post_count {
                continue;
            }
            match tag.category.try_into() {
                Ok(category) => {
                    if ignore_meta_tags && category == Category::Meta {
                        continue;
                    }
                    if ignore_artist_tags && category == Category::Artist {
                        continue;
                    }
                    if category == Category::Character && tag.post_count < min_post_count_character
                    {
                        continue;
                    }
                    if self.banned_tags.contains(&tag.name) {
                        continue;
                    }
                    tags.insert(tag.name, category);
                    *counts.entry(category).or_default() += 1;
                }
                Err(e) => {
                    warn!("can't add tag {}: {}", tag.name, e);
                }
            }
        }
        for (category, count) in counts {
            let category = category.to_human_name();

            info!("inserted {count} {category} tags from csv");
        }

        self.tags = tags;
        self
    }

    #[must_use]

    pub fn add_default_aliases(mut self) -> Self {
        let mut aliases = self.aliases.clone();
        aliases.insert("s".to_string(), "safe".to_string());
        aliases.insert("rating:s".to_string(), "safe".to_string());
        aliases.insert("rating:safe".to_string(), "safe".to_string());
        aliases.insert("q".to_string(), "questionable".to_string());
        aliases.insert("rating:q".to_string(), "questionable".to_string());
        aliases.insert(
            "rating:questionable".to_string(),
            "questionable".to_string(),
        );
        aliases.insert("e".to_string(), "explicit".to_string());
        aliases.insert("rating:e".to_string(), "explicit".to_string());
        aliases.insert("rating:explicit".to_string(), "explicit".to_string());

        aliases.insert("m".to_string(), "male".to_string());
        aliases.insert("f".to_string(), "female".to_string());
        aliases.insert("1".to_string(), "solo".to_string());
        aliases.insert("2".to_string(), "duo".to_string());
        aliases.insert("3".to_string(), "trio".to_string());

        aliases.insert("creator".to_string(), "attribution".to_string());
        aliases.insert("maker".to_string(), "attribution".to_string());
        aliases.insert("stickers_by".to_string(), "attribution".to_string());

        aliases.insert("nal".to_string(), "nowandlater".to_string());
        aliases.insert("nav".to_string(), "nowandlater".to_string()); // logo looks like nav
        aliases.insert("niuka".to_string(), "niuka_folfsky".to_string());
        aliases.insert("niu-ka".to_string(), "niuka_folfsky".to_string());

        aliases.insert("moved_info".to_string(), "meta_sticker".to_string());
        aliases.insert("additional_info".to_string(), "meta_sticker".to_string());
        aliases.insert("information".to_string(), "meta_sticker".to_string());
        aliases.insert("advertisement".to_string(), "meta_sticker".to_string());
        aliases.insert("placeholder".to_string(), "meta_sticker".to_string());
        aliases.insert("artist_signature".to_string(), "meta_sticker".to_string());
        aliases.insert("contact_info".to_string(), "meta_sticker".to_string());
        aliases.insert("creator_sticker".to_string(), "meta_sticker".to_string());
        aliases.insert("author_sticker".to_string(), "meta_sticker".to_string());

        aliases.insert("split_sticker".to_string(), "segmented_sticker".to_string());
        aliases.insert("combined_sticker".to_string(), "segmented_sticker".to_string());
        aliases.insert("puzzle_sticker".to_string(), "segmented_sticker".to_string());
        aliases.insert("composite_sticker".to_string(), "segmented_sticker".to_string());
        aliases.insert("modular_sticker".to_string(), "segmented_sticker".to_string());
        aliases.insert("match-up_sticker".to_string(), "segmented_sticker".to_string());

        aliases.insert("ych".to_string(), "ych_(character)".to_string());
        aliases.insert("you".to_string(), "ych_(character)".to_string());
        aliases.insert(
            "your_character_here".to_string(),
            "ych_(character)".to_string(),
        );
        aliases.insert(
            "placeholder_character".to_string(),
            "ych_(character)".to_string(),
        );

        aliases.insert("innocent".to_string(), "angel".to_string());
        aliases.insert("ambivalent".to_string(), "unsure".to_string());
        aliases.insert("neutral".to_string(), "unsure".to_string());
        aliases.insert("sick".to_string(), "ill".to_string());
        aliases.insert("aaaaaa".to_string(), "a".to_string());
        aliases.insert("aaaa".to_string(), "a".to_string());
        aliases.insert("aa".to_string(), "a".to_string());
        aliases.insert("single_letter_a".to_string(), "a".to_string());
        aliases.insert("loud".to_string(), "screaming".to_string());
        aliases.insert("leggy".to_string(), "maned_wolf".to_string());
        aliases.insert("throw".to_string(), "yeet".to_string());

        aliases.insert("alextheyellowthing".to_string(), "firelex".to_string());
        aliases.insert("alex".to_string(), "firelex".to_string());
        aliases.insert("mountaindewdrawer".to_string(), "rainyote".to_string());
        aliases.insert("russ".to_string(), "rustledfluff".to_string());

        self.aliases = aliases;
        self
    }

    fn allowed_status(&self, status: &str) -> bool {
        self.allowed_statuses
            .iter()
            .any(|allowed_status| allowed_status == status)
    }

        #[must_use]
    
        pub fn add_aliases_from_csv(mut self, csv: Vec<TagAliasCsv>) -> Self {
            let mut aliases = self.aliases.clone();
            for alias in csv {
                if !self.allowed_status(&alias.status) {
                    continue;
                }
                if self.tags.get(&alias.consequent_name).is_some() {
                    aliases.insert(alias.antecedent_name, alias.consequent_name);
                }
            }

            self.aliases = aliases;
            self
        }

    #[must_use]

    pub fn add_default_implications(mut self) -> Self {
        let mut implications = self.implications.clone();

        implications
            .entry("attribution".to_string())
            .or_default()
            .push("meta_sticker".to_string());

        implications
            .entry("thumbs_up".to_string())
            .or_default()
            .push("yes".to_string());
        implications
            .entry("thumbs_down".to_string())
            .or_default()
            .push("no".to_string());

        implications
            .entry("holding_heart".to_string())
            .or_default()
            .push("holding_object".to_string());

        self.implications = implications;
        self
    }

    #[must_use]

    pub fn compute_transitive_implications(mut self) -> Self {
        let mut implications = self.implications.clone();
        let mut has_changed = true;
        while has_changed {
            has_changed = false;
            let old_implications = implications.clone();
            for (tag, implied_tags) in &mut implications {
                for implied_tag in implied_tags.clone() {
                    let transitive_implications = old_implications.get(&implied_tag);
                    if let Some(transitive_implications) = transitive_implications {
                        for transitive_implication in transitive_implications {
                            if !implied_tags.contains(transitive_implication) {
                                implied_tags.push(transitive_implication.clone());
                                has_changed = true;
                            }
                        }
                    }
                }
            }
        }
        self.implications = implications;
        self
    }

    #[must_use]

    pub fn add_implications_from_csv(mut self, csv: Vec<TagImplicationCsv>) -> Self {
        let mut implications = self.implications.clone();
        for implication in csv {
            if !self.allowed_status(&implication.status) {
                continue;
            }
            if let (Some(_), Some(_)) = (
                self.tags.get(&implication.antecedent_name),
                self.tags.get(&implication.consequent_name),
            ) {
                implications
                    .entry(implication.antecedent_name)
                    .or_default()
                    .push(implication.consequent_name);
            }
        }

        self.implications = implications;
        self
    }

    #[must_use]

    pub fn compute_inverse_implications(mut self) -> Self {
        let mut inverse_implications = self.inverse_implications.clone();

        for (antecedent, consequents) in &self.implications {
            for consequent in consequents {
                inverse_implications
                    .entry(consequent.to_string())
                    .or_default()
                    .push(antecedent.to_string());
            }
        }
        for consequents in inverse_implications.values_mut() {
            consequents.sort();
            consequents.dedup();
        }

        self.inverse_implications = inverse_implications;
        self
    }

    // input: Iterator<(alias, tag)>

    #[tracing::instrument(skip(entries))]
    fn get_tags_with_similarities<'a>(
        entries: impl Iterator<Item = (&'a String, &'a String)>,
        query: &'a str,
    ) -> impl Iterator<Item = (String, f64)> + 'a {
        entries
            .map(|(alias, tag)| {
                (
                    alias.to_string(),
                    tag.to_string(),
                    sorensen_dice(query, alias),
                )
            })
            .sorted_by(|(alias_a, _, score_a), (alias_b, _, score_b)| {
                score_b
                    .total_cmp(score_a)
                    .then_with(|| alias_a.len().cmp(&alias_b.len()))
            })
            .unique_by(|(_, tag, _)| tag.to_string())
            .map(|(_, tag, score)| (tag, score))
    }

    /// Returns a list of tags that match the user query (e.g. for autocomplete when tagging or
    /// searching)
    ///
    /// every string in the query must be a substring
    #[must_use]
    #[tracing::instrument(skip(self))]
    pub fn find_tags(&self, query: &[String]) -> Vec<String> {
        let query = query.iter().map(|q| q.to_lowercase()).collect_vec();
        // TODO: doesnt matter if this is slow, we can cache it
        // TODO: use tags and aliases to find matching tags for the query; use prefix matching, fuzzy
        // matching, and double metaphone matching
        let found_entries = self
            .tags
            .keys()
            .map(|tag| (tag, tag))
            .chain(self.aliases.iter())
            .filter(|(_, tag)| query.iter().all(|q| tag.contains(q)));
        let query = query.join(" ");
        let mut tags = Self::get_tags_with_similarities(found_entries, &query)
            .map(|(tag, _)| tag)
            .collect_vec();

        let all_entries = self
            .tags
            .keys()
            .map(|tag| (tag, tag))
            .chain(self.aliases.iter());
        let approximate_tags = Self::get_tags_with_similarities(all_entries, &query)
            .take_while(|(_, score)| *score > self.match_distance)
            .map(|(tag, _)| tag)
            .collect_vec();

        // close matches that are not substrings of the query
        let mut insertions = 0;
        for tag in approximate_tags.into_iter().rev() {
            if !tags.contains(&tag) {
                tags.insert(0, tag);
                insertions += 1;
            }
            if insertions >= 3 {
                break;
            }
        }

        // this re-sorting destroys the order obtained from using the alias to match
        // tags.sort_by(|a, b| normalized_damerau_levenshtein(&query, b).total_cmp(&normalized_damerau_levenshtein(&query, a)).then_with(|| a.len().cmp(&b.len())));
        tags
    }

    #[tracing::instrument(skip(self))]
    fn resolve_exact(&self, query: &str) -> Option<String> {
        if self.tags.get(query).is_some() {
            return Some(query.to_string());
        }
        if let Some(tag) = self.aliases.get(query) {
            return Some(tag.to_string());
        }
        None
    }

    #[must_use]
    pub fn get_category(&self, tag: &str) -> Option<Category> {
        self.tags.get(tag).map(std::borrow::ToOwned::to_owned)
    }

    /// gets the implied tags; empty vec if the tag exists; none if the tag does not exist
    #[must_use]
    pub fn get_implications(&self, tag: &str) -> Option<Vec<String>> {
        self.implications
            .get(tag)
            .map(std::borrow::ToOwned::to_owned)
            .or_else(|| self.tags.get(tag).map(|_| Vec::default()))
    }

    #[must_use]
    pub fn get_implications_including_self(&self, tag: &str) -> Option<Vec<String>> {
        self.implications
            .get(tag)
            .map(std::borrow::ToOwned::to_owned)
            .or_else(|| self.tags.get(tag).map(|_| Vec::default()))
            .map(|tags| {
                tags.into_iter()
                    .chain(std::iter::once(tag.to_string()))
                    .collect_vec()
            })
    }

    #[must_use]

    pub fn get_inverse_implications(&self, tag: &str) -> Option<Vec<String>> {
        self.inverse_implications
            .get(tag)
            .map(std::borrow::ToOwned::to_owned)
    }

    /// discards any tags where we couldnt find a match for
    #[must_use]
    #[tracing::instrument(skip(self))]
    pub fn closest_matching_tags(&self, query: &[String]) -> Vec<(String, Option<String>)> {
        query
            .iter()
            .map(|q| (q.clone(), self.closest_matching_tag(q)))
            .collect_vec()
    }

    /// Returns the tag that most closely matches the query
    /// This is used for resolving each tag in a query to one that actually exists
    #[must_use]
    #[tracing::instrument(skip(self))]
    pub fn closest_matching_tag(&self, query: &str) -> Option<String> {
        // TODO: maybe cache result?
        let query = query.to_lowercase();
        if let Some(tag) = self.resolve_exact(&query) {
            return Some(tag);
        }

        let entries = self
            .tags
            .keys()
            .map(|tag| (tag, tag))
            .chain(self.aliases.iter());
        let highest_match = Self::get_tags_with_similarities(entries, &query).next();

        if let Some((highest_match, score)) = highest_match {
            if score > self.match_distance {
                return Some(highest_match);
            }
        }
        None
    }

    pub fn get_tags(&self) -> Vec<String> {
        self.tags.keys().cloned().collect_vec()
    }

    pub fn iter_all(&self) -> TagAliasIterator<'_> {
        TagAliasIterator {
            tags_iter: self.tags.keys(),
            aliases_iter: self.aliases.keys(),
        }
    }
}

pub struct TagAliasIterator<'a> {
    tags_iter: std::collections::hash_map::Keys<'a, String, Category>,
    aliases_iter: std::collections::hash_map::Keys<'a, String, String>,
}

impl<'a> Iterator for TagAliasIterator<'a> {
    type Item = &'a str; // Change this to the appropriate type

    fn next(&mut self) -> Option<Self::Item> {
        // Combine the keys from tags and aliases
        self.tags_iter
            .next()
            .or_else(|| self.aliases_iter.next())
            .map(String::as_str)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default, Hash, Serialize, Deserialize, Primitive, AsExpression, FromSqlRow)]
#[diesel(sql_type = diesel::sql_types::BigInt)]
pub enum Category {
    #[default]
    General = 0,
    Artist = 1,
    Copyright = 3,
    Character = 4,
    Species = 5,
    Meta = 7,
    Lore = 8,
    Rating = 99,
}

impl Category {
    #[must_use]
    pub const fn to_color_name(self) -> &'static str {
        match self {
            Self::General => "slategray",
            Self::Artist => "orange",
            Self::Character => "green",
            Self::Species => "orangered",
            Self::Lore => "olive",
            Self::Copyright => "mediumorchid",

            Self::Meta | Self::Rating => "lightgray",
        }
    }

    #[must_use]
    pub const fn to_emoji(self) -> &'static str {
        match self {
            Self::General => "⚪️",
            Self::Artist => "🟠",
            Self::Character => "🟢",
            Self::Species => "🔴",
            Self::Lore => "🟤",
            Self::Copyright => "🟣",

            Self::Meta | Self::Rating => "⚫",
        }
    }

    #[must_use]
    pub const fn to_human_name(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Artist => "Artist",
            Self::Character => "Character",
            Self::Species => "Species",
            Self::Lore => "Lore",
            Self::Meta => "Meta",
            Self::Rating => "Rating",
            Self::Copyright => "Copyright",
        }
    }
}
