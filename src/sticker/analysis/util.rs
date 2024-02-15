// adapted from https://docs.rs/acap/latest/src/acap/cos.rs.html#1-580
pub fn cosine_similarity(x: Vec<u8>, y: Vec<u8>) -> f64 {
    debug_assert!(x.len() == y.len());

    let mut dot = 0.0;
    let mut xx = 0.0;
    let mut yy = 0.0;

    for i in 0..x.len() {
        let xi = x[i] as f64;
        let yi = y[i] as f64;
        dot += xi * yi;
        xx += xi * xi;
        yy += yi * yi;
    }

    let similarity = dot / (xx * yy).sqrt();
    if similarity.is_finite() {
        similarity
    } else {
        0.0
    }
}

// pub fn euclidian_distance(x: Vec<u8>, y: Vec<u8>) -> f64 {
//     debug_assert!(x.len() == y.len());

//     let mut sum = 0.0;
//     for i in 0..x.len() {
//         let diff = x[i] as f64 - y[i] as f64;
//         sum += diff * diff;
//     }

//     sum.sqrt()
// }

