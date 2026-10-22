// The full-screen wizard, laid out like a Nuxt app: app, layouts, pages, components,
// composables (logic on the store), stores (state), types, theme and utils.
mod app;
mod components;
mod composables;
mod layouts;
mod pages;
mod stores;
mod theme;
mod types;
mod utils;

#[cfg(test)]
mod tests;

pub use app::run;
pub use stores::selection::Selection;
