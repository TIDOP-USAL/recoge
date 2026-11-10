use eframe::egui;

pub fn textedit_context_menu(ui: &mut egui::Ui, content: &mut String) {
    if ui.button("Copiar todo").clicked() {
        ui.ctx().copy_text(content.clone());
        ui.close();
    }
    if ui.button("Pegar y reemplazar").clicked() {
        content.clear();
        ui.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
        ui.close();
    }
    if ui.button("Borrar").clicked() {
        content.clear();
    }
}
