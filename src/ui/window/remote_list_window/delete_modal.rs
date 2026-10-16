use crate::configuration::AppConfig;
use crate::container;
use crate::tasks::Task;
use crate::zot;
use eframe::egui;
use egui::{Id, Modal};
use egui_extras::{Column, TableBuilder};
use futures::lock::Mutex as AsyncMutex;
use std::sync::{Arc, Mutex};

#[derive(PartialEq, Clone)]
enum DeleteProgress {
    NotStarted,
    InProgress,
    Ok,
    Failed,
}

impl DeleteProgress {
    fn can_start(&self) -> bool {
        *self != Self::Ok && *self != Self::InProgress
    }
}

#[derive(Clone)]
pub struct ModalData {
    pub image_name: String,
    // TODO tag list
    pub config: AppConfig,
    delete_progress: Arc<Mutex<DeleteProgress>>,
}

impl ModalData {
    pub fn image_was_deleted(&self) -> bool {
        *self.delete_progress.lock().unwrap() == DeleteProgress::Ok
    }

    pub fn new(image: String, config: AppConfig) -> Self {
        Self {
            image_name: image.clone(),
            config: config.clone(),
            delete_progress: Arc::new(Mutex::new(DeleteProgress::NotStarted)),
        }
    }

    pub fn modal(&mut self, ui: &mut egui::Ui) -> egui::ModalResponse<()> {
        Modal::new(Id::new("Local delete modal")).show(ui.ctx(), |ui| {
            ui.set_width(400.0);
            ui.heading("Eliminar imagen remota");
            ui.add_space(15.0);

            egui::Grid::new("modal_rows_grid_1")
                .num_columns(2)
                .spacing([20.0, 10.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Imagen:");
                    ui.code(&self.image_name);
                    ui.end_row();
                    // TODO Tags:
                });
            ui.add_space(20.0);

            let Ok(repo_info) =
                container::RepositoryInfo::try_from_repo_tag(self.image_name.clone())
            else {
                ui.close_kind(egui::UiKind::Window);
                println!("Invalid image");
                panic!();
            };

            let delete_progress_mutex = self.delete_progress.lock().unwrap();
            let delete_progress = delete_progress_mutex.clone();
            drop(delete_progress_mutex);
            ui.horizontal(|ui| match delete_progress {
                DeleteProgress::NotStarted => {
                    ui.label("La imagen puede ser eliminada.");
                }
                DeleteProgress::InProgress => {
                    ui.label("Eliminando...");
                    ui.add(egui::widgets::Spinner::new().size(10.0));
                }
                DeleteProgress::Ok => {
                    ui.label("Terminado ✔");
                }
                DeleteProgress::Failed => {
                    ui.label("Fallo al eliminar");
                }
            });
            ui.add_space(10.0);

            egui::Sides::new().show(
                ui,
                |_ui| {},
                |ui| {
                    let download_btn = egui::Button::new("Eliminar");
                    if ui
                        .add_enabled(delete_progress.can_start(), download_btn)
                        .clicked()
                    {
                        let mut tasks: Task<()> = Task::new();

                        *self.delete_progress.lock().unwrap() = DeleteProgress::InProgress;

                        let progress_arc = Arc::clone(&self.delete_progress);
                        let Ok(mut zot_client) = zot::Client::new(&self.config) else {
                            ui.close_kind(egui::UiKind::Window);
                            return println!("Invalid config");
                        };

                        tasks.fire_async(async move {
                            *progress_arc.lock().unwrap() = match zot_client
                                .delete_image(repo_info.repository, repo_info.tag)
                                .await
                            {
                                Ok(mut x) => {
                                    println!("Delete OK");
                                    DeleteProgress::Ok
                                }
                                Err(x) => {
                                    println!("Delete failed: {x:?}");
                                    DeleteProgress::Failed
                                }
                            }
                        });
                    }

                    if ui.button("Cerrar").clicked() {
                        ui.close_kind(egui::UiKind::Modal);
                    }
                },
            );
        })
    }
}
