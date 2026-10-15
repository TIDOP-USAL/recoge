use super::PullProgress;
use crate::configuration::AppConfig;
use crate::container;
use crate::tasks::Task;
use crate::window::{WindowOptions, WindowSize};
use crate::zot;
use bollard::config::CreateImageInfo;
use eframe::egui;
use egui::{Id, Modal};
use egui_extras::{Column, TableBuilder};
use futures::lock::Mutex as AsyncMutex;
use std::sync::{Arc, Mutex};

pub struct ModalData {
    pub image: container::Image,
    pub config: AppConfig,
    full_image_name: Option<String>,
    save_without_prefix: bool,
    pull_progress_tx: std::sync::mpsc::SyncSender<CreateImageInfo>,
    pull_progress_rx: std::sync::mpsc::Receiver<CreateImageInfo>,
    table_rows: Vec<CreateImageInfo>,
    pub pull_progress: Arc<Mutex<PullProgress>>,
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

impl ModalData {
    pub fn modal(&mut self, ui: &mut egui::Ui) -> egui::ModalResponse<()> {
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
                        if ui
                            .checkbox(&mut self.save_without_prefix, "Copiar como")
                            .changed()
                            && *self.pull_progress.lock().unwrap() != PullProgress::InProgress
                        {
                            *self.pull_progress.lock().unwrap() = PullProgress::NotStarted;
                        };
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
            .auto_shrink(false)
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
            });
        table
    }
}
