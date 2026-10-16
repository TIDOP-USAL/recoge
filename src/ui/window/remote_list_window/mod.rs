pub mod delete_modal;
pub mod pull_modal;
pub mod table_list;
pub mod tag_list;

use super::{WindowOptions, WindowSize};
use crate::configuration::AppConfig;
use crate::container;
use crate::tasks::Task;
use crate::zot;
use bollard::config::CreateImageInfo;
use eframe::egui;
use egui::{Id, Modal};
use egui_notify::Toasts;
use futures::executor::block_on;
use futures::lock::Mutex as AsyncMutex;
use std::sync::{Arc, Mutex};
use table_list::table_ui;

#[derive(Default)]
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

impl WindowData {
    pub async fn init(
        &mut self,
        config: &AppConfig,
        reload_flag: Arc<Mutex<bool>>,
        toasts: Arc<AsyncMutex<Toasts>>,
    ) -> Result<(), ()> {
        let toasts_arc = Arc::clone(&toasts);
        // Set window parameters
        self.main_list_reload_flag = Arc::clone(&reload_flag);
        self.toasts_arc = toasts;
        self.app_config = config.clone();
        // Block the window from being opened while the repo list is loaded
        // Tag list will be first initialized in the UI loop
        let repos = self.load_repo_list().await?;
        self.remote_repository_list_status = ListStatus::Loaded;
        // Get first repo (selected)
        if let Some(first_repo) = repos.first() {
            self.selected_repository = Some(first_repo.clone());
        }
        Ok(())
    }

    pub async fn load_repo_list(&mut self) -> Result<Vec<String>, ()> {
        let toasts_arc = Arc::clone(&self.toasts_arc);
        let repos = tag_list::get_repo_list(&self.app_config, toasts_arc).await?;
        self.remote_repository_list = repos.clone();
        Ok(repos)
    }
}

pub fn window_options() -> WindowOptions {
    let min_window_size = egui::vec2(600.0, 225.0);
    WindowOptions {
        id: Id::new("remote_list_window"),
        title: "Imágenes remotas en Registro".to_owned(),
        closable: true,
        collapsible: true,
        resizable: true,
        min_size: WindowSize::Fixed(min_window_size),
        default_size: WindowSize::Fixed(egui::vec2(min_window_size.x / 2.0, 160.0)),
        //max_size: WindowSize::Fixed(max_window_size),
        ..Default::default()
    }
}

pub struct WindowData {
    pub display_mode: DisplayMode,
    pub main_list_reload_flag: Arc<Mutex<bool>>,
    pub app_config: AppConfig,
    // Repo tag list
    pub remote_repository_list_status: ListStatus,
    pub remote_repository_list: Vec<String>,
    pub selected_repository: Option<String>,
    pub remote_tag_list_status: ListStatus,
    pub remote_tag_list: Vec<container::Image>,
    // Image list
    pub remote_image_list_status: ListStatus,
    pub remote_image_list: Vec<container::Image>,
    pub remote_image_list_mode: RemoteImageListMode,
    pub toasts_arc: Arc<AsyncMutex<Toasts>>,
    image_table_options: ImageTableOptions,
    pull_modal_data: Option<pull_modal::ModalData>,
    delete_modal_data: Option<delete_modal::ModalData>,
}

#[derive(PartialEq)]
pub enum ListStatus {
    NotLoaded,
    Loaded,
    NeedsReload,
    Loading,
    Failed,
}

#[derive(PartialEq)]
pub enum DisplayMode {
    ImageList,
    RepoTagList,
}

#[derive(PartialEq, Clone)]
pub enum RemoteImageListMode {
    All,
    LastRepoImage,
}

impl Default for WindowData {
    fn default() -> Self {
        Self {
            display_mode: DisplayMode::RepoTagList,
            main_list_reload_flag: Arc::default(),
            app_config: AppConfig::default(),
            remote_repository_list_status: ListStatus::NotLoaded,
            remote_repository_list: Vec::new(),
            selected_repository: None,
            remote_tag_list_status: ListStatus::NeedsReload,
            remote_tag_list: Vec::new(),
            remote_image_list_status: ListStatus::NeedsReload,
            remote_image_list: Vec::new(),
            remote_image_list_mode: RemoteImageListMode::All,
            toasts_arc: Arc::new(AsyncMutex::new(Toasts::default())),
            image_table_options: ImageTableOptions {
                sort_type: TableSortType::Date,
                ui_date_rev: true,
                ui_name_rev: false,
            },
            pull_modal_data: None,
            delete_modal_data: None,
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
#[derive(Debug, Clone)]
pub struct ImageTableOptions {
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

pub fn ui_fn(ui: &mut egui::Ui, data_arc: &mut Arc<AsyncMutex<WindowData>>) {
    let data_arc_1 = Arc::clone(&data_arc);
    let mut data = block_on(data_arc_1.lock().into_future());
    let WindowData {
        display_mode,
        main_list_reload_flag,
        app_config,
        remote_repository_list_status,
        remote_repository_list,
        selected_repository,
        remote_tag_list_status,
        remote_tag_list,
        remote_image_list_status,
        remote_image_list,
        remote_image_list_mode,
        toasts_arc,
        image_table_options,
        pull_modal_data,
        delete_modal_data,
    } = &mut *data;

    /* TODO this was optional. now could be a dangling Arc (default)
        let Some(main_list_reload_flag) = main_list_reload_flag else {
            eprintln!("Window opened without main_list_reload_flag");
            ui.close_kind(egui::UiKind::Window);
            return;
    };*/

    // Table data load/reload logic
    if *display_mode == DisplayMode::RepoTagList {
        // Repo list update logic (async)
        if *remote_repository_list_status != ListStatus::Loaded {
            ui.disable();
        }

        if *remote_repository_list_status == ListStatus::NeedsReload {
            println!("Updating repo list");
            *remote_repository_list_status = ListStatus::Loading;
            let mut tasks: Task<()> = Task::new();
            let data_arc_2 = Arc::clone(&data_arc);
            let data_arc_3 = Arc::clone(&data_arc);
            let ctx = ui.ctx().clone();
            tasks.fire_async(async move {
                match tag_list::load_repo_list(data_arc_2).await {
                    Ok(repos) => {
                        println!("OK repo load");
                        let mut data = data_arc_3.lock().await;
                        data.remote_repository_list_status = ListStatus::Loaded;
                        ctx.request_repaint();
                    }
                    Err(_) => {
                        let mut data = data_arc_3.lock().await;
                        data.remote_repository_list_status = ListStatus::Failed;
                    }
                }
            });
        }
        // Tag list update logic (async)
        if *remote_repository_list_status == ListStatus::Loaded
            && *remote_tag_list_status == ListStatus::NeedsReload
        {
            if let Some(selected_repo) = selected_repository.clone() {
                println!("Refreshing tags for {selected_repo}");
                *remote_tag_list_status = ListStatus::Loading;
                let mut tasks: Task<()> = Task::new();
                let data_arc_2 = Arc::clone(&data_arc);
                let data_arc_3 = Arc::clone(&data_arc);
                let ctx = ui.ctx().clone();
                tasks.fire_async(async move {
                    match tag_list::load_tag_list(data_arc_2, selected_repo).await {
                        Ok(_) => {
                            let mut data = data_arc_3.lock().await;
                            data.remote_tag_list_status = ListStatus::Loaded;
                            ctx.request_repaint();
                        }
                        Err(_) => {
                            let mut data = data_arc_3.lock().await;
                            data.remote_tag_list_status = ListStatus::Failed;
                        }
                    }
                });
            }
        }
    } else {
        // Image list update logic
        if *remote_image_list_status != ListStatus::Loaded {
            ui.disable();
        }
        if *remote_image_list_status == ListStatus::NeedsReload {
            println!("Updating image list");
            *remote_image_list_status = ListStatus::Loading;
            let mut tasks: Task<()> = Task::new();
            let data_arc_2 = Arc::clone(&data_arc);
            let data_arc_3 = Arc::clone(&data_arc);
            let ctx = ui.ctx().clone();
            tasks.fire_async(async move {
                match table_list::load_list_data(data_arc_2).await {
                    Ok(repos) => {
                        println!("OK repo load");
                        let mut data = data_arc_3.lock().await;
                        data.remote_image_list_status = ListStatus::Loaded;
                        ctx.request_repaint();
                    }
                    Err(_) => {
                        let mut data = data_arc_3.lock().await;
                        data.remote_image_list_status = ListStatus::Failed;
                    }
                }
            });
        }
    }

    ui.horizontal(|ui| {
        ui.label("Imágenes remotas:");
        ui.selectable_value(
            display_mode,
            DisplayMode::RepoTagList,
            "|Ξ|☰| Lista separada",
        )
        .on_hover_text(
            "Mostrar una lista de imágenes separada entre repositorios y versiones (tag)",
        );
        ui.selectable_value(
            display_mode,
            DisplayMode::ImageList,
            "|☰| Lista de imágenes",
        )
        .on_hover_text("Mostrar todas las imágenes en lista");

        if *display_mode == DisplayMode::ImageList {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::LEFT), |ui| {
		if ui
                    .button("⟲")
                    .on_hover_text("Recargar lista de imágenes remotas")
                    .clicked()
                {
                    *remote_image_list_status = ListStatus::NeedsReload;
                }
		let last_btn = ui.button("⏭").on_hover_text("Mostrar sólo la última versión de cada repositorio.\nSe entiende por última versión, la última que se haya subido al registro, no necesariamente la de mayor número de versión.");
		if *remote_image_list_mode == RemoteImageListMode::LastRepoImage {
		    last_btn.clone().highlight();
		}
		if last_btn.clicked() {
		    *remote_image_list_mode = match *remote_image_list_mode {
			RemoteImageListMode::All => RemoteImageListMode::LastRepoImage,
			RemoteImageListMode::LastRepoImage => RemoteImageListMode::All,
    		    };
		    *remote_image_list_status = ListStatus::NeedsReload;
		}
            });
            if *remote_image_list_status == ListStatus::Loading {
                ui.set_cursor_icon(egui::CursorIcon::Wait);
            }
        }
    });
    ui.add_space(6.0);

    match display_mode {
        DisplayMode::RepoTagList => {
            // Repository panel
            egui::Panel::left("left_panel")
                .resizable(true)
                .min_size(150.0)
                .default_size(200.0)
                .max_size(400.0)
                .show(ui, |ui| {
                    ui.group(|ui| {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.heading("Repositorios");
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::LEFT),
                                    |ui| {
                                        if ui
                                            .button("⟲")
                                            .on_hover_text("Recargar lista de repositorios remotos")
                                            .clicked()
                                        {
                                            *remote_repository_list_status =
                                                ListStatus::NeedsReload;
                                        }
                                    },
                                );
                            });
                            ui.add_space(3.0);
                        });
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            if *remote_repository_list_status == ListStatus::Loading {
                                ui.set_cursor_icon(egui::CursorIcon::Wait);
                            }
                            for repo in remote_repository_list.clone() {
                                if ui
                                    .selectable_value(
                                        selected_repository,
                                        Some(repo.to_string()),
                                        repo.to_string(),
                                    )
                                    .on_hover_text(format!("Seleccionar el repositorio {repo}"))
                                    .clicked()
                                {
                                    println!("New selected repo: {selected_repository:?}");
                                    *remote_tag_list_status = ListStatus::NeedsReload;
                                }
                            }
                            ui.allocate_space(ui.available_size());
                        });
                    });
                });

            egui::CentralPanel::default().show(ui, |ui| {
                if *remote_tag_list_status == ListStatus::Loading {
                    ui.set_cursor_icon(egui::CursorIcon::Wait);
                }

                if let Some(repo) = selected_repository {
                    ui.horizontal(|ui| {
                        //ui.label("Repositorio:");
                        ui.label("🗀");
                        ui.code(repo).on_hover_text("El repositorio seleccionado");
                        if *remote_tag_list_status == ListStatus::Loading {
                            ui.spinner();
                        }
                    });
                }
                // Tag table
                ui.group(|ui| {
                    if *remote_tag_list_status == ListStatus::Loading {
                        ui.disable();
                    }
                    tag_list::table_ui(
                        ui,
                        image_table_options,
                        remote_tag_list,
                        &app_config,
                        pull_modal_data,
                        delete_modal_data,
                    );
                });
            });
        }
        DisplayMode::ImageList => {
            ui.group(|ui| {
                table_ui(
                    ui,
                    image_table_options,
                    remote_image_list,
                    &app_config,
                    pull_modal_data,
                    delete_modal_data,
                );
            });
        }
    }

    /*ui.group(|ui| {
        table_ui(
            ui,
            image_table_options,
            remote_image_list,
            &app_config,
            pull_modal_data,
            delete_modal_data,
        );
    });*/

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

    // Delete modal
    if let Some(data) = delete_modal_data {
        let modal = data.modal(ui);
        if modal.should_close() {
            if data.image_was_deleted() {
                // Remove from image list
                let index = remote_image_list
                    .iter()
                    .position(|x| x.get_image_name() == data.image_name);
                if let Some(index) = index {
                    remote_image_list.remove(index);
                }
                // Remove from tag list
                let index = remote_tag_list
                    .iter()
                    .position(|x| x.get_image_name() == data.image_name);
                if let Some(index) = index {
                    remote_tag_list.remove(index);
                }
            }
            // Close modal
            *delete_modal_data = None;
        }
    }

    //let on_top = Some(ui.layer_id()) == ui.ctx().top_layer_id();
    //ui.label(format!("This window is on top: {on_top}."));
}
