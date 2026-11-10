#![allow(unused)]
pub mod config_window;
pub mod image_info_window;
pub mod remote_list_window;
pub mod upload_window;

use crate::configuration::AppConfig;
use crate::tasks::Task;
use eframe::egui;
use egui::Vec2b;
use egui::{Align, Layout, UiBuilder};
//use egui_notify::Toasts;

#[derive(Clone)]
pub struct WindowOptions {
    pub id: egui::Id, // Needs to be unique
    pub title: String,
    pub title_bar: bool,
    pub closable: bool,
    pub collapsible: bool,
    pub movable: bool,
    pub resizable: bool,
    pub constrain: bool,
    pub scroll2: Vec2b,
    pub anchored: bool,
    pub anchor: egui::Align2,
    pub anchor_offset: egui::Vec2,
    pub default_position: WindowPosition,
    pub default_size: WindowSize,
    pub fixed_size: WindowSize,
    pub min_size: WindowSize,
    pub max_size: WindowSize,
    pub default_width: Option<f32>,
    pub data_is_reloading: bool, // For reloads
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            id: egui::Id::new(String::new()),
            title: String::new(),
            title_bar: true,
            closable: false,
            collapsible: false,
            movable: true,
            resizable: false,
            constrain: true,
            scroll2: Vec2b::FALSE,
            anchored: false,
            anchor: egui::Align2::RIGHT_TOP,
            anchor_offset: egui::Vec2::ZERO,
            default_position: WindowPosition::Default,
            default_size: WindowSize::Default,
            fixed_size: WindowSize::Default,
            min_size: WindowSize::Default,
            max_size: WindowSize::Default,
            default_width: None,
            data_is_reloading: false,
        }
    }
}

#[derive(Clone, Default)]
pub enum WindowSize {
    #[default]
    Default,
    Half,
    Fixed(egui::Vec2),
}

#[derive(Clone, Default)]
pub enum WindowPosition {
    #[default]
    Default,
    Center,
    Fixed(egui::Pos2),
}

impl WindowOptions {
    // Run in the ui loop
    pub fn show<T>(
        &mut self,
        data: &mut T,
        ui: &mut egui::Ui,
        open: &mut bool,
        ui_fn: impl FnOnce(&mut egui::Ui, &mut T),
    ) {
        let Self {
            id,
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
            default_position,
            default_size,
            fixed_size,
            min_size,
            max_size,
            default_width,
            data_is_reloading,
        } = self.clone();

        let enabled = !data_is_reloading;
        if !enabled {
            ui.request_repaint();
        }

        let mut window = egui::Window::new(title)
            .id(id) // required since we change the title
            .resizable(resizable)
            .constrain(constrain)
            .collapsible(collapsible)
            .movable(movable)
            .title_bar(title_bar)
            .scroll(scroll2)
            .constrain_to(ui.available_rect_before_wrap())
            .enabled(enabled);
        window = window.open(open);

        if anchored {
            window = window.anchor(anchor, anchor_offset);
        }
        let ui_rect_size = ui.content_rect().size();
        let default_position = match default_position {
            WindowPosition::Default => egui::pos2(60.0, 50.0),
            WindowPosition::Center => egui::pos2(ui_rect_size.x / 2.0, ui_rect_size.y / 2.0),
            WindowPosition::Fixed(pos2) => pos2,
        };
        let default_size = match default_size {
            WindowSize::Default => None,
            WindowSize::Half => Some(egui::vec2(ui_rect_size.x / 2.0, ui_rect_size.y / 2.0)),
            WindowSize::Fixed(vec2) => Some(vec2),
        };
        let fixed_size = match fixed_size {
            WindowSize::Default => None,
            WindowSize::Half => Some(egui::vec2(ui_rect_size.x / 2.0, ui_rect_size.y / 2.0)),
            WindowSize::Fixed(vec2) => Some(vec2),
        };
        let min_size = match min_size {
            WindowSize::Default => None,
            WindowSize::Half => Some(egui::vec2(ui_rect_size.x / 2.0, ui_rect_size.y / 2.0)),
            WindowSize::Fixed(vec2) => Some(vec2),
        };
        let max_size = match max_size {
            WindowSize::Default => None,
            WindowSize::Half => Some(egui::vec2(ui_rect_size.x / 2.0, ui_rect_size.y / 2.0)),
            WindowSize::Fixed(vec2) => Some(vec2),
        };

        window = window.default_pos(default_position);
        if let Some(size) = default_size {
            window = window.default_size(size);
        }
        if let Some(size) = fixed_size {
            window = window.fixed_size(size);
        }
        if let Some(size) = min_size {
            window = window.min_size(size);
        }
        if let Some(size) = max_size {
            window = window.max_size(size);
        }
        if let Some(width) = default_width {
            window = window.max_width(width); // Hack
            window = window.default_width(0.0);
        }

        window.show(ui, |ui| ui_fn(ui, data));
    }
}
