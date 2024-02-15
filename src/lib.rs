#[cfg(feature = "ssr")]
mod bot;
#[cfg(feature = "ssr")]
mod callback;
#[cfg(feature = "ssr")]
mod database;
#[cfg(feature = "ssr")]
mod inline;
#[cfg(feature = "ssr")]
mod message;
#[cfg(feature = "ssr")]
mod tags;
#[cfg(feature = "ssr")]
mod text;
#[cfg(feature = "ssr")]
mod util;
#[cfg(feature = "ssr")]
mod worker;

#[cfg(feature = "ssr")]
pub use bot::{Config, Paths, UpdateListener};

mod sticker;
mod web;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use leptos::*;
    use web::client::*;

    console_error_panic_hook::set_once();

    mount_to_body(App);
}
