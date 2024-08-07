use std::{collections::HashSet, path::PathBuf, sync::Arc};

use tracing::info;
use tokio::try_join;

use crate::tags::clean_dir;

use super::{get_tag_aliases, get_tag_implications, get_tags, TagManager};

// TODO: impl Default for TagManager?
pub async fn get_default_tag_manager(dir: PathBuf) -> anyhow::Result<Arc<TagManager>> {
    info!("setting up default tag manager");
    let tags = get_tags(dir.clone(), "https://e621.net");
    let aliases = get_tag_aliases(dir.clone(), "https://e621.net");
    let implications = get_tag_implications(dir.clone(), "https://e621.net");
    let (tags, aliases, implications) = try_join!(tags, aliases, implications)?;
    clean_dir(dir).await?;

    let tags = TagManager::new()
        .set_match_distance(0.7)
        .set_allowed_statuses(HashSet::from(["active".to_string()]))
        .add_default_tags()
        .add_tags_from_csv(tags, 400, 1_000, true, true)
        .add_default_aliases()
        .add_aliases_from_csv(aliases)
        .add_default_implications()
        .add_implications_from_csv(implications)
        .compute_transitive_implications()
        .compute_inverse_implications();
    Ok(Arc::new(tags))
}

#[cfg(test)]
mod tests {

    // use super::*;
    // use anyhow::Result;
    // use bk_tree::{BKTree, metrics};
    // use itertools::Itertools;


    // #[tokio::test]
    // async fn compare_tag_finders() -> anyhow::Result<()> {
    //     let tag_manager = get_default_tag_manager(std::env::temp_dir()).await?;
    //     let mut tree: BKTree<&str> = BKTree::new(metrics::Levenshtein);
    //     for tag in tag_manager.get_tags() {
    //         tree.add(&tag)
    //     }

    //     dbg!(tree.find("bup", 2).collect_vec());
    //     Ok(())
    // }
}