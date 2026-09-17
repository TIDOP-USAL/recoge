use crate::container;
use crate::container::ImageCollection;
use crate::tasks::Task;
use crate::window::{WindowOptions, WindowSize};
use crate::zot::ImageDisplayMetadata;
use bollard::config::ImageDeleteResponseItem;
use eframe::egui;
use egui::{Id, Modal};
use egui_extras::{Column, TableBuilder};
use std::sync::{Arc, Mutex};

pub struct WindowState {
    pub options: WindowOptions,
    pub data: WindowData,
    pub open: bool,
}

impl WindowState {
    pub fn default() -> Self {
        Self {
            options: window_options(),
            data: WindowData::default(),
            open: false, // closed
        }
    }
    pub fn open(&mut self, data: WindowData) {
        self.data = data;
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }
}

pub fn window_options() -> WindowOptions {
    WindowOptions {
        id: Id::new("image_window"),
        title: "Información de imagen".to_owned(),
        closable: true,
        collapsible: true,
        fixed_size: WindowSize::Half,
        ..Default::default()
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

#[derive(Default)]
pub struct WindowData {
    pub main_list_reload_flag: Option<Arc<Mutex<bool>>>,
    pub selected_image_name: Option<String>,
    pub image_in_local: Option<bool>,
    pub image_in_remote: Option<RemoteImageStatus>,
    pub remote_images_metadata: Vec<ImageDisplayMetadata>,
    pub image_collection: Option<ImageCollection>,
    pub image_inspect: Option<bollard::models::ImageInspect>,
    pub delete_modal_data: Option<ModalData>, // None: closed
}

pub fn ui_fn(ui: &mut egui::Ui, data: &mut WindowData) {
    let WindowData {
        main_list_reload_flag,
        selected_image_name,
        image_in_local,
        image_in_remote,
        remote_images_metadata,
        image_collection,
        image_inspect,
        delete_modal_data,
    } = data;

    let Some(main_list_reload_flag) = main_list_reload_flag else {
        eprintln!("Window opened without main_list_reload_flag");
        ui.close_kind(egui::UiKind::Window);
        return;
    };

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

    if let Some(is_local) = image_in_local
        && *is_local
    {
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
                        println!("delete_modal for: {selected_image_name}");
                        // TODO use image_collection, add selector to modal
                        *delete_modal_data = Some(ModalData::new(selected_image_name.clone()));
                    }
                    /*if ui
                        .button("⬆")
                        .on_hover_text(format!("Subir a registro {selected_image_name}"))
                        .clicked()
                        {
                        println!("Eliminar {}", image_collection.get_id_value());
                    }*/
                });
                ui.end_row();
            });
    }

    if let Some(data) = delete_modal_data {
        let modal = data.modal(ui);
        if modal.should_close() {
            let delete_progress = data.delete_progress.lock().unwrap().clone();
            *delete_modal_data = None;
            if delete_progress == DeleteProgress::Ok {
                *main_list_reload_flag.lock().unwrap() = true;
                ui.close_kind(egui::UiKind::Window);
            }
        }
    }
}

#[derive(PartialEq, Clone)]
enum DeleteProgress {
    NotStarted,
    InProgress,
    Ok,
    Failed,
}

impl DeleteProgress {
    fn can_start(&self) -> bool {
        *self != Self::Ok && *self != Self::InProgress
    }
}

#[derive(Clone)]
pub struct ModalData {
    pub image_name: String,
    force_deletion: bool,
    table_rows: Arc<Mutex<Vec<ImageDeleteResponseItem>>>,
    delete_progress: Arc<Mutex<DeleteProgress>>,
}

impl ModalData {
    pub fn new(image: String) -> Self {
        Self {
            image_name: image.clone(),
            force_deletion: false,
            table_rows: Arc::new(Mutex::new(Vec::new())),
            delete_progress: Arc::new(Mutex::new(DeleteProgress::NotStarted)),
        }
    }

    fn modal(&mut self, ui: &mut egui::Ui) -> egui::ModalResponse<()> {
        Modal::new(Id::new("Local delete modal")).show(ui.ctx(), |ui| {
            ui.set_width(400.0);
            ui.heading("Eliminar imagen local");
            ui.add_space(15.0);

            egui::Grid::new("modal_rows_grid_1")
                .num_columns(2)
                .spacing([20.0, 10.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Imagen:");
                    ui.code(&self.image_name);
                    ui.end_row();

                    ui.label("Forzar:")
                        .on_hover_text(format!("La imagen se borrará aunque esté en uso"));
                    ui.checkbox(&mut self.force_deletion, "")
                        .on_hover_text("Eliminar aunque esté en uso");
                    ui.end_row();
                });
            ui.add_space(20.0);

            let delete_progress_mutex = self.delete_progress.lock().unwrap();
            let delete_progress = delete_progress_mutex.clone();
            drop(delete_progress_mutex);
            ui.horizontal(|ui| match delete_progress {
                DeleteProgress::NotStarted => {
                    ui.label("Resultados de eliminación:");
                }
                DeleteProgress::InProgress => {
                    ui.label("Eliminando...");
                    ui.add(egui::widgets::Spinner::new().size(10.0));
                }
                DeleteProgress::Ok => {
                    ui.label("Terminado ✔");
                }
                DeleteProgress::Failed => {
                    ui.label("Fallo al eliminar");
                }
            });
            ui.add_space(10.0);
            // Print result table
            self.delete_table(ui);
            ui.add_space(10.0);

            egui::Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    let download_btn = egui::Button::new("Eliminar");
                    if ui
                        .add_enabled(delete_progress.can_start(), download_btn)
                        .clicked()
                    {
                        let mut tasks: Task<()> = Task::new();

                        let image_name = self.image_name.clone();
                        let force = self.force_deletion;
                        *self.delete_progress.lock().unwrap() = DeleteProgress::InProgress;

                        let progress_arc = Arc::clone(&self.delete_progress);
                        let table_arc = Arc::clone(&self.table_rows);
                        tasks.fire_async(async move {
                            let Ok(docker) = container::Client::new().await else {
                                return println!("No docker client");
                            };

                            *progress_arc.lock().unwrap() =
                                match docker.delete_image(image_name, force, false).await {
                                    Ok(mut x) => {
                                        println!("Delete OK");
                                        let mut table_rows = table_arc.lock().unwrap();
                                        table_rows.append(&mut x);
                                        DeleteProgress::Ok
                                    }
                                    Err(x) => {
                                        println!("Delete failed: {x:?}");
                                        DeleteProgress::Failed
                                    }
                                }
                        });
                    }

                    if ui.button("Cerrar").clicked() {
                        ui.close();
                    }
                },
            );
        })
    }

    fn delete_table(&self, ui: &mut egui::Ui) {
        let available_height = 50.0;
        let row_height = 18.0;
        let _tags_table = TableBuilder::new(ui)
            .id_salt("tags".to_string())
            .striped(true)
            .resizable(false)
            .cell_layout(egui::Layout::left_to_right(egui::Align::LEFT))
            .column(
                Column::remainder()
                    .clip(true)
                    .resizable(false)
                    .at_least(200.0),
            )
            .min_scrolled_height(available_height)
            .max_scroll_height(available_height)
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong("Tags eliminados");
                });
            })
            .body(|mut body| {
                let table_rows = self.table_rows.lock().unwrap();
                for msg in table_rows.iter().rev() {
                    let Some(tag) = &msg.untagged else {
                        continue;
                    };
                    body.row(row_height, |mut row| {
                        row.col(|ui| {
                            ui.label(tag);
                        });
                    });
                }
                if table_rows.len() < 3 {
                    for _ in 0..(3 - table_rows.len()) {
                        body.row(row_height, |mut row| {
                            row.col(|ui| {
                                ui.label("");
                            });
                        });
                    }
                }
            });

        let _images_table = TableBuilder::new(ui)
            .striped(true)
            .resizable(false)
            .cell_layout(egui::Layout::left_to_right(egui::Align::LEFT))
            .column(
                Column::remainder()
                    .clip(true)
                    .resizable(false)
                    .at_least(200.0),
            )
            .min_scrolled_height(available_height)
            .max_scroll_height(available_height)
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong("Imágenes eliminadas");
                });
            })
            .body(|mut body| {
                let table_rows = self.table_rows.lock().unwrap();
                for msg in table_rows.iter().rev() {
                    let Some(deleted) = &msg.deleted else {
                        continue;
                    };
                    body.row(row_height, |mut row| {
                        row.col(|ui| {
                            ui.label(deleted);
                        });
                    });
                }
                if table_rows.len() < 3 {
                    for _ in 0..(3 - table_rows.len()) {
                        body.row(row_height, |mut row| {
                            row.col(|ui| {
                                ui.label("");
                            });
                        });
                    }
                }
            });
    }
}
