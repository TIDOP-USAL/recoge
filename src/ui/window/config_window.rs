// TODO change to modal
use super::WindowOptions;
use crate::configuration::AppConfig;
use crate::tasks::Task;
use crate::ui::GlobalData;
use crate::zot;
use eframe::egui;
use egui::{Align, Layout, UiBuilder};
use egui_notify::Toasts;
use futures::executor::block_on;
use futures::lock::Mutex as AsyncMutex;
use std::sync::Arc;

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

    pub fn open(
        &mut self,
        config: AppConfig,
        global_data_arc: Arc<AsyncMutex<GlobalData>>,
        toasts_arc: Arc<AsyncMutex<Toasts>>,
    ) {
        self.data.app_config_copy = config;
        self.data.global_data_arc = global_data_arc;
        self.data.toasts_arc = toasts_arc;
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }
}

pub fn window_options() -> WindowOptions {
    WindowOptions {
        id: egui::Id::new("config_window"),
        title: "Configuración".to_owned(),
        ..Default::default()
    }
}

#[derive(Default)]
pub struct WindowData {
    pub app_config_copy: AppConfig, // TODO remove
    pub global_data_arc: Arc<AsyncMutex<GlobalData>>,
    pub toasts_arc: Arc<AsyncMutex<Toasts>>,
    unsaved_changes: bool,
}

pub fn ui_fn(ui: &mut egui::Ui, data: &mut WindowData) {
    let WindowData {
        app_config_copy,
        global_data_arc,
        toasts_arc,
        unsaved_changes,
    } = data;

    // We work on app_config_copy. On edit, we call config_update. On save, we also call config_save

    ui.add_space(2.0);
    ui.label("Credenciales de Registro de Contenedores:");
    ui.add_space(10.0);

    fn update_temporal_config(
        unsaved_changes: &mut bool,
        new_config: &AppConfig,
        global_data_arc: &Arc<AsyncMutex<GlobalData>>,
    ) {
        let mut tasks: Task<()> = Task::new();
        let global_data_arc = Arc::clone(global_data_arc);
        let new_config = new_config.clone();
        *unsaved_changes = true;
        tasks.fire_async(async move {
            println!("Updating temporal config");
            config_update(new_config, &global_data_arc);
        });
    }

    egui::Grid::new("config_rows_grid_1")
        .num_columns(2)
        .spacing([20.0, 5.0])
        .striped(false)
        .show(ui, |ui| {
            ui.label("Dirección del Registro:");
            let registry_ui = ui.text_edit_singleline(&mut app_config_copy.remote_url);
            registry_ui.context_menu(|ui| {
                textedit_context_menu(ui, &mut app_config_copy.remote_url);
                registry_ui.request_focus();
            });
            if registry_ui.changed() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                update_temporal_config(
                    unsaved_changes,
                    &app_config_copy,
                    &Arc::clone(&global_data_arc),
                );
            }
            ui.end_row();

            ui.label("Usuario:");
            let user_ui = ui.text_edit_singleline(&mut app_config_copy.user);
            user_ui.context_menu(|ui| {
                textedit_context_menu(ui, &mut app_config_copy.user);
                user_ui.request_focus();
            });
            if user_ui.changed() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                update_temporal_config(
                    unsaved_changes,
                    &app_config_copy,
                    &Arc::clone(&global_data_arc),
                );
            }
            ui.end_row();

            let state_id = ui.id().with("show_plaintext");
            let mut show_plaintext = ui.data_mut(|d| d.get_temp::<bool>(state_id).unwrap_or(false));
            ui.label("Contraseña:");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                /*if ui.button("X").on_hover_text("Borrar contraseña").clicked() {
                    app_config_copy.password.clear();
                }*/

                let response = ui
                    .selectable_label(show_plaintext, "👁")
                    .on_hover_text("Mostrar/ocultar contraseña");
                if response.clicked() {
                    show_plaintext = !show_plaintext;
                }

                let password_edit = egui::TextEdit::singleline(&mut app_config_copy.password)
                    .password(!show_plaintext);
                let password_ui = ui.add_sized(ui.available_size(), password_edit);
                password_ui.context_menu(|ui| {
                    textedit_context_menu(ui, &mut app_config_copy.password);
                    password_ui.request_focus();
                });
                if password_ui.changed() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    update_temporal_config(
                        unsaved_changes,
                        &app_config_copy,
                        &Arc::clone(&global_data_arc),
                    );
                }
            });

            // Store the (possibly changed) state:
            ui.data_mut(|d| d.insert_temp(state_id, show_plaintext));
            ui.end_row();
            ui.label("");
            if ui.button("Comprobar").clicked() {
                println!("Check creds");
                let mut tasks: Task<()> = Task::new();
                let creds_config = app_config_copy.clone();
                let toasts_arc = Arc::clone(toasts_arc);
                tasks.fire_async(async move {
                    check_creds(creds_config, &toasts_arc).await;
                });
            }
            ui.end_row();
        });

    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let mut bottom_btn_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(ui.max_rect())
                .layout(Layout::right_to_left(Align::Max)),
        );

        let save_button = egui::Button::new("Guardar y salir");
        if bottom_btn_ui
            .add_enabled(*unsaved_changes, save_button)
            .on_hover_text("Guardar la configuración permanentemente")
            .clicked()
        {
            println!("Guardar");
            let mut tasks: Task<()> = Task::new();
            let global_data_arc = Arc::clone(global_data_arc);
            let toasts_arc = Arc::clone(toasts_arc);
            let new_config = app_config_copy.clone();
            ui.close_kind(egui::UiKind::Window);
            tasks.fire_async(async move {
                config_update(new_config, &Arc::clone(&global_data_arc));
                config_save(&Arc::clone(&global_data_arc), &toasts_arc);
            });
        }
        bottom_btn_ui.add_space(1.0);
        if bottom_btn_ui
            .button("Salir")
            .on_hover_text("Salir")
            .clicked()
        {
            ui.close_kind(egui::UiKind::Window);
        }
    });
    ui.add_space(8.0);

    //let on_top = Some(ui.layer_id()) == ui.ctx().top_layer_id();
    //ui.label(format!("This window is on top: {on_top}."));
}

fn config_update(new_config: AppConfig, global_data_arc: &Arc<AsyncMutex<GlobalData>>) {
    let mut global_data = block_on(global_data_arc.lock().into_future());
    println!("locked global data");
    println!("new data: {new_config:?}");
    global_data.config.remote_url = new_config.remote_url;
    global_data.config.user = new_config.user;
    global_data.config.password = new_config.password;
}

fn config_save(
    global_data_arc: &Arc<AsyncMutex<GlobalData>>,
    toasts_arc: &Arc<AsyncMutex<Toasts>>,
) {
    let global_data = block_on(global_data_arc.lock().into_future());
    println!("locked global data");
    let result = global_data.config.serialize_config();
    let mut toasts = block_on(toasts_arc.lock().into_future());
    if let Ok(()) = result {
        println!("Saved config file");
        toasts.success("Configuración guardada correctamente");
    } else {
        println!("Error while saving config file");
        toasts.error("Error al guardar la configuración");
    }
}

fn textedit_context_menu(ui: &mut egui::Ui, content: &mut String) {
    if ui.button("Copiar todo").clicked() {
        ui.ctx().copy_text(content.clone());
        ui.close();
    }
    if ui.button("Pegar y reemplazar").clicked() {
        content.clear();
        ui.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
        ui.close();
    }
    if ui.button("Borrar").clicked() {
        content.clear();
    }
}

async fn check_creds(config: AppConfig, toasts_arc: &Arc<AsyncMutex<Toasts>>) {
    if let Ok(mut client) = zot::Client::new(&config) {
        if client.get_repository_list().await.is_ok() {
            let mut toasts = toasts_arc.lock().await;
            toasts.success("Credenciales correctas. Puedes guardar la configuración.");
        } else {
            let mut toasts = toasts_arc.lock().await;
            toasts.error("Credenciales inccorrectas. Revisa los campos de configuración.");
        }
    } else {
        let mut toasts = toasts_arc.lock().await;
        toasts.error("Credenciales inccorrectas. Revisa los campos de configuración.");
    }
}
