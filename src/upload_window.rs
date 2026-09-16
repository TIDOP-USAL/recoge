use crate::configuration::AppConfig;
use crate::container;
use crate::tasks::Task;
use crate::zot;
use bollard::config::PushImageInfo;
use eframe::egui;
use egui::Vec2b;
use egui::widgets::TextEdit;
use egui_extras::{Column, TableBuilder};
use std::sync::{Arc, Mutex};

pub struct WindowState {
    pub options: WindowOptions,
    pub open: bool,
}

impl WindowState {
    pub fn open(&mut self, config: AppConfig, image_repo_info: container::RepositoryInfo) {
        self.options.selected_image_info = Some(image_repo_info);
        self.options.config = config;
        self.options.edit_info = None;
        self.options.push_info = None;
        self.open = true;
    }
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            options: WindowOptions::default(),
            open: false,
        }
    }
}

#[derive(Clone)]
pub struct ImageInfo {
    pub remote_repo_prefix: String,
    pub selected_name: String,
    pub repo_info: container::RepositoryInfo,
    pub stripped_repo: String,
    // User-modified, stripped
    pub new_repository_name: String,
    // User-modified
    pub new_tag: String,
}

impl ImageInfo {
    pub fn get_new_name(&self) -> String {
        let new_tag = if self.new_tag.is_empty() {
            "latest"
        } else {
            &self.new_tag
        };
        format!(
            "{}/{}:{}",
            self.remote_repo_prefix, self.new_repository_name, new_tag
        )
    }
    pub fn get_new_remote_repo_info(&self) -> container::RepositoryInfo {
        container::RepositoryInfo {
            repository: format!(
                "{}/{}",
                self.remote_repo_prefix,
                self.new_repository_name.clone()
            ),
            tag: self.new_tag.clone(),
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
    pub data_is_loading: bool,
    pub selected_image_info: Option<container::RepositoryInfo>,
    pub config: AppConfig,
    pub edit_info: Option<ImageInfo>,
    pub push_info: Option<PushInfo>,
}

pub struct PushInfo {
    progress_tx: std::sync::mpsc::SyncSender<PushImageInfo>,
    progress_rx: std::sync::mpsc::Receiver<PushImageInfo>,
    table_rows: Vec<PushImageInfo>,
    progress: Arc<Mutex<PushProgress>>,
}

#[derive(PartialEq, Clone)]
pub enum PushProgress {
    NotStarted,
    InProgress,
    Ok,
    Failed,
}

#[allow(unused)]
impl PushProgress {
    fn has_ended(&self) -> bool {
        *self == Self::Ok || *self == Self::Failed
    }
    fn can_start(&self) -> bool {
        *self != Self::Ok && *self != Self::InProgress
    }
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            title: "Subir a registro (Push)".to_owned(),
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
            selected_image_info: None,
            config: AppConfig::default(),
            edit_info: None,
            push_info: None,
        }
    }
}

pub fn show(mut options: &mut WindowOptions, ui: &mut egui::Ui, open: &mut bool) {
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
    let data_is_loading = options.data_is_loading;

    let enabled = !data_is_loading;
    if !enabled {
        ui.request_repaint();
    }

    let mut window = egui::Window::new(title)
        .id(egui::Id::new("upload_window_options")) // required since we change the title
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
    let rect_size = ui.content_rect().size();
    window
        .default_pos(egui::pos2(rect_size.x / 2.0, rect_size.y / 2.0))
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
        selected_image_info,
        config,
        edit_info,
        push_info,
    } = options;

    let Some(selected_image_info) = selected_image_info else {
        eprintln!("Window opened without selected_image_info");
        ui.close_kind(egui::UiKind::Window);
        return;
    };

    if edit_info.is_none() {
        let stripped_repo = selected_image_info.get_stripped_repository();
        //let initial_name = format!("{stripped_repo}",);
        let selected_name = selected_image_info.get_image();
        *edit_info = Some(ImageInfo {
            remote_repo_prefix: "cr.tidop.es".to_string(), // TODO
            selected_name: selected_name.clone(),
            repo_info: selected_image_info.clone(),
            stripped_repo: stripped_repo.clone(),
            new_repository_name: stripped_repo,
            new_tag: selected_image_info.tag.clone(),
        });
    }
    let Some(edit_info) = edit_info else {
        eprintln!("Fatal");
        ui.close_kind(egui::UiKind::Window);
        return;
    };
    if push_info.is_none() {
        let (tx, rx) = std::sync::mpsc::sync_channel(1000);
        *push_info = Some(PushInfo {
            progress_tx: tx,
            progress_rx: rx,
            table_rows: Vec::new(),
            progress: Arc::new(Mutex::new(PushProgress::NotStarted)),
        });
    }
    let Some(push_info) = push_info else {
        eprintln!("Fatal");
        ui.close_kind(egui::UiKind::Window);
        return;
    };

    let local_image_name = edit_info.selected_name.clone();

    ui.set_width(500.0);
    ui.add_space(10.0);

    egui::Grid::new("modal_rows_grid_1")
        .num_columns(2)
        .spacing([20.0, 10.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label("Imagen local:");
            ui.code(local_image_name.clone());
            ui.end_row();
            // TODO show if this image already exists in remote

            ui.label("Nuevo repositorio:");
            ui.horizontal(|ui| {
                ui.label(format!("{}/", edit_info.remote_repo_prefix));
                let repo = TextEdit::singleline(&mut edit_info.new_repository_name)
                    .desired_width(200.0)
                    .show(ui);
                ui.label(":");
                let tag = TextEdit::singleline(&mut edit_info.new_tag)
                    .hint_text("latest")
                    .show(ui);

                let hover_pos = ui.pointer_hover_pos();
                if let Some(hover_pos) = hover_pos {
                    // TODO time
                    if repo.response.rect.contains(hover_pos) {
                        egui::Tooltip::for_widget(&repo.response).show(|ui| {
                            ui.label("Nombre del repositorio para la imagen");
                        });
                    }
                    if tag.response.rect.contains(hover_pos) {
                        egui::Tooltip::for_widget(&tag.response).show(|ui| {
                            ui.label("Versión de la imagen");
                        });
                    }
                }
            });
            ui.end_row();
            ui.label("Se subirá como:");
            ui.code(edit_info.get_new_name());
        });
    ui.add_space(20.0);

    let push_progress_mutex = push_info.progress.lock().unwrap();
    let push_progress = push_progress_mutex.clone();
    drop(push_progress_mutex);
    ui.horizontal(|ui| match push_progress {
        PushProgress::NotStarted => {
            ui.label("Estado de la subida:");
        }
        PushProgress::InProgress => {
            ui.label("Subiendo...");
            ui.add(egui::widgets::Spinner::new().size(10.0));
        }
        PushProgress::Ok => {
            ui.label("Subida correcta ✔");
        }
        PushProgress::Failed => {
            ui.label("Fallo al subir");
        }
    });
    ui.add_space(10.0);
    // Add received messages to buffer
    for row in push_info.progress_rx.try_iter() {
        push_info.table_rows.push(row);
    }
    // Print buffer as table
    ui.group(|ui| {
        push_info.progress_table(ui);
    });
    ui.add_space(10.0);

    // Buttons
    egui::Sides::new().show(
        ui,
        |_ui| {},
        |ui| {
            let download_btn = egui::Button::new("Subir");
            if ui
                .add_enabled(push_progress.can_start(), download_btn)
                .clicked()
            {
                push_info.table_rows.clear();
                let mut tasks: Task<()> = Task::new();
                let Ok(zot_client) = zot::Client::new(&config) else {
                    ui.close_kind(egui::UiKind::Window);
                    return println!("Invalid config");
                };
                let local_image_info = selected_image_info.clone();
                let remote_image_info = edit_info.get_new_remote_repo_info();
                let creds = zot_client.get_credentials();

                *push_info.progress.lock().unwrap() = PushProgress::InProgress;

                let tx_arc = Arc::new(Mutex::new(push_info.progress_tx.clone()));
                let progress_arc = Arc::clone(&push_info.progress);
                tasks.fire_async(async move {
                    let tx_arc = Arc::clone(&tx_arc);
                    let Ok(docker) = container::Client::new().await else {
                        return println!("No docker client");
                    };

                    *progress_arc.lock().unwrap() = match docker
                        .push_image(local_image_info, remote_image_info, creds, tx_arc)
                        .await
                    {
                        Ok(_) => {
                            println!("Push OK");
                            // TODO trigger main list refresh?
                            PushProgress::Ok
                        }
                        Err(x) => {
                            println!("Pull failed: {x:?}");
                            PushProgress::Failed
                        }
                    }
                });
            }

            if ui.button("Cerrar").clicked() {
                ui.close_kind(egui::UiKind::Window);
            }
        },
    );
}

impl PushInfo {
    fn progress_table(&self, ui: &mut egui::Ui) -> egui::scroll_area::ScrollAreaOutput<()> {
        let available_height = 50.0;
        let row_height = 18.0;
        let table = TableBuilder::new(ui)
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
                    ui.strong("Progreso");
                });
            })
            .body(|mut body| {
                for msg in self.table_rows.iter().rev() {
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
                    match &msg.error_detail {
                        Some(error) => body.row(row_height, |mut row| {
                            row.col(|ui| {
                                let str = error.message.clone().unwrap();
                                ui.label(egui::RichText::new(str).color(egui::Color32::RED));
                            });
                        }),
                        None => {
                            body.row(row_height, |mut row| {
                                row.col(|ui| {
                                    ui.label(format!("{status} {progress_str}"));
                                });
                            });
                        }
                    }
                }
                if self.table_rows.len() < 3 {
                    for _ in 0..(3 - self.table_rows.len()) {
                        body.row(row_height, |mut row| {
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
