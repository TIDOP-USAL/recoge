// TODO change to modal
use crate::configuration::AppConfig;
use crate::tasks::Task;
use crate::ui::GlobalData;
use eframe::egui;
use egui::Vec2b;
use egui::{Align, Layout, UiBuilder};
use egui_notify::Toasts;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Default)]
pub struct WindowState {
    pub options: WindowOptions,
    pub open: bool, // default: closed
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
    pub app_config_copy: AppConfig,
    pub global_data_arc: Arc<Mutex<GlobalData>>,
    pub toasts_arc: Arc<Mutex<Toasts>>,
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            title: "Configuración".to_owned(),
            title_bar: true,
            closable: true,
            collapsible: false,
            movable: true,
            resizable: false,
            constrain: true,
            scroll2: Vec2b::FALSE,
            anchored: false,
            anchor: egui::Align2::RIGHT_TOP,
            anchor_offset: egui::Vec2::ZERO,
            app_config_copy: AppConfig::default(),
            global_data_arc: Arc::new(Mutex::new(GlobalData {
                config_state: crate::ui::ConfigState::NotLoaded,
                config: crate::configuration::AppConfig::default(),
                version: String::new(),
            })),
            toasts_arc: Arc::new(Mutex::new(Toasts::default())),
        }
    }
}

pub fn show(options: &mut WindowOptions, ui: &mut egui::Ui, open: &mut bool) {
    /*
    let WindowOptions {
    task_scheduler: _,
        title,
        title_bar,
        closable,
        collapsible,
        movable,
        resizable,
        constrain,
        scroll2,
        anchored,
        anchor,
        anchor_offset,
        should_close,
    app_config: _,
    } = options.clone();*/
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
        .id(egui::Id::new("config_window_options")) // required since we change the title
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
        app_config_copy,
        global_data_arc,
        toasts_arc,
    } = options;

    // We work on app_config_copy. On edit, we call config_update. On save, we also call config_save

    ui.label("Credenciales de Registro de Contenedores:");

    egui::Grid::new("config_rows_grid_1")
        .num_columns(2)
        .spacing([20.0, 2.0])
        .striped(false)
        .show(ui, |ui| {
            ui.label("Dirección del Registro:");
            let registry_ui = ui.text_edit_singleline(&mut app_config_copy.remote_url);
            registry_ui.context_menu(|ui| {
                textedit_context_menu(ui, &mut app_config_copy.password);
                registry_ui.request_focus();
            });
            if registry_ui.changed() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let mut tasks: Task<()> = Task::new();
                let global_data_arc = Arc::clone(global_data_arc);
                let new_config = app_config_copy.clone();
                tasks.fire_async(async move {
                    println!("async works");
                    config_update(new_config, &global_data_arc);
                });
            }
            ui.end_row();

            ui.label("Usuario:");
            let user_ui = ui.text_edit_singleline(&mut app_config_copy.user);
            user_ui.context_menu(|ui| {
                textedit_context_menu(ui, &mut app_config_copy.password);
                user_ui.request_focus();
            });
            if user_ui.changed() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let mut tasks: Task<()> = Task::new();
                let global_data_arc = Arc::clone(global_data_arc);
                let new_config = app_config_copy.clone();
                tasks.fire_async(async move {
                    println!("async works");
                    config_update(new_config, &global_data_arc);
                });
            }
            ui.end_row();

            let state_id = ui.id().with("show_plaintext");
            let mut show_plaintext = ui.data_mut(|d| d.get_temp::<bool>(state_id).unwrap_or(false));
            ui.label("Contraseña:");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                /*if ui.button("X").on_hover_text("Borrar contraseña").clicked() {
                    app_config_copy.password = String::new();
                }*/

                let response = ui
                    .selectable_label(show_plaintext, "👁")
                    .on_hover_text("Show/hide password");
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
                    let mut tasks: Task<()> = Task::new();
                    let global_data_arc = Arc::clone(global_data_arc);
                    let new_config = app_config_copy.clone();
                    tasks.fire_async(async move {
                        println!("async works");
                        config_update(new_config, &global_data_arc);
                    });
                }
            });

            // Store the (possibly changed) state:
            ui.data_mut(|d| d.insert_temp(state_id, show_plaintext));
            ui.end_row();
        });

    ui.separator();
    ui.horizontal(|ui| {
        //ui.label("Acciones: ");

        let mut bottom_btn_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(ui.max_rect())
                .layout(Layout::right_to_left(Align::Max)),
        );
        if bottom_btn_ui
            .button("Guardar")
            .on_hover_text("Guardar la configuración permanentemente")
            .clicked()
        {
            println!("Guardar");
            let mut tasks: Task<()> = Task::new();
            let global_data_arc = Arc::clone(global_data_arc);
            let toasts_arc = Arc::clone(toasts_arc);
            let new_config = app_config_copy.clone();
            tasks.fire_async(async move {
                config_update(new_config, &Arc::clone(&global_data_arc));
                config_save(&Arc::clone(&global_data_arc), &toasts_arc);
            });
        }
        if bottom_btn_ui
            .button("Salir")
            .on_hover_text("Salir")
            .clicked()
        {
            ui.close_kind(egui::UiKind::Window);
        }

        ui.separator();
    });

    //let on_top = Some(ui.layer_id()) == ui.ctx().top_layer_id();
    //ui.label(format!("This window is on top: {on_top}."));
}

fn config_update(new_config: AppConfig, global_data_arc: &Arc<Mutex<GlobalData>>) {
    let mut global_data = global_data_arc.lock().unwrap();
    println!("locked global data");
    println!("new data: {new_config:?}");
    global_data.config.remote_url = new_config.remote_url;
    global_data.config.user = new_config.user;
    global_data.config.password = new_config.password;
}

fn config_save(global_data_arc: &Arc<Mutex<GlobalData>>, toasts_arc: &Arc<Mutex<Toasts>>) {
    let global_data = global_data_arc.lock().unwrap();
    println!("locked global data");
    let result = global_data.config.serialize_config();
    let mut toasts = toasts_arc.lock().unwrap();
    if let Ok(()) = result {
        println!("Saved config file");
        toasts.info("Configuración guardada correctamente");
    } else {
        println!("Error while saving config file");
        toasts.error("Error al guardar la configuración");
    }
}

fn textedit_context_menu(ui: &mut egui::Ui, content: &mut String) {
    if ui.button("Copiar").clicked() {
        ui.ctx().copy_text(content.clone());
        ui.close();
    }
    if ui.button("Pegar y reemplazar").clicked() {
        *content = String::new();
        ui.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
        ui.close();
    }
    if ui.button("Borrar").clicked() {
        *content = String::new();
    }
}
