use std::collections::HashSet;
use std::future::IntoFuture;
use std::io::Read;
use std::sync::Arc;

use flate2::read::GzEncoder;
use flate2::{Compression, GzBuilder};
use itertools::Itertools;

use teloxide::payloads::AnswerCallbackQuerySetters;
use teloxide::prelude::*;
use teloxide::types::{
    InlineKeyboardMarkup, InputFile, LinkPreviewOptions, MediaKind, MessageCommon, MessageKind,
    ReplyMarkup,
};

use crate::bot::{
    report_bot_error, report_internal_error, report_internal_error_result, BotError, BotExt,
    InternalError, RequestContext, UserError, UserErrorSeverity,
};
use crate::callback::TagOperation;

use crate::database::{Database, MergeStatus};
use crate::database::{DialogState, TagCreator};
use crate::message::{send_merge_queue, Keyboard};
use crate::sticker::{
    determine_canonical_sticker_and_merge, fetch_sticker_file, import_all_stickers_from_set,
    FileKind,
};
use crate::tags::{suggest_tags, Category, TagManager};
use crate::text::{Markdown, Text};
use crate::util::{teloxide_error_can_safely_be_ignored, Required};

use crate::callback::CallbackData;

use tracing::Instrument;

#[tracing::instrument(skip(request_context, q))]
async fn change_sticker_locked_status(
    lock: bool,
    unique_id: &str,
    q: CallbackQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    if !request_context.can_tag_stickers() {
        return Err(anyhow::anyhow!(
            "user is not permitted to change locked status"
        ))?;
    };
    let sticker = request_context
        .database
        .get_sticker_by_id(unique_id)
        .await?
        .required()?;
    request_context
        .database
        .update_file_lock(&sticker.sticker_file_id, request_context.user.id, lock)
        .await?;

    send_tagging_keyboard(request_context.clone(), None, unique_id, q).await
}

#[tracing::instrument(skip(request_context))]
async fn handle_sticker_tag_action(
    operation: Option<TagOperation>,
    unique_id: String,
    q: CallbackQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    if !request_context.can_tag_stickers() {
        return Err(UserError::NoPermissionForAction("tag sticker".to_string()).into());
    }

    let notification;

    let file = request_context
        .database
        .get_sticker_file_by_sticker_id(&unique_id)
        .await?
        .required()?;

    match operation {
        Some(TagOperation::Tag(tag)) => {
            if let Some(implications) = request_context.tag_manager.get_implications(&tag) {
                let tags = implications
                    .clone()
                    .into_iter()
                    .chain(std::iter::once(tag.clone()))
                    .collect_vec();
                request_context
                    .database
                    .tag_file(&file.id, &tags, Some(request_context.user.id))
                    .await?;
                request_context.tagging_worker.maybe_recompute().await?;
                notification = Some(if implications.is_empty() {
                    "Saved!".to_string()
                } else {
                    format!("Saved! ({tag} implies {})", implications.join(", "))
                });
            } else {
                notification = Some("Invalid tag!".to_string());
            }
        }
        Some(TagOperation::Untag(tag)) => {
            let tags = tags_that_should_be_removed(
                tag.clone(),
                request_context
                    .database
                    .get_sticker_tags(&unique_id)
                    .await?,
                request_context.tag_manager.clone(),
            );
            request_context
                .database
                .untag_file(&file.id, &tags, request_context.user.id)
                .await?;
            let implications = request_context.tag_manager.get_implications(&tag);
            let tags = tags.join(", ");
            notification = Some(implications.map_or_else(
                || "Tag does not exist!".to_string(),
                |implications| {
                    if implications.is_empty() {
                        format!("Removed {tags}!")
                    } else {
                        format!(
                            "Removed {tags}! ({tag} implies {})",
                            implications.join(", ")
                        )
                    }
                },
            ));
        }
        None => notification = None,
    }

    send_tagging_keyboard(request_context, notification, &unique_id, q).await
}

#[tracing::instrument(skip(request_context, q))]
async fn send_tagging_keyboard(
    request_context: RequestContext,
    notification: Option<String>,
    unique_id: &str,
    q: CallbackQuery,
) -> Result<(), BotError> {
    // database: Database,
    // tag_manager: Arc<TagManager>,
    // bot: Bot,
    let tags = request_context.database.get_sticker_tags(unique_id).await?;
    let suggested_tags = suggest_tags(
        unique_id,
        request_context.bot.clone(),
        request_context.tag_manager.clone(),
        request_context.database.clone(),
        request_context.tagging_worker.clone(),
        request_context.vector_db.clone(),
        // request_context.tag_worker.clone(),
    )
    .await?;
    let is_locked = request_context
        .database
        .get_sticker_file_by_sticker_id(unique_id)
        .await?
        .map_or(false, |file| file.tags_locked_by_user_id.is_some());
    let keyboard = Some(Keyboard::tagging(
        &tags,
        unique_id,
        &suggested_tags,
        is_locked,
        request_context.is_continuous_tag_state(),
    ));

    answer_callback_query(request_context, q, None, keyboard, notification).await
}

#[tracing::instrument(skip(request_context, q))]
pub async fn callback_handler_wrapper(
    q: CallbackQuery,
    request_context: RequestContext,
) -> Result<(), ()> {
    match callback_handler(q.clone(), request_context.clone()).await {
        Ok(_) => {}
        Err(error) => {
            report_bot_error(&error);
            report_internal_error_result(show_error(q, request_context, error).await);
        }
    }
    Ok(())
}

#[tracing::instrument(skip(request_context, q), err(Debug))]
pub async fn show_error(
    q: CallbackQuery,
    request_context: RequestContext,
    error: BotError,
) -> Result<(), BotError> {
    let error = error.end_user_error();
    request_context
        .bot
        .answer_callback_query(&q.id)
        .text(error.0)
        .show_alert(error.1 == UserErrorSeverity::Error)
        .into_future()
        .instrument(tracing::info_span!("telegram_bot_answer_callback_query"))
        .await?;
    Ok(())
}

#[tracing::instrument(skip(request_context, q), err(Debug))]
pub async fn callback_handler(
    q: CallbackQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let data: CallbackData = q.data.clone().unwrap_or_default().try_into()?;
    match data {
        CallbackData::Privacy(privacy) => {
            answer_callback_query(
                request_context,
                q,
                Some(Text::privacy(privacy.unwrap_or_default())),
                Some(Keyboard::privacy(privacy.unwrap_or_default())),
                None,
            )
            .await
        }
        CallbackData::CreateTag => {
            let mut state = match request_context.dialog_state() {
                DialogState::TagCreator(state) => state,
                DialogState::ContinuousTag { .. }
                | DialogState::Normal
                | DialogState::StickerRecommender { .. } => {
                    return Err(UserError::InvalidMode.into());
                }
            };

            create_tag(&state, &request_context).await?;

            // TODO: notify admin (async here)

            request_context
                .bot
                .answer_callback_query(&q.id)
                .into_future()
                .instrument(tracing::info_span!("telegram_bot_answer_callback_query"))
                .await?;

            request_context
                .bot
                .send_markdown(
                    request_context.user_id(),
                    Markdown::escaped("Success! An admin should review your tag soon(ish) :3"),
                )
                .await?;

            let request_context = exit_mode(request_context.clone(), false).await?;

            Ok(())
        }
        CallbackData::RemoveAlias(alias) => {
            let mut state = match request_context.dialog_state() {
                DialogState::TagCreator(state) => state,
                DialogState::ContinuousTag { .. }
                | DialogState::Normal
                | DialogState::StickerRecommender { .. } => {
                    return Err(UserError::InvalidMode.into());
                }
            };
            state.aliases.retain(|s| *s != alias);

            request_context
                .database
                .update_dialog_state(
                    request_context.user.id,
                    &DialogState::TagCreator(state.clone()),
                )
                .await?;

            answer_callback_query(
                request_context.clone(),
                q,
                None,
                Some(Keyboard::tag_creator(&state)),
                None,
            )
            .await
        }
        CallbackData::ToggleExampleSticker { sticker_id } => {
            let mut state = match request_context.dialog_state() {
                DialogState::TagCreator(state) => state,
                DialogState::ContinuousTag { .. }
                | DialogState::Normal
                | DialogState::StickerRecommender { .. } => {
                    return Err(UserError::InvalidMode.into());
                }
            };
            if state.example_sticker_id.contains(&sticker_id) {
                state.example_sticker_id.retain(|s| *s != sticker_id);
            } else {
                state.example_sticker_id.push(sticker_id.clone());
            }

            let sticker = request_context
                .database
                .get_sticker_by_id(&sticker_id)
                .await?
                .required()?;

            request_context
                .database
                .update_dialog_state(
                    request_context.user.id,
                    &DialogState::TagCreator(state.clone()),
                )
                .await?;

            answer_callback_query(
                request_context.clone(),
                q,
                None,
                Some(Keyboard::tag_creator_sticker(&state, &sticker)),
                None,
            )
            .await
        }
        CallbackData::ToggleRecommendSticker {
            positive,
            sticker_id,
        } => {
            let (mut positive_sticker_id, mut negative_sticker_id) =
                match request_context.dialog_state() {
                    DialogState::Normal
                    | DialogState::ContinuousTag { .. }
                    | DialogState::TagCreator { .. } => {
                        return Err(UserError::InvalidMode.into());
                    }
                    DialogState::StickerRecommender {
                        negative_sticker_id,
                        positive_sticker_id,
                    } => (positive_sticker_id, negative_sticker_id),
                };
            if positive {
                if positive_sticker_id.contains(&sticker_id) {
                    positive_sticker_id.retain(|s| *s != sticker_id);
                } else {
                    positive_sticker_id.push(sticker_id.clone());
                    negative_sticker_id.retain(|s| *s != sticker_id);
                }
            } else {
                if negative_sticker_id.contains(&sticker_id) {
                    negative_sticker_id.retain(|s| *s != sticker_id);
                } else {
                    negative_sticker_id.push(sticker_id.clone());
                    positive_sticker_id.retain(|s| *s != sticker_id);
                }
            }

            request_context
                .database
                .update_dialog_state(
                    request_context.user.id,
                    &DialogState::StickerRecommender {
                        positive_sticker_id: positive_sticker_id.clone(),
                        negative_sticker_id: negative_sticker_id.clone(),
                    },
                )
                .await?;

            let sticker_user = request_context
                .database
                .get_sticker_user(&sticker_id, request_context.user.id)
                .await?;
            let is_favorited = sticker_user.map_or(false, |su| su.is_favorite); // TODO: duplicated code

            answer_callback_query(
                request_context.clone(),
                q,
                None,
                Some(Keyboard::recommender(
                    &sticker_id,
                    &positive_sticker_id,
                    &negative_sticker_id,
                    is_favorited,
                )),
                None,
            )
            .await
        }
        CallbackData::SetLock { lock, sticker_id } => {
            change_sticker_locked_status(lock, &sticker_id, q, request_context).await
        }
        CallbackData::Sticker {
            sticker_id,
            operation,
        } => handle_sticker_tag_action(operation, sticker_id, q, request_context).await,
        CallbackData::RemoveBlacklistedTag(tag) => {
            remove_blacklist_tag(q, tag, request_context).await
        }
        CallbackData::RemoveContinuousTag(tag) => {
            remove_continuous_tag(q, tag, request_context).await
        }
        CallbackData::ChangeSetStatus { set_name, banned } => {
            if banned {
                let set = request_context
                    .database
                    .get_sticker_set_by_id(&set_name)
                    .await?;
                ban_set(
                    set_name,
                    set.and_then(|set| set.added_by_user_id),
                    q,
                    request_context,
                )
                .await
            } else {
                unban_set(set_name, q, request_context).await // TODO: re-insert added_by_user_id; or leave in table and only add is_banned column?
            }
        }
        CallbackData::NoAction => {
            answer_callback_query(
                request_context.clone(),
                q,
                None,
                None,
                Some("This doesn't do anything, just some text :3".to_string()),
            )
            .await
        }
        CallbackData::Help => {
            answer_callback_query(
                request_context.clone(),
                q,
                Some(Text::get_help_text(request_context.is_admin())),
                Some(Keyboard::make_help_keyboard()),
                None,
            )
            .await
        }
        CallbackData::Settings => {
            answer_callback_query(
                request_context.clone(),
                q,
                Some(Text::get_settings_text(
                    &request_context.user.settings.clone().unwrap_or_default(),
                )),
                Some(Keyboard::make_settings_keyboard(
                    &request_context.user.settings.clone().unwrap_or_default(),
                )),
                None,
            )
            .await
        }
        CallbackData::LatestSets => {
            let sets = request_context.database.get_n_latest_sets(20).await?;
            answer_callback_query(
                request_context.clone(),
                q,
                Some(Text::latest_sets(sets)),
                Some(Keyboard::latest_sets()),
                None,
            )
            .await
        }
        CallbackData::LatestStickers => {
            let changes = request_context.database.get_n_latest_sticker_changes(20).await?;
            answer_callback_query(
                request_context.clone(),
                q,
                Some(Text::latest_stickers(changes.clone())),
                Some(Keyboard::latest_stickers(changes)),
                None,
            )
            .await
        }
        CallbackData::UserStats => {
            let stats = request_context
                .database
                .get_general_user_stats(20)
                .await?;
            let aggregated_stats = request_context
                .database
                .get_aggregated_user_stats()
                .await?;
            answer_callback_query(
                request_context.clone(),
                q,
                Some(Text::general_user_stats(aggregated_stats)),
                Some(Keyboard::general_user_stats(stats)),
                None,
            )
            .await
        }
        CallbackData::PersonalStats => {
            let stats = request_context
                .database
                .get_personal_stats(request_context.user.id)
                .await?;
            let set_count = request_context.database.get_owned_sticker_set_count(request_context.user.id).await?;
            answer_callback_query(
                request_context.clone(),
                q,
                Some(Text::personal_stats(stats, set_count)),
                Some(Keyboard::personal_stats(request_context.user.id)),
                None,
            )
            .await
        }
        CallbackData::GeneralStats => {
            let stats = request_context.database.get_stats().await?;
            answer_callback_query(
                request_context.clone(),
                q,
                Some(Text::general_stats(stats)),
                Some(Keyboard::general_stats()),
                None,
            )
            .await
        }
        CallbackData::PopularTags => {
            let tags = request_context.database.get_popular_tags(40).await?;
            let tags = tags.into_iter().filter_map(|tag| request_context.tag_manager.get_category(&tag.name).map(|category| (tag, category))).collect_vec();
            answer_callback_query(
                request_context.clone(),
                q,
                Some(Text::popular_tags(tags)),
                Some(Keyboard::popular_tags()),
                None,
            )
            .await
        }
        CallbackData::ExitDialog => {
            exit_mode(request_context.clone(), true).await?;

            _ = request_context.bot.answer_callback_query(&q.id).await; // TODO: should i just ignore the error?
            Ok(())
        }
        CallbackData::FeatureOverview => {
            request_context
                .bot
                .send_markdown(
                    request_context.user_id(),
                    Markdown::escaped("I can search stickers based on text. But be warned, it doesn't work very well and the blacklist is ignored."),
                )
                .reply_markup(Keyboard::embedding())
                .await?;

            _ = request_context.bot.answer_callback_query(&q.id).await?; // TODO: should i just ignore the error?
            Ok(())
        }
        CallbackData::DownloadSticker { sticker_id } => {
            request_context.bot.answer_callback_query(&q.id).await?;
            let sticker = request_context
                .database
                .get_sticker_by_id(&sticker_id)
                .await?
                .required()?;
            let (buf, file) = fetch_sticker_file(
                sticker.telegram_file_identifier,
                request_context.bot.clone(),
            )
            .await?;

            #[cfg(debug_assertions)]
            {
                // TODO: remove; this is just for testing
                std::fs::create_dir("/tmp/stickers");
                std::fs::write(format!("/tmp/stickers/{}", sticker.id.clone()), buf.clone())?;
            }

            let (buf, path) = match FileKind::from(file.path.as_ref()) {
                FileKind::Image | FileKind::Unknown => (buf, create_sticker_name(file.path)),
                FileKind::Tgs => {
                    let mut gz = GzBuilder::new()
                        .filename("sticker.tgs")
                        .buf_read(&*buf, Compression::best());
                    let mut buffer = Vec::new();
                    gz.read_to_end(&mut buffer)?;
                    (buffer, "sticker.tgs.gz".to_string())
                }
            };

            request_context
                .bot
                .send_document(
                    request_context.user_id(),
                    InputFile::memory(buf).file_name(path),
                )
                .disable_content_type_detection(true)
                .await?;
            Ok(())
        }
        CallbackData::OwnerPage { sticker_id } => {
            let set = request_context
                .database
                .get_sticker_set_by_sticker_id(&sticker_id)
                .await?
                .required()?;
            let owner = set.created_by_user_id.required()?;
            let set_count = request_context.database.get_owned_sticker_set_count(owner).await?;
            answer_callback_query(
                request_context.clone(),
                q,
                None,
                Some(Keyboard::owner_page(
                    &sticker_id,
                    owner,
                    set_count,
                )),
                None,
            )
            .await
        }
        CallbackData::StickerSetPage { sticker_id } => {
            let set = request_context
                .database
                .get_sticker_set_by_sticker_id(&sticker_id)
                .await?
                .required()?;
            answer_callback_query(
                request_context.clone(),
                q,
                None,
                Some(Keyboard::sticker_set_page(
                    &sticker_id,
                    &set.id,
                    set.created_at,
                )),
                None,
            )
            .await
        }
        CallbackData::FavoriteSticker {
            sticker_id,
            operation,
        } => {
            let is_favorite = operation == super::FavoriteAction::Favorite;
            request_context
                .database
                .set_recently_used_sticker_favorite(
                    request_context.user.id,
                    &sticker_id,
                    is_favorite,
                )
                .await?;
            match request_context.dialog_state() {
                DialogState::StickerRecommender {
                    positive_sticker_id,
                    negative_sticker_id,
                } => {
                    answer_callback_query(
                        request_context.clone(),
                        q,
                        None,
                        Some(Keyboard::recommender(
                            &sticker_id,
                            &positive_sticker_id,
                            &negative_sticker_id,
                            is_favorite,
                        )),
                        None,
                    )
                    .await
                }
                _ => sticker_explore_page(sticker_id, request_context, q).await,
            }
        }
        CallbackData::StickerExplorePage { sticker_id } => {
            sticker_explore_page(sticker_id, request_context, q).await
        }
        CallbackData::Blacklist => {
            answer_callback_query(
                request_context.clone(),
                q,
                Some(Text::blacklist()),
                Some(Keyboard::blacklist(&request_context.user.blacklist)),
                None,
            )
            .await
        }
        // show main menu: show main menu, edit message, add keyboard
        CallbackData::Start => {
            answer_callback_query(
                request_context,
                q,
                Some(Text::get_main_text()),
                Some(Keyboard::make_main_keyboard()),
                None,
            )
            .await
        }
        CallbackData::Info => {
            answer_callback_query(
                request_context,
                q,
                Some(Text::infos()),
                Some(Keyboard::info()),
                None,
            )
            .await
        }
        CallbackData::UserInfo(user_id) => {
            if !request_context.is_admin() {
                return Ok(());
            }
            request_context.bot.answer_callback_query(&q.id).await?;

            let user_stats = request_context
                .database
                .get_user_stats(user_id as i64)
                .await?;
            // TODO: allow each user to view their own stats

            request_context
                .bot
                .send_markdown(
                    request_context.user_id(),
                    Text::user_stats(user_stats, user_id),
                )
                .reply_markup(Keyboard::user_stats(user_id)?)
                .await?;
            Ok(())
        }
        CallbackData::SetOrder(order) => {
            let mut settings = request_context.user.settings.clone().unwrap_or_default();
            settings.order = Some(order);
            request_context
                .database
                .update_settings(request_context.user.id, &settings)
                .await?;
            answer_callback_query(
                request_context,
                q,
                Some(Text::get_settings_text(&settings)),
                Some(Keyboard::make_settings_keyboard(&settings)),
                None,
            )
            .await
        }
        CallbackData::Merge {
            sticker_id_a,
            sticker_id_b,
            merge,
        } => handle_sticker_merge(sticker_id_a, sticker_id_b, merge, q, request_context).await,
        CallbackData::RemoveLinkedUser => {
            let mut state = match request_context.dialog_state() {
                DialogState::TagCreator(state) => state,
                DialogState::ContinuousTag { .. }
                | DialogState::Normal
                | DialogState::StickerRecommender { .. } => {
                    return Err(UserError::InvalidMode.into());
                }
            };
            state.linked_user = None;
            request_context
                .database
                .update_dialog_state(
                    request_context.user.id,
                    &DialogState::TagCreator(state.clone()),
                )
                .await?;
            answer_callback_query(
                request_context,
                q,
                None,
                Some(Keyboard::tag_creator(&state)),
                None,
            )
            .await
        }
        CallbackData::LinkSelf => {
            let mut state = match request_context.dialog_state() {
                DialogState::TagCreator(state) => state,
                DialogState::ContinuousTag { .. }
                | DialogState::Normal
                | DialogState::StickerRecommender { .. } => {
                    return Err(UserError::InvalidMode.into());
                }
            };
            let username = q.from.username.clone().ok_or_else(|| UserError::UserWithoutUsername)?;
            state.linked_user = Some((request_context.user_id(), username));
            request_context
                .database
                .update_dialog_state(
                    request_context.user.id,
                    &DialogState::TagCreator(state.clone()),
                )
                .await?;
            answer_callback_query(
                request_context,
                q,
                None,
                Some(Keyboard::tag_creator(&state)),
                None,
            )
            .await
        }
        CallbackData::RemoveLinkedChannel => {
            // TODO: method tag_creator_state() that returns Result<TagCreator, UserError>?
            let mut state = match request_context.dialog_state() {
                DialogState::TagCreator(state) => state,
                DialogState::ContinuousTag { .. }
                | DialogState::Normal
                | DialogState::StickerRecommender { .. } => {
                    return Err(UserError::InvalidMode.into());
                }
            };
            state.linked_channel = None; // TODO: this is the same as above, just one line changed
            request_context
                .database
                .update_dialog_state(
                    request_context.user.id,
                    &DialogState::TagCreator(state.clone()),
                )
                .await?;
            answer_callback_query(
                request_context,
                q,
                None,
                Some(Keyboard::tag_creator(&state)),
                None,
            )
            .await
        }
        CallbackData::SetCategory(category) => {
            let mut state = match request_context.dialog_state() {
                DialogState::TagCreator(state) => state,
                DialogState::ContinuousTag { .. }
                | DialogState::Normal
                | DialogState::StickerRecommender { .. } => {
                    return Err(UserError::InvalidMode.into());
                }
            };
            state.category = category; // TODO: this is the same as above, just one line changed
            request_context
                .database
                .update_dialog_state(
                    request_context.user.id,
                    &DialogState::TagCreator(state.clone()),
                )
                .await?;
            answer_callback_query(
                request_context,
                q,
                None,
                Some(Keyboard::tag_creator(&state)),
                None,
            )
            .await
        }
    }
}

#[tracing::instrument(skip(request_context, q), err(Debug))]
async fn handle_sticker_merge(
    sticker_id_a: String,
    sticker_id_b: String,
    merge: bool,
    q: CallbackQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    if !request_context.is_admin() {
        return Err(UserError::NoPermissionForAction("merge stickers".to_string()).into());
    }
    let done_text = if merge { "merged" } else { "not merged" };
    let file_a = request_context
        .database
        .get_sticker_file_by_sticker_id(&sticker_id_a)
        .await?
        .required()?;
    let file_b = request_context
        .database
        .get_sticker_file_by_sticker_id(&sticker_id_b)
        .await?
        .required()?;
    if merge {
        determine_canonical_sticker_and_merge(
            sticker_id_a.clone(),
            sticker_id_b.clone(),
            request_context.database.clone(),
        )
        .await?;
        request_context
            .database
            .add_or_modify_potential_merge(&file_a.id, &file_b.id, MergeStatus::Merged)
            .await?;
    } else {
        request_context
            .database
            .add_or_modify_potential_merge(&file_a.id, &file_b.id, MergeStatus::NotMerged)
            .await?;
    }
    let set_a = request_context
        .database
        .get_sticker_set_by_sticker_id(&sticker_id_a)
        .await?
        .required()?;
    let set_b = request_context
        .database
        .get_sticker_set_by_sticker_id(&sticker_id_b)
        .await?
        .required()?;
    answer_callback_query(
        request_context.clone(),
        q,
        None,
        Some(Keyboard::merge_done(&set_a.id, &set_b.id)?),
        None,
    )
    .await?;
    send_merge_queue(request_context.user_id().into(), request_context).await
}

#[tracing::instrument(skip(request_context), err(Debug))]
#[must_use]
pub async fn exit_mode(
    request_context: RequestContext,
    notify_if_already_in_normal_mode: bool,
) -> Result<RequestContext, BotError> {
    if matches!(request_context.dialog_state(), DialogState::Normal) {
        if notify_if_already_in_normal_mode {
            request_context
                .bot
                .send_markdown(
                    request_context.user_id(),
                    Markdown::escaped("I wasn't doing anything"),
                )
                .reply_markup(ReplyMarkup::kb_remove())
                .await?;
        }
        Ok(request_context)
    } else {
        request_context
            .database
            .update_dialog_state(request_context.user.id, &DialogState::Normal)
            .await?;
        request_context
            .bot
            .send_markdown(
                request_context.user_id(),
                Markdown::escaped("Back in normal mode"),
            )
            .reply_markup(ReplyMarkup::kb_remove())
            .await?;

        Ok(request_context.with_updated_user().await?)
        // request_context
        //     .bot
        //     .unpin_all_chat_messages(request_context.user_id())
        //     .await?;
    }
}


#[tracing::instrument(skip(request_context, q), err(Debug))]
async fn sticker_explore_page(
    sticker_id: String,
    request_context: RequestContext,
    q: CallbackQuery,
) -> Result<(), BotError> {
    answer_callback_query(
        request_context.clone(),
        q,
        None,
        Some(sticker_explore_keyboard(sticker_id, request_context).await?),
        None,
    )
    .await
}

#[tracing::instrument(skip(request_context), err(Debug))]
pub async fn sticker_explore_keyboard(
    sticker_id: String,
    request_context: RequestContext,
) -> Result<InlineKeyboardMarkup, InternalError> {
    let file = request_context
        .database
        .get_sticker_file_by_sticker_id(&sticker_id)
        .await?
        .required()?;
    let set_count = request_context
        .database
        .get_sets_containing_file(&file.id)
        .await?
        .len();
    let sticker_user = request_context
        .database
        .get_sticker_user(&sticker_id, request_context.user.id)
        .await?;
    let is_favorited = sticker_user.map_or(false, |su| su.is_favorite);
Ok(Keyboard::sticker_explore_page(
            &sticker_id,
            set_count,
            file.created_at,
            is_favorited,
        ))
}

fn create_sticker_name(path: String) -> String {
    path.rsplit_once(".")
        .map_or("sticker.bin".to_string(), |(_, ext)| {
            format!("sticker.{ext}")
        })
}

fn tags_that_should_be_removed(
    tag: String,
    current: Vec<String>,
    tag_manager: Arc<TagManager>,
) -> Vec<String> {
    current
        .into_iter()
        .filter(|t| {
            tag_manager
                .get_implications(&t)
                .unwrap_or_default()
                .contains(&tag)
        })
        .chain(std::iter::once(tag.clone()))
        .collect_vec()
}

#[tracing::instrument(skip(request_context, q), err(Debug))]
async fn remove_continuous_tag(
    q: CallbackQuery,
    tag: String,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let (add_tags, remove_tags) = match request_context.dialog_state() {
        DialogState::ContinuousTag {
            add_tags,
            remove_tags,
        } => {
            let to_add = tags_that_should_be_removed(
                tag.clone(),
                add_tags.clone(),
                request_context.tag_manager.clone(),
            );
            let to_remove = tags_that_should_be_removed(
                tag.clone(),
                remove_tags.clone(),
                request_context.tag_manager.clone(),
            );
            (
                add_tags
                    .into_iter()
                    .filter(|t| !to_add.contains(t))
                    .collect_vec(),
                remove_tags
                    .into_iter()
                    .filter(|t| !to_remove.contains(t))
                    .collect_vec(),
            )
        }
        _ => {
            return Err(UserError::InvalidMode.into());
        }
    };
    let new_dialog_state = DialogState::ContinuousTag {
        add_tags: add_tags.clone(),
        remove_tags: remove_tags.clone(),
    };
    request_context
        .database
        .update_dialog_state(request_context.user.id, &new_dialog_state)
        .await?;

    answer_callback_query(
        request_context,
        q,
        Some(Text::get_continuous_tag_mode_text(
            add_tags.as_slice(),
            remove_tags.as_slice(),
        )),
        Some(Keyboard::make_continuous_tag_keyboard(
            true,
            add_tags.as_slice(),
            remove_tags.as_slice(),
        )),
        None,
    )
    .await?;
    Ok(())
}

#[tracing::instrument(skip(request_context, q), err(Debug))]
async fn remove_blacklist_tag(
    q: CallbackQuery,
    tag: String,
    request_context: RequestContext,
) -> Result<(), BotError> {
    let blacklist = request_context
        .user
        .blacklist
        .clone()
        .into_inner()
        .into_iter()
        .filter(|t| t != &tag)
        .collect_vec();
    request_context
        .database
        .update_user_blacklist(request_context.user.id, blacklist.clone().into())
        .await?;
    let keyboard = Keyboard::blacklist(&blacklist);
    answer_callback_query(
        request_context,
        q,
        Some(Text::blacklist()),
        Some(keyboard),
        None,
    )
    .await?;
    Ok(())
}

#[tracing::instrument(skip(request_context, q), err(Debug))]
async fn unban_set(
    set_name: String,
    q: CallbackQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    if !request_context.is_admin() {
        return Err(UserError::NoPermissionForAction("unban set".to_string()).into());
    }
    request_context.database.unban_set(&set_name).await?;
    import_all_stickers_from_set(
        &set_name,
        true,
        request_context.bot.clone(),
        request_context.database.clone(),
        request_context.config.clone(),
        request_context.vector_db.clone(),
    )
    .await?;
    answer_callback_query(
        request_context,
        q,
        Some(Markdown::escaped(format!("Added Set {set_name}"))),
        Some(InlineKeyboardMarkup::new([[]])), // TODO: refactor
        None,
    )
    .await?;
    Ok(())
}

#[tracing::instrument(skip(request_context, q), err(Debug))]
async fn ban_set(
    set_name: String,
    added_by_user_id: Option<i64>,
    q: CallbackQuery,
    request_context: RequestContext,
) -> Result<(), BotError> {
    if !request_context.is_admin() {
        return Err(UserError::NoPermissionForAction("ban set".to_string()).into());
    }
    // TODO: maybe require additional confirmation?

    request_context
        .database
        .delete_sticker_set(&set_name)
        .await?;
    request_context
        .database
        .ban_set(&set_name, added_by_user_id)
        .await?;
    
            request_context
                .bot
                .send_markdown(
                    request_context.user_id(),
                    Text::removed_set(),
                )
                .reply_markup(Keyboard::removed_set(request_context.is_admin(), set_name))
                .await?;
    request_context
        .bot
        .answer_callback_query(&q.id).await?;
    Ok(())
}

/// answers the user by editing the message
#[tracing::instrument(skip(request_context, q))]
async fn answer_callback_query(
    request_context: RequestContext,
    q: CallbackQuery,
    text: Option<Markdown>,
    keyboard: Option<InlineKeyboardMarkup>,
    notification: Option<String>,
) -> Result<(), BotError> {
    let bot = request_context.bot;
    if let Some(keyboard) = keyboard {
        if let Some(Message { id, chat, kind, .. }) = q.message {
            if let Some(text) = text {
                // if the message is a sticker
                if let MessageKind::Common(MessageCommon {
                    media_kind: MediaKind::Sticker(_sticker),
                    ..
                }) = kind
                {
                    // bot.edit_message_media(chat.id, id, sticker).reply_markup(keyboard).await?;
                    bot.edit_message_reply_markup(chat.id, id)
                        .reply_markup(keyboard)
                        .into_future()
                        .instrument(tracing::info_span!("telegram_bot_edit_message"))
                        .await?;
                } else {
                    bot.edit_message_markdown(chat.id, id, text)
                        .reply_markup(keyboard)
                        .link_preview_options(LinkPreviewOptions::new().is_disabled(true))
                        .into_future()
                        .instrument(tracing::info_span!("telegram_bot_edit_message"))
                        .await?;
                }
            } else {
                bot.edit_message_reply_markup(chat.id, id)
                    .reply_markup(keyboard)
                    .into_future()
                    .instrument(tracing::info_span!("telegram_bot_edit_message"))
                    .await?;
            }
            // } else if let Some(id) = q.inline_message_id { // TODO: remove, as I don't send inline
            // messages with buttons
            //     bot.edit_message_text_inline(id, text).reply_markup(keyboard).await?;
        }
    }

    let result = match notification {
        Some(text) => {
            bot.answer_callback_query(&q.id)
                .text(text)
                .show_alert(false)
                .into_future()
                .instrument(tracing::info_span!("telegram_bot_answer_callback_query"))
                .await
        }
        None => {
            bot.answer_callback_query(&q.id)
                .into_future()
                .instrument(tracing::info_span!("telegram_bot_answer_callback_query"))
                .await
        }
    };
    match result {
        Ok(_) => Ok(()),
        Err(err) if teloxide_error_can_safely_be_ignored(&err) => Ok(()),
        Err(err) => Err(err.into()),
    }
}

const EXAMPLES_REQUIRED: usize = 5;

async fn create_tag(state: &TagCreator, request_context: &RequestContext) -> Result<(), BotError> {
    // TODO: check if valid
    if state.example_sticker_id.len() < EXAMPLES_REQUIRED {
        return Err(UserError::ValidationError(format!(
            "You have to provide at least {EXAMPLES_REQUIRED} example stickers, just send them here"
        ))
        .into());
    }
    if state.aliases.contains(&state.tag_id) {
        return Err(UserError::ValidationError(
            "Aliases have to be distinct from the main tag".to_string(),
        )
        .into());
    }
    let category = match state.category {
        Some(Category::Artist) => {
            if state.linked_channel.is_none() {
                return Err(UserError::ValidationError(
                    "Artist tags require a linked channel (user is optional)".to_string(),
                )
                .into());
            }
            Category::Artist
        }
        Some(Category::Character) => {
            if state.linked_user.is_none() {
                return Err(UserError::ValidationError(
                    "Character tags require a linked user (channel is optional)".to_string(),
                )
                .into());
            }
            Category::Character
        }
        Some(category) => {
            return Err(InternalError::Other(anyhow::anyhow!(
                "invalid category {}",
                category.to_human_name()
            ))
            .into())
        }
        _ => {
            return Err(UserError::ValidationError(
                "Pick one of the categories: artist, character".to_string(),
            )
            .into())
        }
    };

    match request_context
        .database
        .create_pending_tag(
            &state.tag_id,
            category,
            &state.linked_channel,
            &state.linked_user,
            &state.example_sticker_id,
            &state.aliases,
            request_context.user.id,
        )
        .await
    {
        Err(crate::database::DatabaseError::UniqueConstraintViolated(message)) => {
            return Err(UserError::AlreadyExists("tag".to_string()).into())
        }
        other => other?,
    }

    Ok(())
}
