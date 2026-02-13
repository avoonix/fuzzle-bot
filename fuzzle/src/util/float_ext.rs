pub trait FloatIteratorExt: Iterator + Sized {
    fn fmax(self) -> Option<Self::Item> where Self::Item: FloatTraits;
    fn fmin(self) -> Option<Self::Item> where Self::Item: FloatTraits;
}

impl<I: Iterator> FloatIteratorExt for I {
    fn fmax(self) -> Option<Self::Item> where Self::Item: FloatTraits {
        self.filter(|x| !x.is_nan_val()).max_by(|a, b| a.total_compare(b))
    }

    fn fmin(self) -> Option<Self::Item> where Self::Item: FloatTraits {
        self.filter(|x| !x.is_nan_val()).min_by(|a, b| a.total_compare(b))
    }
}

pub trait FloatTraits {
    fn is_nan_val(&self) -> bool;
    fn total_compare(&self, other: &Self) -> std::cmp::Ordering;
}

macro_rules! impl_float_traits {
    ($($t:ty),*) => {
        $(impl FloatTraits for $t {
            fn is_nan_val(&self) -> bool { self.is_nan() }
            fn total_compare(&self, other: &Self) -> std::cmp::Ordering { self.total_cmp(other) }
        })*
    };
}
impl_float_traits!(f32, f64);
