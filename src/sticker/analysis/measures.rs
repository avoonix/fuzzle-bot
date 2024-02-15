use serde::{Deserialize, Serialize};

#[derive(PartialEq, PartialOrd, Serialize, Deserialize, Debug, Clone)]
pub struct Match {
    pub distance: f64,
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
pub struct TopMatches {
    n: usize,
    worst_distance: f64,
    max_distance: f64,
    vec: Vec<Match>,
}

impl TopMatches {
    fn new(n: usize, max_distance: f64) -> Self {
        TopMatches {
            max_distance,
            worst_distance: f64::INFINITY,
            n,
            vec: vec![],
        }
    }

    pub(super) fn push(&mut self, distance: f64, sticker_id: String) {
        if distance > self.max_distance
            || (self.vec.len() >= self.n && distance > self.worst_distance)
        {
            return;
        }
        self.vec.push(Match {
            distance,
            sticker_id,
        });
        self.vec.sort_unstable();
        if self.vec.len() > self.n {
            self.vec.pop();
        }
        if let Some(last) = self.vec.last() {
            self.worst_distance = last.distance;
        }
    }

    pub fn items(self) -> Vec<Match> {
        self.vec
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Measures {
    pub histogram_cosine: TopMatches,
    pub visual_hash_cosine: TopMatches,
}

impl Measures {
    pub(super) fn new(
        n: usize,
        max_distance_histogram: f64,
        max_distance_visual_hash: f64,
    ) -> Self {
        Self {
            histogram_cosine: TopMatches::new(n, max_distance_histogram),
            visual_hash_cosine: TopMatches::new(n, max_distance_visual_hash),
        }
    }
}
