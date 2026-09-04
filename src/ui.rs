use crate::config_window;
use crate::configuration as config;
use crate::container;
use crate::image_info_window;
use crate::tasks::Task;
use crate::upload_window;
use crate::zot;
use config::AppConfig;
use eframe::egui;
use egui::{Align, Layout, UiBuilder};
use egui_extras::Column;
use egui_extras::TableBuilder;
use egui_notify::Toasts;
use image_info_window::RemoteImageStatus;
use std::sync::{Arc, Mutex};

use container::Image;

pub fn ui_start() -> eframe::Result {
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([920.0, 440.0]),
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

#[derive(PartialEq)]
pub enum ConfigState {
    NotLoaded,
    Loading,
    Loaded,
}

pub struct GlobalData {
    pub version: String,
    pub config_state: ConfigState,
    pub config: AppConfig,
}

pub struct MyApp {
    global_data: Arc<Mutex<GlobalData>>,
    toasts: Arc<Mutex<Toasts>>,
    image_list: Arc<Mutex<Vec<Image>>>,
    image_list_loaded: Arc<Mutex<bool>>,
    data: Task<()>,
    upload_window_state: Arc<Mutex<upload_window::WindowState>>,
    image_info_window_state: Arc<Mutex<image_info_window::WindowState>>,
    config_window_state: Arc<Mutex<config_window::WindowState>>,
    image_table_data: ImageTableData,
}

impl MyApp {
    pub fn get_config(&mut self) {
        let global_data_arc = Arc::clone(&self.global_data);
        let toasts_arc = Arc::clone(&self.toasts);

        self.data.fire_async(async move {
            let Ok(config) = AppConfig::get_config() else {
                //global_data.config_state = ConfigState::NotLoaded;
                // This would trigger this fn again
                let mut toasts = toasts_arc.lock().unwrap();
                toasts
                    .error("No se ha podido cargar la configuración")
                    .duration(std::time::Duration::from_secs(5));
                return;
            };
            let mut global_data = global_data_arc.lock().unwrap();
            global_data.config = config;
            global_data.config_state = ConfigState::Loaded;
            println!("Config loaded!");
            if global_data.config.is_missing_credentials() {
                let mut toasts = toasts_arc.lock().unwrap();
                toasts
                    .warning("No se ha configurado el usuario del Registro")
                    .duration(std::time::Duration::from_secs(5));
                println!("Config is missing credentials");
            }
        });
    }
    pub fn update_list(&mut self) {
        let image_list_arc = Arc::clone(&self.image_list);
        let image_list_loaded_arc = Arc::clone(&self.image_list_loaded);
        let toasts_arc = Arc::clone(&self.toasts);

        self.data.fire_async(async move {
            //tokio::task::spawn_local(async move {
            *image_list_loaded_arc.lock().unwrap() = false;
            let Ok(mut images) = container::bollard_image_list().await else {
                let mut toasts = toasts_arc.lock().unwrap();
                toasts
                    .error("Error de conexión. Comprueba que Docker esté funcionando.")
                    .duration(std::time::Duration::from_secs(3));
                return;
            };
            let mut image_list = image_list_arc.lock().unwrap();
            image_list.clear();
	    // TODO collection groups
            image_list.append(&mut images);
            image_list.sort_by_key(|x| x.date); // Oldest image first
            *image_list_loaded_arc.lock().unwrap() = true;
        });
    }

    // image: image name or digest
    pub fn inspect_image(&mut self, image: Image, config: AppConfig) {
        let image_info_window_state_arc = Arc::clone(&self.image_info_window_state);
        let toasts_arc = Arc::clone(&self.toasts);

        self.data.fire_async(async move {
            println!("Inspecting: {}", image.get_image_name());
            // Inspect local image
            let inspect_result = container::bollard_image_inspect(image.get_image_name()).await;
	    
            // Get remote image info
            let Ok(mut zot_client) = zot::Client::new(&config) else {
                let mut toasts = toasts_arc.lock().unwrap();
                toasts
                    .error("Error de conexión. Comprueba las credenciales del Registro.")
                    .duration(std::time::Duration::from_secs(3));
                return;
            };
            let mut image_in_remote_option = None;
            let mut remote_image_date_option = None;
            if inspect_result.is_ok()
                && image.has_image_name()
                && let Ok(remote_info) = image.clone().get_remote_info(&mut zot_client).await
            {
                println!("zot returned: {remote_info:#?}");
                // TODO refactor
                image_in_remote_option = Some(remote_info.is_in_remote);
                if remote_info.is_in_remote {
                    remote_image_date_option = Some(remote_info.image_info.unwrap().date);
                }
            }

            let image_info_window_state = &mut *image_info_window_state_arc.lock().unwrap();
            image_info_window_state.options.data_is_loading = false;

            match inspect_result {
                Ok(mut image_inspect) => {
		    //println!("image_inspect: {image_inspect:#?}");

                    let Ok(mut local_images) = container::ImageCollection::try_from(&image_inspect) else {
                        let mut toasts = toasts_arc.lock().unwrap();
                        toasts
                            .error("Error al leer la información de imagen.")
                            .duration(std::time::Duration::from_secs(3));
                        return;
                    };
		    // BUGFIX, wrong size on inspect, use parent size
		    local_images.size_bytes = image.size_bytes;
		    image_inspect.size = image.size_bytes;
		    
		    //println!("local_images: {local_images:#?}");
                    // Set window parameters. TODO send image itself?
                    image_info_window_state.options.image_inspect = Some(image_inspect.clone());
		    image_info_window_state.options.selected_image_name = Some(image.get_image_name());
                    image_info_window_state.options.image_collection = Some(local_images);
		    
                    image_info_window_state.options.image_in_local = Some(true);
                    // Safety. Check both dates
                    let local_date = image.date;
                    // If zot info
                    if let Some(in_remote) = image_in_remote_option {
                        image_info_window_state.options.image_in_remote = if in_remote {
                            // Image is in remote. Compare dates
                            if let Some(remote_date) = remote_image_date_option {
                                println!(
                                    "Date check. Local: {local_date:?}. Remote: {remote_date:?}"
                                );
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
                        };
                    }
		    // TODO try to check through digests?

                    // Open window
                    image_info_window_state.open = true;
                }
                Err(x) => {
                    println!("Error when inspecting local image: {x:?}");
                    let mut toasts = toasts_arc.lock().unwrap();
                    if x.is_not_found() {
                        toasts
                            .info("No se encuentra información de la imagen.")
                            .duration(std::time::Duration::from_secs(3));
                    } else {
                        // TODO 500 normalizing.
                        toasts
                            .error("Error de conexión. Comprueba que Container está funcionando.")
                            .duration(std::time::Duration::from_secs(3));
                    }
                }
            }
        });
    }

    pub fn load_remote_list(&mut self, config: AppConfig) {
        let toasts_arc = Arc::clone(&self.toasts);

        self.data.fire_async(async move {
            let Ok(mut client) = zot::Client::new(&config) else {
                let mut toasts = toasts_arc.lock().unwrap();
                toasts
                    .error("Error de conexión. Comprueba las credenciales del Registro.")
                    .duration(std::time::Duration::from_secs(3));
                return;
            };
            println!("Cargando lista remota");
            let list = match client.get_image_list().await {
                Ok(x) => x,
                Err(x) => {
                    println!("error: {x:?}");
                    let mut toasts = toasts_arc.lock().unwrap();
                    toasts
                        .error("Error de conexión. Comprueba las credenciales del Registro.")
                        .duration(std::time::Duration::from_secs(3));
                    return;
                }
            };
            println!("{list:#?}");
            // TODO testing
            let last_image = list.last().unwrap().clone();
            let is_in_remote = last_image.clone().get_remote_info(&mut client).await;
            println!("is in remote: {is_in_remote:?}");
            let is_in_local = last_image.is_in_local().await;
            println!("is in local: {is_in_local:?}");
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
		digests: vec!["0042152187f577f43a7e621fa39fa54baa6f097f261b9338e3e5c114a0a0dd09".to_string()],
                /*date: chrono::DateTime::<chrono::Utc>::from_timestamp_secs(
                    (i * 1000).try_into().unwrap(),
            )*/
		date: chrono::DateTime::parse_from_rfc3339("2026-04-22T10:48:35.802927736Z")
                .unwrap().into(),
            });
        }
        Self {
            global_data: Arc::new(Mutex::new(GlobalData {
                config_state: ConfigState::NotLoaded,
                config: config::AppConfig::default(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            })),
            toasts: Arc::new(Mutex::new(
                Toasts::default().with_anchor(egui_notify::Anchor::BottomRight),
            )),
            image_list: Arc::new(Mutex::new(image_list)),
            image_list_loaded: Arc::new(Mutex::new(true)),
            data: Task::new(),
            upload_window_state: Arc::new(Mutex::new(upload_window::WindowState::default())),
            config_window_state: Arc::new(Mutex::new(config_window::WindowState::default())),
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
        let mut global_data = global_data_arc.lock().unwrap();
        if global_data.config_state == ConfigState::NotLoaded {
            global_data.config_state = ConfigState::Loading;
            self.get_config();
        }

        let toasts_arc = Arc::clone(&self.toasts);
        let mut toasts = toasts_arc.lock().unwrap();
        toasts.show(ui);

        let upload_window_state_arc = Arc::clone(&self.upload_window_state);
        let upload_window_state = &mut *upload_window_state_arc.lock().unwrap();
        upload_window::show(
            &mut upload_window_state.options,
            ui,
            &mut upload_window_state.open,
        );
        let image_info_window_state_arc = Arc::clone(&self.image_info_window_state);
        let image_info_window_state = &mut *image_info_window_state_arc.lock().unwrap();
        image_info_window::show(
            &mut image_info_window_state.options,
            ui,
            &mut image_info_window_state.open,
        );
        let config_window_state_arc = Arc::clone(&self.config_window_state);
        let config_window_state = &mut *config_window_state_arc.lock().unwrap();
        config_window::show(
            &mut config_window_state.options,
            ui,
            &mut config_window_state.open,
        );
        //println!("{}",image_info_window_state.open);
        if image_info_window_state.options.data_is_loading {
            ui.set_cursor_icon(egui::CursorIcon::Wait);
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading(format!("recoge v{}", global_data.version));

            let mut reset_btn_ui = ui.new_child(
                UiBuilder::new()
                    .max_rect(ui.max_rect())
                    .layout(Layout::right_to_left(Align::Min)),
            );
            if reset_btn_ui
                .button("⟲")
                .on_hover_text("Recargar lista de imágenes")
                .clicked()
            {
                self.update_list();
            }
            if reset_btn_ui
                .button("R")
                .on_hover_text("Lista de imágenes remotas")
                .clicked()
            {
                self.load_remote_list(global_data.config.clone());
            }
            if reset_btn_ui
                .button("⚙")
                .on_hover_text("Configuración")
                .clicked()
            {
                config_window_state.options.app_config_copy = global_data.config.clone();
                config_window_state.options.global_data_arc = Arc::clone(&self.global_data);
                config_window_state.open = true;
            }

            ui.separator();

            ui.horizontal(|ui| {
                if !*self.image_list_loaded.lock().unwrap() {
                    ui.label("Cargando lista ");
                    ui.spinner();
                } else {
                    ui.label("Lista cargada");
                }
            });

            ui.label("Imágenes locales:".to_string());

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
                .header(30.0, |mut header| {
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
        //row.set_selected(self.selection.contains(&row_index));
        //row.set_overline(self.overline && row_index % 7 == 3);
        body.row(row_height, |mut row| {
            row.col(|ui| {
                let mut image_name = image.get_image_name();
		if !image_name.contains(':') {
		    image_name.push_str(" (Sin nombre)");
		}
                ui.label(image_name.to_string());

                ui.with_layout(egui::Layout::right_to_left(egui::Align::LEFT), |ui| {
                    if ui.button("⬆").on_hover_text("Subir a Registro").clicked() {
                        //*boolean = !*boolean;
                        println!("Subir {image_name}");
                        upload_window_state.options.image_name = Some(image_name.to_string());
                        upload_window_state.open = true;
                    }
                    if ui
                        .button("ℹ")
                        .on_hover_text("Información de la imagen")
                        .clicked()
                    {
                        if !image_info_window_state.options.data_is_loading {
                            println!("Info de {image_name}");

                            self.inspect_image(image.clone(), config.clone());

                            //image_info_window_state.open = true;
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
                ui.label(image.get_short_id());
            });
            row.col(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.label(image.get_date_local());
            });

            //self.toggle_row_selection(row_index, &row.response());
        });
    }
}
