use actix_web::web::Data;
use actix_web::web::{route, Form};
use actix_web::{get, post, App, HttpRequest, HttpResponse, HttpServer, Responder, Result as ActixResult};
use actix_web_lab::extract::Path;
use itertools::Itertools;
use maud::{html, Markup, PreEscaped, DOCTYPE};
use nom::combinator::eof;
use nom::sequence::terminated;
use nom::Finish;
use serde::Deserialize;

use crate::background_tasks::FindTags;
use crate::bot::UserError;
use crate::database::{Order, StickerSet};
use crate::inline::{
    get_last_input_match_list_and_other_input_closest_matches, parse_comma_separated_tags,
};
use crate::sticker::resolve_file_hashes_to_sticker_ids_and_clean_up_unreferenced_files;
use crate::util::Required;
use crate::web::server::AppState;

#[get("/")]
pub async fn index(req: HttpRequest, data: Data<AppState>) -> ActixResult<Markup> {
    let host = format!("{}", req.uri());
    let title = "fuzzle bot";
    let desc = "Hi there";
    let lang = "en";
    let stats = data.database.get_stats().await?;

    let content = html! {
        h1 {
            "FuzzleBot"
        }
        div {
            div {
                "Sets " (stats.sets)
            }
            div {
                "Stickers " (stats.stickers)
            }
            div {
                "Tagged Stickers " (stats.tagged_stickers)
            }
            div {
                "Taggings " (stats.taggings)
            }
        }

        h3 {
            "search tags"
            span class="htmx-indicator" {
                "Searching ..."
            }
        }
        input type="search"
       name="name" placeholder="Search Tags ..."
       hx-post="/search-tags"
       hx-trigger="input changed delay:300ms, search"
       hx-target="#content"
       hx-indicator=".htmx-indicator";

        #content {

        }
    };
    Ok(page(&host, title, desc, lang, content))
}

#[post("/search-tags")]
pub async fn search_tags(user_input: Form<SearchTagsForm>, data: Data<AppState>) -> ActixResult<Markup> {
    let suggested_tags = data
        .tag_manager
        .execute(FindTags::new(
            user_input
                .name
                .split(" ")
                .map(|s| s.to_string())
                .collect_vec(),
        ))
        .await?
        .into_iter()
        .take(20)
        .collect_vec();

    Ok(html! {
        #content {

                div class="tag-container" {
                    @for tag in &suggested_tags {
                        a class="tag" href={ "/tag/" (tag) } {
                            (tag)
                        }
                    }
                }
        }
    })
}

pub fn sticker_list_item(sticker_id: &str) -> Markup {
    html! {
                    div {
                        a href={ "/sticker/" (sticker_id) } {
                            img class="sticker-thumbnail" src={ "/files/stickers/" (sticker_id) "/thumbnail.png" };
                        }
                     }
    }
}

pub fn sticker_set_list_item(set_id: &str) -> Markup {
    html! {
                        a class="set-grid-item" href={ "/set/" (set_id) } {
                            div class="set-thumbnail" {
                                img src={ "/thumbnails/sticker-set/" (set_id) "/image.png" };
                            }
                            div class="set-name" {
                                h4 {
                                    (set_id)
                                }
                                small {
                                    "Sticker Set" // TODO: number of stickers
                                }
                            }
                        }
    }
}

#[get("/set/{setId}")]
async fn sticker_set(
    Path(set_id): Path<String>,
    data: Data<AppState>,
    req: HttpRequest,
) -> actix_web::Result<impl Responder> {
    let set = data
        .database
        .get_sticker_set_by_id(&set_id)
        .await?
        .required()?;
    set.last_fetched.required()?; // TODO: better error
    let stickers = data.database.get_all_stickers_in_set(&set_id).await?;
    let overlapping_sets = data.database.get_overlapping_sets(&set_id).await?.into_iter().take(20).collect_vec();
    let owner_sets = data
        .database
        .get_owned_sticker_sets(set.created_by_user_id.required()?, 20, 0)
        .await?;
    let tags = data
        .database
        .get_all_sticker_set_tag_counts(&set_id)
        .await?;

    let host = format!("{}", req.uri());
    let title = "fuzzle bot";
    let desc = "Hi there";
    let lang = "en";
    let set_title = set.title.unwrap_or_else(|| set.id.clone());

    let content = html! {
        #content {
            h1 {
                (set_title)
            }

            "Tags:"
                div class="tag-container" {
                    @for tag in &tags {
                        a class="tag" href={ "/tag/" (tag.0) } {
                            (tag.0)
                            div class="tag-counter" {
                                (tag.1)
                            }
                        }
                    }
                }

            div class="grid" {
                @for sticker in &stickers {
                    (sticker_list_item(&sticker.id))
                }
            }
                h2 {
            "Set Overlaps"
                }

            div class="set-grid" {
                @for set in &overlapping_sets {
                    "Overlap: " (set.1)
                    (sticker_set_list_item(&set.0))
                }
            }

                h2 {
            "Same Owner"
                }
            div class="set-grid" {
                @for set in &owner_sets {
                    (sticker_set_list_item(&set.id))
                }
            }

        }
    };

    Ok(page(&host, title, desc, lang, content))
}

#[get("/sticker/{stickerId}")]
async fn sticker_page(
    Path(sticker_id): Path<String>,
    data: Data<AppState>,
    req: HttpRequest,
) -> actix_web::Result<impl Responder> {
    let set = data
        .database
        .get_sticker_set_by_sticker_id(&sticker_id)
        .await?
        .required()?;
    let sticker = data
        .database
        .get_sticker_by_id(&sticker_id)
        .await?
        .required()?;
    let other_sets = data
        .database
        .get_sets_containing_file(&sticker.sticker_file_id)
        .await?
        .into_iter()
        .take(20)
        .collect_vec();
    let file = data
        .database
        .get_sticker_file_by_sticker_id(&sticker_id)
        .await?
        .required()?;
    let set_stickers = data.database.get_all_stickers_in_set(&set.id).await?;
    let tags = data.database.get_sticker_tags_by_file_id(&file.id).await?;
    let sticker_type = match file.sticker_type {
        crate::database::StickerType::Animated => "animated",
        crate::database::StickerType::Video => "video",
        crate::database::StickerType::Static => "static",
    };

    let similar_color = {
        let file_hashes = data
            .vector_db
            .find_similar_stickers(
                &[sticker.sticker_file_id.clone()],
                &[],
                crate::inline::SimilarityAspect::Color,
                0.0,
                20,
                0,
            )
            .await?
            .required()?;
        let result = resolve_file_hashes_to_sticker_ids_and_clean_up_unreferenced_files(
            data.database.clone(),
            data.vector_db.clone(),
            file_hashes,
        )
        .await?;

        let sticker_ids = result.into_iter().map(|m| m.sticker_id).collect_vec();
        let mut stickers = Vec::new();
        for id in sticker_ids {
            if let Some(sticker) = data.database.get_sticker_by_id(&id).await? {
                stickers.push(sticker);
            }
            // TODO: single query?
        }
        stickers
    };
    let similar_embedding = {
        let file_hashes = data
            .vector_db
            .find_similar_stickers(
                &[sticker.sticker_file_id.clone()],
                &[],
                crate::inline::SimilarityAspect::Embedding,
                0.0,
                20,
                0,
            )
            .await?
            .required()?;
        let result = resolve_file_hashes_to_sticker_ids_and_clean_up_unreferenced_files(
            data.database.clone(),
            data.vector_db.clone(),
            file_hashes,
        )
        .await?;

        let sticker_ids = result.into_iter().map(|m| m.sticker_id).collect_vec();
        let mut stickers = Vec::new();
        for id in sticker_ids {
            if let Some(sticker) = data.database.get_sticker_by_id(&id).await? {
                stickers.push(sticker);
            }
            // TODO: single query?
        }
        stickers
    };

    let host = format!("{}", req.uri());
    let title = "fuzzle bot";
    let desc = "Hi there";
    let lang = "en";
    let set_title = set.title.unwrap_or_else(|| set.id.clone());

    let content = html! {
            #content {
                div class="full-width" {

                div class="sticker-container" {
                    img class="big-sticker" src={ "/files/stickers/" (sticker.id) "/thumbnail.png" };
                    div class="sticker-information" {

                    "Tags: "
                    div class="tag-container" {
                        @for tag in &tags {
                            a class="tag" href={ "/tag/" (tag) } {
                                (tag)
                            }
                        }
                    }
                    h1 {
                        "Sticker from set " (set_title)
                    }
                                a href={ "/set/" (set.id) } {
                                    "open set"
                                }
                                "Emoji" (sticker.emoji.unwrap_or_default())
                                "Animated" (sticker_type)
                    }
                }
                }


                h2 {

                            "Same Set Stickers"

                }
                div class="grid" {
                    @for sticker in &set_stickers {
                        (sticker_list_item(&sticker.id))
                    }
                }

                h2 {
                            "Similar Color"
                }

                div class="grid" {
                    @for sticker in &similar_color {
                        (sticker_list_item(&sticker.id))
                    }
                }

                h2 {
                            "Similar Embedding"
                }

                div class="grid" {
                    @for sticker in &similar_embedding {
                        (sticker_list_item(&sticker.id))
                    }
                }

                            div {
                                "other sets"
                div class="set-grid" {
                    @for set in &other_sets {
                        (sticker_set_list_item(&set.id))
                    }
                }
                            }
            }
        };

    Ok(page(&host, title, desc, lang, content))
}

pub async fn not_found() -> impl Responder {
    (
        html! {
            html lang="en" {
                head {
                    meta charset=(strings::UTF8);
                    title { (strings::NOT_FOUND_TITLE) }
                    meta name=(strings::VIEWPORT) content=(strings::VIEWPORT_CONTENT);
                    style { (strings::NOT_FOUND_STYLE) }
                }
                body {
                    h1 { (strings::NOT_FOUND_TITLE) }
                    p { (strings::NOT_FOUND_MESSAGE) }
                }
                (PreEscaped(strings::NOT_FOUND_COMMENT))
            }
        },
        actix_web::http::StatusCode::NOT_FOUND,
    )
}

#[derive(Deserialize)]
struct TagForm {
    query: String,
    sticker_id: String,
}

#[derive(Deserialize)]
struct SearchTagsForm {
    name: String,
}

fn body(content: Markup) -> Markup {
    html! {
        body {
            (content)
            script src="/assets/js/vendor/htmx.min.js" {}
            script src="/assets/js/main.js" {}
        }
    }
}

fn head(title: &str, desc: &str, url: &str) -> Markup {
    html! {
        head {
            meta charset=(strings::UTF8);
            title { (title) }
            meta name=(strings::DESCRIPTION) content=(desc);
            meta name=(strings::VIEWPORT) content=(strings::VIEWPORT_CONTENT);
            meta property="og:title" content=(title);
            meta property="og:type" content=(strings::WEBSITE);
            meta property="og:url" content=(url);
            meta property="og:image" content="";
            link rel="manifest" href="site.webmanifest";
            link rel="apple-touch-icon" href="icon.png";
            link rel="stylesheet" href="/assets/css/normalize.css";
            link rel="stylesheet" href="/assets/css/main.css";
            meta name="theme-color" content="#fafafa";
            meta name="robots" content="noindex"; // TODO: remove
        }
    }
}

pub fn page(host: &str, title: &str, desc: &str, lang: &str, content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html class="no-js" lang=(lang) {
            (head(title, desc, host))
            (body(content))
        }
    }
}

mod strings {

    pub static DESCRIPTION: &str = "description";
    pub static NOT_FOUND_COMMENT: &str = "<!-- IE needs 512+ bytes: https://docs.microsoft.com/archive/blogs/ieinternals/friendly-http-error-pages -->";
    pub static NOT_FOUND_MESSAGE: &str =
        "Sorry, but the page you were trying to view does not exist.";
    pub static NOT_FOUND_STYLE: &str = "
            * {
                line-height: 1.2;
                margin: 0;
            }

            html {
                color: #888;
                display: table;
                font-family: sans-serif;
                height: 100%;
                text-align: center;
                width: 100%;
            }

            body {
                display: table-cell;
                vertical-align: middle;
                margin: 2em auto;
            }

            h1 {
                color: #555;
                font-size: 2em;
                font-weight: 400;
            }

            p {
                margin: 0 auto;
                width: 280px;
            }

            @media only screen and (max-width: 280px) {

            body,
            p {
                width: 95%;
            }

            h1 {
                font-size: 1.5em;
                margin: 0 0 0.3em;
            }

            }";
    pub static NOT_FOUND_TITLE: &str = "Page Not Found";
    pub static UTF8: &str = "utf-8";
    pub static VIEWPORT: &str = "viewport";
    pub static VIEWPORT_CONTENT: &str = "width=device-width, initial-scale=1";
    pub static WEBSITE: &str = "website";
}

#[get("/tag/{tagId}")]
async fn tag_page(
    Path(tag_id): Path<String>,
    data: Data<AppState>,
    req: HttpRequest,
) -> actix_web::Result<impl Responder> {
    let sets = data
        .database
        .get_sticker_sets_for_tag_query(vec![tag_id.clone()], vec![], 100, 0)
        .await?; // TODO: use default blacklist
    let stickers = data
        .database
        .get_stickers_for_tag_query(
            vec![tag_id.clone()],
            vec![],
            vec![],
            100,
            0,
            Order::LatestFirst,
        )
        .await?;

    let host = format!("{}", req.uri());
    let title = "fuzzle bot";
    let desc = "Hi there";
    let lang = "en";

    let content = html! {
        #content {
            h1 {
                "Tag "
                (tag_id)
            }
                            h2 {
                                "Sets containing this tag"
                            }
                            div class="set-grid" {
                                @for set in &sets {
                                    (sticker_set_list_item(&set.id))
                                }
                            }

                            h2 {
                                "Stickers tagged with this tag"
                            }

            div class="grid" {
                @for sticker in &stickers {
                    (sticker_list_item(&sticker.id))
                }
            }
        }
    };

    Ok(page(&host, title, desc, lang, content))
}
