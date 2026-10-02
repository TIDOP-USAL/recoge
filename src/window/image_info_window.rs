use crate::configuration::AppConfig;
use crate::container;
use crate::container::ImageCollection;
use crate::error::{Error, Result};
use crate::tasks::Task;
use crate::window::{WindowOptions, WindowSize};
use crate::zot;
use crate::zot::ImageDisplayMetadata;
use bollard::config::ImageDeleteResponseItem;
use eframe::egui;
use egui::{Id, Modal};
use egui_extras::{Column, TableBuilder};
use egui_notify::Toasts;
use futures::executor::block_on;
use futures::lock::Mutex as AsyncMutex;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct WindowState {
    pub options: WindowOptions,
    pub data: Arc<AsyncMutex<WindowData>>,
    pub open: bool,
}

impl WindowState {
    pub fn default() -> Self {
        Self {
            options: window_options(),
            data: Arc::new(AsyncMutex::new(WindowData::default())),
            open: false, // closed
        }
    }
    pub fn open(&mut self) {
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

#[derive(Default, Clone)]
pub struct WindowData {
    pub global_config: AppConfig,
    pub toasts_arc: Arc<AsyncMutex<Toasts>>,
    pub main_list_reload_flag: Arc<Mutex<bool>>,
    pub image_data: Option<ImageData>,
    pub data_is_reloading: bool,
}

#[derive(Default, Clone)]
pub struct ImageData {
    pub selected_image_name: String,
    pub image_in_local: Option<bool>,
    pub image_in_remote: Option<RemoteImageStatus>,
    pub remote_images_metadata: Vec<ImageDisplayMetadata>,
    pub image_collection: Option<ImageCollection>,
    pub image_inspect: Option<bollard::models::ImageInspect>,
    pub delete_modal_data: Option<ModalData>, // None: closed
}

// ID + some fields that can replace info
pub struct ImageInspectInput {
    pub id: ImageInspectType,
    pub date: Option<chrono::DateTime<chrono::Utc>>,
    pub size_bytes: Option<i64>,
}

pub enum ImageInspectType {
    LocalNamed(String), // Image name
    LocalId(String),    // Image id
                        //Remote(String),
}

impl WindowData {
    pub async fn init(
        &mut self,
        config: AppConfig,
        toasts_arc: Arc<AsyncMutex<Toasts>>,
        main_list_reload_flag_arc: Arc<Mutex<bool>>,
    ) {
        let data = self;
        data.toasts_arc = toasts_arc;
        data.main_list_reload_flag = main_list_reload_flag_arc;
        data.global_config = config;
    }

    pub async fn load_image_info(&mut self, image: ImageInspectInput) -> Result<Self> {
        let mut data: WindowData = (*self).clone();
        let mut is_named = true;
        let image_name = match image.id {
            ImageInspectType::LocalNamed(image) => image,
            ImageInspectType::LocalId(id) => {
                is_named = false;
                id
            }
        };
        println!("Inspecting: {image_name}");
        let Ok(docker) = container::Client::new().await else {
            data.toasts_arc
                .lock()
                .await
                .error("Error de conexión. Comprueba que Docker esté funcionando.")
                .duration(std::time::Duration::from_secs(3));
            return Err(Error::Program("No docker".to_string()));
        };
        // Inspect local image
        let inspect_result = docker.image_inspect(image_name.clone()).await;

        // Get remote image info
        let Ok(mut zot_client) = zot::Client::new(&data.global_config) else {
            let mut toasts = data.toasts_arc.lock().await;
            toasts
                .error("Error de conexión. Comprueba las credenciales del Registro.")
                .duration(std::time::Duration::from_secs(3));
            return Err(Error::Program("No registry access".to_string()));
        };
        let mut image_in_remote_option = None;
        let mut remote_image_date_option = None;
        let mut remote_images_digests = Vec::new();
        let mut remote_images_metadata = Vec::new();
        // Image info
        if inspect_result.is_ok()
            && is_named
            && let Ok(remote_info) = zot::get_remote_info(image_name.clone(), &mut zot_client).await
        {
            println!("zot returned: {remote_info:#?}");
            // TODO refactor
            image_in_remote_option = Some(remote_info.is_in_remote);
            if remote_info.is_in_remote
                && let Some(image_info) = remote_info.image_info
            {
                remote_image_date_option = Some(image_info.date);
                let mut digests = image_info.digests.clone();
                remote_images_digests.append(&mut digests);
            }
        }

        if let Err(x) = inspect_result {
            println!("Error when inspecting local image: {x:?}");
            let mut toasts = data.toasts_arc.lock().await;
            if x.is_not_found() {
                toasts
                    .info("No se encuentra información de la imagen.")
                    .duration(std::time::Duration::from_secs(3));
            } else {
                // TODO 500 normalizing.
                toasts
                    .error("Error de conexión. Comprueba que Docker está funcionando.")
                    .duration(std::time::Duration::from_secs(3));
            }
            //self.options.data_is_loading = false;
            return Err(Error::Program("Can't inspect".to_string()));
        };
        let mut image_inspect = inspect_result.unwrap();
        //println!("image_inspect: {image_inspect:#?}");

        let Ok(mut local_images) = container::ImageCollection::try_from(&image_inspect) else {
            let mut toasts = data.toasts_arc.lock().await;
            toasts
                .error("Error al leer la información de imagen.")
                .duration(std::time::Duration::from_secs(3));
            //self.options.data_is_loading = false;
            return Err(Error::Program("No image info".to_string()));
        };

        let local_date = match image.date {
            Some(replace_date) => replace_date,
            None => {
                let Some(created) = image_inspect.created.clone() else {
                    return Err(Error::Program("No date for image".to_string()));
                };
                chrono::DateTime::parse_from_rfc3339(&created)
                    .unwrap()
                    .into()
            }
        };

        // zot: Get associated images from all ImageCollection repos
        let mut remote_digests = Vec::new();
        if is_named {
            if let Ok(remote_images) = local_images
                .clone()
                .get_images_in_remote(&mut zot_client)
                .await
            {
                for remote_image in remote_images {
                    // Date checks
                    let remote_date = remote_image.date;
                    println!(
                        "Date check. Local: {local_date:?}. Remote {}: {remote_date:?}",
                        remote_image.get_image_name()
                    );
                    let offset = local_date - remote_date;
                    let date_ok = offset.abs().num_seconds() < 30;
                    // Digest check
                    let mut digest_found = false;
                    if let Some(remote_digest) = remote_image.digests.first() {
                        for local_digest in &local_images.image_digests {
                            // local_digest: repo@sha256:digest
                            // remote_digests: sha256:digest
                            let digest_parts = local_digest.split('@').collect::<Vec<_>>();
                            let Some(local_digest) = digest_parts.last() else {
                                continue;
                            };
                            println!("Checking: {local_digest}");
                            if remote_digest.contains(local_digest) {
                                println!("Digest match!");
                                digest_found = true;
                                break;
                            }
                        }
                    }

                    if date_ok || digest_found {
                        println!("Images match, OK!");
                        if let Some(digest) = remote_image.digests.first() {
                            remote_digests.push(digest.clone());
                        }
                        let image_metadata = zot_client.fill_image_metadata(remote_image);
                        remote_images_metadata.push(image_metadata);
                    }
                }
            }
        }
        // Add any other repos that match the digest
        remote_digests.sort();
        remote_digests.dedup();
        for digest in remote_digests {
            if let Ok(images) = zot_client.get_digest_images(digest).await {
                for image in images {
                    // Not checking against local image, since they have the same digest on the registry
                    let image_metadata = zot_client.fill_image_metadata(image);
                    println!("Added {} by digest", image_metadata.full_alias_name);
                    remote_images_metadata.push(image_metadata);
                }
            }
        }

        // BUGFIX, wrong size on inspect, use parent size
        local_images.size_bytes = image.size_bytes;
        image_inspect.size = image.size_bytes;

        //println!("local_images: {local_images:#?}");
        // Set window parameters. TODO send image itself?
        let image_inspect = Some(image_inspect.clone());
        let selected_image_name = image_name;
        let image_collection = Some(local_images.clone());

        let image_in_local = Some(true);
        // TODO refactor RemoteImageStatus usage
        // Safety. Check both dates
        // If zot info
        let mut image_in_remote = if let Some(in_remote) = image_in_remote_option {
            if in_remote {
                // Image is in remote. Compare dates
                if let Some(remote_date) = remote_image_date_option {
                    println!("Date check. Local: {local_date:?}. Remote: {remote_date:?}");
                    let offset = local_date - remote_date;
                    if offset.abs().num_seconds() < 30 {
                        println!("Dates match, OK!");
                        Some(RemoteImageStatus::Present)
                    } else {
                        println!("Date mismatch.");
                        Some(RemoteImageStatus::DateMismatch)
                    }
                } else {
                    println!("Internal error");
                    None
                }
            } else {
                println!("Image not in remote");
                Some(RemoteImageStatus::Missing)
            }
        } else {
            None
        };
        // Check through digests. Required on Docker@Windows
        // TODO refactor and remove
        println!("Remote digests: {:#?}", remote_images_digests);
        for local_digest in local_images.image_digests {
            // local_digest: repo@sha256:digest
            // remote_digests: sha256:digest
            let digest_parts = local_digest.split('@').collect::<Vec<_>>();
            let Some(local_digest) = digest_parts.last() else {
                continue;
            };
            println!("Checking: {local_digest}");
            if remote_images_digests.contains(&local_digest.to_string()) {
                println!("Digest match!");
                image_in_remote = Some(RemoteImageStatus::Present);
                break;
            }
        }

        // Extra repos from zot
        remote_images_metadata.sort();
        remote_images_metadata.dedup();

        // Open window
        //self.options.data_is_loading = false;
        data.image_data = Some(ImageData {
            selected_image_name,
            image_in_local,
            image_in_remote,
            remote_images_metadata,
            image_collection,
            image_inspect,
            delete_modal_data: None,
        });
        *self = data;
        //self.open(data);
        return Ok(self.clone());
    }

    // Swap a local image's info (reload)
    pub async fn swap_image_info(&mut self, selected_image_name: String) -> Result<()> {
        let Some(ref mut image_data) = self.image_data else {
            return Err(Error::Program("Swap with no info".to_string()));
        };
        let Some(ref image_collection) = image_data.image_collection else {
            return Err(Error::Program("Swap with no info".to_string()));
        };
        let selected_image_repo_info =
            container::RepositoryInfo::try_from_repo_tag(selected_image_name.clone())?;
        if !image_collection
            .image_aliases
            .contains(&selected_image_repo_info)
        {
            return Err(Error::Program(
                "Tried to select image not in collection".to_string(),
            ));
        }
        image_data.selected_image_name = selected_image_name;
        self.data_is_reloading = false;
        Ok(())
    }
}

pub fn ui_fn(ui: &mut egui::Ui, data_arc: &mut Arc<AsyncMutex<WindowData>>) {
    let data_arc_1 = Arc::clone(&data_arc);
    let data_arc_2 = Arc::clone(&data_arc);
    let data_arc_3 = Arc::clone(&data_arc);
    let data = block_on(data_arc_1.lock().into_future());
    if data.data_is_reloading {
        ui.set_cursor_icon(egui::CursorIcon::Wait);
    }

    let Some(image_data) = data.image_data.clone() else {
        eprintln!("Window opened without selected_image_name");
        ui.close_kind(egui::UiKind::Window);
        return;
    };
    drop(data);

    let ImageData {
        selected_image_name,
        image_in_local,
        image_in_remote,
        remote_images_metadata,
        image_collection,
        image_inspect,
        delete_modal_data: _,
    } = image_data;

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
                if ui
                    .button("🗑")
                    .on_hover_text(format!("Eliminar la imagen {selected_image_name}"))
                    .clicked()
                {
                    println!("delete_modal for: {selected_image_name}");
                    // TODO use image_collection, add selector to modal
                    let mut data = block_on(data_arc_2.lock());
                    let WindowData {
                        global_config: _,
                        toasts_arc: _,
                        main_list_reload_flag: _,
                        image_data,
                        data_is_reloading: _,
                    } = &mut *data;
                    image_data.as_mut().unwrap().delete_modal_data =
                        Some(ModalData::new(selected_image_name.clone()));
                    drop(data);
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
                            ui.horizontal(|ui| {
                                if ui.button("📋").on_hover_text("Copiar nombre").clicked() {
                                    ui.ctx().copy_text(image_name.clone());
                                }
                                if ui
                                    .button("ℹ")
                                    .on_hover_text("Información de esta imagen")
                                    .clicked()
                                {
                                    println!("Change dialog to {image_name}");
                                    let arc = Arc::clone(&data_arc_2);
                                    Task::new().fire_async(async move {
                                        let mut data = arc.lock().await;
                                        data.data_is_reloading = true;
                                        let _r = data.swap_image_info(image_name).await;
                                    });
                                }
                            });
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
    ui.add_space(2.0);

    egui::Grid::new("image_rows_grid_2")
        .num_columns(2)
        .spacing([20.0, 8.0])
        .striped(false)
        .show(ui, |ui| {
            ui.label("Está en local:");
            let local_str = match image_in_local {
                Some(local) => {
                    if local {
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
                    RemoteImageStatus::Missing => {
                        if remote_images_metadata.len() > 1 {
                            "No con este nombre"
                        } else {
                            "No"
                        }
                    }
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

    let mut data = block_on(data_arc_3.lock());
    let WindowData {
        global_config: _,
        toasts_arc: _,
        main_list_reload_flag,
        image_data,
        data_is_reloading: _,
    } = &mut *data;
    if image_data.as_mut().unwrap().delete_modal_data.is_some() {
        let modal_data = image_data
            .as_mut()
            .unwrap()
            .delete_modal_data
            .as_mut()
            .unwrap();
        let modal = modal_data.modal(ui);

        if modal.should_close() {
            let delete_progress = modal_data.delete_progress.lock().unwrap().clone();
            image_data.as_mut().unwrap().delete_modal_data = None;
            if delete_progress == DeleteProgress::Ok {
                *main_list_reload_flag.lock().unwrap() = true;
                ui.close_kind(egui::UiKind::Window);
            }
        }
    }
    ui.add_space(4.0);
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
        ui.group(|ui| {
            let _tags_table = TableBuilder::new(ui)
                .id_salt("tags".to_string())
                .striped(true)
                .resizable(false)
                .auto_shrink(false)
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
                });
        });

        ui.group(|ui| {
            let _images_table = TableBuilder::new(ui)
                .striped(true)
                .resizable(false)
                .auto_shrink(false)
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
                });
        });
    }
}
