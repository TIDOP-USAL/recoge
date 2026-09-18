use crate::config_window;
use crate::configuration as config;
use crate::container;
use crate::image_info_window;
use crate::remote_list_window;
use crate::tasks::Task;
use crate::upload_window;
use crate::window::WindowOptions;
use crate::zot;
use config::AppConfig;
use eframe::egui;
use egui::{Align, Layout, UiBuilder};
use egui_extras::Column;
use egui_extras::TableBuilder;
use egui_notify::Toasts;
use futures::executor::block_on;
use futures::lock::Mutex as AsyncMutex;
use image_info_window::RemoteImageStatus;
use std::sync::{Arc, Mutex};

use container::Image;

pub fn ui_start() -> eframe::Result {
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([900.0, 450.0]),
        ..Default::default()
    };
    options.viewport.fullscreen = Some(false);
    options.viewport.maximized = Some(false);
    options.viewport.window_type = Some(egui::X11WindowType::Dialog);
    eframe::run_native(
        "recoge",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);
            //  Get light/dark theme
            let theme = match dark_light::detect()? {
                dark_light::Mode::Dark => egui::style::Visuals::dark(),
                _ => egui::style::Visuals::light(),
            };
            cc.egui_ctx.set_visuals(theme);
            Ok(Box::<MyApp>::default())
        }),
    )
}

pub struct ImageTableData {
    date_reversed: bool,
}

#[derive(PartialEq, Clone)]
pub enum LoadState {
    NotLoaded,
    Loading,
    Loaded,
}

pub struct GlobalData {
    pub version: String,
    pub config_state: LoadState,
    pub config: AppConfig,
}

impl Default for GlobalData {
    fn default() -> Self {
        Self {
            config_state: crate::ui::LoadState::NotLoaded,
            config: crate::configuration::AppConfig::default(),
            version: String::new(),
        }
    }
}

pub struct MyApp {
    global_data: Arc<AsyncMutex<GlobalData>>,
    toasts: Arc<AsyncMutex<Toasts>>,
    image_list: Arc<Mutex<Vec<Image>>>,
    image_list_load_state: Arc<Mutex<LoadState>>,
    image_list_should_reload: Arc<Mutex<bool>>,
    tasks: Task<()>,
    upload_window_state: Arc<Mutex<upload_window::WindowState>>,
    image_info_window_state: Arc<Mutex<image_info_window::WindowState>>,
    remote_list_window_state: Arc<AsyncMutex<remote_list_window::WindowState>>,
    config_window_state: Arc<Mutex<config_window::WindowState>>,
    image_table_data: ImageTableData,
}

impl MyApp {
    pub fn get_config(&mut self) {
        let global_data_arc = Arc::clone(&self.global_data);
        let toasts_arc = Arc::clone(&self.toasts);

        self.tasks.fire_async(async move {
            let Ok(config) = AppConfig::get_config() else {
                //global_data.config_state = LoadState::NotLoaded;
                // This would trigger this fn again
                let mut toasts = toasts_arc.lock().await;
                toasts
                    .error("No se ha podido cargar la configuración")
                    .duration(std::time::Duration::from_secs(5));
                return;
            };
            let mut global_data = global_data_arc.lock().await;
            global_data.config = config;
            global_data.config_state = LoadState::Loaded;
            println!("Config loaded!");
            if global_data.config.is_missing_credentials() {
                let mut toasts = toasts_arc.lock().await;
                toasts
                    .warning("No se ha configurado el usuario del Registro")
                    .duration(std::time::Duration::from_secs(5));
                println!("Config is missing credentials");
            }
        });
    }
    pub fn update_list(&mut self) {
        let image_list_arc = Arc::clone(&self.image_list);
        let image_list_load_state_arc = Arc::clone(&self.image_list_load_state);
        let toasts_arc = Arc::clone(&self.toasts);

        self.tasks.fire_async(async move {
            let Ok(docker) = container::Client::new().await else {
                *image_list_load_state_arc.lock().unwrap() = LoadState::NotLoaded;
                toasts_arc
                    .lock()
                    .await
                    .error("Error de conexión. Comprueba que Docker esté funcionando.")
                    .duration(std::time::Duration::from_secs(3));
                return;
            };
            let Ok(mut images) = docker.image_list().await else {
                *image_list_load_state_arc.lock().unwrap() = LoadState::NotLoaded;
                let mut toasts = toasts_arc.lock().await;
                toasts
                    .error("No se puede obtener la lista de imágenes.")
                    .duration(std::time::Duration::from_secs(3));
                return;
            };
            let mut image_list = image_list_arc.lock().unwrap();
            image_list.clear();
            // TODO collection groups
            image_list.append(&mut images);
            image_list.sort_by_key(|x| x.date); // Oldest image first
            *image_list_load_state_arc.lock().unwrap() = LoadState::Loaded;
        });
    }

    // image: image name or digest
    pub fn inspect_image(&mut self, image: Image, config: AppConfig) {
        let image_info_window_state_arc = Arc::clone(&self.image_info_window_state);
        let main_list_reload_flag_arc = Arc::clone(&self.image_list_should_reload);
        let toasts_arc = Arc::clone(&self.toasts);

        self.tasks.fire_async(async move {
            println!("Inspecting: {}", image.get_image_name());
            let Ok(docker) = container::Client::new().await else {
                toasts_arc
                    .lock()
                    .await
                    .error("Error de conexión. Comprueba que Docker esté funcionando.")
                    .duration(std::time::Duration::from_secs(3));
                return;
            };
            // Inspect local image
            let inspect_result = docker.image_inspect(image.get_image_name()).await;

            // Get remote image info
            let Ok(mut zot_client) = zot::Client::new(&config) else {
                let mut toasts = toasts_arc.lock().await;
                toasts
                    .error("Error de conexión. Comprueba las credenciales del Registro.")
                    .duration(std::time::Duration::from_secs(3));
                return;
            };
            let mut image_in_remote_option = None;
            let mut remote_image_date_option = None;
            let mut remote_images_digests = Vec::new();
            let mut remote_images_metadata = Vec::new();
            // Image info
            if inspect_result.is_ok()
                && image.has_image_name()
                && let Ok(remote_info) = image.clone().get_remote_info(&mut zot_client).await
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
                let mut toasts = toasts_arc.lock().await;
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
                let image_info_window_state = &mut *image_info_window_state_arc.lock().unwrap();
                image_info_window_state.options.data_is_loading = false;
                return;
            };
            let mut image_inspect = inspect_result.unwrap();
            //println!("image_inspect: {image_inspect:#?}");

            let Ok(mut local_images) = container::ImageCollection::try_from(&image_inspect) else {
                let mut toasts = toasts_arc.lock().await;
                toasts
                    .error("Error al leer la información de imagen.")
                    .duration(std::time::Duration::from_secs(3));
                let image_info_window_state = &mut *image_info_window_state_arc.lock().unwrap();
                image_info_window_state.options.data_is_loading = false;
                return;
            };
            let local_date = image.date;

            // zot: Get associated images from all ImageCollection repos
            let mut remote_digests = Vec::new();
            if image.has_image_name() {
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
                        let date_ok = offset.abs().num_seconds() < 2;
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

            let main_list_reload_flag = Some(main_list_reload_flag_arc);

            // BUGFIX, wrong size on inspect, use parent size
            local_images.size_bytes = image.size_bytes;
            image_inspect.size = image.size_bytes;

            //println!("local_images: {local_images:#?}");
            // Set window parameters. TODO send image itself?
            let image_inspect = Some(image_inspect.clone());
            let selected_image_name = Some(image.get_image_name());
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
                        if offset.abs().num_seconds() < 2 {
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
            let image_info_window_state = &mut *image_info_window_state_arc.lock().unwrap();
            image_info_window_state.options.data_is_loading = false;
            let data = image_info_window::WindowData {
                main_list_reload_flag,
                selected_image_name,
                image_in_local,
                image_in_remote,
                remote_images_metadata,
                image_collection,
                image_inspect,
                delete_modal_data: None,
            };
            image_info_window_state.open(data);
        });
    }

    pub fn load_remote_list(&mut self, config: AppConfig) {
        let remote_list_window_state_arc = Arc::clone(&self.remote_list_window_state);
        let main_list_reload_flag_arc = Arc::clone(&self.image_list_should_reload);
        let toasts_arc = Arc::clone(&self.toasts);

        self.tasks.fire_async(async move {
            let mut remote_list_window_state = remote_list_window_state_arc.lock().await;
            let load_result = remote_list_window_state
                .load_list_data(&config, main_list_reload_flag_arc, toasts_arc)
                .await;
            if let Ok(_) = load_result {
                remote_list_window_state.open();
            } else {
                println!("Returned an error");
            }
        });
    }
}

impl Default for MyApp {
    fn default() -> Self {
        let mut image_list = Vec::new();
        for i in 1..4 {
            image_list.push(Image {
                repository_info: Some(container::RepositoryInfo {
                    repository: "cr.tidop.es/lcsc".to_string(),
                    tag: "latest".to_string(),
                }),
                size_bytes: Some(i * 1000),
                id: "0042152187f577f43a7e621fa39fa54baa6f097f261b9338e3e5c114a0a0dd09".to_string(),
                digests: vec![
                    "0042152187f577f43a7e621fa39fa54baa6f097f261b9338e3e5c114a0a0dd09".to_string(),
                ],
                /*date: chrono::DateTime::<chrono::Utc>::from_timestamp_secs(
                        (i * 1000).try_into().unwrap(),
                )*/
                date: chrono::DateTime::parse_from_rfc3339("2026-04-22T10:48:35.802927736Z")
                    .unwrap()
                    .into(),
            });
        }
        Self {
            global_data: Arc::new(AsyncMutex::new(GlobalData {
                config_state: LoadState::NotLoaded,
                config: config::AppConfig::default(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            })),
            toasts: Arc::new(AsyncMutex::new(
                Toasts::default().with_anchor(egui_notify::Anchor::BottomRight),
            )),
            image_list: Arc::new(Mutex::new(image_list)),
            image_list_load_state: Arc::new(Mutex::new(LoadState::NotLoaded)),
            image_list_should_reload: Arc::new(Mutex::new(false)),
            tasks: Task::new(),
            upload_window_state: Arc::new(Mutex::new(upload_window::WindowState::default())),
            config_window_state: Arc::new(Mutex::new(config_window::WindowState::default())),
            remote_list_window_state: Arc::new(AsyncMutex::new(
                remote_list_window::WindowState::default(),
            )),
            image_info_window_state: Arc::new(
                Mutex::new(image_info_window::WindowState::default()),
            ),
            image_table_data: ImageTableData {
                date_reversed: true,
            },
        }
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let global_data_arc = Arc::clone(&self.global_data);
        let mut global_data = block_on(global_data_arc.lock().into_future());
        if global_data.config_state == LoadState::NotLoaded {
            global_data.config_state = LoadState::Loading;
            self.get_config();
        }

        let toasts_arc = Arc::clone(&self.toasts);
        let mut toasts = block_on(toasts_arc.lock().into_future());
        toasts.show(ui);
        drop(toasts);

        let upload_window_state_arc = Arc::clone(&self.upload_window_state);
        let upload_window_state = &mut *upload_window_state_arc.lock().unwrap();
        WindowOptions::show(
            &mut upload_window_state.options,
            &mut upload_window_state.data,
            ui,
            &mut upload_window_state.open,
            upload_window::ui_fn,
        );
        let image_info_window_state_arc = Arc::clone(&self.image_info_window_state);
        let image_info_window_state = &mut *image_info_window_state_arc.lock().unwrap();
        WindowOptions::show(
            &mut image_info_window_state.options,
            &mut image_info_window_state.data,
            ui,
            &mut image_info_window_state.open,
            image_info_window::ui_fn,
        );
        let config_window_state_arc = Arc::clone(&self.config_window_state);
        let config_window_state = &mut *config_window_state_arc.lock().unwrap();
        WindowOptions::show(
            &mut config_window_state.options,
            &mut config_window_state.data,
            ui,
            &mut config_window_state.open,
            config_window::ui_fn,
        );
        let remote_list_window_state_arc = Arc::clone(&self.remote_list_window_state);
        let rlws_future = remote_list_window_state_arc.lock().into_future();
        let remote_list_window_state = &mut *block_on(rlws_future);
        WindowOptions::show(
            &mut remote_list_window_state.options,
            &mut remote_list_window_state.data,
            ui,
            &mut remote_list_window_state.open,
            remote_list_window::ui_fn,
        );

        //println!("{}",image_info_window_state.open);
        if image_info_window_state.options.data_is_loading
            || remote_list_window_state.options.data_is_loading
        {
            ui.set_cursor_icon(egui::CursorIcon::Wait);
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let logo = egui::include_image!("logo.svg");
                ui.set_height(20.4);
                ui.add_space(1.0);
                ui.add(egui::Image::new(logo.clone()));
                ui.add_space(3.2);
                let heading_str = format!("recoge v{}", global_data.version);
                let heading =
                    egui::Label::new(egui::RichText::new(heading_str).heading().size(16.0));
                ui.add(heading);
            });

            let mut top_toolbar_ui = ui.new_child(
                UiBuilder::new()
                    .max_rect(ui.max_rect())
                    .layout(Layout::right_to_left(Align::Min)),
            );
            if top_toolbar_ui
                .button("⟲")
                .on_hover_text("Recargar lista de imágenes")
                .clicked()
            {
                *self.image_list_load_state.lock().unwrap() = LoadState::Loading;
                self.update_list();
            }
            if top_toolbar_ui
                .button("R")
                .on_hover_text("Lista de imágenes remotas")
                .clicked()
            {
                self.load_remote_list(global_data.config.clone());
                remote_list_window_state.options.data_is_loading = true;
            }
            if top_toolbar_ui
                .button("⚙")
                .on_hover_text("Configuración")
                .clicked()
            {
                config_window_state.open(
                    global_data.config.clone(),
                    Arc::clone(&self.global_data),
                    Arc::clone(&self.toasts),
                );
            }

            ui.separator();

            ui.horizontal(|ui| match *self.image_list_load_state.lock().unwrap() {
                LoadState::Loaded => {
                    ui.label("Lista cargada");
                }
                LoadState::Loading => {
                    ui.label("Cargando lista ");
                    ui.spinner();
                }
                LoadState::NotLoaded => {
                    ui.label("Lista no cargada");
                }
            });
            let image_list_should_reload = self.image_list_should_reload.lock().unwrap().clone();
            if image_list_should_reload {
                println!("Some window flagged the main list to be updated");
                let mut image_list_should_reload = self.image_list_should_reload.lock().unwrap();
                *image_list_should_reload = false;
                drop(image_list_should_reload);
                *self.image_list_load_state.lock().unwrap() = LoadState::Loading;
                self.update_list();
            }

            ui.label("Imágenes locales:".to_string());
            ui.add_space(10.0);
            ui.separator();
            let date_rev = &mut self.image_table_data.date_reversed;
            let available_height = ui.available_height();

            let _table = TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::LEFT))
                .column(
                    Column::remainder()
                        //.at_least(80.0)
                        .clip(true)
                        .resizable(true),
                )
                .column(Column::auto())
                .column(Column::auto())
                .column(Column::auto())
                .min_scrolled_height(0.0)
                .max_scroll_height(available_height)
                .header(21.0, |mut header| {
                    header.col(|ui| {
                        ui.strong("Nombre/Tag");
                    });
                    header.col(|ui| {
                        ui.strong("Tamaño");
                    });
                    header.col(|ui| {
                        ui.strong("ID local");
                    });
                    header.col(|ui| {
                        egui::Sides::new().show(
                            ui,
                            |ui| {
                                ui.strong("Fecha de creación");
                            },
                            |ui| {
                                *date_rev ^= ui
                                    .button(if *date_rev { "⬆" } else { "⬇" })
                                    .on_hover_text(if *date_rev {
                                        "Ascendente"
                                    } else {
                                        "Descendente"
                                    })
                                    .clicked();
                            },
                        );
                    });
                })
                .body(|mut body| {
                    let image_list: Vec<Image> = self.image_list.lock().unwrap().to_vec();
                    let date_rev = self.image_table_data.date_reversed;
                    if date_rev {
                        for image in image_list.iter().rev() {
                            self.image_table_row(
                                &global_data.config,
                                image,
                                &mut body,
                                upload_window_state,
                                image_info_window_state,
                            );
                        }
                    } else {
                        for image in image_list.iter() {
                            self.image_table_row(
                                &global_data.config,
                                image,
                                &mut body,
                                upload_window_state,
                                image_info_window_state,
                            );
                        }
                    }
                });
        });
    }
}

impl MyApp {
    fn image_table_row(
        &mut self,
        config: &AppConfig,
        image: &Image,
        body: &mut egui_extras::TableBody,
        upload_window_state: &mut upload_window::WindowState,
        image_info_window_state: &mut image_info_window::WindowState,
    ) {
        let row_height = 18.0;
        body.row(row_height, |mut row| {
            row.col(|ui| {
                let mut image_name = image.get_image_name();
                if !image_name.contains(':') {
                    image_name.push_str(" (Sin nombre)");
                }
                ui.label(image_name.to_string())
                    .on_hover_text(image_name.clone());

                ui.with_layout(egui::Layout::right_to_left(egui::Align::LEFT), |ui| {
                    if ui.button("⬆").on_hover_text("Subir a Registro").clicked() {
                        //*boolean = !*boolean;
                        println!("Subir {image_name}");
                        let Ok(selected_repo_info) =
                            container::RepositoryInfo::try_from_repo_tag(image_name.clone())
                        else {
                            eprintln!("Invalid repo_tag");
                            return;
                        };
                        upload_window_state.open(config.clone(), selected_repo_info);
                    }
                    if ui
                        .button("ℹ")
                        .on_hover_text("Información de la imagen")
                        .clicked()
                    {
                        if !image_info_window_state.options.data_is_loading {
                            println!("Info de {image_name}");

                            self.inspect_image(image.clone(), config.clone());
                            image_info_window_state.options.data_is_loading = true;
                        }
                    }
                });
            });
            row.col(|ui| {
                ui.label(image.get_human_size());
            });
            row.col(|ui| {
                //ui.add(egui::Separator::default().horizontal());
                ui.label(image.get_short_id())
                    .on_hover_text(image.id.clone());
            });
            row.col(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.label(format!(
                    "({}) {}",
                    image.get_relative_date(),
                    image.get_date_local()
                ));
            });

            //self.toggle_row_selection(row_index, &row.response());
        });
    }
}
