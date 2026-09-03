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

#[derive(Debug, Clone, PartialEq)]
pub enum RemoteImageStatus {
    Present,
    Missing,
    SizeMismatch,
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
    anchored: bool,
    anchor: egui::Align2,
    anchor_offset: egui::Vec2,
    pub data_is_loading: bool,
    pub image_in_local: Option<bool>,
    pub image_in_remote: Option<RemoteImageStatus>,
    pub image_name: Option<String>,
    pub image_digest: Option<String>,
    pub image_inspect: Option<bollard::models::ImageInspect>,
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            title: "Información de imagen".to_owned(),
            title_bar: true,
            closable: true,
            collapsible: true,
            movable: true,
            resizable: false,
            constrain: true,
            scroll2: Vec2b::FALSE,
            anchored: false,
            anchor: egui::Align2::RIGHT_TOP,
            anchor_offset: egui::Vec2::ZERO,
            data_is_loading: false,
            image_in_local: None,
            image_in_remote: None,
            image_name: None,
            image_digest: None,
            image_inspect: None,
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
        anchored,
        anchor,
        anchor_offset,
        data_is_loading,
        image_name: _,
        image_in_local: _,
        image_in_remote: _,
        image_digest: _,
        image_inspect: _,
    } = options.clone();

    let enabled = !data_is_loading;
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
        title: _,
        title_bar: _,
        closable: _,
        collapsible: _,
        movable: _,
        resizable: _,
        constrain: _,
        scroll2: _,
        anchored: _,
        anchor: _,
        anchor_offset: _,
        data_is_loading: _,
        image_in_local,
        image_in_remote,
        image_name,
        image_digest,
        image_inspect,
    } = options;

    let Some(image_name) = image_name else {
        eprintln!("Window opened without image_name");
        return;
    };
    let Some(image_digest) = image_digest else {
        eprintln!("Window opened without image_digest");
        return;
    };
    let Some(image_inspect) = image_inspect else {
        ui.close_kind(egui::UiKind::Window);
        eprintln!("Window opened without image_inspect");
        return;
    };
    ui.label(format!("Imagen: \"{image_name}\""));
    //ui.label(format!("Digest: \"{image_digest}\""));
    //ui.label(format!("Inspect: \"{image_inspect:?}\""));
    ui.label(format!(
        "Tamaño: {}",
        humansize::format_size(image_inspect.size.unwrap() as u64, humansize::DECIMAL)
    ));
    ui.label(format!("Autor: {:?}", image_inspect.author));
    ui.label(format!("Arquitectura: {:?}", image_inspect.architecture));

    ui.separator();
    let local_str = match image_in_local {
        Some(local) => {
            if *local {
                "Sí"
            } else {
                "No"
            }
        }
        None => "Desconocido",
    };
    ui.label(format!("Está en local: {local_str}"));
    let remote_str = match image_in_remote {
        Some(remote_status) => match remote_status {
            RemoteImageStatus::Present => "Sí",
            RemoteImageStatus::Missing => "No",
            RemoteImageStatus::SizeMismatch => "No. Versión distinta ⚠",
        },
        None => "Desconocido",
    };
    ui.label(format!("Subida en el Registro: {remote_str}"));

    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Acciones: ");
        if ui.button("🗑").on_hover_text("Eliminar la imagen").clicked() {
            println!("Eliminar {image_digest}");
        }
    });
    //let on_top = Some(ui.layer_id()) == ui.ctx().top_layer_id();
    //ui.label(format!("This window is on top: {on_top}."));

    /*ui.separator();
    ui.horizontal(|ui| {
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
