use super::download::{TagAliasCsv, TagCsv, TagImplicationCsv};
use itertools::Itertools;
use log::warn;
use std::collections::HashMap;
use strsim::sorensen_dice;

#[derive(Debug, Clone)]
pub struct TagManager {
    tags: HashMap<String, Category>,
    aliases: HashMap<String, String>,
    implications: HashMap<String, Vec<String>>,
    match_distance: f64,
    allowed_statuses: Vec<String>,
    inverse_implications: HashMap<String, Vec<String>>,
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
            allowed_statuses: vec!["active".to_string()],
            inverse_implications: HashMap::new(),
        }
    }

    #[must_use]
    pub const fn set_match_distance(mut self, distance: f64) -> Self {
        self.match_distance = distance;
        self
    }

    #[must_use]
    pub fn set_allowed_statuses(mut self, statuses: Vec<String>) -> Self {
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

        tags.insert("attribution".to_string(), Category::Artist);

        tags.insert("nowandlater".to_string(), Category::Artist);
        tags.insert("keavemind".to_string(), Category::Artist);
        tags.insert("yuniwusky".to_string(), Category::Artist);
        tags.insert("pulex".to_string(), Category::Artist);
        tags.insert("firelex".to_string(), Category::Artist);
        tags.insert("felisrandomis".to_string(), Category::Artist);
        tags.insert("stumblinbear".to_string(), Category::Artist);

        tags.insert("yes".to_string(), Category::General);
        tags.insert("no".to_string(), Category::General);
        tags.insert("unsure".to_string(), Category::General);
        tags.insert("thumbs_down".to_string(), Category::General);
        tags.insert("ill".to_string(), Category::General);
        tags.insert("a".to_string(), Category::General);
        tags.insert("yeet".to_string(), Category::General);

        self.tags = tags;
        self
    }

    #[must_use]
    pub fn add_tags_from_csv(
        mut self,
        csv: Vec<TagCsv>,
        min_post_count: i64,
        ignore_meta_tags: bool,
    ) -> Self {
        let mut tags = self.tags.clone();
        for tag in csv {
            if tag.post_count < min_post_count {
                continue;
            }
            match tag.category.try_into() {
                Ok(category) => {
                    if ignore_meta_tags && category == Category::Meta {
                        continue;
                    }
                    tags.insert(tag.name, category);
                }
                Err(e) => {
                    warn!("error adding tag {}: {}", tag.name, e);
                }
            }
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

        aliases.insert("moved_info".to_string(), "meta_sticker".to_string());
        aliases.insert("additional_info".to_string(), "meta_sticker".to_string());
        aliases.insert("information".to_string(), "meta_sticker".to_string());
        aliases.insert("advertisement".to_string(), "meta_sticker".to_string());
        aliases.insert("placeholder".to_string(), "meta_sticker".to_string());

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
    pub fn get_inverse_implications(&self, tag: &str) -> Option<Vec<String>> {
        self.inverse_implications
            .get(tag)
            .map(std::borrow::ToOwned::to_owned)
    }

    /// discards any tags where we couldnt find a match for
    #[must_use]
    pub fn closest_matching_tags(&self, query: &[String]) -> Vec<String> {
        query
            .iter()
            .filter_map(|q| self.closest_matching_tag(q))
            .collect_vec()
    }

    /// Returns the tag that most closely matches the query
    /// This is used for resolving each tag in a query to one that actually exists
    #[must_use]
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
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum Category {
    #[default]
    General,
    Artist,
    Copyright,
    Character,
    Species,
    Meta,
    Lore,
    Rating,
}

impl TryFrom<i8> for Category {
    type Error = String;

    fn try_from(value: i8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::General),
            1 => Ok(Self::Artist),
            3 => Ok(Self::Copyright),
            4 => Ok(Self::Character),
            5 => Ok(Self::Species),
            7 => Ok(Self::Meta),
            8 => Ok(Self::Lore),
            _ => Err(format!("invalid category: {value}")),
        }
    }
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
