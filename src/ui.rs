use crate::configuration as config;
use crate::container;
use crate::tasks::Task;
use crate::window;
use crate::window::WindowOptions;
use arc_swap::ArcSwap;
use config::AppConfig;
use container::Image;
use eframe::egui;
use egui::containers::scroll_area::DragScroll;
use egui::{Align, Layout, UiBuilder};
use egui_extras::Column;
use egui_extras::TableBuilder;
use egui_notify::Toasts;
use futures::executor::block_on;
use futures::lock::Mutex as AsyncMutex;
use std::sync::{Arc, Mutex};
use window::config_window;
use window::image_info_window;
use window::remote_list_window;
use window::upload_window;

pub fn ui_start() -> eframe::Result {
    let a = std::include_bytes!("assets/icon.png");
    let image = image::load_from_memory(a)
        .expect("Failed to open icon path")
        .into_rgba8();
    //let (width, height) = image.dimensions();
    let width = 512;
    let height = 512;
    let rgba = image.into_raw();
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

#[derive(Debug, Clone)]
pub struct ImageTableOptions {
    search_field: Option<String>, // None: disabled
    sort_type: TableSortType,
    ui_date_rev: bool, // 0: ASC
    ui_name_rev: bool, // 0: A-Z
}

#[derive(Debug, PartialEq, Clone)]
pub enum TableSortType {
    None,
    Date,
    Name,
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
                toasts_arc
                    .lock()
                    .await
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
        let toasts_arc = Arc::clone(&self.toasts);

        self.tasks.fire_async(async move {
            let mut image_info_window_state = image_info_window_state_arc.lock().await;
            window_is_loading.store(Arc::new(true.to_owned()));
            let arc = Arc::clone(&image_info_window_state.data);
            let arc_2 = Arc::clone(&image_info_window_state.data);
            let mut data = block_on(arc.lock());
            data.init(config, toasts_arc, main_list_reload_flag_arc)
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
                let logo = egui::include_image!("assets/logo.svg");
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
                }
            });
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(5.0);
            let date_rev = self.image_table_options.ui_date_rev.clone();
            let name_rev = self.image_table_options.ui_name_rev.clone();
            let sort_type = self.image_table_options.sort_type.clone();
            let available_height = ui.available_height();

            let mut table_ui = ui.new_child(UiBuilder::new());
            match *self.image_list_load_state.lock().unwrap() {
                LoadState::Loading | LoadState::NotLoaded => {
                    table_ui.disable();
                }
                _ => {}
            }
            let _table = TableBuilder::new(&mut table_ui)
                .striped(true)
                .resizable(true)
                .auto_shrink(false)
                .drag_to_scroll(DragScroll::Always)
                .cell_layout(egui::Layout::left_to_right(egui::Align::LEFT))
                .column(
                    Column::remainder()
                        //.at_least(80.0)
                        .clip(true)
                        .resizable(true),
                )
                .column(Column::auto().resizable(false).at_least(65.0))
                .column(Column::auto().resizable(false).at_least(89.0))
                .column(Column::auto().resizable(false))
                .min_scrolled_height(0.0)
                .max_scroll_height(available_height)
                .header(21.0, |mut header| {
                    header.col(|ui| {
                        ui.horizontal(|ui| {
                            ui.strong("Nombre local").on_hover_text(
                                "Nombre de la imagen de contenedor, en formato repositorio:tag",
                            );
                            let btn = ui
                                .button(if name_rev { "⬇" } else { "⬆" })
                                .on_hover_text(if name_rev { "A-Z" } else { "Z-A" });
                            if sort_type == TableSortType::Name {
                                btn.clone().highlight();
                            }
                            if btn.clicked() {
                                self.image_table_options.ui_name_rev ^= true;
                                self.image_table_options.sort_type = TableSortType::Name;
                                self.sort_table();
                            }
                        });
                    });
                    header.col(|ui| {
                        ui.strong("Tamaño");
                    });
                    header.col(|ui| {
                        ui.strong("ID local");
                    });
                    header.col(|ui| {
                        ui.horizontal(|ui| {
                            ui.strong("Fecha de creación");
                            let btn = ui.button(if date_rev { "⬇" } else { "⬆" }).on_hover_text(
                                if date_rev {
                                    "Descendente"
                                } else {
                                    "Ascendente"
                                },
                            );
                            if sort_type == TableSortType::Date {
                                btn.clone().highlight();
                            }
                            if btn.clicked() {
                                self.image_table_options.ui_date_rev ^= true;
                                self.image_table_options.sort_type = TableSortType::Date;
                                self.sort_table();
                            }
                        });
                    });
                })
                .body(|mut body| {
                    let image_list: Vec<Image> = self.image_list.lock().unwrap().to_vec();
                    for image in image_list.iter() {
                        // TODO global filtered list
                        if let Some(search_filter) = self.image_table_options.search_field.clone() {
                            if !image.get_image_name().contains(&search_filter) {
                                continue;
                            }
                        }
                        self.image_table_row(
                            &global_data.config,
                            image,
                            &mut body,
                            upload_window_state,
                        );
                    }
                });
            if !table_ui.response().contains_pointer() && self.image_list_hovered_id.is_some() {
                self.image_list_hovered_id = None;
            }
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
    ) {
        let row_height = 18.0;
        body.row(row_height, |mut row| {
            row.col(|ui| {
                let mut image_name = image.get_image_name();
                if !image_name.contains(':') {
                    image_name.push_str(" (Sin nombre)");
                }
                let label = egui::Label::new(image_name.to_string());
                let rect = ui.add(label).on_hover_text(image_name.clone()).rect;
                if let Some(hid) = &self.image_list_hovered_id
                    && *hid == image.id
                {
                    let color = egui::Color32::from_rgb(165, 238, 0);
                    ui.debug_painter().rect(
                        rect,
                        0.0,
                        egui::Color32::TRANSPARENT,
                        (1.0, color),
                        egui::StrokeKind::Outside,
                    );
                }

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
                        let image_info_window_is_loading =
                            *self.image_info_window_is_loading.load().clone();
                        if !image_info_window_is_loading {
                            println!("Inspect {image_name}");

                            self.image_info_window_is_loading
                                .store(Arc::new(true.to_owned()));
                            self.inspect_image(image.clone(), config.clone(), ui.ctx().clone());
                        }
                    }
                });
            });
            row.col(|ui| {
                ui.label(image.get_human_size());
            });
            row.col(|ui| {
                //ui.add(egui::Separator::default().horizontal());
                if ui
                    .label(image.get_short_id())
                    .on_hover_text(image.id.clone())
                    .hovered()
                {
                    self.image_list_hovered_id = Some(image.id.clone());
                }
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

    fn sort_table(&mut self) {
        let options = self.image_table_options.clone();
        println!("Sorting image table: {options:#?}");
        match options.sort_type {
            TableSortType::None => {}
            TableSortType::Date => {
                let mut image_list = self.image_list.lock().unwrap();
                image_list.sort_by_key(|x| x.date); // Oldest image first
                if options.ui_date_rev {
                    image_list.reverse();
                }
            }
            TableSortType::Name => {
                let mut image_list = self.image_list.lock().unwrap();
                image_list.sort_by_key(|x| x.repository_info.as_ref().unwrap().repository.clone());
                if !options.ui_name_rev {
                    image_list.reverse();
                }
            }
        }
    }
}
