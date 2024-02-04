use std::fmt::{Display, Formatter};

const ZERO_CHARACTERS: &str = "zero_pictured";
const ONE_CHARACTER: &str = "solo";
const TWO_CHARACTERS: &str = "duo";
const THREE_CHARACTERS: &str = "trio";
const MULTIPLE_CHARACTERS: &str = "group";

const RATING_SAFE: &str = "safe";
const RATING_QUESTIONABLE: &str = "questionable";
const RATING_EXPLICIT: &str = "explicit";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Characters {
    Zero = 0,
    One = 1,
    Two = 2,
    Three = 3,
    Multiple = 4,
}

#[derive(Debug, Clone, Copy)]
pub enum Rating {
    Safe,
    Questionable,
    Explicit,
}

#[must_use] pub fn character_count(tag: &str) -> Option<Characters> {
    match tag {
        ZERO_CHARACTERS => Some(Characters::Zero),
        ONE_CHARACTER => Some(Characters::One),
        TWO_CHARACTERS => Some(Characters::Two),
        THREE_CHARACTERS => Some(Characters::Three),
        MULTIPLE_CHARACTERS => Some(Characters::Multiple),
        _ => None,
    }
}

#[must_use] pub fn rating(tag: &str) -> Option<Rating> {
    match tag {
        RATING_SAFE => Some(Rating::Safe),
        RATING_QUESTIONABLE => Some(Rating::Questionable),
        RATING_EXPLICIT => Some(Rating::Explicit),
        _ => None,
    }
}

#[must_use] pub fn all_count_tags() -> Vec<String> {
    vec![ZERO_CHARACTERS.to_string(), ONE_CHARACTER.to_string(), TWO_CHARACTERS.to_string(), THREE_CHARACTERS.to_string(), MULTIPLE_CHARACTERS.to_string()]
}

#[must_use] pub fn all_rating_tags() -> Vec<String> {
    vec![RATING_SAFE.to_string(), RATING_QUESTIONABLE.to_string(), RATING_EXPLICIT.to_string()]
}

impl Display for Characters {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Zero => write!(f, "{ZERO_CHARACTERS}"),
            Self::One => write!(f, "{ONE_CHARACTER}"),
            Self::Two => write!(f, "{TWO_CHARACTERS}"),
            Self::Three => write!(f, "{THREE_CHARACTERS}"),
            Self::Multiple => write!(f, "{MULTIPLE_CHARACTERS}"),
        }
    }
}

impl Display for Rating {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Safe => write!(f, "{RATING_SAFE}"),
            Self::Questionable => write!(f, "{RATING_QUESTIONABLE}"),
            Self::Explicit => write!(f, "{RATING_EXPLICIT}"),
        }
    }
}
