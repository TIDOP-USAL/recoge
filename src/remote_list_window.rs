//use crate::tasks::Task;
use crate::configuration::AppConfig;
use crate::container;
use crate::tasks::Task;
use crate::window::{WindowOptions, WindowSize};
use crate::zot;
use bollard::config::CreateImageInfo;
use eframe::egui;
use egui::{Id, Modal};
use egui_extras::{Column, TableBuilder};
use egui_notify::Toasts;
use futures::lock::Mutex as AsyncMutex;
use std::sync::{Arc, Mutex};

#[derive(Default)]
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

    pub async fn load_list_data(
        &mut self,
        config: &AppConfig,
        reload_flag: Arc<Mutex<bool>>,
        toasts: Arc<AsyncMutex<Toasts>>,
    ) -> Result<(), ()> {
        let window_state = self;
        let Ok(mut client) = zot::Client::new(&config) else {
            window_state.options.data_is_loading = false;
            let mut toasts = toasts.lock().await;
            toasts
                .error("Error de conexión. Comprueba las credenciales del Registro.")
                .duration(std::time::Duration::from_secs(3));
            return Err(());
        };

        println!("Cargando lista remota");
        let mut remote_image_list = match client.get_image_list().await {
            Ok(x) => x,
            Err(x) => {
                println!("error: {x:?}");
                window_state.options.data_is_loading = false;
                toasts
                    .lock()
                    .await
                    .error("Error de conexión. Comprueba las credenciales del Registro.")
                    .duration(std::time::Duration::from_secs(3));
                return Err(());
            }
        };
        remote_image_list.sort_by_key(|x| x.date); // Oldest image first
        println!("{remote_image_list:#?}");
        // TODO testing
        let last_image = remote_image_list.last().unwrap().clone();
        let is_in_remote = last_image.clone().get_remote_info(&mut client).await;
        println!("is in remote: {is_in_remote:?}");
        let is_in_local = last_image.is_in_local().await;
        println!("is in local: {is_in_local:?}");
        // Set window parameters
        let main_list_reload_flag = Some(reload_flag);
        let app_config = config.clone();
        // Open window

        window_state.options.data_is_loading = false;
        let mut data = WindowData::default();
        data.main_list_reload_flag = main_list_reload_flag;
        data.app_config = app_config;
        data.remote_image_list = remote_image_list;
        //data.toasts_arc = toasts;
        window_state.data = data;
        return Ok(());
        //window_state.open = true;
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }
}

pub fn window_options() -> WindowOptions {
    let max_window_size = egui::vec2(650.0, 225.0);
    let min_window_size = egui::vec2(600.0, 225.0);
    WindowOptions {
        id: Id::new("remote_list_window"),
        title: "Imágenes en Registro".to_owned(),
        closable: true,
        collapsible: true,
        min_size: WindowSize::Fixed(min_window_size),
        max_size: WindowSize::Fixed(max_window_size),
        ..Default::default()
    }
}

pub struct WindowData {
    pub main_list_reload_flag: Option<Arc<Mutex<bool>>>,
    pub app_config: AppConfig,
    pub remote_image_list: Vec<container::Image>,
    pub toasts_arc: Arc<AsyncMutex<Toasts>>,
    date_rev: bool,
    pull_modal_data: Option<ModalData>,
}

impl Default for WindowData {
    fn default() -> Self {
        Self {
            main_list_reload_flag: None,
            app_config: AppConfig::default(),
            remote_image_list: Vec::new(),
            toasts_arc: Arc::new(AsyncMutex::new(Toasts::default())),
            date_rev: true,
            pull_modal_data: None,
        }
    }
}

#[derive(PartialEq, Clone)]
pub enum PullProgress {
    NotStarted,
    InProgress,
    Ok,
    Failed,
}

#[allow(unused)]
impl PullProgress {
    fn has_ended(&self) -> bool {
        *self == Self::Ok || *self == Self::Failed
    }
    fn can_start(&self) -> bool {
        *self != Self::Ok && *self != Self::InProgress
    }
}

pub struct ModalData {
    pub image: container::Image,
    pub config: AppConfig,
    full_image_name: Option<String>,
    save_without_prefix: bool,
    pull_progress_tx: std::sync::mpsc::SyncSender<CreateImageInfo>,
    pull_progress_rx: std::sync::mpsc::Receiver<CreateImageInfo>,
    table_rows: Vec<CreateImageInfo>,
    pull_progress: Arc<Mutex<PullProgress>>,
}

impl ModalData {
    pub fn new(image: container::Image, config: AppConfig) -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel(1000);
        Self {
            image: image.clone(),
            config: config.clone(),
            full_image_name: None,
            save_without_prefix: false,
            pull_progress_rx: rx,
            pull_progress_tx: tx,
            table_rows: Vec::new(),
            pull_progress: Arc::new(Mutex::new(PullProgress::NotStarted)),
        }
    }
}

pub fn ui_fn(ui: &mut egui::Ui, data: &mut WindowData) {
    let WindowData {
        main_list_reload_flag,
        app_config,
        remote_image_list,
        toasts_arc: _,
        date_rev,
        pull_modal_data,
    } = data;

    let Some(main_list_reload_flag) = main_list_reload_flag else {
        eprintln!("Window opened without main_list_reload_flag");
        ui.close_kind(egui::UiKind::Window);
        return;
    };

    ui.label("Imágenes remotas:");
    ui.add_space(10.0);

    ui.group(|ui| {
        table_ui(
            ui,
            date_rev,
            remote_image_list.to_vec(),
            &app_config,
            pull_modal_data,
        );
    });

    // Pull modal
    if let Some(data) = pull_modal_data {
        let modal = data.modal(ui);
        if modal.should_close() {
            let pull_progress = data.pull_progress.lock().unwrap().clone();
            *pull_modal_data = None;
            if pull_progress == PullProgress::Ok {
                *main_list_reload_flag.lock().unwrap() = true;
                //ui.close_kind(egui::UiKind::Window);
            }
        }
    }

    //let on_top = Some(ui.layer_id()) == ui.ctx().top_layer_id();
    //ui.label(format!("This window is on top: {on_top}."));
}

fn table_ui(
    ui: &mut egui::Ui,
    date_rev: &mut bool,
    images: Vec<container::Image>,
    config: &AppConfig,
    pull_modal_data: &mut Option<ModalData>,
) -> egui::scroll_area::ScrollAreaOutput<()> {
    let available_height = ui.available_height();
    let table = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::LEFT))
        .column(
            Column::remainder()
                .at_least(10.0)
                .clip(true)
                .resizable(true)
                .at_least(200.0),
        )
        .column(Column::auto().resizable(false))
        .column(Column::auto().resizable(false))
        .column(Column::auto().resizable(false))
        .min_scrolled_height(0.0)
        .max_scroll_height(available_height)
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.strong("Nombre/Tag");
            });
            header.col(|ui| {
                ui.strong("Tamaño*").on_hover_text(
                    "Tamaño comprimido en Registro. El tamaño real puede ser 3-5 veces mayor",
                );
            });
            header.col(|ui| {
                ui.strong("ID local");
            });
            header.col(|ui| {
                egui::Sides::new().show(
                    ui,
                    |ui| {
                        ui.strong("Fecha de subida");
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
            if *date_rev {
                for image in images.iter().rev() {
                    image_table_row(image, &mut body, &config, pull_modal_data);
                }
            } else {
                for image in images.iter() {
                    image_table_row(image, &mut body, &config, pull_modal_data);
                }
            }
        });
    table
}

fn image_table_row(
    image: &container::Image,
    body: &mut egui_extras::TableBody,
    config: &AppConfig,
    pull_modal_data: &mut Option<ModalData>,
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
                if ui
                    .button("⬇")
                    .on_hover_text("Descargar de Registro")
                    .clicked()
                {
                    println!("Pull modal for {image_name}");
                    //println!("{image:#?}");
                    *pull_modal_data = Some(ModalData::new(image.clone(), config.clone()));
                }
                /*if ui
                    .button("ℹ")
                    .on_hover_text("Información de la imagen")
                    .clicked()
                {
                    println!("Info de {image_name}");
                }*/
            });
        });
        row.col(|ui| {
            ui.label(image.get_human_size());
        });
        row.col(|ui| {
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
    });
}

impl ModalData {
    fn modal(&mut self, ui: &mut egui::Ui) -> egui::ModalResponse<()> {
        Modal::new(Id::new("Pull modal")).show(ui.ctx(), |ui| {
            let pull_name = match self.full_image_name.clone() {
                Some(x) => x,
                None => {
                    let Ok(zot_client) = zot::Client::new(&self.config) else {
                        ui.close_kind(egui::UiKind::Window);
                        return println!("Invalid config");
                    };
                    let image_name = self.image.get_image_name();
                    let registry_name = zot_client.get_registry_name();
                    let pull_name = format!("{registry_name}/{image_name}");
                    self.full_image_name = Some(pull_name.clone());
                    pull_name
                }
            };
            ui.set_width(400.0);
            ui.heading("Descargar imagen (Pull)");
            ui.add_space(15.0);

            egui::Grid::new("modal_rows_grid_1")
                .num_columns(2)
                .spacing([20.0, 10.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Imagen:");
                    ui.code(pull_name);
                    ui.end_row();

                    let image_name = self.image.get_image_name();
                    ui.label("Copia adicional:").on_hover_text(format!(
                        "La imagen se descargará también sin el dominio del Registro"
                    ));
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.save_without_prefix, "Copiar como");
                        ui.code(image_name);
                    });
                });
            ui.add_space(20.0);

            let pull_progress_mutex = self.pull_progress.lock().unwrap();
            let pull_progress = pull_progress_mutex.clone();
            drop(pull_progress_mutex);
            ui.horizontal(|ui| match pull_progress {
                PullProgress::NotStarted => {
                    ui.label("Estado de la descarga:");
                }
                PullProgress::InProgress => {
                    ui.label("Descargando...");
                    ui.add(egui::widgets::Spinner::new().size(10.0));
                }
                PullProgress::Ok => {
                    ui.label("Descarga correcta ✔");
                }
                PullProgress::Failed => {
                    ui.label("Fallo al descargar");
                }
            });
            ui.add_space(10.0);
            // Add received messages to buffer
            for row in self.pull_progress_rx.try_iter() {
                self.table_rows.push(row);
            }
            // Print buffer as table
            ui.group(|ui| {
                self.progress_table(ui);
            });
            ui.add_space(10.0);

            egui::Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    let download_btn = egui::Button::new("Descargar");
                    if ui
                        .add_enabled(pull_progress.can_start(), download_btn)
                        .clicked()
                    {
                        self.table_rows.clear();
                        let mut tasks: Task<()> = Task::new();
                        let Ok(zot_client) = zot::Client::new(&self.config) else {
                            ui.close_kind(egui::UiKind::Window);
                            return println!("Invalid config");
                        };
                        let image_name = self.image.get_image_name();
                        let registry_name = zot_client.get_registry_name();
                        let pull_name = format!("{registry_name}/{image_name}");
                        let creds = zot_client.get_credentials();
                        let mut rename_str = None;
                        if self.save_without_prefix {
                            rename_str = Some(image_name);
                        }

                        *self.pull_progress.lock().unwrap() = PullProgress::InProgress;

                        let tx_arc = Arc::new(Mutex::new(self.pull_progress_tx.clone()));
                        let progress_arc = Arc::clone(&self.pull_progress);
                        tasks.fire_async(async move {
                            let tx_arc = Arc::clone(&tx_arc);
                            let Ok(docker) = container::Client::new().await else {
                                return println!("No docker client");
                            };

                            *progress_arc.lock().unwrap() = match docker
                                .pull_image(pull_name, creds, tx_arc, rename_str)
                                .await
                            {
                                Ok(_) => {
                                    println!("Pull OK");
                                    // TODO trigger main list refresh?
                                    PullProgress::Ok
                                }
                                Err(x) => {
                                    println!("Pull failed: {x:?}");
                                    PullProgress::Failed
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

    fn progress_table(&self, ui: &mut egui::Ui) -> egui::scroll_area::ScrollAreaOutput<()> {
        let available_height = 50.0;
        let row_height = 18.0;
        let table = TableBuilder::new(ui)
            .striped(true)
            .resizable(false)
            .cell_layout(egui::Layout::left_to_right(egui::Align::LEFT))
            .column(Column::auto().at_least(100.0).resizable(false))
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
                    ui.strong("Capa");
                });
                header.col(|ui| {
                    ui.strong("Progreso");
                });
            })
            .body(|mut body| {
                for msg in self.table_rows.iter().rev() {
                    let Some(id) = &msg.id else {
                        continue;
                    };
                    let status = msg.status.clone().unwrap_or(String::new());
                    let mut progress_str = String::new();
                    match &msg.progress_detail {
                        Some(detail) => {
                            if let Some(total) = detail.total
                                && let Some(current) = detail.current
                                && total != 0
                            {
                                let progress = (current as f64 / total as f64) * 100.0;
                                progress_str = format!("({progress:.2}%)");
                            }
                        }
                        None => {}
                    };
                    body.row(row_height, |mut row| {
                        row.col(|ui| {
                            ui.label(id);
                        });
                        row.col(|ui| {
                            ui.label(format!("{status} {progress_str}"));
                        });
                    });
                }
                if self.table_rows.len() < 3 {
                    for _ in 0..(3 - self.table_rows.len()) {
                        body.row(row_height, |mut row| {
                            row.col(|ui| {
                                ui.label("");
                            });
                            row.col(|ui| {
                                ui.label("");
                            });
                        });
                    }
                }
            });
        table
    }
}
