use crate::bot::InternalError;

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
            if message == "Bad Request: wrong file_id or the file is temporarily unavailable" =>
        {
            true
        }
        // this happens if we manually change the sticker file id - not sure if this can happen during regular use
        teloxide::ApiError::Unknown(message) if message == "Bad Request: invalid file_id" => true,
        _ => false,
    }
}

#[derive(Debug)]
pub struct DecodedStickerId {
    pub owner_id: i64,
    pub set_id: i64,
}

#[must_use]
// https://github.com/LyoSU/fStikBot/blob/fca2d4b4c3433332f0f4d7a994b0f2d84d69bc0f/update-packs.js#L15
pub fn decode_sticker_set_id(set_id: String) -> Result<DecodedStickerId, anyhow::Error> {
    let set_id: i64 = set_id.parse()?;
    let upper ;
    let lower ;

    if ((set_id >> 24 & 0xff) == 0xff) { // for 64-bit ids
        upper = (set_id >> 32) + 0x100000000;
        lower = (set_id & 0xf);
    } else {
        upper = set_id >> 32;
        lower = set_id & 0xffffffff;
    }

    Ok(
        DecodedStickerId {
            owner_id: upper,
            set_id: lower,
        }
    )
}
