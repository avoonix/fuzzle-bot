use log::info;

use crate::database::Database;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct ExportedData {
    sets: std::collections::HashMap<String, Vec<String>>,
    files: std::collections::HashMap<String, ExportedStickerFile>,
    // users: std::collections::HashMap<u64, ExportedUser>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct ExportedStickerFile {
    sticker_unique_ids: Vec<String>,
    tags: Vec<String>,
    visual_hash: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct ExportedUser {
    blacklist: Vec<String>,
}

impl ExportedStickerFile {
    fn new() -> Self {
        Self {
            sticker_unique_ids: Vec::new(),
            tags: Vec::new(),
            visual_hash: None,
        }
    }

    fn add_sticker(&mut self, sticker_unique_id: String) {
        self.sticker_unique_ids.push(sticker_unique_id);
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

    info!("Querying files->visual hashes");
    let visual_hashes = database
        .export_file_looks_like_visual_hash_relationships()
        .await?;

    // info!("Querying users");
    // let user_data = database.export_user_data().await?;

    info!("Preparing sets->stickers for export");
    let mut sets = std::collections::HashMap::new();
    for relationship in contains {
        let set_name = relationship.in_;
        let sticker_unique_id = relationship.out;
        sets.entry(set_name)
            .or_insert_with(Vec::new)
            .push(sticker_unique_id);
    }

    info!("Preparing stickers->files for export");
    let mut files = std::collections::HashMap::new();
    for relationship in is_a {
        let sticker_unique_id = relationship.in_;
        let file_hash = relationship.out;
        files
            .entry(file_hash)
            .or_insert_with(ExportedStickerFile::new)
            .add_sticker(sticker_unique_id);
    }

    info!("Preparing files->tags for export");
    for relationship in tagged {
        let file_hash = relationship.in_;
        let tag = relationship.out;
        files
            .entry(file_hash)
            .or_insert_with(ExportedStickerFile::new)
            .add_tag(tag);
    }

    info!("Preparing files->visual hashes for export");
    for relationship in visual_hashes {
        let file_hash = relationship.in_;
        let visual_hash = relationship.out;
        files
            .entry(file_hash)
            .or_insert_with(ExportedStickerFile::new)
            .visual_hash = Some(visual_hash);
    }

    // info!("Preparing users for export");
    // let mut users = std::collections::HashMap::new();
    // for user in user_data {
    //     users.insert(
    //         user.id,
    //         ExportedUser {
    //             blacklist: user.blacklist,
    //         },
    //     );
    // }

    // Ok(ExportedData { sets, files, users })
    Ok(ExportedData { sets, files })
    // let mut file = File::create(file_path)?;
    // let json = serde_json::to_string_pretty(&data)?;
    // file.write_all(json.as_bytes())?;

    // Ok(())
}
