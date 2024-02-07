#[must_use]
pub fn teloxide_error_can_safely_be_ignored(err: &teloxide::RequestError) -> bool {
    match err {
        teloxide::RequestError::Api(teloxide::ApiError::Unknown(message)) => {
            // this just tells us that we took longer than 15 seconds to answer the callback query
            // nothing we can do about it
            message == "Bad Request: query is too old and response timeout expired or query ID is invalid"
        }
        _ => false,
    }
}

#[must_use]
pub fn is_wrong_file_id_error(err: &teloxide::ApiError) -> bool {
    match err {
        // some sticker packs have broken stickers
        teloxide::ApiError::Unknown(message)
            if message == "Bad Request: wrong file_id or the file is temporarliy unavailable" =>
        {
            true
        }
        // this happens if we manually change the sticker file id - not sure if this can happen during regular use
        teloxide::ApiError::Unknown(message) if message == "Bad Request: invalid file_id" => true,
        _ => false,
    }
}
