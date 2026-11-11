use eframe::egui;

pub fn textedit_context_menu(ui: &mut egui::Ui, content: &mut String, content_changed: &mut bool) {
    if ui.button("Copiar todo").clicked() {
        ui.ctx().copy_text(content.clone());
        ui.close();
    }
    if ui.button("Pegar y reemplazar").clicked() {
        *content_changed = true;
        content.clear();
        ui.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
        ui.close();
    }
    if ui.button("Borrar").clicked() {
        *content_changed = true;
        content.clear();
    }
}
