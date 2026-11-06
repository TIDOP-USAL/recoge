pub mod main_list;
pub mod widgets;
pub mod window;

use crate::configuration as config;
use crate::container;
use crate::tasks::Task;
use arc_swap::ArcSwap;
use config::AppConfig;
use container::Image;
use eframe::egui;
use egui::{Align, Layout, UiBuilder};
use egui_notify::Toasts;
use futures::executor::block_on;
use futures::lock::Mutex as AsyncMutex;
use main_list::ImageTableOptions;
use main_list::TableSortType;
use std::sync::{Arc, Mutex};
use window::{WindowOptions, config_window, image_info_window, remote_list_window, upload_window};

pub fn ui_start() -> eframe::Result {
    let icon = std::include_bytes!("../assets/icon.png");
    let rgba = image::load_from_memory(icon)
        .expect("Failed to open icon path")
        .into_rgba8()
        .into_raw();
    let width = 512;
    let height = 512;
    let icon = egui::viewport::IconData {
        rgba,
        width,
        height,
    };

    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 450.0])
            .with_icon(icon),
        ..Default::default()
    };
    options.viewport.fullscreen = Some(false);
    options.viewport.maximized = Some(false);
    options.viewport.window_type = Some(egui::X11WindowType::Dialog);
    eframe::run_native(
        "recoge",
        options,
        Box::new(|cc| {
            // Add image support:
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

#[derive(PartialEq, Clone)]
pub enum LoadState {
    NotLoaded,
    NeedsReload,
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
    image_table_options: ImageTableOptions,
    image_list_hovered_id: Option<String>,
    tasks: Task<()>,
    upload_window_state: Arc<Mutex<upload_window::WindowState>>,
    upload_window_open_flag: Arc<Mutex<Option<container::RepositoryInfo>>>,
    image_info_window_state: Arc<AsyncMutex<image_info_window::WindowState>>,
    image_info_window_is_loading: Arc<ArcSwap<bool>>,
    remote_list_window_state: Arc<AsyncMutex<remote_list_window::WindowState>>,
    remote_list_window_is_loading: Arc<ArcSwap<bool>>,
    config_window_state: Arc<Mutex<config_window::WindowState>>,
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
                    .warning(
                        "No se ha configurado el usuario del Registro. Entra en la configuración ⚙",
                    )
                    .duration(std::time::Duration::from_secs(20));
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
                let mut toasts = toasts_arc.lock().await;
                toasts
                    .error("Error de conexión. Comprueba que Docker esté funcionando.")
                    .duration(std::time::Duration::from_secs(5));
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
            image_list.reverse();
            *image_list_load_state_arc.lock().unwrap() = LoadState::Loaded;
        });
        // Re-sort as default
        self.image_table_options.sort_type = TableSortType::Date;
        self.image_table_options.ui_date_rev = true;
        self.image_table_options.ui_name_rev = false;
    }

    // image: image name or digest
    pub fn inspect_image(&mut self, image: Image, config: AppConfig, ctx: egui::Context) {
        let image_info_window_state_arc = Arc::clone(&self.image_info_window_state);
        let main_list_reload_flag_arc = Arc::clone(&self.image_list_should_reload);
        let window_is_loading = Arc::clone(&self.image_info_window_is_loading);
        let upload_window_open_flag_arc = Arc::clone(&self.upload_window_open_flag);
        let toasts_arc = Arc::clone(&self.toasts);

        self.tasks.fire_async(async move {
            let mut image_info_window_state = image_info_window_state_arc.lock().await;
            window_is_loading.store(Arc::new(true.to_owned()));
            let arc = Arc::clone(&image_info_window_state.data);
            let arc_2 = Arc::clone(&image_info_window_state.data);
            let mut data = block_on(arc.lock());
            data.init(
                config,
                toasts_arc,
                main_list_reload_flag_arc,
                upload_window_open_flag_arc,
            )
            .await;
            drop(data);
            let input_data = image_info_window::ImageInspectInput {
                id: image_info_window::ImageInspectType::LocalNamed(image.get_image_name()),
                date: Some(image.date),
                size_bytes: image.size_bytes,
            };
            let _r = image_info_window::load_image_info(arc_2, input_data).await;
            // TODO if ok
            window_is_loading.store(Arc::new(false.to_owned()));
            image_info_window_state.open();
            ctx.request_repaint();
        });
    }

    pub fn load_remote_list(&mut self, config: AppConfig, ctx: egui::Context) {
        let remote_list_window_state_arc = Arc::clone(&self.remote_list_window_state);
        let main_list_reload_flag_arc = Arc::clone(&self.image_list_should_reload);
        let window_is_loading = Arc::clone(&self.remote_list_window_is_loading);
        let toasts_arc = Arc::clone(&self.toasts);

        self.tasks.fire_async(async move {
            let mut remote_list_window_state = remote_list_window_state_arc.lock().await;
            let arc = Arc::clone(&remote_list_window_state.data);
            let mut data = block_on(arc.lock());
            let load_result = data
                .init(&config, main_list_reload_flag_arc, toasts_arc)
                .await;
            window_is_loading.store(Arc::new(false.to_owned()));
            if let Ok(_) = load_result {
                remote_list_window_state.open();
                ctx.request_repaint();
            } else {
                println!("Returned an error");
            }
        });
    }
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            global_data: Arc::new(AsyncMutex::new(GlobalData {
                config_state: LoadState::NotLoaded,
                config: config::AppConfig::default(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            })),
            toasts: Arc::new(AsyncMutex::new(
                Toasts::default().with_anchor(egui_notify::Anchor::BottomRight),
            )),
            image_list: Arc::new(Mutex::new(Vec::new())),
            image_list_load_state: Arc::new(Mutex::new(LoadState::NeedsReload)),
            image_list_should_reload: Arc::new(Mutex::new(false)),
            image_table_options: ImageTableOptions {
                search_field: None,
                sort_type: TableSortType::Date,
                ui_date_rev: true,
                ui_name_rev: false,
            },
            image_list_hovered_id: None,
            tasks: Task::new(),
            upload_window_state: Arc::new(Mutex::new(upload_window::WindowState::default())),
            upload_window_open_flag: Arc::new(Mutex::new(None)),
            config_window_state: Arc::new(Mutex::new(config_window::WindowState::default())),
            remote_list_window_state: Arc::new(AsyncMutex::new(
                remote_list_window::WindowState::default(),
            )),
            remote_list_window_is_loading: Arc::new(ArcSwap::new(Arc::new(false.to_owned()))),
            image_info_window_state: Arc::new(AsyncMutex::new(
                image_info_window::WindowState::default(),
            )),
            image_info_window_is_loading: Arc::new(ArcSwap::new(Arc::new(false.to_owned()))),
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
        //	let upload_window_state_arc2 = Arc::clone(&self.upload_window_state);
        let upload_window_state = &mut *upload_window_state_arc.lock().unwrap();
        WindowOptions::show(
            &mut upload_window_state.options,
            &mut upload_window_state.data,
            ui,
            &mut upload_window_state.open,
            upload_window::ui_fn,
        );
        let image_info_window_is_loading = *self.image_info_window_is_loading.load().clone();
        if !image_info_window_is_loading {
            let image_info_window_state_arc = Arc::clone(&self.image_info_window_state);
            let image_info_window_state =
                &mut *block_on(image_info_window_state_arc.lock().into_future());
            WindowOptions::show(
                &mut image_info_window_state.options,
                &mut image_info_window_state.data,
                ui,
                &mut image_info_window_state.open,
                image_info_window::ui_fn,
            );
        }
        let config_window_state_arc = Arc::clone(&self.config_window_state);
        let config_window_state = &mut *config_window_state_arc.lock().unwrap();
        WindowOptions::show(
            &mut config_window_state.options,
            &mut config_window_state.data,
            ui,
            &mut config_window_state.open,
            config_window::ui_fn,
        );
        let remote_list_window_is_loading = *self.remote_list_window_is_loading.load().clone();
        if !remote_list_window_is_loading {
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
        }

        if image_info_window_is_loading || remote_list_window_is_loading {
            ui.set_cursor_icon(egui::CursorIcon::Wait);
        }

        // List refresh logic
        if *self.image_list_load_state.lock().unwrap() == LoadState::NeedsReload {
            *self.image_list_load_state.lock().unwrap() = LoadState::Loading;
            self.update_list();
        }

        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                let logo = egui::include_image!("../assets/logo.svg");
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
                .button("⟲ Local")
                .on_hover_text("Recargar lista de imágenes")
                .clicked()
            {
                *self.image_list_load_state.lock().unwrap() = LoadState::NeedsReload;
            }
            if top_toolbar_ui
                .button("🔍 Registro")
                .on_hover_text("Lista de imágenes remotas")
                .clicked()
            {
                self.load_remote_list(global_data.config.clone(), ui.ctx().clone());
                self.remote_list_window_is_loading
                    .store(Arc::new(true.to_owned()));
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
                    let image_count = self.image_list.lock().unwrap().len();
                    ui.label(format!("Lista cargada ({image_count})"));
                }
                LoadState::Loading => {
                    ui.label("Cargando lista ");
                    ui.set_cursor_icon(egui::CursorIcon::Wait);
                    ui.spinner();
                }
                LoadState::NotLoaded | LoadState::NeedsReload => {
                    ui.label("Lista no cargada");
                }
            });
            let image_list_should_reload = self.image_list_should_reload.lock().unwrap().clone();
            if image_list_should_reload {
                println!("Some window flagged the main list to be updated");
                let mut image_list_should_reload = self.image_list_should_reload.lock().unwrap();
                *image_list_should_reload = false;
                drop(image_list_should_reload);
                *self.image_list_load_state.lock().unwrap() = LoadState::NeedsReload;
            }

            let upload_window_open_flag = self.upload_window_open_flag.lock().unwrap().clone();
            if let Some(repo) = upload_window_open_flag {
                println!("Open upload window for {repo:?}");
                let mut upload_window_open_flag = self.upload_window_open_flag.lock().unwrap();
                *upload_window_open_flag = None;
                drop(upload_window_open_flag);
                upload_window_state.open(global_data.config.clone(), repo);
            }

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Imágenes locales:".to_string());
                let mut focus = false;
                let search_btn = ui.button("🔍").on_hover_text("Filtrar por nombre");
                if self.image_table_options.search_field.is_some() {
                    search_btn.clone().highlight();
                }
                if search_btn.clicked() {
                    if self.image_table_options.search_field.is_none() {
                        self.image_table_options.search_field = Some(String::new());
                        focus = true;
                    } else {
                        self.image_table_options.search_field = None;
                    }
                };
                if self.image_table_options.search_field.is_some() {
                    let filter = ui
                        .text_edit_singleline(
                            self.image_table_options.search_field.as_mut().unwrap(),
                        )
                        .on_hover_text("Nombre de registro y tag para filtrar");
                    if focus {
                        filter.request_focus();
                    }
                    if filter.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.image_table_options.search_field = None;
                    }
                    filter.context_menu(|ui| {
                        widgets::textedit_context_menu(
                            ui,
                            self.image_table_options.search_field.as_mut().unwrap(),
                        );
                        filter.request_focus();
                    });
                }
            });
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(5.0);

            let mut table_ui = ui.new_child(UiBuilder::new());
            match *self.image_list_load_state.lock().unwrap() {
                LoadState::Loading | LoadState::NotLoaded => {
                    table_ui.disable();
                }
                _ => {}
            }
            drop(global_data);
            main_list::image_list_table(&mut table_ui, self);
            if !table_ui.response().contains_pointer() && self.image_list_hovered_id.is_some() {
                self.image_list_hovered_id = None;
            }
        });
    }
}
