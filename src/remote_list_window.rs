//use crate::tasks::Task;
use crate::container;
use eframe::egui;
use egui::Vec2b;
use egui_extras::{Column, TableBuilder};
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
    pub remote_image_list: Vec<container::Image>,
    pub toasts_arc: Arc<Mutex<Toasts>>,
    date_rev: bool,
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
            remote_image_list: Vec::new(),
            toasts_arc: Arc::new(Mutex::new(Toasts::default())),
            date_rev: false,
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
        remote_image_list,
        toasts_arc: _,
        date_rev,
    } = options;

    ui.label("Imágenes remotas:");
    ui.add_space(10.0);
    table_ui(ui, date_rev, remote_image_list.to_vec());

    //let on_top = Some(ui.layer_id()) == ui.ctx().top_layer_id();
    //ui.label(format!("This window is on top: {on_top}."));
}

fn table_ui(
    ui: &mut egui::Ui,
    date_rev: &mut bool,
    images: Vec<container::Image>,
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
                .resizable(true),
        )
        .column(Column::auto())
        .column(Column::auto())
        .column(Column::auto())
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
                    image_table_row(image, &mut body);
                }
            } else {
                for image in images.iter() {
                    image_table_row(image, &mut body);
                }
            }
        });
    table
}

fn image_table_row(image: &container::Image, body: &mut egui_extras::TableBody) {
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
                    //*boolean = !*boolean;
                    println!("Pull {image_name}");
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

        //self.toggle_row_selection(row_index, &row.response());
    });
}
