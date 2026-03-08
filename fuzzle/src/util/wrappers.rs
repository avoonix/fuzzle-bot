use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;
use diesel::sql_types::BigInt;
use diesel::{
    backend::Backend,
    deserialize::{self, FromSqlRow},
    expression::AsExpression,
    prelude::*,
    serialize::{self, IsNull},
    sqlite::Sqlite,
};

macro_rules! define_string_wrapper {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize, AsExpression, FromSqlRow)]
        #[diesel(sql_type = diesel::sql_types::Text)]
        #[serde(transparent)]
        pub struct $name(String);

        impl<DB> diesel::serialize::ToSql<diesel::sql_types::Text, DB> for $name
        where
            DB: diesel::backend::Backend,
            String: diesel::serialize::ToSql<diesel::sql_types::Text, DB>,
        {
            fn to_sql<'b>(&'b self, out: &mut diesel::serialize::Output<'b, '_, DB>) -> diesel::serialize::Result {
                self.0.to_sql(out)
            }
        }

        impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Text, DB> for $name
        where
            DB: diesel::backend::Backend,
            String: diesel::deserialize::FromSql<diesel::sql_types::Text, DB>,
        {
            fn from_sql(bytes: <DB as diesel::backend::Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
                String::from_sql(bytes).map(Self)
            }
        }

        impl $name {
            pub fn new(value: impl Into<String>) -> Self { Self(value.into()) }
            pub fn into_inner(self) -> String { self.0 }
        }

        impl std::ops::Deref for $name {
            type Target = str;
            fn deref(&self) -> &Self::Target { &self.0 }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.0) }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self { Self(s) }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self { Self(s.to_string()) }
        }

        // Added From<&String> for direct conversion
        impl From<&String> for $name {
            fn from(s: &String) -> Self { Self(s.to_string()) }
        }

        impl From<$name> for String {
            fn from(w: $name) -> Self { w.0 }
        }

        impl std::str::FromStr for $name {
            type Err = std::convert::Infallible;
            fn from_str(s: &str) -> Result<Self, Self::Err> { Ok(Self(s.to_string())) }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str { &self.0 }
        }

        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str { &self.0 }
        }
    };
}

macro_rules! define_number_wrapper {
    // Added a third parameter for the specific Diesel SQL type mapping
    ($name:ident, $inner:ty, $sql_type:ty) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, serde::Serialize, serde::Deserialize, diesel::AsExpression, diesel::FromSqlRow)]
        #[diesel(sql_type = $sql_type)]
        #[serde(transparent)]
        pub struct $name($inner);

        impl<DB> diesel::serialize::ToSql<$sql_type, DB> for $name
        where
            DB: diesel::backend::Backend,
            $inner: diesel::serialize::ToSql<$sql_type, DB>,
        {
            fn to_sql<'b>(&'b self, out: &mut diesel::serialize::Output<'b, '_, DB>) -> diesel::serialize::Result {
                self.0.to_sql(out)
            }
        }

        impl<DB> diesel::deserialize::FromSql<$sql_type, DB> for $name
        where
            DB: diesel::backend::Backend,
            $inner: diesel::deserialize::FromSql<$sql_type, DB>,
        {
            fn from_sql(bytes: <DB as diesel::backend::Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
                <$inner>::from_sql(bytes).map(Self)
            }
        }

        impl $name {
            pub fn new(value: $inner) -> Self { Self(value) }
            pub fn into_inner(self) -> $inner { self.0 }
        }

        impl std::ops::Deref for $name {
            type Target = $inner;
            fn deref(&self) -> &Self::Target { &self.0 }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.0) }
        }

        impl From<$inner> for $name {
            fn from(val: $inner) -> Self { Self(val) }
        }

        impl From<$name> for $inner {
            fn from(w: $name) -> Self { w.0 }
        }
    };
}

define_string_wrapper!(StickerFileId);
define_string_wrapper!(StickerSetId);
define_string_wrapper!(StickerId);
// define_string_wrapper!(UserId);
// define_number_wrapper!(Timestamp, i64, diesel::sql_types::BigInt);