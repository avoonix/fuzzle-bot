use leptos::*;
use leptos_meta::*;
use leptos_router::*;

use crate::web::client::sticker_context::StickerContext;

use super::pages::*;

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    provide_context(StickerContext::new());

    view! {
        <Stylesheet id="leptos" href="/pkg/web.css"/>
        <script src="https://cdn.tailwindcss.com"></script>
        <Title text="FuzzleBot"/>

        <Router>
            <main>
                <Routes>
                    <Route path="" view=HomePage/>
                    <Route path="/sticker/:sticker_id" view=StickerPage/>
                    <Route path="/merge/:sticker_id_a/:sticker_id_b" view=MergePage/>
                    <Route path="/*any" view=NotFound/>
                </Routes>
            </main>
        </Router>
    }
}
