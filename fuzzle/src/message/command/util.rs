use regex::Regex;

#[must_use]
pub fn fix_underline_command_separator_and_normalize(text: &str) -> String {
    let re = Regex::new(r"^/([A-Za-z]+)_").expect("static regex to compile");
    re.replace(text, "/$1 ").trim().to_string()
}

/// "-" is not recognized as part of a command
/// only a-zA-Z0-9_ is allowed
#[must_use]
pub fn escape_sticker_unique_id_for_command(sticker_unique_id: &str) -> String {
    sticker_unique_id.replace('-', "_uwu_") // lets hope this never occurs in a sticker id; alternative would be to give stickers integer ids in the database
}

#[must_use]
pub fn unescape_sticker_unique_id_from_command(sticker_unique_id: &str) -> String {
    sticker_unique_id.replace("_uwu_", "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn escape_unescape(text: &str) {
        assert_eq!(
            text,
            unescape_sticker_unique_id_from_command(&escape_sticker_unique_id_for_command(text))
        )
    }

    #[test]
    fn test_escape_unescape() {
        escape_unescape("asdf");
        escape_unescape("as-d-f");
        escape_unescape("as_d-f");
        escape_unescape("as__d-f");
        escape_unescape("as_-_d-f");
        escape_unescape("as_-df");
        escape_unescape("as-_df");
        escape_unescape("as--df");
        escape_unescape("as___df");
        escape_unescape("as---df");
    }
}
