use anyhow::Result;
use flate2::read::GzDecoder;
use log::info;
use std::path::PathBuf;
use std::{fs, io};
use tokio::fs::{create_dir_all, File};
use tokio::io::AsyncWriteExt;

#[derive(Debug, serde::Deserialize, Clone)]
pub struct TagCsv {
    pub id: u64,
    pub name: String,
    pub category: i8,
    pub post_count: i64,
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct TagAliasCsv {
    pub id: u64,
    pub antecedent_name: String,
    pub consequent_name: String,
    pub created_at: Option<String>,
    pub status: String,
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct TagImplicationCsv {
    pub id: u64,
    pub antecedent_name: String,
    pub consequent_name: String,
    pub created_at: Option<String>,
    pub status: String,
}

pub async fn get_tag_implications(
    dir: PathBuf,
    base_url: impl Into<String> + Send,
) -> Result<Vec<TagImplicationCsv>> {
    let base_url: String = base_url.into();
    let today = chrono::Local::now();
    let yesterday = (today - chrono::Duration::days(1)).format("%Y-%m-%d");
    get_parsed_csv(dir, base_url, yesterday.to_string(), "tag_implications").await
}

pub async fn get_tag_aliases(
    dir: PathBuf,
    base_url: impl Into<String> + Send,
) -> Result<Vec<TagAliasCsv>> {
    let base_url: String = base_url.into();
    let today = chrono::Local::now();
    let yesterday = (today - chrono::Duration::days(1)).format("%Y-%m-%d");
    get_parsed_csv(dir, base_url, yesterday.to_string(), "tag_aliases").await
}

pub async fn get_tags(dir: PathBuf, base_url: impl Into<String> + Send) -> Result<Vec<TagCsv>> {
    let base_url: String = base_url.into();
    let today = chrono::Local::now();
    let yesterday = (today - chrono::Duration::days(1)).format("%Y-%m-%d");
    get_parsed_csv(dir, base_url, yesterday.to_string(), "tags").await
}

pub async fn clean_dir(dir: PathBuf) -> Result<()> {
    let today = chrono::Local::now();
    let yesterday = (today - chrono::Duration::days(1)).format("%Y-%m-%d");
    let yesterday = yesterday.to_string();
    let paths = fs::read_dir(dir)?;
    for res in paths {
        let path = res.unwrap().path();
        let filename = path.file_name().unwrap().to_str().unwrap();
        let is_correct_file_type = filename.ends_with(".csv.gz");
        let is_old = !filename.contains(yesterday.as_str());
        if is_correct_file_type && is_old {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

async fn get_parsed_csv<T>(
    dir: PathBuf,
    base_url: impl Into<String> + Send,
    current_date: impl Into<String> + Send,
    kind: impl Into<String> + Send,
) -> Result<Vec<T>>
where
    T: serde::de::DeserializeOwned,
{
    let current_date: String = current_date.into();
    let kind: String = kind.into();
    let path: PathBuf = dir.join(format!(
        "{}-{}.csv.gz",
        kind.as_str(),
        current_date.as_str()
    ));
    let url = format!(
        "{}/db_export/{}-{}.csv.gz",
        base_url.into(),
        kind.as_str(),
        current_date.as_str()
    );
    info!("{:?}", path); // TODO: remove
    let mut rdr = get_csv_reader(path, url).await?;
    let mut tags = Vec::new();
    for result in rdr.deserialize() {
        let record: T = result?;
        tags.push(record);
    }
    Ok(tags)
}

async fn get_csv_reader(path: PathBuf, url: String) -> Result<csv::Reader<impl io::Read>> {
    if !path.exists() {
        let resp = reqwest::get(&url).await?;
        let bytes = resp.bytes().await?;
        if let Some(parent) = path.parent() {
            create_dir_all(parent).await?;
        }
        let mut file = File::create(path.clone()).await?;
        file.write_all(&bytes).await?;
        file.sync_all().await?;
    }
    let file = std::fs::File::open(path)?;
    let gz = GzDecoder::new(file);
    let reader = csv::Reader::from_reader(gz);
    Ok(reader)
}
