mod defaults;
mod implied;
mod rules;
mod same_set_tags;
mod similar_stickers_tags;
mod similar_tags;
mod suggest_tags;
mod tag_suggestion;
mod tfidf;
mod image_tag_similarity;

pub use suggest_tags::suggest_tags;
pub use tag_suggestion::ScoredTagSuggestion;
pub use tfidf::*;
