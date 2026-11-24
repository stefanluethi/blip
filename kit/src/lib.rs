//! The library blip is built from: the store of what the targets send, the widgets
//! that show it and the workbench, the layout the user arranges them in.
//!
//! - [`store`]: the traces, logs and actions received, recording files and the invoker
//!   of actions. Always there, without a GUI.
//! - [`widgets`] (feature `widgets`, on by default): the look, the time range with its
//!   controls and timeline, and the views.
//! - [`workbench`] (feature `workbench`): the layout of views the user arranges, the
//!   presets, the connections, the toasts and the file dialogs.
//!
//! The `blip` crate is the reference application built on all three.

/// Defines a fieldless enum with the name shown for each variant, written
/// `Variant => "Name"`, and gives it `ALL` its variants in their order, `CHOICES` each
/// with its name, e.g. for `widgets::view::choice_row`, and `name()`.
macro_rules! named_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($(#[$variant_meta:meta])* $variant:ident => $text:literal),* $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis enum $name {
            $($(#[$variant_meta])* $variant),*
        }

        #[allow(dead_code)]
        impl $name {
            pub const ALL: [Self; [$($text),*].len()] = [$(Self::$variant),*];
            pub const CHOICES: [(Self, &'static str); [$($text),*].len()] =
                [$((Self::$variant, $text)),*];

            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $text),*
                }
            }
        }
    };
}

/// The wire types, the crate `blip-proto`: the messages of the targets and the
/// requests to them.
pub use proto;

pub mod store;
#[cfg(feature = "widgets")]
pub mod widgets;
#[cfg(feature = "workbench")]
pub mod workbench;
