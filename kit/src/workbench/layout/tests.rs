use super::*;
use crate::store::MemoryStore;
use crate::widgets::time::TimeRange;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::cell::RefCell;
use std::rc::Rc;

fn children(layout: &Layout, tile_id: TileId) -> Vec<TileId> {
    layout.container(tile_id).unwrap().children_vec()
}

fn kind(layout: &Layout, tile_id: TileId) -> GroupKind {
    GroupKind::of(layout.container(tile_id).unwrap())
}

/// A layout with a tab holding a plot and a log view, and their ids.
fn default_layout() -> (Layout, TileId, TileId, TileId) {
    let layout = Layout::with_plot_and_logs();
    let tab = children(&layout, layout.root())[0];
    let views = children(&layout, tab);
    (layout, tab, views[0], views[1])
}

/// The layout `nested()` builds and the ids of its groups and views, by their names in
/// the fixture: groups three levels below the top level, tabs and empty ones among them.
pub(super) struct Nested {
    pub layout: Layout,
    pub tab: TileId,
    pub xy: TileId,
    pub tabs2: TileId,
    pub plot: TileId,
    pub page: TileId,
    pub logs: TileId,
    pub row: TileId,
    pub table: TileId,
    pub tabs3: TileId,
    pub states: TileId,
    pub empty: TileId,
}

fn view_node(view_type: ViewType, name: &str) -> file::Node {
    file::Node::View(View::new(view_type, name.to_owned()))
}

fn group_node(
    name: &str,
    kind: GroupKind,
    active: Option<usize>,
    children: Vec<file::Node>,
) -> file::Node {
    file::Node::Group(Group {
        name: name.to_owned(),
        kind,
        children,
        shares: vec![],
        active,
    })
}

/// The tile of `layout` named `name`.
pub(super) fn named(layout: &Layout, name: &str) -> TileId {
    layout
        .tree
        .tiles
        .tile_ids()
        .find(|&id| layout.name(id) == name)
        .unwrap_or_else(|| panic!("no tile named {name}"))
}

/// The open tab of the group of tabs `group`.
fn active(layout: &Layout, group: TileId) -> Option<TileId> {
    match layout.container(group) {
        Some(Container::Tabs(tabs)) => tabs.active,
        _ => panic!("{group:?} is no group of tabs"),
    }
}

/// The names of the children of `group`.
fn names(layout: &Layout, group: TileId) -> Vec<String> {
    (children(layout, group).into_iter())
        .map(|id| layout.name(id).to_owned())
        .collect()
}

/// The saved form of `nested()`, see there.
fn nested_saved() -> Group {
    use GroupKind::{Horizontal, Tabs, Vertical};
    let tab = group_node(
        "Tab 1",
        Vertical,
        None,
        vec![
            view_node(ViewType::Plot, "XY 1"),
            group_node(
                "Group 1",
                Tabs,
                Some(0),
                vec![
                    view_node(ViewType::Plot, "Plot 1"),
                    group_node(
                        "Tab 2",
                        Vertical,
                        None,
                        vec![view_node(ViewType::Logs, "Logs 1")],
                    ),
                ],
            ),
            group_node(
                "Group 2",
                Horizontal,
                None,
                vec![
                    view_node(ViewType::Table, "Table 1"),
                    group_node(
                        "Group 3",
                        Tabs,
                        Some(0),
                        vec![
                            view_node(ViewType::States, "States 1"),
                            group_node("Tab 3", Vertical, None, vec![]),
                        ],
                    ),
                ],
            ),
        ],
    );
    Group {
        active: Some(0),
        ..Group::tabs(vec![tab])
    }
}

/// A layout with groups at every depth, to test the tree operations on:
///
/// ```text
/// root  Tabs, open: tab                                      depth 0
/// tab   "Tab 1"   Vertical [xy, tabs2, row]                  1
/// xy    "XY 1"    view                                       2
/// tabs2 "Group 1" Tabs [plot, page], open: plot              2
/// plot  "Plot 1"  view, a tab                                3
/// page  "Tab 2"   Vertical [logs], a tab                     3
/// logs  "Logs 1"  view                                       4
/// row   "Group 2" Horizontal [table, tabs3]                  2
/// table "Table 1" view                                       3
/// tabs3 "Group 3" Tabs [states, empty], open: states         3
/// states "States 1" view, a tab                              4
/// empty "Tab 3"   Vertical [], an empty tab                  4
/// ```
pub(super) fn nested() -> Nested {
    let layout = Layout::from_saved(nested_saved());
    Nested {
        tab: named(&layout, "Tab 1"),
        xy: named(&layout, "XY 1"),
        tabs2: named(&layout, "Group 1"),
        plot: named(&layout, "Plot 1"),
        page: named(&layout, "Tab 2"),
        logs: named(&layout, "Logs 1"),
        row: named(&layout, "Group 2"),
        table: named(&layout, "Table 1"),
        tabs3: named(&layout, "Group 3"),
        states: named(&layout, "States 1"),
        empty: named(&layout, "Tab 3"),
        layout,
    }
}

#[test]
fn the_default_layout_is_empty() {
    let layout = Layout::default();
    assert!(children(&layout, layout.root()).is_empty());
    assert_eq!(layout.selected, None);
}

#[test]
fn a_view_added_at_a_side_across_the_group_is_grouped_with_the_view() {
    let (mut layout, tab, plot, logs) = default_layout();
    let table = layout
        .add_view_beside(plot, Side::Right, ViewType::Table)
        .unwrap();
    let group = children(&layout, tab)[0];
    assert_eq!(children(&layout, tab), [group, logs]);
    assert_eq!(kind(&layout, group), GroupKind::Horizontal);
    assert_eq!(children(&layout, group), [plot, table]);
    assert_eq!(layout.view(table).unwrap().view_type(), ViewType::Table);
    assert_eq!(layout.selected, Some(table));
}

#[test]
fn a_view_added_at_a_side_along_the_group_goes_next_to_the_view() {
    let (mut layout, tab, plot, logs) = default_layout();
    let states = layout
        .add_view_beside(plot, Side::Bottom, ViewType::States)
        .unwrap();
    assert_eq!(children(&layout, tab), [plot, states, logs]);
    assert_eq!(layout.name(states), "States 1");
}

#[test]
fn dropping_before_and_after_reorders() {
    let (mut layout, tab, plot, logs) = default_layout();
    layout.drop_tile(logs, plot, Drop::Before);
    assert_eq!(children(&layout, tab), [logs, plot]);
    layout.drop_tile(logs, plot, Drop::After);
    assert_eq!(children(&layout, tab), [plot, logs]);
}

#[test]
fn dropping_at_a_side_along_the_group_reorders() {
    let (mut layout, tab, plot, logs) = default_layout();
    layout.drop_tile(logs, plot, Drop::Side(Side::Top));
    assert_eq!(children(&layout, tab), [logs, plot]);
    layout.drop_tile(logs, plot, Drop::Side(Side::Bottom));
    assert_eq!(children(&layout, tab), [plot, logs]);
}

#[test]
fn dropping_at_a_side_across_a_group_of_just_the_two_turns_the_group() {
    let (mut layout, tab, plot, logs) = default_layout();
    layout.drop_tile(logs, plot, Drop::Side(Side::Left));
    assert_eq!(children(&layout, layout.root()), [tab]);
    assert_eq!(kind(&layout, tab), GroupKind::Horizontal);
    assert_eq!(children(&layout, tab), [logs, plot]);
}

#[test]
fn dropping_at_a_side_across_the_group_groups_in_that_direction() {
    let (mut layout, tab, plot, logs) = default_layout();
    let states = layout.add_view_into(tab, ViewType::States);
    layout.drop_tile(logs, plot, Drop::Side(Side::Left));
    let group = children(&layout, tab)[0];
    assert_eq!(children(&layout, tab), [group, states]);
    assert_eq!(kind(&layout, group), GroupKind::Horizontal);
    assert_eq!(children(&layout, group), [logs, plot]);
    assert_eq!(layout.name(group), "Group 1");

    // the side decides the direction, not the level
    let table = layout.add_view_into(group, ViewType::Table);
    layout.drop_tile(table, logs, Drop::Side(Side::Left));
    assert_eq!(children(&layout, group), [table, logs, plot]);
    layout.drop_tile(table, plot, Drop::Side(Side::Bottom));
    let inner = children(&layout, group)[1];
    assert_eq!(children(&layout, group), [logs, inner]);
    assert_eq!(kind(&layout, inner), GroupKind::Vertical);
    assert_eq!(children(&layout, inner), [plot, table]);
}

#[test]
fn dropping_at_a_side_of_a_tab_groups_it_in_its_place() {
    let (mut layout, tab, plot, logs) = default_layout();
    layout.drop_tile(plot, tab, Drop::Before);
    assert_eq!(children(&layout, layout.root()), [plot, tab]);
    layout.drop_tile(logs, plot, Drop::Side(Side::Bottom));
    let new_tab = children(&layout, layout.root())[0];
    assert_eq!(kind(&layout, new_tab), GroupKind::Vertical);
    assert_eq!(children(&layout, new_tab), [plot, logs]);
    assert!(layout.name(new_tab).starts_with("Tab "));
    // the old tab was left empty by the move
    assert_eq!(children(&layout, layout.root()), [new_tab]);
}

#[test]
fn moving_the_last_view_out_of_a_group_removes_the_group() {
    let (mut layout, tab, plot, logs) = default_layout();
    let new_tab = layout.add_tab();
    layout.drop_tile(plot, new_tab, Drop::Into);
    layout.drop_tile(logs, new_tab, Drop::Into);
    assert_eq!(children(&layout, layout.root()), [new_tab]);
    assert!(layout.tree.tiles.get(tab).is_none());
}

#[test]
fn deleting_the_last_view_of_a_tab_keeps_the_tab() {
    let (mut layout, tab, plot, logs) = default_layout();
    let table = layout
        .add_view_beside(plot, Side::Right, ViewType::Table)
        .unwrap();
    let group = children(&layout, tab)[0];
    layout.remove(table);
    layout.remove(plot);
    assert!(layout.tree.tiles.get(group).is_none());
    layout.remove(logs);
    assert_eq!(children(&layout, layout.root()), [tab]);
    assert!(children(&layout, tab).is_empty());
}

#[test]
fn a_group_cannot_be_dropped_into_itself() {
    let (mut layout, tab, plot, logs) = default_layout();
    layout.drop_tile(tab, plot, Drop::Side(Side::Left));
    layout.drop_tile(tab, tab, Drop::Into);
    assert_eq!(children(&layout, layout.root()), [tab]);
    assert_eq!(children(&layout, tab), [plot, logs]);
}

#[test]
fn double_clicking_a_tab_renames_it_and_a_click_only_selects_it() {
    let mut layout = Layout::with_plot_and_logs();
    let main = children(&layout, layout.root())[0];
    layout.add_tab();
    let renaming = Rc::new(RefCell::new(None));
    let renamed = renaming.clone();
    let selection = Rc::new(RefCell::new(None));
    let selected = selection.clone();
    let model = MemoryStore::default();
    let mut time = TimeRange::default();
    let mut invoker = crate::store::actions::Invoker::default();
    // a step for each event of a click, short enough for two clicks to make a double
    // click
    let mut harness = Harness::builder().with_step_dt(0.04).build_ui(move |ui| {
        let mut context = ViewContext {
            store: &model,
            time: &mut time,
            invoker: &mut invoker,
            formats: &crate::widgets::view::NO_FORMATS,
            titled: false,
            plots: &crate::widgets::view::NO_PLOTS,
        };
        layout.ui(ui, &mut context);
        *renamed.borrow_mut() = layout.renaming.as_ref().map(|renaming| renaming.tile_id);
        *selected.borrow_mut() = layout.selected;
    });
    // a second, so that the next click is a click of its own
    let wait = |harness: &mut Harness<'_>| {
        for _ in 0..25 {
            harness.step();
        }
    };
    harness.run();

    // the first click opens the tab, the second selects it
    harness.get_by_label("Tab 1").click();
    wait(&mut harness);
    assert_eq!(*renaming.borrow(), None);
    harness.get_by_label("Tab 1").click();
    wait(&mut harness);
    assert_eq!(*renaming.borrow(), None);
    assert_eq!(*selection.borrow(), Some(main));
    harness.get_by_label("Tab 1").click();
    harness.get_by_label("Tab 1").click();
    harness.run();
    assert_eq!(*renaming.borrow(), Some(main));
}

/// A harness showing `layout` in the center pane.
fn center_harness(layout: Rc<RefCell<Layout>>) -> Harness<'static> {
    center_harness_with(
        layout,
        Harness::builder().with_size(egui::vec2(800.0, 600.0)),
    )
}

/// A harness of `builder` showing `layout` in the center pane.
fn center_harness_with(
    layout: Rc<RefCell<Layout>>,
    builder: egui_kittest::HarnessBuilder,
) -> Harness<'static> {
    center_harness_in(layout, builder, None)
}

/// A harness of `builder` showing `layout` in the center pane, in the top left corner of
/// `shown` size if given, so that menus have the rest of the screen.
fn center_harness_in(
    layout: Rc<RefCell<Layout>>,
    builder: egui_kittest::HarnessBuilder,
    shown: Option<egui::Vec2>,
) -> Harness<'static> {
    let model = MemoryStore::default();
    let mut time = TimeRange::default();
    let mut invoker = crate::store::actions::Invoker::default();
    builder.build_ui(move |ui| {
        let mut context = ViewContext {
            store: &model,
            time: &mut time,
            invoker: &mut invoker,
            formats: &crate::widgets::view::NO_FORMATS,
            titled: false,
            plots: &crate::widgets::view::NO_PLOTS,
        };
        match shown {
            None => layout.borrow_mut().ui(ui, &mut context),
            Some(size) => {
                let rect = egui::Rect::from_min_size(ui.max_rect().min, size);
                let builder = egui::UiBuilder::new().max_rect(rect);
                ui.scope_builder(builder, |ui| {
                    ui.set_clip_rect(rect);
                    layout.borrow_mut().ui(ui, &mut context);
                });
            }
        }
    })
}

#[test]
fn dragging_a_view_by_its_title_to_the_side_of_another_moves_it_there() {
    let (layout, tab, plot, logs) = default_layout();
    let layout = Rc::new(RefCell::new(layout));
    let mut harness = center_harness(layout.clone());
    harness.run();

    let title = harness.get_by_label("Plot 1").rect().center();
    let logs_title = harness.get_by_label("Logs 1").rect().center();
    // the right side of the log view, below its title
    let target = egui::pos2(780.0, logs_title.y + 80.0);
    harness.drag_at(title);
    harness.run();
    harness.hover_at(title + egui::vec2(40.0, 40.0));
    harness.run();
    harness.hover_at(target);
    harness.run();
    harness.drop_at(target);
    harness.run();

    // the tab held just the two views, so it turns
    let layout = layout.borrow();
    assert_eq!(kind(&layout, tab), GroupKind::Horizontal);
    assert_eq!(children(&layout, tab), [logs, plot]);
    assert_eq!(layout.selected, Some(plot));
}

#[test]
fn dragging_a_tab_onto_another_tab_reorders_the_tabs() {
    let (mut layout, tab, _, _) = default_layout();
    let second = layout.add_tab();
    let layout = Rc::new(RefCell::new(layout));
    let mut harness = center_harness(layout.clone());
    harness.run();

    let from = harness.get_by_label("Tab 2").rect().center();
    let to = harness.get_by_label("Tab 1").rect().left_center() + egui::vec2(4.0, 0.0);
    harness.drag_at(from);
    harness.run();
    harness.hover_at(from + egui::vec2(-20.0, 0.0));
    harness.run();
    harness.hover_at(to);
    harness.run();
    harness.drop_at(to);
    harness.run();

    let layout = layout.borrow();
    assert_eq!(children(&layout, layout.root()), [second, tab]);
}

#[test]
fn the_plus_after_the_tabs_adds_a_tab_and_opens_it() {
    let (layout, tab, _, _) = default_layout();
    let layout = Rc::new(RefCell::new(layout));
    let mut harness = center_harness(layout.clone());
    harness.run();
    harness.get_by_label(egui_phosphor::regular::PLUS).click();
    harness.run();

    let layout = layout.borrow();
    let tabs = children(&layout, layout.root());
    assert_eq!(tabs.len(), 2);
    assert_eq!(tabs[0], tab);
    assert_eq!(layout.name(tabs[1]), "Tab 2");
    assert_eq!(layout.selected, Some(tabs[1]));
}

#[test]
fn a_view_type_clicked_in_an_empty_tab_is_added_to_it() {
    let (mut layout, _, _, _) = default_layout();
    let tab = layout.add_tab();
    let layout = Rc::new(RefCell::new(layout));
    let mut harness = center_harness(layout.clone());
    harness.run();
    harness.get_by_label("Heatmap").click();
    harness.run();

    let layout = layout.borrow();
    let views = children(&layout, tab);
    assert_eq!(views.len(), 1);
    assert_eq!(
        layout.view(views[0]).unwrap().view_type(),
        ViewType::Heatmap
    );
    assert_eq!(layout.selected, Some(views[0]));
}

#[test]
fn a_view_type_clicked_without_tabs_is_added_in_a_new_tab() {
    let (mut layout, tab, _, _) = default_layout();
    layout.remove_tile(tab);
    let layout = Rc::new(RefCell::new(layout));
    let mut harness = center_harness(layout.clone());
    harness.run();
    harness.get_by_label("Plot").click();
    harness.run();

    let layout = layout.borrow();
    let tabs = children(&layout, layout.root());
    assert_eq!(tabs.len(), 1);
    assert_eq!(children(&layout, tabs[0]).len(), 1);
}

#[test]
fn the_menu_of_a_title_adds_a_view_below() {
    let (layout, tab, plot, logs) = default_layout();
    let layout = Rc::new(RefCell::new(layout));
    let mut harness = center_harness(layout.clone());
    harness.run();
    harness.get_by_label("Plot 1").click_secondary();
    harness.run();
    harness.get_by_label_contains("Add below").hover();
    harness.run();
    harness.get_by_label_contains("XY").click();
    harness.run();

    let layout = layout.borrow();
    let views = children(&layout, tab);
    assert_eq!(views.len(), 3);
    assert_eq!((views[0], views[2]), (plot, logs));
    assert_eq!(layout.view(views[1]).unwrap().view_type(), ViewType::Xy);
}

#[test]
fn deleting_a_view_while_renaming_it_ends_the_renaming() {
    let (mut layout, _, plot, _) = default_layout();
    layout.start_rename(plot);
    layout.remove(plot);
    assert!(layout.renaming.is_none());
}

#[test]
fn the_menu_of_a_title_deletes_the_view() {
    let (layout, tab, plot, logs) = default_layout();
    let layout = Rc::new(RefCell::new(layout));
    let mut harness = center_harness(layout.clone());
    harness.run();
    harness.get_by_label("Logs 1").click_secondary();
    harness.run();
    harness.get_by_label_contains("Delete").click();
    harness.run();

    let layout = layout.borrow();
    assert_eq!(children(&layout, tab), [plot]);
    assert!(layout.view(logs).is_none());
}

#[test]
fn a_view_is_not_added_beside_the_top_level() {
    let (mut layout, _, _, _) = default_layout();
    let tiles = layout.tree.tiles.len();
    let root = layout.root();
    assert_eq!(
        layout.add_view_beside(root, Side::Right, ViewType::Plot),
        None
    );
    assert_eq!(layout.tree.tiles.len(), tiles);
}

#[test]
fn the_menu_of_a_tab_deletes_the_tab_with_its_views() {
    let (mut layout, tab, plot, _) = default_layout();
    let second = layout.add_tab();
    layout.add_view_into(second, ViewType::Table);
    let layout = Rc::new(RefCell::new(layout));
    let mut harness = center_harness(layout.clone());
    harness.run();
    harness.get_by_label("Tab 1").click_secondary();
    harness.run();
    harness.get_by_label_contains("Delete").click();
    harness.run();

    let layout = layout.borrow();
    assert_eq!(children(&layout, layout.root()), [second]);
    assert!(layout.tree.tiles.get(tab).is_none());
    assert!(layout.view(plot).is_none());
}

#[test]
fn the_menu_of_a_tab_renames_it() {
    let (layout, tab, _, _) = default_layout();
    let layout = Rc::new(RefCell::new(layout));
    let mut harness = center_harness(layout.clone());
    harness.run();
    harness.get_by_label("Tab 1").click_secondary();
    harness.run();
    harness.get_by_label_contains("Rename").click();
    harness.run();

    let layout = layout.borrow();
    assert_eq!(
        layout.renaming.as_ref().map(|renaming| renaming.tile_id),
        Some(tab)
    );
}

#[test]
fn switching_tabs_keeps_the_selected_view_and_the_open_tab_is_selected_by_a_click() {
    let (mut layout, tab, plot, _) = default_layout();
    let second = layout.add_tab();
    layout.add_view_into(second, ViewType::Table);
    layout.select(plot);
    let layout = Rc::new(RefCell::new(layout));
    let mut harness = center_harness(layout.clone());
    harness.run();

    harness.get_by_label("Tab 2").click();
    harness.run();
    assert_eq!(layout.borrow().selected, Some(plot));
    harness.get_by_label("Tab 1").click();
    harness.run();
    assert_eq!(layout.borrow().selected, Some(plot));
    harness.get_by_label("Tab 1").click();
    harness.run();
    assert_eq!(layout.borrow().selected, Some(tab));
}

#[test]
fn a_view_from_the_menu_bar_goes_after_the_selected_view() {
    let (mut layout, tab, plot, logs) = default_layout();
    layout.select(plot);
    layout.add_view(ViewType::Table);
    let table = children(&layout, tab)[1];
    assert_eq!(children(&layout, tab), [plot, table, logs]);
    assert_eq!(layout.selected, Some(table));
}

#[test]
fn a_view_from_the_menu_bar_goes_into_the_open_tab_if_the_selection_is_hidden() {
    let (mut layout, _, plot, _) = default_layout();
    layout.select(plot);
    let second = layout.add_tab();
    layout.selected = Some(plot);
    layout.add_view(ViewType::States);
    assert_eq!(children(&layout, second).len(), 1);
}

#[test]
fn a_view_from_the_menu_bar_without_tabs_goes_into_a_new_tab() {
    let (mut layout, tab, _, _) = default_layout();
    layout.remove_tile(tab);
    layout.add_view(ViewType::Plot);
    let tabs = children(&layout, layout.root());
    assert_eq!(tabs.len(), 1);
    assert_eq!(children(&layout, tabs[0]).len(), 1);
}

#[test]
fn the_menu_button_of_a_title_opens_its_menu() {
    let (layout, tab, plot, logs) = default_layout();
    let layout = Rc::new(RefCell::new(layout));
    let mut harness = center_harness(layout.clone());
    harness.run();
    let buttons: Vec<_> = harness
        .get_all_by_label(egui_phosphor::regular::DOTS_THREE)
        .collect();
    assert_eq!(buttons.len(), 2);
    buttons[1].click();
    harness.run();
    harness.get_by_label_contains("Delete").click();
    harness.run();

    let layout = layout.borrow();
    assert_eq!(children(&layout, tab), [plot]);
    assert!(layout.view(logs).is_none());
}

#[test]
fn clicking_a_title_renames_the_view_without_moving_it() {
    let (layout, tab, plot, logs) = default_layout();
    let layout = Rc::new(RefCell::new(layout));
    let mut harness = center_harness(layout.clone());
    harness.run();
    harness.get_by_label("Plot 1").click();
    harness.run();
    let layout = layout.borrow();
    assert_eq!(children(&layout, tab), [plot, logs]);
    assert_eq!(
        layout.renaming.as_ref().map(|renaming| renaming.tile_id),
        Some(plot)
    );
}

#[test]
fn removing_a_group_removes_its_views() {
    let (mut layout, tab, plot, _) = default_layout();
    layout.remove(tab);
    assert!(children(&layout, layout.root()).is_empty());
    assert!(layout.view(plot).is_none());
}

/// The distribution at `tile_id` of `layout`.
fn distribution(
    layout: &mut Layout,
    tile_id: TileId,
) -> &mut crate::widgets::view::distribution::Distribution {
    match layout.tree.tiles.get_mut(tile_id) {
        Some(Tile::Pane(View {
            kind: ViewKind::Distribution(distribution),
            ..
        })) => distribution,
        _ => panic!("{tile_id:?} is no distribution"),
    }
}

/// The plot at `tile_id` of `layout`.
fn plot(layout: &mut Layout, tile_id: TileId) -> &mut crate::widgets::view::plot::Plot {
    match layout.tree.tiles.get_mut(tile_id) {
        Some(Tile::Pane(View {
            kind: ViewKind::Plot(plot),
            ..
        })) => plot,
        _ => panic!("{tile_id:?} is no plot"),
    }
}

#[test]
fn the_plots_of_the_layout_are_listed_with_the_y_ranges_they_show() {
    let (mut layout, tab, first, _) = default_layout();
    let distribution = layout.add_view_into(tab, ViewType::Distribution);
    layout.add_view_into(tab, ViewType::Plot);
    let model = MemoryStore::default();
    let names =
        |plots: &Plots| -> Vec<String> { plots.0.iter().map(|plot| plot.name.clone()).collect() };
    // only plots, in the order of the layout
    let plots = layout.plots(&model);
    assert_eq!(names(&plots), ["Plot 1", "Plot 2"]);
    assert_eq!(plots.0[0].id, view_id(first));
    assert!(plots.get(view_id(distribution)).is_none());
    assert_eq!(plots.0[0].y_range, None);
    // fixed, then zoomed or panned
    (
        plot(&mut layout, first).y_min,
        plot(&mut layout, first).y_max,
    ) = (Some(-1.0), Some(4.0));
    assert_eq!(layout.plots(&model).0[0].y_range, Some((-1.0, 4.0)));
    plot(&mut layout, first).y_view = Some((0.5, 2.0));
    assert_eq!(layout.plots(&model).0[0].y_range, Some((0.5, 2.0)));
}

#[test]
fn deleting_a_plot_drops_the_links_to_it() {
    let (mut layout, tab, plot, logs) = default_layout();
    let shown = layout.add_view_into(tab, ViewType::Distribution);
    distribution(&mut layout, shown).link = Some(view_id(plot));
    layout.remove(logs);
    assert_eq!(distribution(&mut layout, shown).link, Some(view_id(plot)));
    layout.remove(plot);
    assert_eq!(distribution(&mut layout, shown).link, None);
}

#[test]
fn a_linked_histogram_takes_the_traces_of_the_plot() {
    let (mut layout, tab, source, logs) = default_layout();
    let shown = layout.add_view_into(tab, ViewType::Distribution);
    let view = |layout: &mut Layout, tile_id| match layout.tree.tiles.get_mut(tile_id) {
        Some(Tile::Pane(view)) => view.clone(),
        _ => panic!("{tile_id:?} is no view"),
    };
    distribution(&mut layout, shown).traces = ["own".to_owned()].into();
    let traces: std::collections::BTreeSet<String> = ["a".to_owned(), "b".to_owned()].into();
    plot(&mut layout, source).traces = traces.clone();
    if let Some(Tile::Pane(view)) = layout.tree.tiles.get_mut(source) {
        view.wildcards.insert("motor".to_owned());
        view.formats
            .set("a", crate::store::numbers::DisplayFormat::Hex);
    }
    // unlinked, it keeps its own
    layout.take_linked_traces();
    assert_eq!(
        distribution(&mut layout, shown).traces,
        ["own".to_owned()].into()
    );
    layout.select(shown);
    assert_eq!(layout.selected_link(), None);

    distribution(&mut layout, shown).link = Some(view_id(source));
    layout.take_linked_traces();
    let linked = view(&mut layout, shown);
    assert_eq!(distribution(&mut layout, shown).traces, traces);
    assert_eq!(linked.wildcards, ["motor".to_owned()].into());
    assert_eq!(
        linked.formats.of("a"),
        crate::store::numbers::DisplayFormat::Hex
    );
    assert_eq!(layout.selected_link(), Some("Plot 1"));
    // as the plot changes
    plot(&mut layout, source).traces.remove("b");
    layout.take_linked_traces();
    assert_eq!(
        distribution(&mut layout, shown).traces,
        ["a".to_owned()].into()
    );
    // a link to another view takes nothing
    distribution(&mut layout, shown).link = Some(view_id(logs));
    distribution(&mut layout, shown).traces.clear();
    layout.take_linked_traces();
    assert!(distribution(&mut layout, shown).traces.is_empty());
    assert_eq!(layout.selected_link(), None);
    // unlinked, it keeps what it took
    distribution(&mut layout, shown).link = Some(view_id(source));
    layout.take_linked_traces();
    distribution(&mut layout, shown).link = None;
    plot(&mut layout, source).traces.clear();
    layout.take_linked_traces();
    assert_eq!(
        distribution(&mut layout, shown).traces,
        ["a".to_owned()].into()
    );
}

#[test]
fn links_between_views_survive_saving_and_loading() {
    let (mut layout, tab, first, _) = default_layout();
    let second = layout.add_view_into(tab, ViewType::Plot);
    let shown = layout.add_view_into(tab, ViewType::Distribution);
    // the tiles no longer count from the first
    layout.remove(first);
    distribution(&mut layout, shown).link = Some(view_id(second));
    let saved = layout.to_saved();
    let json = serde_json::to_string(&saved).unwrap();
    // the plot is the second view saved, after the logs
    assert!(json.contains(r#""link":1"#), "{json}");

    let mut loaded = Layout::from_saved(serde_json::from_str(&json).unwrap());
    let tab = children(&loaded, loaded.root())[0];
    let [_, plot, shown] = children(&loaded, tab)[..] else {
        panic!("three views");
    };
    assert_eq!(loaded.view(plot).unwrap().name, "Plot 2");
    assert_eq!(distribution(&mut loaded, shown).link, Some(view_id(plot)));
    assert_eq!(serde_json::to_string(&loaded.to_saved()).unwrap(), json);
}

#[test]
fn a_deleted_view_comes_back_by_undo_with_its_links() {
    let (mut layout, tab, plot, logs) = default_layout();
    let shown = layout.add_view_into(tab, ViewType::Distribution);
    distribution(&mut layout, shown).link = Some(view_id(plot));
    assert!(!layout.can_undo_delete());

    layout.select(plot);
    layout.delete_selected();
    assert_eq!(children(&layout, tab), [logs, shown]);
    assert_eq!(distribution(&mut layout, shown).link, None);
    assert!(layout.can_undo_delete());
    // the app is told once
    assert_eq!(layout.take_deleted().as_deref(), Some("Plot 1"));
    assert_eq!(layout.take_deleted(), None);

    layout.undo_delete();
    assert_eq!(children(&layout, tab), [plot, logs, shown]);
    assert_eq!(distribution(&mut layout, shown).link, Some(view_id(plot)));
    assert_eq!(layout.selected, Some(plot));
    assert!(!layout.can_undo_delete());
}

#[test]
fn a_deleted_tab_comes_back_by_undo_and_opens() {
    let (mut layout, tab, plot, logs) = default_layout();
    let second = layout.add_tab();
    layout.add_view_into(second, ViewType::Table);
    layout.delete(tab);
    assert_eq!(layout.take_deleted().as_deref(), Some("Tab 1"));
    assert_eq!(children(&layout, layout.root()), [second]);

    layout.undo_delete();
    assert_eq!(children(&layout, layout.root()), [tab, second]);
    assert_eq!(children(&layout, tab), [plot, logs]);
    assert_eq!(layout.selected, Some(tab));
    assert!(layout.tree.active_tiles().contains(&plot));
}

#[test]
fn undo_brings_back_the_layout_before_the_last_delete() {
    let (mut layout, tab, plot, logs) = default_layout();
    layout.delete(plot);
    layout.delete(logs);
    layout.undo_delete();
    assert_eq!(children(&layout, tab), [logs]);
    assert!(!layout.can_undo_delete());
}

#[test]
fn changing_the_layout_after_a_delete_forgets_the_undo() {
    let changes: [fn(&mut Layout, TileId); 4] = [
        |layout, _| layout.add_view(ViewType::Table),
        |layout, _| {
            layout.add_tab();
        },
        |layout, tab| layout.set_group_kind(tab, GroupKind::Horizontal),
        |layout, tab| layout.set_name(tab, "Main".to_owned()),
    ];
    for change in changes {
        let (mut layout, tab, plot, _) = default_layout();
        layout.delete(plot);
        change(&mut layout, tab);
        assert!(!layout.can_undo_delete());
        layout.undo_delete();
        assert!(layout.view(plot).is_none());
    }
}

#[test]
fn clearing_the_layout_can_be_undone() {
    let (mut layout, tab, plot, logs) = default_layout();
    layout.select(logs);
    layout.clear();
    assert!(children(&layout, layout.root()).is_empty());
    assert_eq!(layout.selected, None);
    assert_eq!(layout.take_deleted().as_deref(), Some("all views"));

    layout.undo_delete();
    assert_eq!(children(&layout, layout.root()), [tab]);
    assert_eq!(children(&layout, tab), [plot, logs]);
    assert_eq!(layout.selected, Some(logs));
}

#[test]
fn clearing_an_empty_layout_keeps_the_undo_of_the_last_delete() {
    let (mut layout, tab, _, _) = default_layout();
    layout.delete(tab);
    layout.take_deleted();
    layout.clear();
    assert_eq!(layout.take_deleted(), None);
    layout.undo_delete();
    assert_eq!(children(&layout, layout.root()), [tab]);
}

#[test]
fn deleting_without_a_selection_does_nothing() {
    let (mut layout, tab, plot, logs) = default_layout();
    layout.selected = None;
    layout.delete_selected();
    assert_eq!(children(&layout, tab), [plot, logs]);
    assert!(!layout.can_undo_delete());
    assert_eq!(layout.take_deleted(), None);
}

#[test]
fn renaming_the_selected_view_opens_its_tab() {
    let (mut layout, _, plot, _) = default_layout();
    let second = layout.add_tab();
    layout.add_view_into(second, ViewType::Table);
    layout.selected = Some(plot);
    layout.rename_selected();
    assert_eq!(
        layout.renaming.as_ref().map(|renaming| renaming.tile_id),
        Some(plot)
    );
    assert!(layout.tree.active_tiles().contains(&plot));
}

#[test]
fn the_cross_on_a_hovered_tab_deletes_it() {
    let (mut layout, tab, _, _) = default_layout();
    let second = layout.add_tab();
    let layout = Rc::new(RefCell::new(layout));
    let mut harness = center_harness(layout.clone());
    harness.run();
    // only the tab under the pointer shows it
    assert!(harness.query_by_label(egui_phosphor::regular::X).is_none());
    harness.get_by_label("Tab 1").hover();
    harness.run();
    harness.get_by_label(egui_phosphor::regular::X).click();
    harness.run();

    let mut layout = layout.borrow_mut();
    assert_eq!(children(&layout, layout.root()), [second]);
    assert_eq!(layout.take_deleted().as_deref(), Some("Tab 1"));
    layout.undo_delete();
    assert_eq!(children(&layout, layout.root()), [tab, second]);
}

#[test]
fn a_tab_tells_how_to_rename_and_move_it() {
    let (layout, _, _, _) = default_layout();
    let layout = Rc::new(RefCell::new(layout));
    let mut harness = center_harness(layout);
    harness.run();
    harness.get_by_label("Tab 1").hover();
    harness.run();
    harness.get_by_label("Double-click to rename, drag to move, right-click for more");
}

#[test]
fn a_view_shown_as_a_tab_has_the_menu_of_a_view() {
    let (mut layout, tab, plot, logs) = default_layout();
    layout.set_group_kind(tab, GroupKind::Tabs);
    let layout = Rc::new(RefCell::new(layout));
    let mut harness = center_harness(layout.clone());
    harness.run();
    harness.get_by_label("Plot 1").click_secondary();
    harness.run();
    harness.get_by_label_contains("Add below").hover();
    harness.run();
    harness.get_by_label_contains("Table").click();
    harness.run();

    let layout = layout.borrow();
    let group = children(&layout, tab)[0];
    assert_eq!(children(&layout, tab), [group, logs]);
    assert_eq!(children(&layout, group)[0], plot);
}

#[test]
fn each_delete_has_its_own_undo_number() {
    let mut layout = Layout::with_plot_and_logs();
    assert_eq!(layout.undo_number(), None);
    layout.select_view_showing_traces();
    layout.delete(layout.selected.unwrap());
    let first = layout.undo_number().unwrap();
    layout.clear();
    let second = layout.undo_number().unwrap();
    assert_ne!(first, second);
    layout.undo_delete();
    assert_eq!(layout.undo_number(), None);
}

#[test]
fn undoing_a_delete_keeps_the_changes_of_the_other_views() {
    let mut layout = Layout::with_plot_and_logs();
    layout.select_view_showing_traces();
    let plot = layout.selected.unwrap();
    // the log view is deleted, then the plot is renamed
    let logs = (layout.tree.tiles.iter())
        .find_map(|(&id, tile)| {
            matches!(tile, Tile::Pane(view) if view.view_type() == ViewType::Logs).then_some(id)
        })
        .unwrap();
    layout.delete(logs);
    let Some(Tile::Pane(view)) = layout.tree.tiles.get_mut(plot) else {
        panic!("the plot stays");
    };
    view.name = "Speed".to_owned();
    layout.undo_delete();
    assert!(layout.tree.tiles.get(logs).is_some());
    assert_eq!(layout.name(plot), "Speed");
}

const BEFORE_NESTING: &str = r#"{"name":"","kind":"Tabs","active":0,"children":[{"Group":{"name":"Main","kind":"Vertical","shares":[1.0,2.0],"children":[{"View":{"name":"Speed","kind":{"Plot":{"traces":["motor.speed_rpm"]}}}},{"Group":{"name":"Details","kind":"Tabs","active":1,"children":[{"View":{"name":"Log","kind":{"Logs":{}}}},{"Group":{"name":"Empty","kind":"Vertical","children":[]}}]}}]}}]}"#;

#[test]
fn groups_of_each_kind_hold_views_and_groups_at_every_depth() {
    let mut n = nested();
    let layout = &mut n.layout;
    assert_eq!(kind(layout, n.tabs2), GroupKind::Tabs);
    assert_eq!(kind(layout, n.tabs3), GroupKind::Tabs);
    assert_eq!(kind(layout, n.row), GroupKind::Horizontal);
    assert_eq!(kind(layout, n.page), GroupKind::Vertical);
    assert_eq!(children(layout, n.row), [n.table, n.tabs3]);

    layout.set_group_kind(n.row, GroupKind::Tabs);
    assert_eq!(kind(layout, n.row), GroupKind::Tabs);
    assert_eq!(children(layout, n.row), [n.table, n.tabs3]);
    layout.set_group_kind(n.tabs3, GroupKind::Vertical);
    assert_eq!(kind(layout, n.tabs3), GroupKind::Vertical);
    assert_eq!(children(layout, n.tabs3), [n.states, n.empty]);
    layout.set_group_kind(n.page, GroupKind::Horizontal);
    assert_eq!(kind(layout, n.page), GroupKind::Horizontal);
    assert_eq!(children(layout, n.page), [n.logs]);
}

#[test]
fn a_nested_tab_is_renamed_and_moved() {
    let mut n = nested();
    let layout = &mut n.layout;
    for (tile, old, new) in [(n.page, "Tab 2", "Motor"), (n.empty, "Tab 3", "Spare")] {
        layout.start_rename(tile);
        assert_eq!(layout.renaming.as_ref().unwrap().name, old);
        layout.renaming.as_mut().unwrap().name = new.to_owned();
        layout.end_rename(true);
        assert_eq!(layout.name(tile), new);
    }

    layout.drop_tile(n.page, n.plot, Drop::Before);
    assert_eq!(children(layout, n.tabs2), [n.page, n.plot]);
    layout.drop_tile(n.states, n.empty, Drop::After);
    assert_eq!(children(layout, n.tabs3), [n.empty, n.states]);
}

#[test]
fn a_nested_tab_is_moved_by_dragging_it() {
    let drag = |from: &str, to: &str| {
        let n = nested();
        let layout = Rc::new(RefCell::new(n.layout));
        let mut harness = center_harness(layout.clone());
        harness.run();
        let from = harness.get_by_label(from).rect().center();
        let to = harness.get_by_label(to).rect().left_center() + egui::vec2(4.0, 0.0);
        harness.drag_at(from);
        harness.run();
        harness.hover_at(from + egui::vec2(-20.0, 0.0));
        harness.run();
        harness.hover_at(to);
        harness.run();
        harness.drop_at(to);
        harness.run();
        drop(harness);
        (layout, n.tabs2, n.page, n.plot, n.tabs3, n.empty, n.states)
    };
    let (layout, tabs2, page, plot, ..) = drag("Tab 2", "Plot 1");
    assert_eq!(children(&layout.borrow(), tabs2), [page, plot]);
    let (layout, _, _, _, tabs3, empty, states) = drag("Tab 3", "States 1");
    assert_eq!(children(&layout.borrow(), tabs3), [empty, states]);
}

#[test]
fn the_tab_of_a_nested_group_is_deleted_with_its_views_and_comes_back_by_undo() {
    let mut n = nested();
    let layout = &mut n.layout;
    let plot2 = layout.add_view_into(n.page, ViewType::Plot);
    let inner = layout.add_view_into(n.page, ViewType::Distribution);
    distribution(layout, inner).link = Some(view_id(plot2));
    let outer = layout.add_view_into(n.row, ViewType::Distribution);
    distribution(layout, outer).link = Some(view_id(plot2));

    layout.delete(n.page);
    assert_eq!(children(layout, n.tabs2), [n.plot]);
    assert_eq!(layout.take_deleted().as_deref(), Some("Tab 2"));
    assert_eq!(distribution(layout, outer).link, None);

    layout.undo_delete();
    assert_eq!(children(layout, n.tabs2), [n.plot, n.page]);
    assert_eq!(children(layout, n.page), [n.logs, plot2, inner]);
    assert_eq!(distribution(layout, inner).link, Some(view_id(plot2)));
    assert_eq!(distribution(layout, outer).link, Some(view_id(plot2)));
    assert_eq!(layout.selected, Some(n.page));
    assert!(layout.tree.active_tiles().contains(&n.logs));
}

/// `layout` saved to JSON and loaded again.
fn through_json(layout: &Layout) -> (Layout, String) {
    let json = serde_json::to_string(&layout.to_saved()).unwrap();
    (
        Layout::from_saved(serde_json::from_str(&json).unwrap()),
        json,
    )
}

#[test]
fn nested_groups_survive_saving_and_loading() {
    let mut n = nested();
    n.layout.select(n.page);
    n.layout.select(n.empty);
    let (loaded, json) = through_json(&n.layout);
    let layout = &loaded;

    let tab = named(layout, "Tab 1");
    assert_eq!(names(layout, tab), ["XY 1", "Group 1", "Group 2"]);
    let tabs2 = named(layout, "Group 1");
    assert_eq!(names(layout, tabs2), ["Plot 1", "Tab 2"]);
    assert_eq!(active(layout, tabs2), Some(named(layout, "Tab 2")));
    let row = named(layout, "Group 2");
    assert_eq!(names(layout, row), ["Table 1", "Group 3"]);
    assert_eq!(kind(layout, row), GroupKind::Horizontal);
    let tabs3 = named(layout, "Group 3");
    assert_eq!(names(layout, tabs3), ["States 1", "Tab 3"]);
    assert_eq!(active(layout, tabs3), Some(named(layout, "Tab 3")));
    let empty = named(layout, "Tab 3");
    assert!(children(layout, empty).is_empty());
    assert_eq!(kind(layout, empty), GroupKind::Vertical);
    assert_eq!(kind(layout, tab), GroupKind::Vertical);
    assert_eq!(kind(layout, tabs2), GroupKind::Tabs);
    assert_eq!(kind(layout, tabs3), GroupKind::Tabs);
    assert_eq!(kind(layout, named(layout, "Tab 2")), GroupKind::Vertical);

    assert_eq!(through_json(layout).1, json);
}

#[test]
fn a_presets_file_with_nested_groups_from_before_loads_as_it_did() {
    let layout = Layout::from_saved(serde_json::from_str(BEFORE_NESTING).unwrap());
    let main = named(&layout, "Main");
    assert_eq!(kind(&layout, main), GroupKind::Vertical);
    assert_eq!(names(&layout, main), ["Speed", "Details"]);
    assert_eq!(layout.to_saved().children.len(), 1);
    let file::Node::Group(saved_main) = &layout.to_saved().children[0] else {
        panic!("Main is no group");
    };
    assert_eq!(saved_main.shares, [1.0, 2.0]);
    let details = named(&layout, "Details");
    assert_eq!(kind(&layout, details), GroupKind::Tabs);
    assert_eq!(names(&layout, details), ["Log", "Empty"]);
    let empty = named(&layout, "Empty");
    assert_eq!(active(&layout, details), Some(empty));
    assert!(children(&layout, empty).is_empty());
}

#[test]
fn a_group_left_empty_by_a_delete_is_removed_with_the_groups_above_it_it_empties() {
    // (a) a group in a tab
    let mut n = nested();
    let layout = &mut n.layout;
    let t = layout
        .add_view_beside(n.logs, Side::Right, ViewType::Table)
        .unwrap();
    let g = children(layout, n.page)[0];
    assert_eq!(children(layout, n.page), [g]);
    assert_eq!(kind(layout, g), GroupKind::Horizontal);
    assert_eq!(children(layout, g), [n.logs, t]);
    layout.delete(n.logs);
    layout.delete(t);
    assert!(layout.tree.tiles.get(g).is_none());
    assert_eq!(children(layout, n.tabs2), [n.plot, n.page]);
    assert!(children(layout, n.page).is_empty());
    layout.undo_delete();
    assert_eq!(children(layout, n.page), [g]);
    assert_eq!(children(layout, g), [t]);

    // (b) groups up to a group that is no tab
    let mut n = nested();
    let layout = &mut n.layout;
    let p = layout
        .add_view_beside(n.table, Side::Bottom, ViewType::Plot)
        .unwrap();
    let v = children(layout, n.row)[0];
    assert_eq!(children(layout, n.row), [v, n.tabs3]);
    assert_eq!(children(layout, v), [n.table, p]);
    layout.delete(n.tabs3);
    layout.delete(n.table);
    layout.delete(p);
    assert!(layout.tree.tiles.get(v).is_none());
    assert!(layout.tree.tiles.get(n.row).is_none());
    assert_eq!(children(layout, n.tab), [n.xy, n.tabs2]);
    layout.undo_delete();
    assert_eq!(children(layout, n.row), [v]);
    assert_eq!(children(layout, v), [p]);
}

#[test]
fn a_group_left_empty_by_a_move_is_removed() {
    let mut n = nested();
    let layout = &mut n.layout;
    let t = layout
        .add_view_beside(n.logs, Side::Right, ViewType::Table)
        .unwrap();
    let g = children(layout, n.page)[0];
    layout.drop_tile(n.logs, n.xy, Drop::Side(Side::Bottom));
    layout.drop_tile(t, n.xy, Drop::Side(Side::Bottom));
    assert!(layout.tree.tiles.get(g).is_none());
    assert!(layout.tree.tiles.get(n.page).is_none());
    assert_eq!(children(layout, n.tabs2), [n.plot]);
}

#[test]
fn a_tab_left_empty_by_a_delete_stays() {
    let mut n = nested();
    let layout = &mut n.layout;
    layout.delete(n.logs);
    assert!(children(layout, n.page).is_empty());
    assert_eq!(children(layout, n.tabs2), [n.plot, n.page]);
    layout.delete(n.states);
    assert_eq!(children(layout, n.tabs3), [n.empty]);
}

#[test]
fn a_nested_tab_group_without_tabs_is_removed() {
    let mut n = nested();
    let layout = &mut n.layout;
    layout.delete(n.states);
    layout.delete(n.empty);
    assert!(layout.tree.tiles.get(n.tabs3).is_none());
    assert_eq!(children(layout, n.row), [n.table]);
    assert_eq!(layout.take_deleted().as_deref(), Some("Tab 3"));
    layout.undo_delete();
    assert_eq!(children(layout, n.tabs3), [n.empty]);
    assert_eq!(layout.selected, Some(n.empty));

    let mut n = nested();
    let layout = &mut n.layout;
    layout.drop_tile(n.plot, n.xy, Drop::Side(Side::Bottom));
    layout.drop_tile(n.page, n.xy, Drop::Side(Side::Bottom));
    assert!(layout.tree.tiles.get(n.tabs2).is_none());

    // the top level stays
    let mut n = nested();
    n.layout.delete(n.tab);
    assert!(children(&n.layout, n.layout.root()).is_empty());
}

#[test]
fn side_drops_at_depth_3_and_4_go_next_to_the_view_or_group_with_it() {
    let mut n = nested();
    n.layout.drop_tile(n.xy, n.table, Drop::Side(Side::Left));
    assert_eq!(children(&n.layout, n.row), [n.xy, n.table, n.tabs3]);

    let mut n = nested();
    let layout = &mut n.layout;
    layout.drop_tile(n.xy, n.table, Drop::Side(Side::Bottom));
    let v = children(layout, n.row)[0];
    assert_eq!(children(layout, n.row), [v, n.tabs3]);
    assert_eq!(kind(layout, v), GroupKind::Vertical);
    assert_eq!(children(layout, v), [n.table, n.xy]);
    assert_eq!(layout.name(v), "Group 4");

    let mut n = nested();
    let layout = &mut n.layout;
    layout.drop_tile(n.table, n.states, Drop::Side(Side::Right));
    let h = children(layout, n.tabs3)[0];
    assert_eq!(children(layout, n.tabs3), [h, n.empty]);
    assert_eq!(kind(layout, h), GroupKind::Horizontal);
    assert_eq!(children(layout, h), [n.states, n.table]);
    layout.drop_tile(n.table, n.states, Drop::Side(Side::Top));
    assert_eq!(kind(layout, h), GroupKind::Vertical);
    assert_eq!(children(layout, h), [n.table, n.states]);
}

#[test]
fn a_middle_drop_makes_a_tab_group_in_the_place_of_the_view() {
    let mut n = nested();
    let layout = &mut n.layout;
    layout.drop_tile(n.table, n.xy, Drop::Into);
    let g = children(layout, n.tab)[0];
    assert_eq!(children(layout, n.tab), [g, n.tabs2, n.row]);
    assert_eq!(kind(layout, g), GroupKind::Tabs);
    assert_eq!(children(layout, g), [n.xy, n.table]);
    assert_eq!(layout.name(g), "Group 4");
    assert_eq!(active(layout, g), Some(n.table));
    assert_eq!(layout.selected, Some(n.table));

    let mut n = nested();
    let layout = &mut n.layout;
    layout.drop_tile(n.xy, n.table, Drop::Into);
    let g = children(layout, n.row)[0];
    assert_eq!(children(layout, n.row), [g, n.tabs3]);
    assert_eq!(children(layout, g), [n.table, n.xy]);
    assert_eq!(active(layout, g), Some(n.xy));

    let mut n = nested();
    let layout = &mut n.layout;
    layout.drop_tile(n.xy, n.logs, Drop::Into);
    let g = children(layout, n.page)[0];
    assert_eq!(children(layout, n.page), [g]);
    assert_eq!(children(layout, g), [n.logs, n.xy]);

    let (mut layout, tab, plot, logs) = default_layout();
    layout.drop_tile(plot, tab, Drop::Before);
    layout.drop_tile(logs, plot, Drop::Into);
    let g = children(&layout, layout.root())[0];
    assert_eq!(children(&layout, layout.root()), [g]);
    assert_eq!(kind(&layout, g), GroupKind::Tabs);
    assert_eq!(children(&layout, g), [plot, logs]);
    assert!(layout.name(g).starts_with("Tab "));
}

#[test]
fn a_middle_drop_on_the_only_other_child_turns_the_group_into_tabs() {
    let (mut layout, tab, plot, logs) = default_layout();
    layout.drop_tile(logs, plot, Drop::Into);
    assert_eq!(kind(&layout, tab), GroupKind::Tabs);
    assert_eq!(children(&layout, tab), [plot, logs]);
    assert_eq!(active(&layout, tab), Some(logs));
    assert_eq!(children(&layout, layout.root()), [tab]);

    let mut n = nested();
    n.layout.drop_tile(n.tabs3, n.table, Drop::Into);
    assert_eq!(kind(&n.layout, n.row), GroupKind::Tabs);
    assert_eq!(children(&n.layout, n.row), [n.table, n.tabs3]);
    assert_eq!(active(&n.layout, n.row), Some(n.tabs3));
}

#[test]
fn a_group_is_not_dropped_into_itself_at_any_depth() {
    let mut n = nested();
    let before = serde_json::to_string(&n.layout.to_saved()).unwrap();
    n.layout.drop_tile(n.row, n.states, Drop::Into);
    n.layout.drop_tile(n.tabs2, n.logs, Drop::Side(Side::Left));
    n.layout.drop_tile(n.tabs3, n.tabs3, Drop::Into);
    assert_eq!(serde_json::to_string(&n.layout.to_saved()).unwrap(), before);
}

#[test]
fn dropping_on_a_tab_row_adds_a_tab_at_every_depth() {
    let mut n = nested();
    let layout = &mut n.layout;
    layout.drop_tile(n.xy, n.plot, Drop::Before);
    assert_eq!(children(layout, n.tabs2), [n.xy, n.plot, n.page]);
    layout.drop_tile(n.table, n.page, Drop::After);
    assert_eq!(children(layout, n.tabs2), [n.xy, n.plot, n.page, n.table]);

    let mut n = nested();
    let layout = &mut n.layout;
    layout.drop_tile(n.xy, n.tabs3, Drop::Into);
    assert_eq!(children(layout, n.tabs3), [n.states, n.empty, n.xy]);
    assert_eq!(active(layout, n.tabs3), Some(n.xy));
    assert_eq!(layout.selected, Some(n.xy));
    let root = layout.root();
    layout.drop_tile(n.table, root, Drop::Into);
    assert_eq!(children(layout, root), [n.tab, n.table]);
    layout.drop_tile(n.row, n.tabs2, Drop::Into);
    assert_eq!(children(layout, n.tabs2), [n.plot, n.page, n.row]);
}

#[test]
fn dropping_onto_an_empty_tab_puts_it_in() {
    let mut n = nested();
    n.layout.drop_tile(n.xy, n.empty, Drop::Into);
    assert_eq!(children(&n.layout, n.empty), [n.xy]);
    assert_eq!(children(&n.layout, n.tabs3), [n.states, n.empty]);
}

/// The one of the nodes labelled `label` whose centre lies in `rect`, e.g. the + of one of
/// several tab bars.
fn button_in<'h>(
    harness: &'h Harness<'static>,
    label: &'h str,
    rect: egui::Rect,
) -> egui_kittest::Node<'h> {
    harness
        .get_all_by_label(label)
        .find(|node| rect.contains(node.rect().center()))
        .unwrap_or_else(|| panic!("no {label} in {rect:?}"))
}

/// The rect of `tile_id` after the harness ran.
fn rect_of(layout: &Rc<RefCell<Layout>>, tile_id: TileId) -> egui::Rect {
    layout.borrow().tree.tiles.rect(tile_id).expect("shown")
}

/// A harness of the nested layout, and what it shows.
fn nested_harness(
    builder: egui_kittest::HarnessBuilder,
) -> (Rc<RefCell<Layout>>, Nested2, Harness<'static>) {
    let n = nested();
    let ids = Nested2 {
        tab: n.tab,
        xy: n.xy,
        tabs2: n.tabs2,
        plot: n.plot,
        page: n.page,
        row: n.row,
        table: n.table,
        tabs3: n.tabs3,
        states: n.states,
        empty: n.empty,
    };
    let layout = Rc::new(RefCell::new(n.layout));
    let mut harness = center_harness_with(layout.clone(), builder);
    harness.run();
    (layout, ids, harness)
}

/// The ids of `Nested` without the layout.
#[derive(Clone, Copy)]
struct Nested2 {
    tab: TileId,
    xy: TileId,
    tabs2: TileId,
    plot: TileId,
    page: TileId,
    row: TileId,
    table: TileId,
    tabs3: TileId,
    states: TileId,
    empty: TileId,
}

fn default_builder() -> egui_kittest::HarnessBuilder {
    Harness::builder().with_size(egui::vec2(800.0, 600.0))
}

fn assert_new_tab(layout: &Layout, group: TileId, tab: TileId, name: &str) {
    assert_eq!(layout.name(tab), name);
    assert_eq!(kind(layout, tab), GroupKind::Vertical);
    assert!(children(layout, tab).is_empty());
    assert_eq!(children(layout, group).last(), Some(&tab));
    assert_eq!(active(layout, group), Some(tab));
    assert_eq!(layout.selected, Some(tab));
}

#[test]
fn the_plus_adds_an_empty_tab_at_the_end_of_its_group() {
    let mut n = nested();
    let new = n.layout.add_tab_to(n.tabs3);
    assert_eq!(children(&n.layout, n.tabs3), [n.states, n.empty, new]);
    assert_new_tab(&n.layout, n.tabs3, new, "Tab 4");
    let new = n.layout.add_tab_to(n.tabs2);
    assert_eq!(children(&n.layout, n.tabs2), [n.plot, n.page, new]);
    assert_new_tab(&n.layout, n.tabs2, new, "Tab 5");
}

#[test]
fn the_plus_of_a_nested_tab_bar_adds_a_tab_and_opens_it() {
    let (layout, n, mut harness) = nested_harness(default_builder());
    let plus = egui_phosphor::regular::PLUS;
    button_in(&harness, plus, rect_of(&layout, n.tabs3)).click();
    harness.run();
    {
        let layout = layout.borrow();
        let new = *children(&layout, n.tabs3).last().unwrap();
        assert_eq!(children(&layout, n.tabs3).len(), 3);
        assert_new_tab(&layout, n.tabs3, new, "Tab 4");
    }
    let top = harness
        .get_all_by_label(plus)
        .min_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
        .unwrap();
    top.click();
    harness.run();
    let layout = layout.borrow();
    let root = children(&layout, layout.root());
    assert_eq!(root.len(), 2);
    assert_eq!(layout.name(root[1]), "Tab 5");
}

#[test]
fn nested_tabs_are_opened_selected_and_renamed_by_clicks() {
    for (label, group) in [("Tab 2", 2), ("Tab 3", 3)] {
        let builder = default_builder().with_step_dt(0.04);
        let (layout, n, mut harness) = nested_harness(builder);
        let (group, tab, first) = if group == 2 {
            (n.tabs2, n.page, n.plot)
        } else {
            (n.tabs3, n.empty, n.states)
        };
        let wait = |harness: &mut Harness<'_>| {
            for _ in 0..25 {
                harness.step();
            }
        };
        let selected_before = layout.borrow().selected;
        assert_eq!(active(&layout.borrow(), group), Some(first));
        harness.get_by_label(label).click();
        wait(&mut harness);
        assert_eq!(active(&layout.borrow(), group), Some(tab));
        assert_eq!(layout.borrow().selected, selected_before);
        harness.get_by_label(label).click();
        wait(&mut harness);
        assert_eq!(layout.borrow().selected, Some(tab));
        assert!(layout.borrow().renaming.is_none());
        harness.get_by_label(label).click();
        harness.get_by_label(label).click();
        harness.run();
        let renaming = layout.borrow().renaming.as_ref().map(|r| r.tile_id);
        assert_eq!(renaming, Some(tab));
    }
}

#[test]
fn the_cross_on_a_hovered_nested_tab_deletes_it() {
    let (layout, n, mut harness) = nested_harness(default_builder());
    harness.get_by_label("Tab 2").hover();
    harness.run();
    harness.get_by_label(egui_phosphor::regular::X).click();
    harness.run();
    let mut layout = layout.borrow_mut();
    assert_eq!(children(&layout, n.tabs2), [n.plot]);
    assert_eq!(layout.take_deleted().as_deref(), Some("Tab 2"));
    layout.undo_delete();
    assert_eq!(children(&layout, n.tabs2), [n.plot, n.page]);
}

#[test]
fn a_nested_tab_tells_how_to_rename_and_move_it() {
    let (_, _, mut harness) = nested_harness(default_builder());
    harness.get_by_label("States 1").hover();
    harness.run();
    harness.get_by_label("Double-click to rename, drag to move, right-click for more");
}

#[test]
fn a_view_shown_as_a_nested_tab_has_the_menu_of_a_view() {
    let (layout, n, mut harness) = nested_harness(default_builder());
    harness.get_by_label("States 1").click_secondary();
    harness.run();
    harness.get_by_label_contains("Add below").hover();
    harness.run();
    harness
        .get_by_label(&format!("{} Table", ViewType::Table.icon()))
        .click();
    harness.run();
    let layout = layout.borrow();
    let v = children(&layout, n.tabs3)[0];
    assert_eq!(children(&layout, n.tabs3), [v, n.empty]);
    assert_eq!(kind(&layout, v), GroupKind::Vertical);
    let new = children(&layout, v)[1];
    assert_eq!(children(&layout, v)[0], n.states);
    assert_eq!(layout.name(new), "Table 2");
}

#[test]
fn empty_group_from_the_menu_adds_a_tab_group_with_one_empty_tab() {
    let check = |n: &Nested, new: TileId, group_name: &str| {
        let g = n.layout.parent(new).unwrap();
        assert_eq!(kind(&n.layout, g), GroupKind::Tabs);
        assert_eq!(n.layout.name(g), group_name);
        assert_eq!(children(&n.layout, g), [new]);
        assert_eq!(n.layout.name(new), "Tab 4");
        assert_eq!(kind(&n.layout, new), GroupKind::Vertical);
        assert!(children(&n.layout, new).is_empty());
        assert_eq!(n.layout.selected, Some(new));
        assert_eq!(active(&n.layout, g), Some(new));
        g
    };
    let mut n = nested();
    let new = n.layout.add_empty_group_beside(n.xy, Side::Bottom).unwrap();
    let g = check(&n, new, "Group 4");
    assert_eq!(children(&n.layout, n.tab), [n.xy, g, n.tabs2, n.row]);
    let mut n = nested();
    let new = n.layout.add_empty_group_beside(n.xy, Side::Right).unwrap();
    let g = check(&n, new, "Group 4");
    let h = n.layout.parent(g).unwrap();
    assert_eq!(children(&n.layout, n.tab), [h, n.tabs2, n.row]);
    assert_eq!(kind(&n.layout, h), GroupKind::Horizontal);
    assert_eq!(n.layout.name(h), "Group 5");
    assert_eq!(children(&n.layout, h), [n.xy, g]);

    let mut n = nested();
    let new = n
        .layout
        .add_empty_group_beside(n.table, Side::Right)
        .unwrap();
    let g = check(&n, new, "Group 4");
    assert_eq!(children(&n.layout, n.row), [n.table, g, n.tabs3]);

    let mut n = nested();
    let new = n
        .layout
        .add_empty_group_beside(n.states, Side::Bottom)
        .unwrap();
    let g = check(&n, new, "Group 4");
    let v = children(&n.layout, n.tabs3)[0];
    assert_eq!(children(&n.layout, n.tabs3), [v, n.empty]);
    assert_eq!(kind(&n.layout, v), GroupKind::Vertical);
    assert_eq!(children(&n.layout, v), [n.states, g]);

    let mut n = nested();
    let tiles = n.layout.tree.tiles.len();
    let root = n.layout.root();
    assert_eq!(n.layout.add_empty_group_beside(root, Side::Right), None);
    assert_eq!(n.layout.tree.tiles.len(), tiles);
}

#[test]
fn add_right_and_below_on_a_nested_tab_go_at_the_side_of_its_whole_group() {
    let mut n = nested();
    let l = &mut n.layout;
    let t = l
        .add_view_beside(n.page, Side::Right, ViewType::Table)
        .unwrap();
    assert_eq!(kind(l, n.page), GroupKind::Horizontal);
    assert_eq!(children(l, n.page), [n.logs, t]);
    assert_eq!(l.name(n.page), "Tab 2");
    assert_eq!(children(l, n.tabs2), [n.plot, n.page]);
    let p = l
        .add_view_beside(n.page, Side::Right, ViewType::Plot)
        .unwrap();
    assert_eq!(children(l, n.page), [n.logs, t, p]);
    let g = l.add_empty_group_beside(n.page, Side::Bottom).unwrap();
    let g = l.parent(g).unwrap();
    assert_eq!(kind(l, n.page), GroupKind::Vertical);
    let inner = children(l, n.page)[0];
    assert_eq!(children(l, n.page), [inner, g]);
    assert_eq!(kind(l, inner), GroupKind::Horizontal);
    assert_eq!(children(l, inner), [n.logs, t, p]);
    assert!(l.name(inner).starts_with("Group "));

    let mut n = nested();
    let p = n
        .layout
        .add_view_beside(n.empty, Side::Right, ViewType::Plot)
        .unwrap();
    assert_eq!(kind(&n.layout, n.empty), GroupKind::Horizontal);
    assert_eq!(children(&n.layout, n.empty), [p]);

    let mut n = nested();
    n.layout.set_group_kind(n.page, GroupKind::Tabs);
    let t = n
        .layout
        .add_view_beside(n.page, Side::Right, ViewType::Table)
        .unwrap();
    assert_eq!(kind(&n.layout, n.page), GroupKind::Horizontal);
    let inner = children(&n.layout, n.page)[0];
    assert_eq!(children(&n.layout, n.page), [inner, t]);
    assert_eq!(kind(&n.layout, inner), GroupKind::Tabs);
    assert_eq!(children(&n.layout, inner), [n.logs]);
    assert_eq!(active(&n.layout, inner), Some(n.logs));
}

#[test]
fn the_menus_list_the_view_types_then_empty_group_and_top_tabs_have_none() {
    let (layout, n, mut harness) = nested_harness(default_builder());
    harness.get_by_label("XY 1").click_secondary();
    harness.run();
    harness.get_by_label_contains("Add below").hover();
    harness.run();
    for view_type in ViewType::all() {
        harness.get_by_label(&format!("{} {}", view_type.icon(), view_type.name()));
    }
    let actions_label = format!("{} {}", ViewType::Actions.icon(), ViewType::Actions.name());
    let actions = harness.get_by_label(&actions_label);
    let group = harness.get_by_label_contains("Empty group");
    assert!(group.rect().top() > actions.rect().top());
    group.click();
    harness.run();
    {
        let layout = layout.borrow();
        let g = children(&layout, n.tab)[1];
        assert_eq!(children(&layout, n.tab), [n.xy, g, n.tabs2, n.row]);
    }

    harness.get_by_label("Tab 2").click_secondary();
    harness.run();
    harness.get_by_label_contains("Add right").hover();
    harness.run();
    harness.get_by_label_contains("Empty group").click();
    harness.run();
    {
        let layout = layout.borrow();
        let g = children(&layout, n.page)[1];
        assert_eq!(kind(&layout, n.page), GroupKind::Horizontal);
        assert_eq!(layout.name(children(&layout, n.page)[0]), "Logs 1");
        assert_eq!(kind(&layout, g), GroupKind::Tabs);
    }

    harness.get_by_label("Tab 1").click_secondary();
    harness.run();
    assert!(harness.query_by_label_contains("Add right").is_none());
    assert!(harness.query_by_label_contains("Add below").is_none());
    harness.get_by_label_contains("Rename");
}

/// Drags the tile with the title `from` to `to` and drops it there.
fn drag_to(harness: &mut Harness<'_>, from: &str, to: egui::Pos2) {
    let title = harness.get_by_label(from).rect().center();
    harness.drag_at(title);
    harness.run();
    harness.hover_at(title + egui::vec2(40.0, 40.0));
    harness.run();
    harness.hover_at(to);
    harness.run();
    harness.drop_at(to);
    harness.run();
}

/// A point in the empty part of the tab row of `group`, left of its right end.
fn empty_row_of(layout: &Rc<RefCell<Layout>>, group: TileId) -> egui::Pos2 {
    let rect = rect_of(layout, group);
    egui::pos2(rect.right() - 20.0, rect.top() + 14.0)
}

#[test]
fn dragging_a_view_to_the_middle_of_another_stacks_them_as_tabs() {
    let (layout, n, mut harness) = nested_harness(default_builder());
    let to = rect_of(&layout, n.table).center();
    drag_to(&mut harness, "XY 1", to);
    let layout = layout.borrow();
    let g = children(&layout, n.row)[0];
    assert_eq!(children(&layout, n.row), [g, n.tabs3]);
    assert_eq!(kind(&layout, g), GroupKind::Tabs);
    assert_eq!(children(&layout, g), [n.table, n.xy]);
    assert_eq!(layout.selected, Some(n.xy));
}

#[test]
fn dropping_on_the_empty_part_of_a_nested_tab_row_adds_the_last_tab() {
    let (layout, n, mut harness) = nested_harness(default_builder());
    let to = empty_row_of(&layout, n.tabs2);
    drag_to(&mut harness, "XY 1", to);
    let layout = layout.borrow();
    assert_eq!(children(&layout, n.tabs2), [n.plot, n.page, n.xy]);
    assert_eq!(active(&layout, n.tabs2), Some(n.xy));
}

#[test]
fn hovering_a_nested_tab_while_dragging_opens_it_and_an_empty_tab_takes_the_drop() {
    let (layout, n, mut harness) = nested_harness(default_builder());
    let title = harness.get_by_label("XY 1").rect().center();
    harness.drag_at(title);
    harness.run();
    harness.hover_at(title + egui::vec2(40.0, 40.0));
    harness.run();
    harness.hover_at(harness.get_by_label("Tab 3").rect().center());
    harness.run();
    assert_eq!(active(&layout.borrow(), n.tabs3), Some(n.empty));
    let to = rect_of(&layout, n.empty).center();
    harness.hover_at(to);
    harness.run();
    harness.drop_at(to);
    harness.run();
    assert_eq!(children(&layout.borrow(), n.empty), [n.xy]);
}

#[test]
fn the_empty_part_of_a_nested_tab_row_drags_nothing_and_has_no_menu() {
    let (layout, n, mut harness) = nested_harness(default_builder());
    let from = empty_row_of(&layout, n.tabs2);
    harness.drag_at(from);
    harness.run();
    harness.hover_at(from + egui::vec2(-30.0, 40.0));
    harness.run();
    let to = rect_of(&layout, n.table).center();
    harness.hover_at(to);
    harness.run();
    harness.drop_at(to);
    harness.run();
    {
        let layout = layout.borrow();
        assert_eq!(children(&layout, n.tab), [n.xy, n.tabs2, n.row]);
        assert_eq!(children(&layout, n.tabs2), [n.plot, n.page]);
        assert_eq!(children(&layout, n.row), [n.table, n.tabs3]);
    }
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }
    assert!(harness.query_by_label_contains("Rename").is_none());
    assert!(harness.query_by_label_contains("Delete").is_none());
}

/// The tab of a new layout, empty, shown in a harness of `size`.
fn empty_tab_harness(size: egui::Vec2) -> (Rc<RefCell<Layout>>, TileId, Harness<'static>) {
    let (mut layout, _, _, _) = default_layout();
    let tab = layout.add_tab();
    let layout = Rc::new(RefCell::new(layout));
    // in a larger screen, so that the menu of the smallest picker fits on it
    let builder = Harness::builder().with_size(size + egui::vec2(0.0, 300.0));
    let mut harness = center_harness_in(layout.clone(), builder, Some(size));
    harness.run();
    (layout, tab, harness)
}

/// Asserts that `tab` holds one Heatmap and that it is selected.
fn assert_holds_a_heatmap(layout: &Rc<RefCell<Layout>>, tab: TileId) {
    let layout = layout.borrow();
    let views = children(&layout, tab);
    assert_eq!(views.len(), 1);
    assert_eq!(
        layout.view(views[0]).unwrap().view_type(),
        ViewType::Heatmap
    );
    assert_eq!(layout.selected, Some(views[0]));
}

#[test]
fn a_small_empty_tab_shows_the_view_types_as_icons() {
    let (layout, tab, mut harness) = empty_tab_harness(egui::vec2(300.0, 160.0));
    assert!(harness.query_by_label("Heatmap").is_none());
    let icon = ViewType::Heatmap.icon();
    harness.get_by_label(icon).hover();
    harness.run();
    harness.run();
    harness.get_by_label("Heatmap");
    harness.get_by_label(icon).click();
    harness.run();
    assert_holds_a_heatmap(&layout, tab);
}

#[test]
fn a_tiny_empty_tab_shows_the_view_types_in_a_menu() {
    let (layout, tab, mut harness) = empty_tab_harness(egui::vec2(300.0, 80.0));
    for view_type in ViewType::all() {
        assert!(harness.query_by_label(view_type.icon()).is_none());
    }
    harness.get_by_label_contains("Add a view").click();
    harness.run();
    harness.get_by_label_contains("Heatmap").click();
    harness.run();
    assert_holds_a_heatmap(&layout, tab);
}

/// Asserts that the Heatmap icon lies in the card of `group`, below its tab row, and in
/// `tab`, then clicks it.
fn click_the_heatmap_icon_in(
    layout: &Rc<RefCell<Layout>>,
    harness: &mut Harness<'static>,
    group: TileId,
    tab: TileId,
) {
    // the form with icons, not the list with names
    assert!(harness.query_by_label("Heatmap").is_none());
    let icon = harness.get_by_label(ViewType::Heatmap.icon());
    let rect = icon.rect();
    assert!(rect_of(layout, tab).contains_rect(rect));
    let card = rect_of(layout, group);
    assert!(card.contains_rect(rect));
    assert!(rect.top() >= card.top() + TAB_BAR_HEIGHT);
    icon.click();
    harness.run();
}

#[test]
fn an_empty_nested_tab_shows_the_view_types_and_takes_a_view() {
    let mut n = nested();
    n.layout.select(n.empty);
    let (tabs3, empty) = (n.tabs3, n.empty);
    let layout = Rc::new(RefCell::new(n.layout));
    let mut harness = center_harness(layout.clone());
    harness.run();
    click_the_heatmap_icon_in(&layout, &mut harness, tabs3, empty);
    {
        let layout = layout.borrow();
        let views = children(&layout, empty);
        assert_eq!(views.len(), 1);
        assert_eq!(
            layout.view(views[0]).unwrap().view_type(),
            ViewType::Heatmap
        );
    }

    // the new tab of a group at depth 2
    let mut n = nested();
    let new = n.layout.add_tab_to(n.tabs2);
    let tabs2 = n.tabs2;
    let layout = Rc::new(RefCell::new(n.layout));
    let mut harness = center_harness(layout.clone());
    harness.run();
    click_the_heatmap_icon_in(&layout, &mut harness, tabs2, new);
    assert_eq!(children(&layout.borrow(), new).len(), 1);
}

#[test]
fn an_empty_nested_tab_stays_until_filled_or_deleted() {
    let mut n = nested();
    let layout = &mut n.layout;
    layout.delete(n.logs);
    assert_eq!(children(layout, n.tabs2), [n.plot, n.page]);
    assert!(children(layout, n.page).is_empty());
    layout.set_name(n.page, "Spare".to_owned());
    assert_eq!(layout.name(n.page), "Spare");
    layout.delete(n.page);
    assert!(layout.tree.tiles.get(n.page).is_none());
    assert_eq!(children(layout, n.tabs2), [n.plot]);
    layout.undo_delete();
    assert_eq!(children(layout, n.tabs2), [n.plot, n.page]);
    assert!(children(layout, n.page).is_empty());
}

#[test]
fn the_plus_of_a_nested_tab_bar_opens_a_tab_that_shows_the_view_types() {
    let (layout, n, mut harness) = nested_harness(default_builder());
    let plus = egui_phosphor::regular::PLUS;
    button_in(&harness, plus, rect_of(&layout, n.tabs2)).click();
    harness.run();
    let new = *children(&layout.borrow(), n.tabs2).last().unwrap();
    click_the_heatmap_icon_in(&layout, &mut harness, n.tabs2, new);
    assert_eq!(children(&layout.borrow(), new).len(), 1);
}

#[test]
fn nested_tabs_lie_in_the_title_row_of_one_card() {
    let (layout, n, harness) = nested_harness(default_builder());
    for (group, label, view) in [(n.tabs3, "States 1", n.states), (n.tabs2, "Plot 1", n.plot)] {
        let r = rect_of(&layout, group);
        let b = harness
            .get_all_by_label(label)
            .map(|node| node.rect())
            .find(|rect| r.contains(rect.center()))
            .unwrap_or_else(|| panic!("no {label} in {r:?}"));
        assert!(b.top() >= r.top() - 0.5, "{label}: {b:?} in {r:?}");
        assert!(b.bottom() <= r.top() + TAB_BAR_HEIGHT + 0.5);
        let body = rect_of(&layout, view);
        assert!((body.top() - (r.top() + TAB_BAR_HEIGHT + theme::GAP)).abs() < 0.5);
        assert!((body.left() - r.left()).abs() < 0.5);
        assert!((body.right() - r.right()).abs() < 0.5);
    }
    let top = harness.get_by_label("Tab 1").rect();
    let xy = harness.get_by_label("XY 1").rect();
    assert!(xy.top() >= top.bottom() + theme::GAP);
}

/// A harness of a layout of one top tab holding `top`, and the layout.
fn top_tab_harness(top: Vec<file::Node>) -> (Rc<RefCell<Layout>>, Harness<'static>) {
    let tab = group_node("Tab 1", GroupKind::Horizontal, None, top);
    let saved = Group {
        active: Some(0),
        ..Group::tabs(vec![tab])
    };
    let layout = Rc::new(RefCell::new(Layout::from_saved(saved)));
    let mut harness = center_harness_with(layout.clone(), default_builder());
    harness.run();
    (layout, harness)
}

fn tabs_node(name: &str, open: usize, children: Vec<file::Node>) -> file::Node {
    group_node(name, GroupKind::Tabs, Some(open), children)
}

#[test]
fn a_tabbed_card_starts_as_high_as_the_card_of_a_view_beside_it() {
    let tabs = tabs_node(
        "Group 1",
        0,
        vec![
            view_node(ViewType::Plot, "Plot 1"),
            view_node(ViewType::Logs, "Logs 1"),
        ],
    );
    let (layout, harness) = top_tab_harness(vec![view_node(ViewType::Table, "Table 1"), tabs]);
    let (plain, tabbed) = (
        named(&layout.borrow(), "Table 1"),
        named(&layout.borrow(), "Group 1"),
    );
    let (plain, tabbed) = (rect_of(&layout, plain), rect_of(&layout, tabbed));
    assert!((plain.top() - tabbed.top()).abs() < 0.5);
    // a gap under the top tabs, which the tab row of the card starts after
    let top = harness.get_by_label("Tab 1").rect();
    assert!(tabbed.top() >= top.bottom() + theme::GAP);
    let row = harness.get_by_label("Plot 1").rect();
    assert!(row.top() >= tabbed.top() && row.bottom() <= tabbed.top() + TAB_BAR_HEIGHT);
}

#[test]
fn a_top_tab_of_only_a_tab_group_leaves_the_gap_above_its_card() {
    let tabs = tabs_node(
        "Group 1",
        0,
        vec![
            view_node(ViewType::Plot, "Plot 1"),
            view_node(ViewType::Logs, "Logs 1"),
        ],
    );
    let saved = Group {
        active: Some(0),
        ..Group::tabs(vec![tabs])
    };
    let layout = Rc::new(RefCell::new(Layout::from_saved(saved)));
    let mut harness = center_harness_with(layout.clone(), default_builder());
    harness.run();
    let root = rect_of(&layout, layout.borrow().root());
    let card = rect_of(&layout, named(&layout.borrow(), "Group 1"));
    assert!((card.top() - (root.top() + TAB_BAR_HEIGHT + theme::GAP)).abs() < 0.5);
}

#[test]
fn a_tab_group_in_the_open_tab_of_a_tabbed_card_lies_a_gap_inside_its_card() {
    let inner = tabs_node(
        "Inner",
        0,
        vec![
            view_node(ViewType::Plot, "Plot 1"),
            view_node(ViewType::Logs, "Logs 1"),
        ],
    );
    let outer = tabs_node(
        "Outer",
        0,
        vec![inner, view_node(ViewType::Table, "Table 1")],
    );
    let (layout, harness) = top_tab_harness(vec![outer]);
    let outer = rect_of(&layout, named(&layout.borrow(), "Outer"));
    let inner = rect_of(&layout, named(&layout.borrow(), "Inner"));
    assert!(outer.contains_rect(inner));
    assert!(inner.top() >= outer.top() + TAB_BAR_HEIGHT + theme::GAP - 0.5);
    let row = harness.get_by_label("Plot 1").rect();
    assert!(inner.contains_rect(row));
}

#[test]
fn the_remaining_open_tab_of_a_card_is_marked_open_in_the_frame_the_open_tab_leaves() {
    let (layout, n, mut harness) = nested_harness(default_builder());
    layout
        .borrow_mut()
        .drop_tile(n.states, n.xy, Drop::Side(Side::Right));
    harness.step();
    let accent = harness.ctx.global_style().visuals.tokens().accent;
    let row =
        rect_of(&layout, n.tabs3).with_max_y(rect_of(&layout, n.tabs3).top() + TAB_BAR_HEIGHT);
    let underlines = harness
        .output()
        .shapes
        .iter()
        .filter(|clipped| match &clipped.shape {
            egui::epaint::Shape::Rect(rect) => {
                rect.fill == accent && row.contains(rect.rect.center())
            }
            _ => false,
        })
        .count();
    assert_eq!(underlines, 1);
}
