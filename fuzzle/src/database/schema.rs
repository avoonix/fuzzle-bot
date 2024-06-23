// @generated automatically by Diesel CLI.

diesel::table! {
    sticker_file (id) {
        id -> Text,
        created_at -> Timestamp,
        tags_locked_by_user_id -> Nullable<BigInt>,
        thumbnail_file_id -> Nullable<Text>,
        is_animated -> Bool,
    }
}

diesel::table! {
    sticker_file_tag (sticker_file_id, tag) {
        sticker_file_id -> Text,
        tag -> Text,
        added_by_user_id -> Nullable<BigInt>,
        created_at -> Timestamp,
    }
}

diesel::table! {
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
    potentially_similar_file (file_id_a, file_id_b) {
        file_id_a -> Text,
        file_id_b -> Text,
        status -> BigInt,
    }
}

diesel::table! {
    removed_set (id) {
        id -> Text,
        added_by_user_id -> Nullable<BigInt>,
    }
}

diesel::table! {
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
    sticker_set (id) {
        id -> Text,
        title -> Nullable<Text>,
        last_fetched -> Nullable<Timestamp>,
        created_at -> Timestamp,
        added_by_user_id -> Nullable<BigInt>,
    }
}

diesel::table! {
    sticker_user (sticker_id, user_id) {
        sticker_id -> Text,
        user_id -> BigInt,
        is_favorite -> Bool,
        last_used -> Timestamp,
    }
}

diesel::table! {
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

diesel::joinable!(sticker_file -> user (tags_locked_by_user_id));
diesel::joinable!(sticker_file_tag -> sticker_file (sticker_file_id));
diesel::joinable!(sticker_file_tag -> user (added_by_user_id));
diesel::joinable!(sticker_file_tag -> sticker (sticker_file_id));
diesel::joinable!(sticker_file_tag_history -> sticker_file (sticker_file_id));
diesel::joinable!(merged_sticker -> user (created_by_user_id));
diesel::joinable!(removed_set -> user (added_by_user_id));
diesel::joinable!(sticker -> sticker_file (sticker_file_id));
diesel::joinable!(sticker -> sticker_set (sticker_set_id));
diesel::joinable!(sticker_set -> user (added_by_user_id));
diesel::joinable!(sticker_user -> sticker (sticker_id));
diesel::joinable!(sticker_user -> user (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    sticker_file,
    sticker_file_tag,
    sticker_file_tag_history,
    merged_sticker,
    potentially_similar_file,
    removed_set,
    sticker,
    sticker_set,
    sticker_user,
    user,
);
