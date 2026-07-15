use crate::docker;
use crate::tasks::Task;
use crate::upload_window;
use crate::image_info_window;
use eframe::egui;
use egui::{Align, Layout, UiBuilder};
use egui_extras::Column;
use egui_extras::TableBuilder;
use std::sync::{Arc, Mutex};

use docker::Image;

pub fn ui_start() -> eframe::Result {
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([920.0, 440.0]),
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

pub struct ImageTableData {
    date_reversed: bool,
}

pub struct MyApp {
    image_list: Arc<Mutex<Vec<Image>>>,
    image_list_loaded: Arc<Mutex<bool>>,
    data: Task<()>,
    upload_window_state: Arc<Mutex<upload_window::WindowState>>,
    image_info_window_state: Arc<Mutex<image_info_window::WindowState>>,
    image_table_data: ImageTableData,
}

impl MyApp {
    pub fn update_list(&mut self) {
        let image_list_arc = Arc::clone(&self.image_list);
        let image_list_loaded_arc = Arc::clone(&self.image_list_loaded);

        self.data.fire_async(async move {
            //tokio::task::spawn_local(async move {
	    *image_list_loaded_arc.lock().unwrap() = false;
            let mut images = docker::bollard_image_list().await;

            let mut image_list = image_list_arc.lock().unwrap();
            image_list.clear();
            image_list.append(&mut images);
            image_list.sort_by_key(|x| x.date); // Oldest image first
            *image_list_loaded_arc.lock().unwrap() = true;
        });
    }
}

impl Default for MyApp {
    fn default() -> Self {
        let mut image_list = Vec::new();
        for i in 1..4 {
            image_list.push(Image {
                name: Some(format!("Test {i}")),
                size_bytes: i * 1000,
                digest: String::new(),
                date: chrono::DateTime::<chrono::Utc>::from_timestamp_secs(
                    (i * 1000).try_into().unwrap(),
                )
                .unwrap(),
            });
        }
        Self {
            image_list: Arc::new(Mutex::new(image_list)),
            image_list_loaded: Arc::new(Mutex::new(false)),
            data: Task::new(),
            upload_window_state: Arc::new(Mutex::new(upload_window::WindowState::default())),
            image_info_window_state: Arc::new(Mutex::new(image_info_window::WindowState::default())),
            image_table_data: ImageTableData {
                date_reversed: true,
            },
        }
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let upload_window_state_arc = Arc::clone(&self.upload_window_state);
        let upload_window_state = &mut *upload_window_state_arc.lock().unwrap();
        upload_window::show(&mut upload_window_state.options, ui, &mut upload_window_state.open);
        let image_info_window_state_arc = Arc::clone(&self.image_info_window_state);
        let image_info_window_state = &mut *image_info_window_state_arc.lock().unwrap();
        image_info_window::show(&mut image_info_window_state.options, ui, &mut image_info_window_state.open);
	//println!("{}",image_info_window_state.open);
	
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("recoge");

            ui.horizontal(|ui| {
                if !*self.image_list_loaded.lock().unwrap() {
                    ui.label("Cargando lista ");
                    ui.spinner();
                } else {
                    ui.label("Lista cargada");
                }
            });

            ui.label("Imágenes locales:".to_string());

            let mut reset_btn_ui = ui.new_child(
                UiBuilder::new()
                    .max_rect(ui.max_rect())
                    .layout(Layout::right_to_left(Align::Min)),
            );
            if reset_btn_ui
                .button("⟲")
                .on_hover_text("Recargar lista de imágenes")
                .clicked()
            {
                self.update_list();
            }

            let date_rev = &mut self.image_table_data.date_reversed;
            let available_height = ui.available_height();
            let _table = TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::LEFT))
                .column(
                    Column::remainder()
                        //.at_least(80.0)
                        .clip(true)
                        .resizable(true),
                )
                .column(Column::auto())
                .column(Column::auto())
                .column(Column::auto())
                .min_scrolled_height(0.0)
                .max_scroll_height(available_height)
                .header(30.0, |mut header| {
                    header.col(|ui| {
                        ui.strong("Nombre/Tag");
                    });
                    header.col(|ui| {
                        ui.strong("Tamaño");
                    });
                    header.col(|ui| {
                        ui.strong("Digest");
                    });
                    header.col(|ui| {
                        egui::Sides::new().show(
                            ui,
                            |ui| {
                                ui.strong("Fecha de creación");
                            },
                            |ui| {
                                *date_rev ^= ui.button(if *date_rev { "⬆" } else { "⬇" })
				    .on_hover_text(if *date_rev { "Ascendente" } else { "Descendente" })
				    .clicked();
                            },
                        );
                    });
                })
                .body(|mut body| {
                    let list = self.image_list.lock().unwrap();
		    let date_rev = self.image_table_data.date_reversed;
                    if date_rev {
                        for image in list.iter().rev() {
                            self.image_table_row(image, &mut body, upload_window_state, image_info_window_state);
                        }
                    } else {
                        for image in list.iter() {
                            self.image_table_row(image, &mut body, upload_window_state, image_info_window_state);
                        }
                    }
                });
        });
    }
}

impl MyApp {
    fn image_table_row(&self, image: &Image, body: &mut egui_extras::TableBody, upload_window_state: &mut upload_window::WindowState, image_info_window_state: &mut image_info_window::WindowState) {
	let row_height = 18.0;
	//row.set_selected(self.selection.contains(&row_index));
	//row.set_overline(self.overline && row_index % 7 == 3);
	body.row(row_height, |mut row| {
            row.col(|ui| {
		let image_name = &image.get_name();
		ui.label(image_name);

		ui.with_layout(egui::Layout::right_to_left(egui::Align::LEFT), |ui| {
                    if ui.button("⬆").on_hover_text("Subir a registro").clicked() {
			//*boolean = !*boolean;
			println!("Subir {image_name}");
			upload_window_state.options.image_name =
                            Some(image_name.to_string());
			upload_window_state.open = true;
                    }
                    if ui
			.button("ℹ")
			.on_hover_text("Información de la imagen")
			.clicked()
                    {
			println!("Info de {image_name}");
			image_info_window_state.options.image_name =
                            Some(image_name.to_string());
			image_info_window_state.open = true;
                    }

                    if ui.button("🗑").on_hover_text("Eliminar la imagen").clicked() {
			println!("Eliminar {image_name}");
                    }
		});
            });
            row.col(|ui| {
		ui.label(image.get_human_size());
            });
            row.col(|ui| {
		//ui.add(egui::Separator::default().horizontal());
		ui.label(image.get_id());
            });
            row.col(|ui| {
		ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
		ui.label(image.get_date_local());
            });

            //self.toggle_row_selection(row_index, &row.response());
	});
    }
}
