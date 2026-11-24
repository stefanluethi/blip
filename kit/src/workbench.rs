//! The workbench: the layout of views the user arranges, the presets it is kept in,
//! the connections to the outposts, the toasts and the dialogs to open and save files.

pub mod connections;
pub mod files;
pub mod layout;
pub mod opening;
pub mod presets;
pub mod toasts;

/// A name that is not `taken`: `base`, else `base` with the first number from 2 that
/// gives one, or always numbered, from 1, if `numbered`. E.g. for connections and
/// presets.
pub fn unused_name(base: &str, numbered: bool, taken: impl Fn(&str) -> bool) -> String {
    let plain = (!numbered).then(|| base.to_owned());
    let from = if numbered { 1 } else { 2 };
    plain
        .into_iter()
        .chain((from..).map(|n| format!("{base} {n}")))
        .find(|name| !taken(name))
        .expect("there is always an unused name")
}
