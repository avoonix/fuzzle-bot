// adapted from https://docs.rs/acap/latest/src/acap/cos.rs.html#1-580
pub fn vec_u8_to_f32(x: Vec<u8>) -> Vec<f32> {
    x.into_iter().map(f32::from).collect()
}

// pub fn cosine_similarity(x: Vec<f32>, y: Vec<f32>) -> f32 {
//     debug_assert!(x.len() == y.len());

//     let mut dot = 0.0;
//     let mut xx = 0.0;
//     let mut yy = 0.0;

//     for i in 0..x.len() {
//         let xi = x[i];
//         let yi = y[i];
//         dot += xi * yi;
//         xx += xi * xi;
//         yy += yi * yi;
//     }

//     let similarity = dot / (xx * yy).sqrt();
//     if similarity.is_finite() {
//         similarity
//     } else {
//         0.0
//     }
// }

// pub fn euclidian_distance(x: Vec<u8>, y: Vec<u8>) -> f64 {
//     debug_assert!(x.len() == y.len());

//     let mut sum = 0.0;
//     for i in 0..x.len() {
//         let diff = x[i] as f64 - y[i] as f64;
//         sum += diff * diff;
//     }

//     sum.sqrt()
// }

