use serde::{Deserialize, Serialize};

#[derive(PartialEq, PartialOrd, Serialize, Deserialize, Debug, Clone)]
pub struct Match {
    pub distance: f32,
    pub sticker_id: String,
}

impl Eq for Match {
    fn assert_receiver_is_total_eq(&self) {}
}

impl Ord for Match {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.distance
            .total_cmp(&other.distance)
            .then(self.sticker_id.cmp(&other.sticker_id))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Measures {
    pub histogram_cosine: Vec<Match>,
    pub embedding_cosine: Vec<Match>,
}
