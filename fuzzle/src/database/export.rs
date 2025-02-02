use std::collections::HashMap;

use itertools::Itertools;
use tracing::info;

use crate::database::Database;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct ExportedData {
    pub sets: HashMap<String, Vec<String>>,
    files: Vec<ExportedStickerFile>,
    // users: std::collections::HashMap<u64, ExportedUser>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct ExportedStickerFile {
    sticker_ids: Vec<String>,
    tags: Vec<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct ExportedUser {
    blacklist: Vec<String>,
}

impl ExportedStickerFile {
    fn new() -> Self {
        Self {
            sticker_ids: Vec::new(),
            tags: Vec::new(),
        }
    }

    fn add_sticker(&mut self, sticker_unique_id: String) {
        self.sticker_ids.push(sticker_unique_id);
    }

    fn add_tag(&mut self, tag: String) {
        self.tags.push(tag);
    }
}

pub async fn export_database(database: Database) -> anyhow::Result<ExportedData> {
    // TODO: flag to include or exlude user data (eg who added the tag)
    // let mut tags = db::get_all_tags().await?;
    // tags.sort_by(|a, b| a.name.cmp(&b.name));

    // let sets = db::export_sets().await?;
    // let tags = db::export_taggings().await?;

    info!("Querying sets->stickers");
    let contains = database.export_set_contains_sticker_relationship().await?;

    info!("Querying stickers->files");
    let is_a = database.export_sticker_is_a_file_relationship().await?;

    info!("Querying files->tags");
    let tagged = database.export_file_tagged_tag_relationships().await?;

    // info!("Querying users");
    // let user_data = database.export_user_data().await?;

    info!("Preparing sets->stickers for export");
    let mut sets = std::collections::HashMap::new();
    for relationship in contains {
        let set_name = relationship.0;
        let sticker_unique_id = relationship.1;
        sets.entry(set_name)
            .or_insert_with(Vec::new)
            .push(sticker_unique_id);
    }

    info!("Preparing stickers->files for export");
    let mut files = std::collections::HashMap::new();
    for relationship in is_a {
        let sticker_unique_id = relationship.0;
        let sticker_file_id = relationship.1;
        files
            .entry(sticker_file_id)
            .or_insert_with(ExportedStickerFile::new)
            .add_sticker(sticker_unique_id);
    }

    info!("Preparing files->tags for export");
    for relationship in tagged {
        let sticker_file_id = relationship.0;
        let tag = relationship.1;
        files
            .entry(sticker_file_id)
            .or_insert_with(ExportedStickerFile::new)
            .add_tag(tag);
    }

    let files = files.into_values().collect_vec();

    Ok(ExportedData { sets, files })
}
