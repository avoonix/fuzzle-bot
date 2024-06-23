mod bot;
mod emoji;
mod parsers;
mod required;

pub use bot::*;
use chrono::{Duration, NaiveDateTime, TimeDelta};
pub use emoji::*;
pub use parsers::*;
pub use required::*;

pub fn format_relative_time(time: NaiveDateTime) -> String {
    let now = chrono::Utc::now().naive_utc();
    let duration = now - time;

    if duration < TimeDelta::zero() {
        tracing::error!("unexpected future time {duration}");
        format!("in {} minutes", duration.num_minutes())
    } else if duration < Duration::minutes(1) {
        "less than a minute ago".to_string()
    } else if duration < Duration::hours(1) {
        format!("{} minutes ago", duration.num_minutes())
    } else if duration < Duration::days(1) {
        format!("{} hours ago", duration.num_hours())
    } else if duration < Duration::days(2) {
        "today".to_string()
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
