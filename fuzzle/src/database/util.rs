#[derive(Clone, Copy, Debug)]
pub enum Order {
    LatestFirst,
    Random { seed: i32 },
}

pub fn min_max<T: Ord>(a: T, b: T) -> (T, T) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}
