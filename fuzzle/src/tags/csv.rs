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