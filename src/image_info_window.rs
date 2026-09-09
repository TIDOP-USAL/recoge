use crate::container::ImageCollection;
use crate::zot::ImageDisplayMetadata;
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
    DateMismatch,
    //SizeMismatch,
    //DigestMismatch,
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
    pub selected_image_name: Option<String>,
    pub image_in_local: Option<bool>,
    pub image_in_remote: Option<RemoteImageStatus>,
    pub remote_images_metadata: Vec<ImageDisplayMetadata>,
    pub image_collection: Option<ImageCollection>,
    pub image_inspect: Option<bollard::models::ImageInspect>,
}

// TODO educe default
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
            selected_image_name: None,
            image_in_local: None,
            image_in_remote: None,
            remote_images_metadata: Vec::new(),
            image_collection: None,
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
        selected_image_name: _,
        image_in_local: _,
        image_in_remote: _,
        remote_images_metadata: _,
        image_collection: _,
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
    let ui_rect_size = ui.content_rect().size();
    let window_size = egui::vec2(ui_rect_size.x / 2.0, ui_rect_size.y / 2.0);
    window
        .default_pos(egui::pos2(ui_rect_size.x / 2.0, ui_rect_size.y / 2.0))
        .fixed_size(window_size)
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
        selected_image_name,
        image_in_local,
        image_in_remote,
        remote_images_metadata,
        image_collection,
        image_inspect,
    } = options;

    let Some(selected_image_name) = selected_image_name else {
        eprintln!("Window opened without selected_image_name");
        ui.close_kind(egui::UiKind::Window);
        return;
    };
    let Some(image_collection) = image_collection else {
        eprintln!("Window opened without image_collection");
        ui.close_kind(egui::UiKind::Window);
        return;
    };
    let Some(image_inspect) = image_inspect else {
        eprintln!("Window opened without image_inspect");
        ui.close_kind(egui::UiKind::Window);
        return;
    };
    let image_names = &mut image_collection.image_aliases.clone();

    egui::Grid::new("image_rows_grid_1")
        .num_columns(2)
        .spacing([20.0, 10.0])
        .striped(true)
        .show(ui, |ui| {
            let id = image_collection.get_short_id();
            ui.label("Imagen local:");
            ui.horizontal(|ui| {
                ui.code(selected_image_name.clone());
                if ui.button("📋").on_hover_text("Copiar nombre").clicked() {
                    ui.ctx().copy_text(selected_image_name.clone());
                }
                ui.add_sized(ui.available_size(), egui::Label::new("")); // fill grid
            });
            ui.end_row();

            if image_names.len() > 1 {
                ui.label("Otros nombres locales:");
                // Remove the image from the list
                if let Some(name_index) = image_names
                    .iter()
                    .position(|x| x.get_image() == selected_image_name.clone())
                {
                    image_names.remove(name_index);
                }

                egui::Grid::new("image_names_grid")
                    .num_columns(1)
                    .spacing([10.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        for image in image_names {
                            let image_name = image.get_image();
                            ui.label(&image_name);
                            if ui.button("📋").on_hover_text("Copiar nombre").clicked() {
                                ui.ctx().copy_text(image_name.clone());
                            }
                            ui.end_row();
                        }
                    });
                ui.end_row();
            };

            ui.label("ID local:");
            ui.label(format!("{id}"));
            ui.end_row();

            ui.label("Tamaño:");
            if let Some(image_size) = image_inspect.size {
                ui.label(humansize::format_size(
                    image_size as u64,
                    humansize::DECIMAL,
                ));
            };
            ui.end_row();

            ui.label("Arquitectura:");
            let architecture = image_inspect.architecture.clone().unwrap_or(String::new());
            let mut os = image_inspect.os.clone().unwrap_or(String::new());
            if let Some(r) = os.get_mut(0..1) {
                r.make_ascii_uppercase();
            }
            ui.label(format!("{architecture} ({os})"));
            ui.end_row();

            ui.label("Autor:");
            let author = image_inspect.author.clone().unwrap_or(String::new());
            ui.label(format!("{author}"));
            ui.end_row();
        });
    ui.separator();

    egui::Grid::new("image_rows_grid_2")
        .num_columns(2)
        .spacing([20.0, 5.0])
        .striped(false)
        .show(ui, |ui| {
            ui.label("Está en local:");
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
            ui.label(format!("{local_str}"));
            ui.end_row();

            ui.label("Subida en el Registro:");
            let mut any_image_in_remote = false;
            let remote_str = match image_in_remote {
                Some(remote_status) => match remote_status {
                    RemoteImageStatus::Present => {
                        any_image_in_remote = true;
                        "Sí, misma versión"
                    }
                    RemoteImageStatus::Missing => "No",
                    RemoteImageStatus::DateMismatch => "Sí, pero una versión distinta ⚠",
                },
                None => "Desconocido",
            };
            ui.label(format!("{remote_str}"));
            ui.end_row();
            // TODO Zot URL

            if any_image_in_remote || remote_images_metadata.len() > 1 {
                ui.label("Nombres en Registro:");
                egui::Grid::new("image_rows_grid_2_alias_list")
                    .num_columns(1)
                    .spacing([10.0, 4.0])
                    .striped(false)
                    .show(ui, |ui| {
                        for image in remote_images_metadata {
                            ui.horizontal(|ui| {
                                ui.label(image.full_alias_name.clone());
                                if ui.button("📋").on_hover_text("Copiar nombre").clicked() {
                                    ui.ctx().copy_text(image.full_alias_name.clone());
                                }
                                if ui
                                    .button("🌐")
                                    .on_hover_text("Abrir en navegador")
                                    .clicked()
                                {
                                    let url = egui::OpenUrl {
                                        url: image.http_url.clone(),
                                        new_tab: true,
                                    };
                                    ui.open_url(url);
                                }
                            });
                            ui.end_row();
                        }
                    });
                ui.end_row();
            }
        });

    ui.separator();

    egui::Grid::new("image_rows_grid_3")
        .num_columns(2)
        .spacing([20.0, 10.0])
        .striped(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Acciones locales: ");
                if ui
                    .button("🗑")
                    .on_hover_text(format!("Eliminar la imagen {selected_image_name}"))
                    .clicked()
                {
                    println!("Eliminar {}", image_collection.get_id_value());
                }
            });
            ui.end_row();
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
