use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use tokio::try_join;

use crate::{
    bot::InternalError,
    tags::{clean_dir, get_tag_aliases, get_tag_implications, get_tags},
};

use super::{
    csv::{TagAliasCsv, TagCsv, TagImplicationCsv},
    Category, TagRepository,
};

pub struct E621Tags {
    tags: HashMap<String, Category>,
    aliases: HashMap<String, String>,
    implications: HashMap<String, Vec<String>>,
}

impl E621Tags {
    pub fn get_implications(&self) -> HashMap<String, Vec<String>> {
        self.implications.clone()
    }

    pub fn get_tags(&self) -> HashMap<String, Category> {
        self.tags.clone()
    }

    pub fn get_aliases(&self) -> HashMap<String, String> {
        self.aliases.clone()
    }
}

impl E621Tags {
    pub async fn new(dir: PathBuf) -> Result<E621Tags, InternalError> {
        let tags = get_tags(dir.clone(), "https://e621.net");
        let aliases = get_tag_aliases(dir.clone(), "https://e621.net");
        let implications = get_tag_implications(dir.clone(), "https://e621.net");
        let (tags, aliases, implications) = try_join!(tags, aliases, implications)?;
        clean_dir(dir).await?;

        let banned_tags = HashSet::from([
            "sticker".to_string(),
            "telegram_sticker".to_string(),
            "sticker_pack".to_string(),
        ]);
        let allowed_statuses = HashSet::from(["active".to_string()]);

        let e6 = Self {
            tags: HashMap::new(),
            aliases: HashMap::new(),
            implications: HashMap::new(),
        };

        let e6 = e6
            .add_tags_from_csv(tags, 400, 1_000, true, true, banned_tags)
            .add_aliases_from_csv(aliases, &allowed_statuses)
            .add_implications_from_csv(implications, &allowed_statuses);

        Ok(e6)
    }

    #[must_use]
    pub fn add_tags_from_csv(
        mut self,
        csv: Vec<TagCsv>,
        min_post_count: i64,
        min_post_count_character: i64,
        ignore_meta_tags: bool,
        ignore_artist_tags: bool,
        banned_tags: HashSet<String>,
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
                    if banned_tags.contains(&tag.name) {
                        continue;
                    }
                    tags.insert(tag.name, category);
                    *counts.entry(category).or_default() += 1;
                }
                Err(e) => {
                    tracing::warn!("can't add tag {}: {}", tag.name, e);
                }
            }
        }
        for (category, count) in counts {
            let category = category.to_human_name();

            tracing::info!("inserted {count} {category} tags from csv");
        }

        self.tags = tags;
        self
    }

    #[must_use]
    pub fn add_aliases_from_csv(
        mut self,
        csv: Vec<TagAliasCsv>,
        allowed_statuses: &HashSet<String>,
    ) -> Self {
        let mut aliases = self.aliases.clone();
        for alias in csv {
            if !allowed_statuses.contains(&alias.status) {
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
    pub fn add_implications_from_csv(
        mut self,
        csv: Vec<TagImplicationCsv>,
        allowed_statuses: &HashSet<String>,
    ) -> Self {
        let mut implications = self.implications.clone();
        for implication in csv {
            if !allowed_statuses.contains(&implication.status) {
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
}
