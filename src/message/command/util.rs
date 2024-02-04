use regex::Regex;

#[must_use]
pub fn fix_underline_command_separator(text: &str) -> String {
    let re = Regex::new(r"^/([A-Za-z]+)_").unwrap();
    re.replace(text, "/$1 ").to_string()
}

/// "-" is not recognized as part of a command
/// "___" is very unlikely to be part of a sticker unique id
#[must_use]
pub fn escape_sticker_unique_id_for_command(sticker_unique_id: &str) -> String {
    sticker_unique_id.replace('-', "___")
}

#[must_use]
pub fn unescape_sticker_unique_id_from_command(sticker_unique_id: &str) -> String {
    sticker_unique_id.replace("___", "-")
}
