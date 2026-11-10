use crate::configuration::AppConfig;
use crate::container::{Image, RepositoryInfo};
use eframe::egui;
use egui::containers::scroll_area::DragScroll;
use egui_extras::Column;
use egui_extras::TableBuilder;
use futures::executor::block_on;
use log::{debug, error};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ImageTableOptions {
    pub search_field: Option<String>, // None: disabled
    pub sort_type: TableSortType,
    pub ui_date_rev: bool, // 0: ASC
    pub ui_name_rev: bool, // 0: A-Z
}

#[derive(Debug, PartialEq, Clone)]
pub enum TableSortType {
    None,
    Date,
    Name,
}

pub fn image_list_table(mut ui: &mut egui::Ui, app: &mut super::MyApp) {
    let global_data_arc = Arc::clone(&app.global_data);
    let global_data = block_on(global_data_arc.lock().into_future());
    let config = global_data.config.clone();
    drop(global_data);
    let available_height = ui.available_height();
    let date_rev = app.image_table_options.ui_date_rev.clone();
    let name_rev = app.image_table_options.ui_name_rev.clone();
    let sort_type = app.image_table_options.sort_type.clone();
    let _table = TableBuilder::new(&mut ui)
        .striped(true)
        .resizable(true)
        .auto_shrink(false)
        .drag_to_scroll(DragScroll::Always)
        .cell_layout(egui::Layout::left_to_right(egui::Align::LEFT))
        .column(
            Column::remainder()
                //.at_least(80.0)
                .clip(true)
                .resizable(false),
        )
        .column(Column::auto().resizable(false).at_least(65.0))
        .column(Column::auto().resizable(false).at_least(89.0))
        .column(Column::auto().resizable(false))
        .min_scrolled_height(0.0)
        .max_scroll_height(available_height)
        .header(21.0, |mut header| {
            header.col(|ui| {
                ui.horizontal(|ui| {
                    ui.strong("Nombre local").on_hover_text(
                        "Nombre de la imagen de contenedor, en formato repositorio:tag",
                    );
                    let btn = ui
                        .button(if name_rev { "⬇" } else { "⬆" })
                        .on_hover_text(if name_rev { "A-Z" } else { "Z-A" });
                    if sort_type == TableSortType::Name {
                        btn.clone().highlight();
                    }
                    if btn.clicked() {
                        app.image_table_options.ui_name_rev ^= true;
                        app.image_table_options.sort_type = TableSortType::Name;
                        sort_table(app);
                    }
                });
            });
            header.col(|ui| {
                ui.strong("Tamaño");
            });
            header.col(|ui| {
                ui.strong("ID local");
            });
            header.col(|ui| {
                ui.horizontal(|ui| {
                    ui.strong("Fecha de creación");
                    let btn =
                        ui.button(if date_rev { "⬇" } else { "⬆" })
                            .on_hover_text(if date_rev {
                                "Descendente"
                            } else {
                                "Ascendente"
                            });
                    if sort_type == TableSortType::Date {
                        btn.clone().highlight();
                    }
                    if btn.clicked() {
                        app.image_table_options.ui_date_rev ^= true;
                        app.image_table_options.sort_type = TableSortType::Date;
                        sort_table(app);
                    }
                });
            });
        })
        .body(|mut body| {
            let image_list: Vec<Image> = app.image_list.lock().unwrap().to_vec();
            for image in image_list.iter() {
                // TODO global filtered list
                if let Some(search_filter) = app.image_table_options.search_field.clone() {
                    if !image.get_image_name().contains(&search_filter) {
                        continue;
                    }
                }
                image_table_row(app, &config, image, &mut body);
            }
        });
}

fn sort_table(app: &mut super::MyApp) {
    let options = app.image_table_options.clone();
    debug!("Sorting image table: {options:#?}");
    match options.sort_type {
        TableSortType::None => {}
        TableSortType::Date => {
            let mut image_list = app.image_list.lock().unwrap();
            image_list.sort_by_key(|x| x.date); // Oldest image first
            if options.ui_date_rev {
                image_list.reverse();
            }
        }
        TableSortType::Name => {
            let mut image_list = app.image_list.lock().unwrap();
            image_list.sort_by_key(|x| x.repository_info.as_ref().unwrap().repository.clone());
            if !options.ui_name_rev {
                image_list.reverse();
            }
        }
    }
}

fn image_table_row(
    app: &mut super::MyApp,
    config: &AppConfig,
    image: &Image,
    body: &mut egui_extras::TableBody,
    //upload_window_state: &mut upload_window::WindowState,
) {
    let row_height = 18.0;
    body.row(row_height, |mut row| {
        row.col(|ui| {
            let mut image_name = image.get_image_name();
            if !image_name.contains(':') {
                image_name.push_str(" (Sin nombre)");
            }
            let label = egui::Label::new(image_name.to_string());
            let rect = ui.add(label).on_hover_text(image_name.clone()).rect;
            if let Some(hid) = &app.image_list_hovered_id
                && *hid == image.id
            {
                let color = egui::Color32::from_rgb(165, 238, 0);
                ui.debug_painter().rect(
                    rect,
                    0.0,
                    egui::Color32::TRANSPARENT,
                    (1.0, color),
                    egui::StrokeKind::Outside,
                );
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::LEFT), |ui| {
                if ui.button("⬆").on_hover_text("Subir a Registro").clicked() {
                    //*boolean = !*boolean;
                    debug!("Push {image_name}");
                    let Ok(selected_repo_info) =
                        RepositoryInfo::try_from_repo_tag(image_name.clone())
                    else {
                        error!("Invalid repo_tag: {image_name}");
                        return;
                    };
                    *Arc::clone(&app.upload_window_open_flag).lock().unwrap() =
                        Some(selected_repo_info);
                    //upload_window_state.open(config.clone(), selected_repo_info);
                }
                if ui
                    .button("ℹ")
                    .on_hover_text("Información de la imagen")
                    .clicked()
                {
                    let image_info_window_is_loading =
                        *app.image_info_window_is_loading.load().clone();
                    if !image_info_window_is_loading {
                        debug!("Inspect {image_name}");

                        app.image_info_window_is_loading
                            .store(Arc::new(true.to_owned()));
                        app.inspect_image(image.clone(), config.clone(), ui.ctx().clone());
                    }
                }
            });
        });
        row.col(|ui| {
            ui.label(image.get_human_size());
        });
        row.col(|ui| {
            //ui.add(egui::Separator::default().horizontal());
            if ui
                .label(image.get_short_id())
                .on_hover_text(image.id.clone())
                .hovered()
            {
                app.image_list_hovered_id = Some(image.id.clone());
            }
        });
        row.col(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            ui.label(format!(
                "({}) {}",
                image.get_relative_date(),
                image.get_date_local()
            ));
        });
        //self.toggle_row_selection(row_index, &row.response());
    });
}
