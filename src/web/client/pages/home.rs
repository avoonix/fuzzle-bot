use std::vec;

use itertools::Itertools;
use leptos::html::*;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use serde::{Deserialize, Serialize};

use crate::web::client::components::*;
use crate::web::shared::*;

#[derive(Params, PartialEq)]
struct HomeSearch {
    q: String,
    offset: usize,
}

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <LogoutButton/>

        <QueryInput/>
    }
}

#[component]
fn LogoutButton() -> impl IntoView {
    view! {
        <a href="/logout" rel="external">
            "Logout"
        </a>
    }
}

#[component]
fn ResultsList(#[prop(into)] results: Vec<StickerDto>) -> impl IntoView {
    view! {
        <div class="grid sticker-grid-auto-fill gap-2">
            {results
                .into_iter()
                .map(|r| {
                    view! {
                        <div>
                            <A href=format!("/sticker/{}", r.id.clone())>
                                <Sticker id=r.id/>
                            </A>
                        </div>
                    }
                })
                .collect_vec()}
        </div>
    }
}

#[component]
fn QueryInput() -> impl IntoView {
    let query = use_query::<HomeSearch>();
    let search = move || {
        query.with(|query| {
            query
                .as_ref()
                .map(|query| query.q.clone())
                .unwrap_or_default()
        })
    };
    let offset =
        move || query.with(|query| query.as_ref().map(|query| query.offset).unwrap_or_default());

    let limit = 100;
    let search_results = create_resource(
        move || (search(), limit, offset()),
        |(search, limit, offset)| fetch_results(search, limit, offset),
    );

    view! {
        <Form method="GET" action="">
            <input type="search" name="q" value=search/>
            <input type="hidden" name="offset" value=0/>
            <input type="submit"/>
        </Form>
        <Form method="GET" action="">
            <input type="hidden" name="q" value=search/>
            <input type="hidden" name="offset" value=move || offset().saturating_sub(limit)/>
            <input type="submit" value="prev"/>
        </Form>
        <Form method="GET" action="">
            <input type="hidden" name="q" value=search/>
            <input type="hidden" name="offset" value=move || offset() + limit/>
            <input type="submit" value="next"/>
        </Form>
        <Transition fallback=move || ()>
            <div>
                "search results"
                {move || match search_results.get() {
                    None => view! { <p>"Loading..."</p> }.into_view(),
                    Some(results) => {
                        match results {
                            Ok(results) => view! { <ResultsList results/> }.into_view(),
                            Err(err) => {
                                view! {
                                    <pre class="error">"Server Error: " {err.to_string()}</pre>
                                }
                                    .into_view()
                            }
                        }
                    }
                }}

            </div>
        </Transition>
    }
}
