mod bot;
mod emoji;
mod parsers;
mod required;
mod float_ext;

pub use bot::*;
use chrono::{Duration, NaiveDateTime, TimeDelta};
pub use emoji::*;
pub use parsers::*;
use rand::Rng;
use regex::Regex;
pub use required::*;
pub use float_ext::*;

pub fn format_relative_time(time: NaiveDateTime) -> String {
    let now = chrono::Utc::now().naive_utc();
    let duration = now - time;

    if duration < TimeDelta::zero() {
        tracing::error!("unexpected future time {duration}");
        format!("in {} minutes", duration.num_minutes())
    } else if duration < Duration::minutes(1) {
        "less than a minute ago".to_string()
    } else if duration < Duration::minutes(5) {
        "a few minutes ago".to_string()
    } else if duration < Duration::hours(1) {
        format!("{} minutes ago", duration.num_minutes())
    } else if duration < Duration::days(1) {
        let hours = duration.num_hours();
        if hours == 1 {
            "1 hour ago".to_string()
        } else {
            format!("{} hours ago", duration.num_hours())
        }
    } else if duration < Duration::days(2) {
        "yesterday".to_string()
    } else if duration < Duration::weeks(1) {
        format!("{} days ago", duration.num_days())
    } else if duration < Duration::weeks(2) {
        "1 week ago".to_string()
    } else if duration < Duration::days(60) {
        format!("{} weeks ago", duration.num_weeks())
    } else {
        format!("{} months ago", duration.num_days() / 30)
    }
}

pub fn create_tag_id(input: &str) -> String {
    let input = input.trim();

    let invalid_characters = Regex::new(r"[^-A-Za-z0-9_)(]").expect("hardcoded regex to compile");
    let input = invalid_characters.replace_all(&input, "_").into_owned();

    let consecutive_underscores = Regex::new(r"(__+)").expect("hardcoded regex to compile");
    consecutive_underscores.replace_all(&input, "_").into_owned()
}

pub fn create_sticker_set_id(set_title: &str, bot_username: &str) -> String {
    let invalid_characters = Regex::new(r"[^A-Za-z0-9_]").expect("hardcoded regex to compile");
    let set_title = invalid_characters.replace_all(&set_title, "_").into_owned();

    let set_title: String = set_title.chars().take(24).collect(); // max set id length: 64 characters in total; but we need to make sure the callback handlers can deal with this name
    let set_title = format!("_{set_title}_");

    let consecutive_underscores = Regex::new(r"(__+)").expect("hardcoded regex to compile");
    let set_title = consecutive_underscores.replace_all(&set_title, "_");

    let mut rng = rand::rng();
    let number: u32 = rng.random_range(100_000..=999_999);
    format!("pack{set_title}{number}_by_{bot_username}")
}
