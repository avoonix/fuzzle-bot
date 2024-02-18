#[derive(Clone, Copy, Debug)]
pub enum Order {
    LatestFirst,
    Random { seed: i32 },
}
