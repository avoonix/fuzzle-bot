#[cfg(feature = "ssr")]
mod download;
#[cfg(feature = "ssr")]
mod hash;
#[cfg(feature = "ssr")]
mod import;

mod analysis;

#[cfg(feature = "ssr")]
pub use analysis::*; // TODO: don't expose everything

#[cfg(feature = "ssr")]
mod merge;

#[cfg(feature = "ssr")]
pub use merge::*;

pub use analysis::{Match, Measures, TopMatches}; // TODO: don't expose everything

#[cfg(feature = "ssr")]
pub use download::{fetch_possibly_cached_sticker_file};
#[cfg(feature = "ssr")]
pub use import::{import_all_stickers_from_set, import_individual_sticker_and_queue_set, notify_admin_if_set_new};
