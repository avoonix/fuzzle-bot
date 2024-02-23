use std::collections::HashMap;

use faiss::{index::IndexImpl, index_factory, Idx, Index, MetricType};
use itertools::Itertools;

use crate::{database::FileAnalysisWithStickerId, sticker::TopMatches};

use super::model::ModelEmbedding;

fn normalize(vec: &[f32]) -> Vec<f32> {
    let sum: f32 = vec.iter().map(|i| i * i).sum();
    let sqrt = sum.sqrt();
    vec.iter().map(|i| i / sqrt).collect_vec()
}

pub struct IndexInput {
    pub label: u64,
    pub vec: Vec<f32>,
}

pub struct IndexResult {
    pub label: u64,
    pub distance: f32,
}

pub struct MyIndex {
    index: IndexImpl,
}

impl MyIndex {
    pub fn new(input: Vec<IndexInput>) -> anyhow::Result<Self> {
        let d = input.get(0).ok_or(anyhow::anyhow!("no elements for indexing"))?.vec.len();
        let mut index = index_factory(d as u32, "Flat,IDMap", MetricType::InnerProduct)?;
        let vecs = input
            .iter()
            .map(|i| normalize(&i.vec))
            .flatten()
            .collect_vec();
        let labels = input.into_iter().map(|i| Idx::new(i.label)).collect_vec();
        index.add_with_ids(vecs.as_slice(), labels.as_slice())?;
        Ok(Self { index })
    }

    pub fn lookup(&mut self, query: Vec<f32>, n: usize) -> anyhow::Result<Vec<IndexResult>> {
        let result = self.index.search(&normalize(&query), n)?;
        Ok(result
            .labels
            .into_iter()
            .zip(result.distances.into_iter())
            .filter_map(|(label, distance)| label.get().map(|label| IndexResult {
                label,
                distance,
            }))
            .collect_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize() {
        assert_eq!(normalize(&vec![0.0, 1.0, 0.0]), vec![0.0, 1.0, 0.0]);
        assert_eq!(normalize(&vec![0.0, 10.0, 0.0]), vec![0.0, 1.0, 0.0]);
        assert_eq!(
            normalize(&vec![3.0, 5.0, -2.0]),
            vec![
                3.0 / 38.0_f32.sqrt(),
                5.0 / 38.0_f32.sqrt(),
                -(2.0_f32 / 19.0_f32).sqrt()
            ]
        );
    }
}
