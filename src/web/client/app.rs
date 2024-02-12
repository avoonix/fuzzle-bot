use leptos::*;
use leptos_meta::*;
use leptos_router::*;

use super::pages::*;

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/web.css"/>
        <script src="https://cdn.tailwindcss.com"></script>
        <Title text="FuzzleBot"/>

        <Router>
            <main>
                <Routes>
                    <Route path="" view=HomePage/>
                    <Route path="/*any" view=NotFound/>
                </Routes>
            </main>
        </Router>
    }
}
