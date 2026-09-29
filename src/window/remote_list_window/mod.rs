pub mod pull_modal;
pub mod table_list;

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
use table_list::table_ui;

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
        let is_in_remote = zot::get_remote_info(last_image.clone().id, &mut client).await; // TODO check refactor
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
        remote_image_list.reverse();
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
    let min_window_size = egui::vec2(600.0, 225.0);
    WindowOptions {
        id: Id::new("remote_list_window"),
        title: "Imágenes remotas en Registro".to_owned(),
        closable: true,
        collapsible: true,
        resizable: true,
        min_size: WindowSize::Fixed(min_window_size),
        //max_size: WindowSize::Fixed(max_window_size),
        ..Default::default()
    }
}

pub struct WindowData {
    pub main_list_reload_flag: Option<Arc<Mutex<bool>>>,
    pub app_config: AppConfig,
    pub remote_image_list: Vec<container::Image>,
    pub toasts_arc: Arc<AsyncMutex<Toasts>>,
    image_table_options: ImageTableOptions,
    pull_modal_data: Option<pull_modal::ModalData>,
    delete_modal_data: Option<delete_modal::ModalData>,
}

impl Default for WindowData {
    fn default() -> Self {
        Self {
            main_list_reload_flag: None,
            app_config: AppConfig::default(),
            remote_image_list: Vec::new(),
            toasts_arc: Arc::new(AsyncMutex::new(Toasts::default())),
            image_table_options: ImageTableOptions {
                sort_type: TableSortType::Date,
                ui_date_rev: true,
                ui_name_rev: false,
            },
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

pub fn ui_fn(ui: &mut egui::Ui, data: &mut WindowData) {
    let WindowData {
        main_list_reload_flag,
        app_config,
        remote_image_list,
        toasts_arc: _,
        image_table_options,
        pull_modal_data,
    } = &mut *data;

    let Some(main_list_reload_flag) = main_list_reload_flag else {
        eprintln!("Window opened without main_list_reload_flag");
        ui.close_kind(egui::UiKind::Window);
        return;
    };

    ui.label("Imágenes remotas (sólo última versión):");
    ui.add_space(10.0);

    ui.group(|ui| {
        table_ui(
            ui,
            image_table_options,
            remote_image_list,
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
