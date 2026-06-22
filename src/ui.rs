
use crate::docker;
use eframe::egui;
use egui_extras::TableBuilder;
use egui_extras::Column;
use egui::{UiBuilder, Layout, Align};
use std::sync::{Arc, Mutex};
use crate::tasks::Task;

use docker::Image;

pub fn ui_start() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([920.0, 440.0]),
        ..Default::default()
    };
    eframe::run_native(
        "recoge",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);

            Ok(Box::<MyApp>::default())
        }),
    )
}


pub struct MyApp {
    image_list: Arc<Mutex<Vec<Image>>>,
    image_list_loaded: Arc<Mutex<bool>>,
    data: Task<()>,
}

impl MyApp {
    pub fn update_list(&mut self) {
	let image_list_arc = Arc::clone(&self.image_list);
	let image_list_loaded_arc = Arc::clone(&self.image_list_loaded);

	self.data.fire_async(async move {
	    //tokio::task::spawn_local(async move {
	    let mut images = docker::bollard_image_list().await;

	    let mut image_list = image_list_arc.lock().unwrap();
	    image_list.clear();
	    image_list.append(&mut images);
	    *image_list_loaded_arc.lock().unwrap() = true;
	});
    }
}


impl Default for MyApp {
    fn default() -> Self {
	let mut image_list = Vec::new();
	for i in 1..4 {
	    image_list.push(Image { name: Some(format!("Test {i}")), size_bytes: i*1000, digest: String::new(), date: chrono::DateTime::<chrono::Utc>::from_timestamp_secs(0).unwrap()});
	}
        Self {
	    image_list: Arc::new(Mutex::new(image_list)),
	    image_list_loaded: Arc::new(Mutex::new(false)),
	    data: Task::new(),
        }
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
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
	    
            let available_height = ui.available_height();
	    let _table = TableBuilder::new(ui)
		.striped(true)
		.resizable(true)
		.cell_layout(egui::Layout::left_to_right(egui::Align::Center))
		.column(Column::auto())
		.column(
                    Column::remainder()
			.at_least(60.0)
			.clip(false)
			.resizable(true)
		)
		.column(Column::auto())
		.column(Column::remainder())
		.column(Column::remainder())
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
			ui.strong("Fecha de creación");
                    });
		})
		.body(|mut body| {
		    let list = self.image_list.lock().unwrap();
		    for image in list.iter() {
			let row_height = 18.0;
			body.row(row_height, |mut row| {
                            //row.set_selected(self.selection.contains(&row_index));
                            //row.set_overline(self.overline && row_index % 7 == 3);

                            row.col(|ui| {
                                ui.label(image.get_name());
                            });
                            row.col(|ui| {
                                ui.label(image.get_human_size());
                            });
                            row.col(|ui| {
                                //ui.add(egui::Separator::default().horizontal());
				ui.label(image.get_id());
                            });/*
                            row.col(|ui| {
                                ui.checkbox(&mut self.checked, "Click me");
                            });*/
                            row.col(|ui| {
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                                ui.label(image.get_date_local());
                            });

                            //self.toggle_row_selection(row_index, &row.response());
                        });
		    }
		});
        });
    }
}
