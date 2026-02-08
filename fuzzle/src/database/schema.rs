// @generated automatically by Diesel CLI.

diesel::table! {
    use crate::database::sqlite_mapping::*;

    banned_sticker (id) {
        id -> Text,
        telegram_file_identifier -> Text,
        sticker_set_id -> Text,
        sticker_file_id -> Text,
        thumbnail_file_id -> Nullable<Text>,
        sticker_type -> Integer,
        clip_max_match_distance -> Float,
        ban_reason -> Integer,
        created_at -> Timestamp,
    }
}

diesel::table! {
    use crate::database::sqlite_mapping::*;

    merged_sticker (canonical_sticker_file_id, removed_sticker_file_id, removed_sticker_id) {
        canonical_sticker_file_id -> Text,
        removed_sticker_file_id -> Text,
        removed_sticker_id -> Text,
        removed_sticker_set_id -> Text,
        created_by_user_id -> Nullable<BigInt>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    use crate::database::sqlite_mapping::*;

    moderation_task (id) {
        id -> BigInt,
        created_at -> Timestamp,
        created_by_user_id -> BigInt,
        details -> Text,
        completion_status -> Integer,
    }
}

diesel::table! {
    use crate::database::sqlite_mapping::*;

    potentially_similar_file (file_id_a, file_id_b) {
        file_id_a -> Text,
        file_id_b -> Text,
        status -> Integer,
    }
}

diesel::table! {
    use crate::database::sqlite_mapping::*;

    removed_set (id) {
        id -> Text,
        added_by_user_id -> Nullable<BigInt>,
    }
}

diesel::table! {
    use crate::database::sqlite_mapping::*;

    sticker (id) {
        id -> Text,
        sticker_set_id -> Text,
        telegram_file_identifier -> Text,
        sticker_file_id -> Text,
        emoji -> Nullable<Text>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    use crate::database::sqlite_mapping::*;

    sticker_file (id) {
        id -> Text,
        created_at -> Timestamp,
        tags_locked_by_user_id -> Nullable<BigInt>,
        thumbnail_file_id -> Nullable<Text>,
        sticker_type -> Integer,
    }
}

diesel::table! {
    use crate::database::sqlite_mapping::*;

    sticker_file_tag (sticker_file_id, tag) {
        sticker_file_id -> Text,
        tag -> Text,
        added_by_user_id -> Nullable<BigInt>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    use crate::database::sqlite_mapping::*;

    sticker_file_tag_history (id) {
        id -> BigInt,
        sticker_file_id -> Text,
        tag -> Text,
        removed_by_user_id -> Nullable<BigInt>,
        added_by_user_id -> Nullable<BigInt>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    use crate::database::sqlite_mapping::*;

    sticker_set (id) {
        id -> Text,
        title -> Nullable<Text>,
        last_fetched -> Nullable<Timestamp>,
        created_at -> Timestamp,
        added_by_user_id -> Nullable<BigInt>,
        created_by_user_id -> Nullable<Integer>,
        is_pending -> Bool,
    }
}

diesel::table! {
    use crate::database::sqlite_mapping::*;

    sticker_user (sticker_id, user_id) {
        sticker_id -> Text,
        user_id -> BigInt,
        is_favorite -> Bool,
        last_used -> Timestamp,
    }
}

diesel::table! {
    use crate::database::sqlite_mapping::*;

    tag (id) {
        id -> Text,
        category -> Integer,
        created_by_user_id -> Nullable<BigInt>,
        created_at -> Timestamp,
        linked_channel_id -> Nullable<Integer>,
        linked_user_id -> Nullable<Integer>,
        aliases -> Nullable<Text>,
        implications -> Nullable<Text>,
    }
}

diesel::table! {
    use crate::database::sqlite_mapping::*;

    user (id) {
        id -> BigInt,
        blacklist -> Text,
        can_tag_stickers -> Bool,
        can_tag_sets -> Bool,
        created_at -> Timestamp,
        settings -> Nullable<Text>,
        dialog_state -> Nullable<Text>,
    }
}

diesel::table! {
    use crate::database::sqlite_mapping::*;

    username (tg_username) {
        tg_username -> Text,
        tg_id -> Nullable<Integer>,
        kind -> Nullable<Integer>,
        updated_at -> Timestamp,
    }
}

diesel::joinable!(merged_sticker -> user (created_by_user_id));
diesel::joinable!(moderation_task -> user (created_by_user_id));
diesel::joinable!(removed_set -> user (added_by_user_id));
diesel::joinable!(sticker -> sticker_file (sticker_file_id));
diesel::joinable!(sticker -> sticker_set (sticker_set_id));
diesel::joinable!(sticker_file -> user (tags_locked_by_user_id));
diesel::joinable!(sticker_file_tag -> sticker_file (sticker_file_id));
diesel::joinable!(sticker_file_tag -> user (added_by_user_id));
diesel::joinable!(sticker_file_tag_history -> sticker_file (sticker_file_id));
diesel::joinable!(sticker_set -> user (added_by_user_id));
diesel::joinable!(sticker_user -> sticker (sticker_id));
diesel::joinable!(sticker_user -> user (user_id));
diesel::joinable!(tag -> user (created_by_user_id));

diesel::allow_tables_to_appear_in_same_query!(
    banned_sticker,
    merged_sticker,
    moderation_task,
    potentially_similar_file,
    removed_set,
    sticker,
    sticker_file,
    sticker_file_tag,
    sticker_file_tag_history,
    sticker_set,
    sticker_user,
    tag,
    user,
    username,
);
