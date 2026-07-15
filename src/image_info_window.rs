use eframe::egui;
use egui::Vec2b;

pub struct WindowState {
    pub options: WindowOptions,
    pub open: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            options: WindowOptions::default(),
	    open: false,
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct WindowOptions {
    title: String,
    title_bar: bool,
    closable: bool,
    collapsible: bool,
    movable: bool,
    resizable: bool,
    constrain: bool,
    scroll2: Vec2b,
    disabled_time: f64,
    anchored: bool,
    anchor: egui::Align2,
    anchor_offset: egui::Vec2,
    pub image_name: Option<String>,
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            title: "Información de imagen".to_owned(),
            title_bar: true,
            closable: true,
            collapsible: false,
            movable: true,
            resizable: false,
            constrain: true,
            scroll2: Vec2b::FALSE,
            disabled_time: f64::NEG_INFINITY,
            anchored: false,
            anchor: egui::Align2::RIGHT_TOP,
            anchor_offset: egui::Vec2::ZERO,
            image_name: None,
        }
    }
}

pub fn show(mut options: &mut WindowOptions, ui: &mut egui::Ui, open: &mut bool) {
    let WindowOptions {
        title,
        title_bar,
        closable,
        collapsible,
        movable,
        resizable,
        constrain,
        scroll2,
        disabled_time,
        anchored,
        anchor,
        anchor_offset,
        image_name: _,
    } = options.clone();

    let enabled = ui.input(|i| i.time) - disabled_time > 2.0;
    if !enabled {
        ui.request_repaint();
    }

    let mut window = egui::Window::new(title)
        .id(egui::Id::new("image_info_window_options")) // required since we change the title
        .resizable(resizable)
        .constrain(constrain)
        .collapsible(collapsible)
        .movable(movable)
        .title_bar(title_bar)
        .scroll(scroll2)
        .constrain_to(ui.available_rect_before_wrap())
        .enabled(enabled);
    if closable {
        window = window.open(open);
    }
    if anchored {
        window = window.anchor(anchor, anchor_offset);
    }
    let rect_size = ui.content_rect().size();
    window
        .default_pos(egui::pos2(rect_size.x / 2.0, rect_size.y / 2.0))
        .show(ui, |ui| ui_fn(&mut options, ui));
}

fn ui_fn(options: &mut WindowOptions, ui: &mut egui::Ui) {
    let WindowOptions {
        title,
        title_bar: _,
        closable: _,
        collapsible: _,
        movable: _,
        resizable: _,
        constrain: _,
        scroll2: _,
        disabled_time: _,
        anchored: _,
        anchor: _,
        anchor_offset: _,
        image_name,
    } = options;

    if let Some(image_name) = image_name {
        ui.label(format!("Imagen: \"{image_name}\""));
    } else {
	eprintln!("Window opened without image_name");
    }

    ui.horizontal(|ui| {
        ui.label("title:");
        ui.text_edit_singleline(title);
    });

    ui.separator();
    let on_top = Some(ui.layer_id()) == ui.ctx().top_layer_id();
    ui.label(format!("This window is on top: {on_top}."));

    /*ui.separator();
    ui.horizontal(|ui| {
    if ui.button("Disable for 2 seconds").clicked() {
    self.disabled_time = ui.input(|i| i.time);
    }
    egui::reset_button(ui, self, "Reset");
    if ui
    .button("Close")
    .on_hover_text("You can collapse / close Windows via Ui::close")
    .clicked()
    {
    // Calling close would close the collapsible within the window
    // ui.close();
    // Instead, we close the window itself
    ui.close_kind(UiKind::Window);
    }
    });
     */
}
