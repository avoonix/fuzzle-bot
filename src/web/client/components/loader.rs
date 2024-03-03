use leptos::html::*;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;

#[component]
pub fn Loader() -> impl IntoView {
    view! {
        <div class="flex items-center justify-center animate-pulse">
            <img
                class="motion-safe:animate-bounce m-4"
                src="/assets/fuzzle.svg"
                alt="loading animation"
                title="Loading ..."
                width="64"
                height="64"
            />
        </div>
    }
}
