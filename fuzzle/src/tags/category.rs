use diesel::{deserialize::FromSqlRow, expression::AsExpression, Queryable, Selectable};
use enum_primitive_derive::Primitive;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default, Hash, Serialize, Deserialize, Primitive, AsExpression, FromSqlRow)]
#[diesel(sql_type = diesel::sql_types::BigInt)]
pub enum Category {
    #[default]
    General = 0,
    Artist = 1,
    Copyright = 3,
    Character = 4,
    Species = 5,
    Meta = 7,
    Lore = 8,
    Rating = 99,
}

impl Category {
    #[must_use]
    pub const fn to_color_name(self) -> &'static str {
        match self {
            Self::General => "slategray",
            Self::Artist => "orange",
            Self::Character => "green",
            Self::Species => "orangered",
            Self::Lore => "olive",
            Self::Copyright => "mediumorchid",

            Self::Meta | Self::Rating => "lightgray",
        }
    }

    #[must_use]
    pub const fn to_emoji(self) -> &'static str {
        match self {
            Self::General => "⚪️",
            Self::Artist => "🟠",
            Self::Character => "🟢",
            Self::Species => "🔴",
            Self::Lore => "🟤",
            Self::Copyright => "🟣",

            Self::Meta | Self::Rating => "⚫",
        }
    }

    #[must_use]
    pub const fn to_human_name(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Artist => "Artist",
            Self::Character => "Character",
            Self::Species => "Species",
            Self::Lore => "Lore",
            Self::Meta => "Meta",
            Self::Rating => "Rating",
            Self::Copyright => "Copyright",
        }
    }
}
