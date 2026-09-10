//use crate::tasks::Task;
use crate::configuration::AppConfig;
use crate::container;
use crate::tasks::Task;
use crate::zot;
use bollard::config::CreateImageInfo;
use eframe::egui;
use egui::{Id, Modal, Vec2b};
use egui_extras::{Column, TableBuilder};
use egui_notify::Toasts;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Default)]
pub struct WindowState {
    pub options: WindowOptions,
    pub open: bool, // default: closed
}

#[derive(PartialEq, Clone)]
pub enum PullProgress {
    NotStarted,
    InProgress,
    Ended, // TODO split into Done, Error,
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
    pub app_config: AppConfig,
    pub remote_image_list: Vec<container::Image>,
    pub toasts_arc: Arc<Mutex<Toasts>>,
    date_rev: bool,
    pull_modal_data: Option<ModalData>,
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            title: "Imágenes en Registro".to_owned(),
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
            app_config: AppConfig::default(),
            remote_image_list: Vec::new(),
            toasts_arc: Arc::new(Mutex::new(Toasts::default())),
            date_rev: false,
            pull_modal_data: None,
        }
    }
}

pub fn show(options: &mut WindowOptions, ui: &mut egui::Ui, open: &mut bool) {
    let title = options.title.clone();
    let title_bar = options.title_bar;
    let closable = options.closable;
    let collapsible = options.collapsible;
    let movable = options.movable;
    let resizable = options.resizable;
    let constrain = options.constrain;
    let scroll2 = options.scroll2;
    let anchored = options.anchored;
    let anchor = options.anchor;
    let anchor_offset = options.anchor_offset;

    let enabled = true;
    if !enabled {
        ui.request_repaint();
    }

    let mut window = egui::Window::new(title)
        .id(egui::Id::new("remote_list_window_options"))
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
    let max_window_size = egui::vec2(ui_rect_size.x / 1.4, ui_rect_size.y / 2.0);
    let min_window_size = egui::vec2(600.0, ui_rect_size.y / 2.0);
    window
        .default_pos(egui::pos2(ui_rect_size.x / 2.0, ui_rect_size.y / 2.0))
        .max_size(max_window_size)
        .min_size(min_window_size)
        .show(ui, |ui| ui_fn(options, ui));
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
        app_config,
        remote_image_list,
        toasts_arc: _,
        date_rev,
        pull_modal_data,
    } = options;

    ui.label("Imágenes remotas:");
    ui.add_space(10.0);
    table_ui(
        ui,
        date_rev,
        remote_image_list.to_vec(),
        &app_config,
        pull_modal_data,
    );

    if let Some(data) = pull_modal_data {
        let modal = data.modal(ui);
        if modal.should_close() {
            *pull_modal_data = None;
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
    //row.set_selected(self.selection.contains(&row_index));
    //row.set_overline(self.overline && row_index % 7 == 3);
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
            ui.add_space(4.0);

            egui::Grid::new("modal_rows_grid_1")
                .num_columns(2)
                .spacing([20.0, 10.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Imagen:");
                    ui.label(pull_name);
                    ui.end_row();

                    let image_name = self.image.get_image_name();
                    ui.label("Copia adicional").on_hover_text(format!(
                        "La imagen se descargará también sin el dominio del Registro"
                    ));
                    let mut layout = egui::text::LayoutJob::default();
                    let style = egui::Style::default();
                    egui::RichText::new("Copiar como  ")
                        .color(style.visuals.text_color())
                        .append_to(
                            &mut layout,
                            &style,
                            egui::FontSelection::Default,
                            egui::Align::Center,
                        );
                    egui::RichText::new(image_name)
                        .color(style.visuals.strong_text_color())
                        .background_color(style.visuals.code_bg_color)
                        .code()
                        .append_to(
                            &mut layout,
                            &style,
                            egui::FontSelection::Default,
                            egui::Align::Center,
                        );
                    ui.checkbox(&mut self.save_without_prefix, layout); // TODO
                });
            ui.add_space(10.0);

            let pull_progress = self.pull_progress.lock().unwrap();
            let pull_progress_copy = pull_progress.clone();
            drop(pull_progress);
            if pull_progress_copy == PullProgress::InProgress {
                ui.horizontal(|ui| {
                    ui.label("Descargando...");
                    ui.spinner();
                });
            }
            if pull_progress_copy == PullProgress::Ended {
                ui.label("Finalizado"); // TODO split
            }
            if pull_progress_copy != PullProgress::NotStarted {
                ui.add_space(10.0);
                ui.label("Estado:");
                ui.add_space(10.0);
                // Add received messages to buffer
                for row in self.pull_progress_rx.try_iter() {
                    self.table_rows.push(row);
                }
                // Print buffer as table
                self.progress_table(ui);
            } else {
                ui.add_space(120.0);
            }
            if pull_progress_copy == PullProgress::Ended {
                ui.add_space(14.0);
            }

            ui.add_space(10.0);

            egui::Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    let download_btn = egui::Button::new("Descargar");
                    if ui
                        .add_enabled(pull_progress_copy != PullProgress::Ended, download_btn)
                        .clicked()
                    {
                        let mut tasks: Task<()> = Task::new();
                        let Ok(zot_client) = zot::Client::new(&self.config) else {
                            ui.close_kind(egui::UiKind::Window);
                            return println!("Invalid config");
                        };
                        let image_name = self.image.get_image_name();
                        let registry_name = zot_client.get_registry_name();
                        let pull_name = format!("{registry_name}/{image_name}");
                        let creds = zot_client.get_credentials();

                        *self.pull_progress.lock().unwrap() = PullProgress::InProgress;

                        let tx_arc = Arc::new(Mutex::new(self.pull_progress_tx.clone()));
                        let progress_arc = Arc::clone(&self.pull_progress);
                        tasks.fire_async(async move {
                            let tx_arc = Arc::clone(&tx_arc);
                            let Ok(docker) = container::Client::new().await else {
                                return println!("No docker client");
                            };
                            match docker.pull_image(pull_name, creds, tx_arc).await {
                                Ok(_) => println!("Pull OK"),
                                Err(x) => println!("Pull failed: {x:?}"),
                            }
                            *progress_arc.lock().unwrap() = PullProgress::Ended;
                            // TODO trigger main list refresh?
                        });
                    }

                    if ui.button("Cerrar").clicked() {
                        let v = self
                            .pull_progress_rx
                            .recv_timeout(std::time::Duration::from_millis(1));
                        println!("{v:?}");
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
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::LEFT))
            .column(Column::auto().resizable(false))
            .column(
                Column::remainder()
                    .at_least(10.0)
                    .clip(true)
                    .resizable(true)
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
                                progress_str = format!("({progress}%)");
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
