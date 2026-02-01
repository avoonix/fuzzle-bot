use std::str::FromStr;
use std::sync::Arc;

use crate::background_tasks::BackgroundTaskExt;
use crate::bot::{BotError, BotExt, InternalError, RequestContext, UserError};
use crate::callback::{exit_mode, sticker_explore_keyboard, TagOperation};

use crate::database::{
    ContinuousTag, Database, DialogState, Order, ReportReason, Sticker, TagCreator,
};
use crate::inline::{SetOperation, SimilarityAspect, TagKind};
use crate::message::message_handler::handle_readonly;
use crate::message::Keyboard;
use crate::qdrant::{StickerMatch, VectorDatabase};
use crate::simple_bot_api;
use crate::sticker::resolve_file_hashes_to_sticker_ids_and_clean_up_unreferenced_files;
use crate::tags::suggest_tags;
use crate::text::{Markdown, Text};
use crate::util::{
    create_sticker_set_id, create_tag_id, set_name_literal, sticker_id_literal, Required,
};
use futures::future::try_join_all;
use itertools::Itertools;
use nom::bytes::complete::take_while1;
use nom::character::complete::{i64, multispace1};
use nom::combinator::{eof, map, map_res};
use nom::sequence::tuple;
use nom::Finish;
use num_traits::FromPrimitive;
use rand::Rng;
use teloxide::dispatching::dialogue::GetChatId;
use teloxide::types::{
    ButtonRequest, InlineKeyboardButton, InlineKeyboardMarkup, InputFile, KeyboardButton,
    KeyboardButtonRequestChat, KeyboardButtonRequestUsers, KeyboardMarkup, ReplyMarkup,
};

use teloxide::utils::command::ParseError;
use teloxide::{prelude::*, utils::command::BotCommands};

use super::{send_sticker_with_tag_input, unescape_sticker_unique_id_from_command};

#[derive(Debug)]
pub struct TagList(Vec<String>);

impl FromStr for TagList {
    type Err = UserError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.split(",").map(|s| s.to_string()).collect_vec()))
    }
}

#[derive(BotCommands, Debug)]
#[command(rename_rule = "lowercase", description = "Hidden commands")]
pub enum HiddenCommand {
    #[command(
        description = "tag a sticker (do not use manually)",
        parse_with = "split"
    )]
    TagSticker {
        sticker_unique_id: String,
        tag: TagList,
    },
    #[command(description = "add a tag to the blacklist (do not use manually)")]
    BlacklistTag { tag: String },
    #[command(
        description = "tags all stickers in a set (do not use manually)",
        parse_with = "split"
    )]
    TagSet { set_name: String, tag: TagList },
    #[command(
        description = "untags all stickers in a set (do not use manually)",
        parse_with = "split"
    )]
    UntagSet { set_name: String, tag: TagList },
    #[command(description = "tags stickers continuously upon reply (do not use manually)")]
    TagContinuous { tag: TagList },
    #[command(description = "untags stickers continuously upon reply (do not use manually)")]
    UntagContinuous { tag: TagList },
    #[command(description = "cancel a pending operation (continuous tag mode)", aliases = ["x", "exit"], hide_aliases)]
    Cancel,
    #[command(description = "get a random sticker")]
    Random,
    #[command(
        description = "suggest a sticker in continuous tag mode to be tagged (do not use outside continuous tag mode)"
    )]
    AutoSuggest,
    #[command(
        description = "set the id of the tag to be created (do not use manually)",
        parse_with = "split"
    )]
    SetTag { kind: TagKind, tag_id: String },
    #[command(
        description = "create a new sticker set (do not use manually)",
        parse_with = new_sticker_set_custom_parser
    )]
    CreateSet {
        sticker_id: String,
        set_title: String,
    },
    #[command(
        description = "add sticker to existing set (do not use manually)",
        parse_with = "split"
    )]
    AddSticker { set_id: String, sticker_id: String },
    #[command(
        description = "remove sticker from existing set (do not use manually)",
        parse_with = "split"
    )]
    RemoveSticker { set_id: String, sticker_id: String },
    #[command(description = "show information about an user", parse_with = "split")]
    User { user_id: i64 },
    #[command(
        description = "report a sticker set (do not use manually)",
        parse_with = report_set_custom_parser
    )]
    ReportSet {
        reason: ReportReason,
        set_id: String,
    },
}

fn report_set_custom_parser(input: String) -> Result<(ReportReason, String), ParseError> {
    Ok(Finish::finish(map_res(
        tuple((i64, multispace1, set_name_literal)),
        |(reason, _, set_id)| {
            Ok::<_, anyhow::Error>((
                ReportReason::from_i64(reason).ok_or_else(|| anyhow::anyhow!("invalid reason"))?,
                set_id.to_string(),
            ))
        },
    )(&input))
    .map_err(|err| {
        ParseError::Custom(Box::new(UserError::ParseError(
            input.len() - err.input.len(),
            err.input.to_string(),
        )))
    })?
    .1)
}

fn new_sticker_set_custom_parser(input: String) -> Result<(String, String), ParseError> {
    Ok(Finish::finish(map(
        tuple((sticker_id_literal, multispace1, take_while1(|_| true), eof)),
        |(sticker_id, _, set_title, _)| (sticker_id.to_string(), set_title.to_string()),
    )(&input))
    .map_err(|err| {
        ParseError::Custom(Box::new(UserError::ParseError(
            input.len() - err.input.len(),
            err.input.to_string(),
        )))
    })?
    .1)
}

async fn set_editor_keyboard(
    request_context: RequestContext,
    sticker_id: &str,
) -> Result<InlineKeyboardMarkup, InternalError> {
    match request_context.dialog_state() {
        DialogState::StickerRecommender {
            positive_sticker_id,
            negative_sticker_id,
        } => {
            // TODO: refactor is_favorited
            let sticker_user = request_context
                .database
                .get_sticker_user(sticker_id, request_context.user.id)
                .await?;
            let is_favorited = sticker_user.map_or(false, |su| su.is_favorite);

            Ok(Keyboard::recommender(
                sticker_id,
                &positive_sticker_id,
                &negative_sticker_id,
                is_favorited,
            ))
        }
        _ => sticker_explore_keyboard(sticker_id.to_string(), request_context).await,
    }
}

impl HiddenCommand {
    #[tracing::instrument(skip(self, msg, request_context))]
    pub async fn execute(
        self,
        msg: Message,
        request_context: RequestContext,
    ) -> Result<(), BotError> {
        match self {
            Self::ReportSet { reason, set_id } => {
                if handle_readonly(&request_context, &msg).await? { return Ok(()); }

                request_context
                    .database
                    .create_moderation_task(
                        &crate::database::ModerationTaskDetails::ReportStickerSet {
                            set_id,
                            reason,
                        },
                        request_context.user.id,
                    )
                    .await?;

                request_context
                    .bot
                    .send_markdown(
                        request_context.user_id(),
                        Markdown::escaped(
                            "Success! An admin should review your report soon(ish) :3",
                        ),
                    )
                    .await?;
            }
            Self::User { user_id } => {
                let set_count = request_context
                    .database
                    .get_owned_sticker_set_count(user_id)
                    .await?;
                if set_count == 0 {
                    return Err(anyhow::anyhow!("user is not a set owner").into());
                    // TODO: user error?
                }
                let owner_username = request_context
                    .database
                    .get_username(crate::database::UsernameKind::User, user_id)
                    .await?;
                let owner_tags = request_context
                    .database
                    .get_all_tags_by_linked_user_id(user_id)
                    .await?;
                let channel_usernames = request_context
                    .database
                    .get_usernames(
                        crate::database::UsernameKind::Channel,
                        owner_tags
                            .iter()
                            .filter_map(|tag| tag.linked_channel_id)
                            .collect_vec(),
                    )
                    .await?;
                let keyboard = Keyboard::owner_standalone(
                    Some(user_id),
                    set_count,
                    owner_username,
                    owner_tags,
                    channel_usernames,
                );

                request_context
                    .bot
                    .send_markdown(msg.chat.id, Markdown::escaped("User Info"))
                    .reply_markup(keyboard)
                    .reply_to_message_id(msg.id)
                    .allow_sending_without_reply(true)
                    .await?;
            }
            Self::RemoveSticker { set_id, sticker_id } => {
                let sticker = request_context
                    .database
                    .get_sticker_by_id(&sticker_id)
                    .await?
                    .required()?;
                let tags = request_context
                    .database
                    .get_sticker_tags(&sticker_id)
                    .await?
                    .into_iter()
                    .take(20)
                    .collect_vec();

                request_context
                    .bot
                    .delete_sticker_from_set(&sticker.telegram_file_identifier)
                    .await?;
                request_context.database.delete_sticker(&sticker_id).await?;

                request_context
                    .bot
                    .send_markdown(
                        msg.chat.id,
                        // TODO: proper text
                        Markdown::escaped(format!(
                            "Removed the sticker from the set: https://t.me/addstickers/{}\n\nThe set should update within an hour",
                            set_id
                        )),
                    )
                    .await?;
                if let Some(sticker) = request_context
                    .database
                    .get_some_sticker_by_file_id(&sticker.sticker_file_id)
                    .await?
                {
                    // sticker is deleted; find another equivalent sticker
                    request_context
                        .bot
                        .send_sticker(
                            msg.chat.id,
                            InputFile::file_id(sticker.telegram_file_identifier),
                        )
                        .reply_markup(set_editor_keyboard(request_context, &sticker.id).await?)
                        .reply_to_message_id(msg.id)
                        .allow_sending_without_reply(true)
                        .await?;
                }
            }
            Self::AddSticker { set_id, sticker_id } => {
                let sticker = request_context
                    .database
                    .get_sticker_by_id(&sticker_id)
                    .await?
                    .required()?;
                let tags = request_context
                    .database
                    .get_sticker_tags(&sticker_id)
                    .await?
                    .into_iter()
                    .take(20)
                    .collect_vec();

                simple_bot_api::add_sticker_to_set(
                    &request_context.config.telegram_bot_token,
                    request_context.user_id(),
                    &set_id,
                    &sticker.telegram_file_identifier,
                    "static", // TODO: store format?
                    &[sticker.emoji.unwrap_or("😊".to_string())],
                    &tags,
                )
                .await?;

                request_context.importer.queue_sticker_set_import(&set_id, true, Some(request_context.user_id())).await;

                request_context
                    .bot
                    .send_markdown(
                        msg.chat.id,
                        // TODO: proper text
                        Markdown::escaped(format!(
                            "Added the sticker to set: https://t.me/addstickers/{}\n\nThe set should update within an hour",
                            set_id
                        )),
                    )
                    .await?;
                request_context
                    .bot
                    .send_sticker(
                        msg.chat.id,
                        InputFile::file_id(sticker.telegram_file_identifier),
                    )
                    .reply_markup(set_editor_keyboard(request_context, &sticker.id).await?)
                    .reply_to_message_id(msg.id)
                    .allow_sending_without_reply(true)
                    .await?;
            }
            Self::CreateSet {
                sticker_id,
                set_title,
            } => {
                let set_id = create_sticker_set_id(
                    &set_title,
                    &request_context.config.telegram_bot_username,
                );

                let sticker = request_context
                    .database
                    .get_sticker_by_id(&sticker_id)
                    .await?
                    .required()?;
                let tags = request_context
                    .database
                    .get_sticker_tags(&sticker_id)
                    .await?
                    .into_iter()
                    .take(20)
                    .collect_vec();
                let sticker_file = request_context
                    .database
                    .get_sticker_file_by_sticker_id(&sticker_id)
                    .await?
                    .required()?;

                simple_bot_api::create_new_sticker_set(
                    &request_context.config.telegram_bot_token,
                    request_context.user_id(),
                    &set_id,
                    &set_title,
                    &sticker.telegram_file_identifier,
                    match sticker_file.sticker_type {
                        crate::database::StickerType::Animated => "animated",
                        crate::database::StickerType::Video => "video",
                        crate::database::StickerType::Static => "static",
                    },
                    &[sticker.emoji.unwrap_or("😊".to_string())],
                    &tags,
                )
                .await?;

                request_context
                    .database
                    .upsert_sticker_set_with_title(
                        &set_id,
                        &set_title,
                        Some(request_context.user.id),
                    )
                    .await?;
                request_context
                    .database
                    .upsert_sticker_set_with_creator(
                        &set_id,
                        request_context.user.id,
                        Some(request_context.user.id),
                    )
                    .await?;

                request_context.importer.queue_sticker_set_import(&set_id, true, Some(request_context.user_id())).await;

                request_context
                    .bot
                    .send_markdown(
                        msg.chat.id,
                        // TODO: proper text
                        Markdown::escaped(format!(
                            "Created sticker set: https://t.me/addstickers/{}",
                            set_id
                        )),
                    )
                    .await?;
                request_context
                    .bot
                    .send_sticker(
                        msg.chat.id,
                        InputFile::file_id(sticker.telegram_file_identifier),
                    )
                    .reply_markup(set_editor_keyboard(request_context, &sticker.id).await?)
                    .reply_to_message_id(msg.id)
                    .allow_sending_without_reply(true)
                    .await?;
            }
            Self::Random => {
                let request_context = if request_context.is_continuous_tag_state() {
                    exit_mode(request_context.clone(), false).await?
                } else {
                    request_context
                };
                let sticker = request_context
                    .database
                    .get_random_sticker_to_tag()
                    .await?
                    .required()?;
                send_sticker_with_tag_input(sticker, request_context.clone(), msg.chat.id, msg.id)
                    .await?;
            }
            Self::AutoSuggest => match request_context.dialog_state() {
                DialogState::ContinuousTag(ct) => {
                    suggest_sticker_continuous_tag(ct, &request_context, &msg).await?
                }
                DialogState::StickerRecommender {
                    positive_sticker_id,
                    negative_sticker_id,
                } => {
                    suggest_sticker_recommender(
                        positive_sticker_id,
                        negative_sticker_id,
                        &request_context,
                        &msg,
                    )
                    .await?
                }
                DialogState::Normal | DialogState::TagCreator(..) => {
                    return Err(UserError::InvalidMode.into())
                }
            },
            Self::TagSticker {
                sticker_unique_id,
                tag,
            } => {
                if handle_readonly(&request_context, &msg).await? { return Ok(()); }
                if !request_context.can_tag_stickers() {
                    return Err(anyhow::anyhow!(
                        "user is not permitted to tag stickers (hidden command)"
                    ))?;
                }
                let sticker = request_context
                    .database
                    .get_sticker_by_id(&sticker_unique_id)
                    .await?;
                if let Some(sticker) = sticker {
                    if let Some(tags) =
                        get_tag_implications_including_self(&tag.0, request_context.clone()).await?
                    {
                        request_context
                            .database
                            .tag_file(
                                &sticker.sticker_file_id,
                                &tags,
                                Some(request_context.user.id),
                            )
                            .await?;
                        request_context.tfidf.request_recompute().await;
                    }
                    let tags = request_context
                        .database
                        .get_sticker_tags(&sticker_unique_id)
                        .await?;
                    let suggested_tags = suggest_tags(
                        &sticker_unique_id,
                        request_context.bot.clone(),
                        request_context.tag_manager.clone(),
                        request_context.database.clone(),
                        request_context.tfidf.clone(),
                        request_context.vector_db.clone(),
                        // request_context.tag_worker.clone(),
                    )
                    .await?;
                    let is_locked = request_context
                        .database
                        .get_sticker_file_by_sticker_id(&sticker_unique_id)
                        .await?
                        .map_or(false, |file| file.tags_locked_by_user_id.is_some());
                    request_context
                        .bot
                        .send_sticker(
                            msg.chat.id,
                            InputFile::file_id(sticker.telegram_file_identifier),
                        ) // TODO: fetch
                        // file id
                        // from
                        // database
                        .reply_markup(Keyboard::tagging(
                            &tags,
                            &sticker_unique_id,
                            &suggested_tags,
                            is_locked,
                            request_context.is_continuous_tag_state(),
                            request_context.tag_manager.clone(),
                        )?)
                        .await?;
                } else {
                    request_context
                        .bot
                        .send_markdown(
                            msg.chat.id,
                            Markdown::escaped(format!(
                                "Could not find sticker `{}`",
                                sticker_unique_id.clone()
                            )),
                        )
                        .await?;
                }
            }
            Self::BlacklistTag { tag } => {
                let blacklist = request_context
                    .user
                    .blacklist
                    .clone()
                    .into_inner()
                    .into_iter()
                    .filter(|t| t != &tag)
                    .chain(std::iter::once(tag.clone()))
                    .collect_vec();
                request_context
                    .database
                    .update_user_blacklist(request_context.user.id, blacklist.clone().into())
                    .await?;
                request_context
                    .bot
                    .send_markdown(msg.chat.id, Text::blacklist())
                    .reply_markup(Keyboard::blacklist(&blacklist))
                    .await?;
            }
            Self::TagSet { set_name, tag } => {
                if handle_readonly(&request_context, &msg).await? { return Ok(()); }
                set_tag_operation(tag.0, set_name, SetOperation::Tag, msg, request_context).await?;
            }
            Self::UntagSet { set_name, tag } => {
                if handle_readonly(&request_context, &msg).await? { return Ok(()); }
                set_tag_operation(tag.0, set_name, SetOperation::Untag, msg, request_context)
                    .await?;
            }
            Self::TagContinuous { tag } => {
                if handle_readonly(&request_context, &msg).await? { return Ok(()); }
                modify_continuous_tag(tag.0, vec![], request_context, msg).await?;
            }
            Self::UntagContinuous { tag } => {
                if handle_readonly(&request_context, &msg).await? { return Ok(()); }
                modify_continuous_tag(vec![], tag.0, request_context, msg).await?;
            }
            Self::SetTag { tag_id, kind } => {
                if handle_readonly(&request_context, &msg).await? { return Ok(()); }
                set_tag_id(request_context, msg.chat.id, tag_id, kind, None).await?;
            }
            Self::Cancel => {
                exit_mode(request_context.clone(), true).await?;
            }
        }

        Ok(())
    }
}

async fn suggest_sticker(
    user_id: i64,
    tag_state: &ContinuousTag,
    database: Database,
    vector_db: VectorDatabase,
) -> Result<Sticker, BotError> {
    // TODO: also use selected_tags + excluded_tags in a vector_db query directly
    let seed = { rand::thread_rng().gen() };

    // TODO: limit stickers to only ones tagged by the current user?
    // TODO: also use excluded tags to find stickers and exclude those
    let stickers_already_tagged = database
        .get_stickers_for_tag_query(
            tag_state.add_tags.clone(),
            tag_state.remove_tags.clone(),
            vec![],
            50,
            0,
            Order::Random { seed },
        )
        .await?;
    if stickers_already_tagged.is_empty() {
        return Err(UserError::NoSuitableStickerFound.into());
    }
    let sticker_file_ids = stickers_already_tagged
        .into_iter()
        .map(|sticker| sticker.sticker_file_id)
        .collect_vec();
    // last 30 elements as negative examples, only if they are not tagged with all add_tags tags
    let negative_examples = try_join_all(
        tag_state
            .already_recommended_sticker_file_ids
            .clone()
            .into_iter()
            .rev()
            .take(30)
            .map(|file_id| {
                let database = &database;
                let tag_state = &tag_state;
                async move {
                    // TODO: single query instead of 30 separate ones
                    let sticker_tags = database.get_sticker_tags_by_file_id(&file_id).await?;
                    for add_tag in &tag_state.add_tags {
                        if !sticker_tags.contains(add_tag) {
                            return Ok::<_, InternalError>(Some(file_id));
                        }
                    }
                    Ok::<_, InternalError>(None)
                }
            }),
    )
    .await?
    .into_iter()
    .filter_map(|tag| tag)
    .collect_vec();
    let stickers_similar_to_already_tagged = vector_db
        .find_similar_stickers(
            &sticker_file_ids,
            &negative_examples,
            crate::inline::SimilarityAspect::Embedding,
            0.0,
            100,
            0,
        )
        .await?;
    let stickers_similar_to_already_tagged = stickers_similar_to_already_tagged.unwrap_or_default();

    let sticker = pick_sticker(
        database.clone(),
        &stickers_similar_to_already_tagged,
        &tag_state.add_tags,
        &tag_state.already_recommended_sticker_file_ids,
    )
    .await?;

    let Some(sticker) = sticker else {
        return Err(UserError::NoSuitableStickerFound.into());
    };

    Ok(sticker)
}

async fn pick_sticker(
    database: Database,
    stickers_similar_to_already_tagged: &[StickerMatch],
    selected_tags: &[String],
    already_recommended_file_ids: &[String],
) -> Result<Option<Sticker>, InternalError> {
    for StickerMatch { file_hash, .. } in stickers_similar_to_already_tagged {
        if already_recommended_file_ids.contains(file_hash) {
            continue;
        }
        let tags = database.get_sticker_tags_by_file_id(&file_hash).await?;
        // returns the sticker if at least one selected tag is not present
        for selected_tag in selected_tags {
            if !tags.contains(selected_tag) {
                let sticker = database.get_some_sticker_by_file_id(&file_hash).await?;
                if let Some(sticker) = sticker {
                    return Ok(Some(sticker));
                }
            }
        }
    }
    return Ok(None);
}

async fn get_tag_implications_including_self(
    tags: &[String],
    request_context: RequestContext,
) -> Result<Option<Vec<String>>, InternalError> {
    // TODO: change return type?
    let tags = tags
        .into_iter()
        .map(|tag| {
            request_context
                .tag_manager
                .get_implications_including_self(&tag)
        })
        .flatten()
        .flatten()
        .sorted()
        .dedup()
        .collect_vec();
    Ok(if tags.is_empty() { None } else { Some(tags) })
}

fn add_new_tags_to_continuous_tag(
    current: Vec<String>,
    added: Vec<String>,
    fixed_tags_from_other_section: &[String],
) -> Vec<String> {
    current
        .into_iter()
        .chain(added)
        .filter(|tag| !fixed_tags_from_other_section.contains(tag))
        .sorted()
        .dedup()
        .collect_vec()
}

async fn modify_continuous_tag(
    new_add_tags: Vec<String>,
    new_remove_tags: Vec<String>,
    request_context: RequestContext,
    msg: Message,
) -> Result<(), BotError> {
    let new_add_tags = new_add_tags
        .into_iter()
        .map(|tag| {
            request_context
                .tag_manager
                .get_implications_including_self(&tag)
                .unwrap_or_default()
        })
        .flatten()
        .collect_vec();
    let new_remove_tags = new_remove_tags
        .into_iter()
        .map(|tag| {
            request_context
                .tag_manager
                .get_implications_including_self(&tag)
                .unwrap_or_default()
        })
        .flatten()
        .collect_vec();
    let (state_changed, continuous_tag) = match request_context.dialog_state() {
        DialogState::ContinuousTag(ct) => (false, ct),
        _ => (true, Default::default()),
    };

    let remove_tags =
        add_new_tags_to_continuous_tag(continuous_tag.remove_tags, new_remove_tags, &new_add_tags);
    let add_tags =
        add_new_tags_to_continuous_tag(continuous_tag.add_tags, new_add_tags, &remove_tags);

    let new_dialog_state = DialogState::ContinuousTag(crate::database::ContinuousTag {
        add_tags: add_tags.clone(),
        remove_tags: remove_tags.clone(),
        already_recommended_sticker_file_ids: continuous_tag.already_recommended_sticker_file_ids,
    });
    request_context
        .database
        .update_dialog_state(request_context.user.id, &new_dialog_state)
        .await?;
    if state_changed {
        let message = request_context.bot.send_markdown(
                    msg.chat.id,
                    Markdown::escaped("You are now in continuous tag mode. Send stickers to tag them with all selected tags")
                )
                .reply_markup(
                    ReplyMarkup::Keyboard(KeyboardMarkup::new(vec![
                        vec![
                           KeyboardButton::new("/continuoustagmode"),
                        ],
                        vec![
                           KeyboardButton::new("/autosuggest"),
                        ],
                        vec![
                           KeyboardButton::new("/cancel"),
                        ]
                    ])
                    .resize_keyboard()
                    .persistent()
                .input_field_placeholder("Continuous Tag Mode"))
            )
                .await?;
        // request_context.bot.pin_chat_message(msg.chat.id, message.id).await?; // TODO: pin does not work until the user leaves and opens the chat??
    }
    let message = request_context
        .bot
        .send_markdown(
            msg.chat.id,
            Text::get_continuous_tag_mode_text(add_tags.as_slice(), remove_tags.as_slice()),
        )
        .reply_markup(Keyboard::make_continuous_tag_keyboard(
            true,
            add_tags.as_slice(),
            remove_tags.as_slice(),
        ))
        .await?;
    Ok(())
}

async fn set_tag_operation(
    tags: Vec<String>,
    set_name: String,
    operation: SetOperation,
    msg: Message,
    request_context: RequestContext,
) -> Result<(), BotError> {
    if !request_context.can_tag_sets() {
        return Err(UserError::NoPermissionForAction("tag set".to_string()).into());
    }
    let message = match operation {
        SetOperation::Tag => {
            if let Some(tags) =
                get_tag_implications_including_self(&tags, request_context.clone()).await?
            {
                let taggings_changed = request_context
                    .database
                    .tag_all_files_in_set(&set_name, tags.as_ref(), request_context.user.id)
                    .await?;
                request_context.tfidf.request_recompute().await;
                Text::tagged_set(&set_name, &tags, taggings_changed)
            } else {
                Markdown::escaped("No tags changed".to_string())
            }
        }
        SetOperation::Untag => {
            let taggings_changed = request_context
                .database
                .untag_all_files_in_set(&set_name, &tags, request_context.user.id)
                .await?;
            Text::untagged_set(&set_name, &tags, taggings_changed)
        }
    };
    request_context
        .bot
        .send_markdown(
            msg.chat.id,
            message, // TODO: do not escape the `` -> tags should be inline code
        )
        .reply_to_message_id(msg.id)
        .allow_sending_without_reply(true)
        .reply_markup(Keyboard::make_set_keyboard(set_name))
        .await?;
    Ok(())
}

pub async fn set_tag_id(
    request_context: RequestContext,
    chat_id: ChatId,
    tag_id: String,
    kind: TagKind,
    linked_user_id: Option<i64>,
) -> Result<(), BotError> {
    let tag_id = create_tag_id(&tag_id);
    // TODO: validate with parser (and trim whitespace before)
    let (state_changed, mut tag_creator) = match request_context.dialog_state() {
        DialogState::TagCreator(tag_creator) => (false, (tag_creator)),
        _ => (
            true,
            (TagCreator {
                tag_id: tag_id.clone(),
                ..Default::default()
            }),
        ),
    };

    match kind {
        TagKind::Alias => {
            if !tag_creator.aliases.contains(&tag_id) {
                tag_creator.aliases.push(tag_id);
            }
        }
        TagKind::Main => tag_creator.tag_id = tag_id,
    }

    if let Some(linked_user_id) = linked_user_id {
        let username = request_context
            .database
            .get_username(crate::database::UsernameKind::User, linked_user_id)
            .await?;
        if let Some(username) = username {
            tag_creator.linked_user = Some(linked_user_id);
        }
        // TODO: error if no username found?
    }

    let new_dialog_state = DialogState::TagCreator(tag_creator.clone());
    request_context
        .database
        .update_dialog_state(request_context.user.id, &new_dialog_state)
        .await?;
    if state_changed {
        let message = request_context
            .bot
            .send_markdown(
                chat_id,
                Markdown::escaped("You are now in the tag creator."),
            )
            .reply_markup(ReplyMarkup::Keyboard(
                KeyboardMarkup::new(vec![
                    vec![KeyboardButton::new("/tagcreator")],
                    vec![
                        KeyboardButton::new("Link channel").request(ButtonRequest::RequestChat(
                            KeyboardButtonRequestChat::new(1, true)
                                .chat_has_username(true)
                                .request_username(true),
                        )),
                        KeyboardButton::new("Link user").request(ButtonRequest::RequestUser(
                            KeyboardButtonRequestUsers::new(2)
                                .user_is_bot(false)
                                .request_username(true),
                        )),
                    ],
                    vec![KeyboardButton::new("/cancel")],
                ])
                .resize_keyboard()
                .persistent()
                .input_field_placeholder("Tag Creator"),
            ))
            .await?;
    }
    let message = request_context
        .bot
        .send_markdown(
            chat_id,
            Markdown::escaped("tag creator text"), // TODO: better text, from Text struct
        )
        .reply_markup(Keyboard::tag_creator(&tag_creator))
        .await?;
    Ok(())
}

async fn suggest_sticker_continuous_tag(
    continuous_tag: ContinuousTag,
    request_context: &RequestContext,
    msg: &Message,
) -> Result<(), BotError> {
    let sticker = suggest_sticker(
        request_context.user.id,
        &continuous_tag,
        request_context.database.clone(),
        request_context.vector_db.clone(),
    )
    .await?;

    let new_dialog_state = DialogState::ContinuousTag(ContinuousTag {
        add_tags: continuous_tag.add_tags,
        remove_tags: continuous_tag.remove_tags,
        already_recommended_sticker_file_ids: continuous_tag
            .already_recommended_sticker_file_ids
            .into_iter()
            .chain(std::iter::once(sticker.sticker_file_id))
            .collect_vec(),
    });
    request_context
        .database
        .update_dialog_state(request_context.user.id, &new_dialog_state)
        .await?;

    request_context
        .bot
        .send_sticker(
            msg.chat.id,
            InputFile::file_id(sticker.telegram_file_identifier),
        )
        .reply_markup(Keyboard::continuous_tag_confirm(&sticker.id))
        .reply_to_message_id(msg.id)
        .allow_sending_without_reply(true)
        .await?;
    Ok(())
}

async fn suggest_sticker_recommender(
    positive: Vec<String>,
    negative: Vec<String>,
    request_context: &RequestContext,
    msg: &Message,
) -> Result<(), BotError> {
    let positive_file_ids = request_context
        .database
        .get_sticker_file_ids_by_sticker_id(&positive)
        .await?;
    let mut negative_file_ids = request_context
        .database
        .get_sticker_file_ids_by_sticker_id(&negative)
        .await?;
    let recommended_file_hashes = request_context
        .vector_db
        .find_similar_stickers(
            &positive_file_ids,
            &negative_file_ids,
            SimilarityAspect::Embedding,
            0.0,
            50,
            0,
        )
        .await?
        .required()?;
    let recommended = resolve_file_hashes_to_sticker_ids_and_clean_up_unreferenced_files(
        request_context.database.clone(),
        request_context.vector_db.clone(),
        recommended_file_hashes,
    )
    .await?;

    let sticker_ids = recommended.into_iter().map(|m| m.sticker_id).collect_vec();
    for id in sticker_ids {
        if let Some(sticker) = request_context.database.get_sticker_by_id(&id).await? {
            send_sticker_with_tag_input(sticker, request_context.clone(), msg.chat.id, msg.id)
                .await?;
            return Ok(());
        }
    }

    return Err(UserError::NoSuitableStickerFound.into());
}
