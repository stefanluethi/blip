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
    let model = MemoryStore::default();
    let mut time = TimeRange::default();
    let mut invoker = crate::store::actions::Invoker::default();
    Harness::builder()
        .with_size(egui::vec2(800.0, 600.0))
        .build_ui(move |ui| {
            let mut context = ViewContext {
                store: &model,
                time: &mut time,
                invoker: &mut invoker,
                formats: &crate::widgets::view::NO_FORMATS,
                titled: false,
                plots: &crate::widgets::view::NO_PLOTS,
            };
            layout.borrow_mut().ui(ui, &mut context);
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
