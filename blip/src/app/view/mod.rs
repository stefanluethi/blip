pub mod plot;
pub mod log_table;

pub trait Viewable {
    fn view(&mut self, ui: &mut egui::Ui, model: &mut crate::app::model::Model);
}
