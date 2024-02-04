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

