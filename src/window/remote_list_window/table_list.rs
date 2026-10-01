use super::{
    ImageTableOptions, TableSortType, delete_modal::ModalData as DeleteModalData,
    pull_modal::ModalData as PullModalData,
};
use crate::configuration::AppConfig;
use crate::container;
use crate::tasks::Task;
use crate::window::{WindowOptions, WindowSize};
use eframe::egui;
use egui::containers::scroll_area::DragScroll;
use egui::{Id, Modal};
use egui_extras::{Column, TableBuilder};

pub fn table_ui(
    ui: &mut egui::Ui,
    image_table_options: &mut ImageTableOptions,
    images: &mut Vec<container::Image>,
    config: &AppConfig,
    pull_modal_data: &mut Option<PullModalData>,
    delete_modal_data: &mut Option<DeleteModalData>,
) -> egui::scroll_area::ScrollAreaOutput<()> {
    let available_height = ui.available_height();
    let date_rev = image_table_options.ui_date_rev.clone();
    let name_rev = image_table_options.ui_name_rev.clone();
    let sort_type = image_table_options.sort_type.clone();
    let table = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .drag_to_scroll(DragScroll::Always)
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
                ui.horizontal(|ui| {
                    ui.strong("Nombre/Tag");
                    let btn = ui
                        .button(if name_rev { "⬇" } else { "⬆" })
                        .on_hover_text(if name_rev { "A-Z" } else { "Z-A" });
                    if sort_type == TableSortType::Name {
                        btn.clone().highlight();
                    }
                    if btn.clicked() {
                        image_table_options.ui_name_rev ^= true;
                        image_table_options.sort_type = TableSortType::Name;
                        sort_table(image_table_options.clone(), images);
                    }
                });
            });
            header.col(|ui| {
                ui.strong("Tamaño*").on_hover_text(
                    "Tamaño comprimido en Registro. El tamaño real puede ser 3-5 veces mayor",
                );
            });
            header.col(|ui| {
                ui.strong("ID remoto");
            });
            header.col(|ui| {
                ui.horizontal(|ui| {
                    ui.strong("Fecha de subida");
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
                        image_table_options.ui_date_rev ^= true;
                        image_table_options.sort_type = TableSortType::Date;
                        sort_table(image_table_options.clone(), images);
                    }
                });
            });
        })
        .body(|mut body| {
            for image in images.iter() {
                image_table_row(
                    image,
                    &mut body,
                    &config,
                    pull_modal_data,
                    delete_modal_data,
                );
            }
        });
    table
}

fn image_table_row(
    image: &container::Image,
    body: &mut egui_extras::TableBody,
    config: &AppConfig,
    pull_modal_data: &mut Option<PullModalData>,
    delete_modal_data: &mut Option<DeleteModalData>,
) {
    let row_height = 18.0;
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
                    *pull_modal_data = Some(PullModalData::new(image.clone(), config.clone()));
                }
                if ui
                    .button("🗑")
                    .on_hover_text(format!("Eliminar la imagen {image_name}"))
                    .clicked()
                {
                    println!("Delete modal for: {image_name}");
                    *delete_modal_data = Some(DeleteModalData::new(image_name, config.clone()));
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

fn sort_table(options: ImageTableOptions, list: &mut Vec<container::Image>) {
    println!("Sorting image table: {options:#?}");
    match options.sort_type {
        TableSortType::None => {}
        TableSortType::Date => {
            list.sort_by_key(|x| x.date); // Newest image first
            if options.ui_date_rev {
                list.reverse();
            }
        }
        TableSortType::Name => {
            list.sort_by_key(|x| x.repository_info.as_ref().unwrap().repository.clone());
            if !options.ui_name_rev {
                list.reverse();
            }
        }
    }
}
