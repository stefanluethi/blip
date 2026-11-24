//! View types an application adds to those of the library: registered once with
//! [`register`], offered like the others to add to a layout, and saved in presets by
//! their name.

use super::{PropertiesContext, Selectable, ViewContext, ViewInfo, Viewable};
use crate::store::Store;
use crate::store::traces::Trace;
use std::collections::BTreeSet;
use std::sync::RwLock;

/// A registered type of view: what [`ViewInfo`] says of it, and how to make and load a
/// view of it.
pub struct CustomType {
    /// The name shown to the user and saved in presets, unlike all others.
    pub name: &'static str,
    pub icon: &'static str,
    pub has_selection: bool,
    pub writes_values: bool,
    pub shows_one_trace: bool,
    pub shows_actions: bool,
    pub shows_logs: bool,
    pub(super) selectable: fn(Trace<'_>) -> Selectable,
    pub(super) shows_apart: fn(Trace<'_>) -> bool,
    create: fn() -> Box<dyn Custom>,
    load: fn(serde_json::Value) -> Result<Box<dyn Custom>, String>,
}

impl PartialEq for CustomType {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for CustomType {}

impl std::fmt::Debug for CustomType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CustomType").field(&self.name).finish()
    }
}

/// The state of a view of a registered type, as the layout keeps it.
trait Custom: Viewable {
    fn viewable(&mut self) -> &mut dyn Viewable;
    fn viewable_ref(&self) -> &dyn Viewable;
    fn save(&self) -> serde_json::Value;
    fn clone_box(&self) -> Box<dyn Custom>;
}

impl<T> Custom for T
where
    T: Viewable + Clone + serde::Serialize + 'static,
{
    fn viewable(&mut self) -> &mut dyn Viewable {
        self
    }

    fn viewable_ref(&self) -> &dyn Viewable {
        self
    }

    fn save(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_else(|error| {
            log::warn!("a view cannot be saved: {error}");
            serde_json::Value::Null
        })
    }

    fn clone_box(&self) -> Box<dyn Custom> {
        Box::new(self.clone())
    }
}

/// The registered types, in the order they were registered.
static TYPES: RwLock<Vec<&'static CustomType>> = RwLock::new(Vec::new());

/// The type of the views whose type is not registered, e.g. of a preset made with
/// another application.
static UNKNOWN: CustomType = CustomType {
    name: "Unknown",
    icon: egui_phosphor::regular::QUESTION,
    has_selection: false,
    writes_values: false,
    shows_one_trace: false,
    shows_actions: false,
    shows_logs: false,
    selectable: |_| Selectable::No("A view of an unknown type shows no traces".into()),
    shows_apart: |_| false,
    create: || Box::new(Missing),
    load: |_| Ok(Box::new(Missing)),
};

/// Registers the view type `T` under its [`ViewInfo::NAME`], so that it can be added
/// to a layout and views of it are saved in presets and loaded again. Call it once
/// when the application starts, before presets are loaded. A type with the name of a
/// registered one or of one of the library is not registered again.
pub fn register<T>()
where
    T: Viewable
        + ViewInfo
        + Default
        + Clone
        + serde::Serialize
        + serde::de::DeserializeOwned
        + 'static,
{
    let built_in = super::ViewType::BUILT_IN
        .iter()
        .any(|built_in| built_in.name() == T::NAME);
    let mut types = TYPES
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if built_in || T::NAME == UNKNOWN.name || types.iter().any(|known| known.name == T::NAME) {
        log::warn!("the view type {} is registered already", T::NAME);
        return;
    }
    // kept for as long as the application runs, like the types of the library
    types.push(Box::leak(Box::new(CustomType {
        name: T::NAME,
        icon: T::ICON,
        has_selection: T::HAS_SELECTION,
        writes_values: T::WRITES_VALUES,
        shows_one_trace: T::SHOWS_ONE_TRACE,
        shows_actions: T::SHOWS_ACTIONS,
        shows_logs: T::SHOWS_LOGS,
        selectable: T::selectable,
        shows_apart: T::shows_apart,
        create: || Box::new(T::default()),
        load: |state| {
            serde_json::from_value::<T>(state)
                .map(|view| Box::new(view) as Box<dyn Custom>)
                .map_err(|error| error.to_string())
        },
    })));
}

/// The registered types, in the order they were registered.
pub fn registered() -> Vec<&'static CustomType> {
    TYPES
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn find(name: &str) -> Option<&'static CustomType> {
    registered().into_iter().find(|known| known.name == name)
}

/// A view of a registered type, or of a type that is not registered, which is kept as
/// it was saved so that saving the layout does not lose it.
pub struct CustomView {
    custom_type: &'static CustomType,
    /// The name of the type as saved, also of one that is not registered.
    name: String,
    state: Box<dyn Custom>,
    /// The saved state of a view whose type is not registered.
    unknown: Option<serde_json::Value>,
}

impl CustomView {
    /// A new view of `custom_type`.
    pub fn new(custom_type: &'static CustomType) -> Self {
        Self {
            custom_type,
            name: custom_type.name.to_owned(),
            state: (custom_type.create)(),
            unknown: None,
        }
    }

    pub fn custom_type(&self) -> &'static CustomType {
        self.custom_type
    }

    pub(super) fn viewable(&mut self) -> &mut dyn Viewable {
        self.state.viewable()
    }

    pub(super) fn viewable_ref(&self) -> &dyn Viewable {
        self.state.viewable_ref()
    }
}

impl Clone for CustomView {
    fn clone(&self) -> Self {
        Self {
            custom_type: self.custom_type,
            name: self.name.clone(),
            state: self.state.clone_box(),
            unknown: self.unknown.clone(),
        }
    }
}

/// A custom view as a layout saves it.
#[derive(serde::Deserialize, serde::Serialize)]
struct Saved {
    #[serde(rename = "type")]
    name: String,
    #[serde(default)]
    state: serde_json::Value,
}

impl serde::Serialize for CustomView {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let state = match &self.unknown {
            Some(state) => state.clone(),
            None => self.state.save(),
        };
        let name = self.name.clone();
        Saved { name, state }.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for CustomView {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let Saved { name, state } = Saved::deserialize(deserializer)?;
        let Some(custom_type) = find(&name) else {
            return Ok(Self {
                custom_type: &UNKNOWN,
                name,
                state: Box::new(Missing),
                unknown: Some(state),
            });
        };
        let state = (custom_type.load)(state).map_err(serde::de::Error::custom)?;
        Ok(Self {
            custom_type,
            name,
            state,
            unknown: None,
        })
    }
}

/// What is shown in place of a view whose type is not registered.
#[derive(Clone, serde::Serialize)]
struct Missing;

impl Viewable for Missing {
    fn view(&mut self, ui: &mut egui::Ui, _context: &mut ViewContext<'_>) {
        crate::widgets::theme::hint(ui, "This application does not have this type of view");
    }

    fn properties_ui(&mut self, _ui: &mut egui::Ui, _context: &PropertiesContext<'_>) {}

    fn selection(&mut self) -> Option<(&mut BTreeSet<String>, bool)> {
        None
    }

    fn colored_paths(&self, _store: &dyn Store) -> Vec<String> {
        vec![]
    }
}

#[cfg(test)]
mod tests;
