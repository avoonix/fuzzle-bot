use faiss::{index::IndexImpl, index_factory, Idx, Index, MetricType};
use itertools::Itertools;

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
    index: Option<IndexImpl>,
}

impl MyIndex {
    pub fn new(input: Vec<IndexInput>) -> anyhow::Result<Self> {
        let Some(d) = input.first() else {
            return Ok(Self { index: None });
        };
        let d = d.vec.len();
        let mut index = index_factory(d as u32, "Flat,IDMap", MetricType::InnerProduct)?;
        let vecs = input.iter().flat_map(|i| normalize(&i.vec)).collect_vec();
        let labels = input.into_iter().map(|i| Idx::new(i.label)).collect_vec();
        index.add_with_ids(vecs.as_slice(), labels.as_slice())?;
        Ok(Self { index: Some(index) })
    }

    pub fn lookup(&mut self, query: Vec<f32>, n: usize) -> anyhow::Result<Vec<IndexResult>> {
        match &mut self.index {
            Some(index) => {
                let result = index.search(&normalize(&query), n)?;
                Ok(result
                    .labels
                    .into_iter()
                    .zip(result.distances)
                    .filter_map(|(label, distance)| {
                        label.get().map(|label| IndexResult { label, distance })
                    })
                    .collect_vec())
            }
            None => Ok(vec![]),
        }
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
