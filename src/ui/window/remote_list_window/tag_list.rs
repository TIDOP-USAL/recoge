use super::{
    ImageTableOptions, TableSortType, WindowData, delete_modal::ModalData as DeleteModalData,
    pull_modal::ModalData as PullModalData,
};
use super::{
    ListStatus, NestedRepoListItem, RepositoriesDisplayMode, WindowOptions, WindowSize, tag_list,
};
use crate::configuration::AppConfig;
use crate::container;
use crate::tasks::Task;
use crate::zot;
use eframe::egui;
use egui::containers::scroll_area::DragScroll;
use egui::{Id, Modal};
use egui_extras::{Column, TableBuilder};
use futures::lock::Mutex as AsyncMutex;
use log::{debug, error, info, trace};
use std::sync::Arc;

pub async fn reload_repo_list(data: Arc<AsyncMutex<WindowData>>) -> Result<Vec<String>, ()> {
    let data_locked = data.lock().await;
    let toasts = Arc::clone(&data_locked.toasts_arc);
    let config = data_locked.app_config.clone();
    drop(data_locked);
    let repos = get_repo_list(&config, toasts).await?;
    let mut data_locked = data.lock().await;
    update_new_repo_list(repos.clone(), &mut data_locked);
    drop(data_locked);
    Ok(repos)
}

pub fn update_new_repo_list(remote_repository_list: Vec<String>, output: &mut WindowData) {
    // Unnested list
    output.remote_repository_list = remote_repository_list.clone();

    // Nested list
    output.remote_repository_nested_list.clear();
    let mut filled_directories = Vec::new();
    for repo in &remote_repository_list {
        // Follow A-Z order
        let parts: Vec<&str> = repo.split('/').collect();
        if repo.contains('/')
            && let Some(dir) = parts.first()
        {
            if filled_directories.contains(dir) {
                continue;
            }
            let matching_full_repos: Vec<String> = remote_repository_list
                .iter()
                .filter(|x| x.starts_with(&format!("{dir}/")))
                .map(|x| x[(dir.len() + 1..)].to_string())
                .collect();
            let dir_item =
                super::NestedRepoListItem::Directory((dir.to_string(), matching_full_repos));
            output.remote_repository_nested_list.push(dir_item);
            filled_directories.push(dir);
        } else {
            let repo_item = super::NestedRepoListItem::Repo(repo.to_string());
            output.remote_repository_nested_list.push(repo_item);
        }
    }
}

pub async fn get_repo_list(
    config: &AppConfig,
    toasts: Arc<AsyncMutex<egui_notify::Toasts>>,
) -> Result<Vec<String>, ()> {
    let Ok(mut client) = zot::Client::new(&config) else {
        let mut toasts = toasts.lock().await;
        toasts
            .error("Error de conexión. Comprueba las credenciales del Registro.")
            .duration(std::time::Duration::from_secs(3));
        return Err(());
    };
    info!("Loading remote repository list");
    let mut remote_repository_list = match client.get_repository_list().await {
        Ok(x) => x,
        Err(x) => {
            error!("Can't load remote repositories: {x:?}");
            toasts
                .lock()
                .await
                .error("Error de conexión. Comprueba las credenciales del Registro.")
                .duration(std::time::Duration::from_secs(3));
            return Err(());
        }
    };
    trace!("{remote_repository_list:#?}");
    remote_repository_list.sort();
    Ok(remote_repository_list)
}

pub async fn load_tag_list(
    data: Arc<AsyncMutex<WindowData>>,
    repository: String,
) -> Result<(), ()> {
    let data_locked = data.lock().await;
    let toasts = Arc::clone(&data_locked.toasts_arc);
    drop(data_locked);
    let data_locked = data.lock().await;
    let config = data_locked.app_config.clone();
    drop(data_locked);
    let Ok(mut client) = zot::Client::new(&config) else {
        let mut toasts = toasts.lock().await;
        toasts
            .error("Error de conexión. Comprueba las credenciales del Registro.")
            .duration(std::time::Duration::from_secs(3));
        return Err(());
    };
    info!("Loading remote tag list for {repository}");
    let mut remote_image_list = match client.get_repo_images(repository).await {
        Ok(x) => x,
        Err(x) => {
            error!("Can't load remote tags: {x:?}");
            toasts
                .lock()
                .await
                .error("Error de conexión. Comprueba las credenciales del Registro.")
                .duration(std::time::Duration::from_secs(3));
            return Err(());
        }
    };
    remote_image_list.sort_by_key(|x| x.date); // Oldest image first
    remote_image_list.reverse();
    trace!("{remote_image_list:#?}");
    let mut data_locked = data.lock().await;
    data_locked.remote_tag_list = remote_image_list;
    Ok(())
}

// UI functions

pub fn ui(
    ui: &mut egui::Ui,
    repositories_display_mode: &mut RepositoriesDisplayMode,
    app_config: &AppConfig,
    remote_repository_list_status: &mut ListStatus,
    remote_repository_list: &mut Vec<String>,
    remote_repository_nested_list: &mut Vec<NestedRepoListItem>,
    selected_repository: &mut Option<String>,
    remote_tag_list_status: &mut ListStatus,
    remote_tag_list: &mut Vec<container::Image>,
    image_table_options: &mut ImageTableOptions,
    pull_modal_data: &mut Option<PullModalData>,
    delete_modal_data: &mut Option<DeleteModalData>,
) {
    let mut search_focus = false;
    // Repository panel
    egui::Panel::left("left_panel")
        .resizable(true)
        .min_size(150.0)
        .default_size(200.0)
        .max_size(400.0)
        .show(ui, |ui| {
            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.heading("Repositorios");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::LEFT), |ui| {
                            if ui
                                .button("⟲")
                                .on_hover_text("Recargar lista de repositorios remotos")
                                .clicked()
                            {
                                *remote_repository_list_status = ListStatus::NeedsReload;
                                *remote_tag_list_status = ListStatus::NeedsReload;
                            }
                            let btn = ui
                                .button(
                                    if *repositories_display_mode == RepositoriesDisplayMode::Nested
                                    {
                                        "☰"
                                    } else {
                                        "☰"
                                    },
                                )
                                .on_hover_text(
                                    if *repositories_display_mode == RepositoriesDisplayMode::Nested
                                    {
                                        "Mostrar lista completa"
                                    } else {
                                        "Mostrar lista anidada"
                                    },
                                );
                            if *repositories_display_mode == RepositoriesDisplayMode::Unnested {
                                btn.clone().highlight();
                            }
                            if btn.clicked() {
                                //*remote_image_list_status = ListStatus::NeedsReload;
                                *repositories_display_mode = match repositories_display_mode {
                                    RepositoriesDisplayMode::Nested => {
                                        RepositoriesDisplayMode::Unnested
                                    }
                                    RepositoriesDisplayMode::Unnested => {
                                        RepositoriesDisplayMode::Nested
                                    }
                                }
                            }
                            let search_btn = ui.button("🔍").on_hover_text("Buscar por nombre");
                            if image_table_options.repo_search_field.is_some() {
                                *repositories_display_mode = RepositoriesDisplayMode::Unnested;
                                search_btn.clone().highlight();
                            }
                            if search_btn.clicked() {
                                if image_table_options.repo_search_field.is_none() {
                                    image_table_options.repo_search_field = Some(String::new());
                                    search_focus = true;
                                } else {
                                    image_table_options.repo_search_field = None;
                                }
                            };
                        });
                    });
                    ui.add_space(3.0);
                });

                fn repo_ui_selection_item(
                    ui: &mut egui::Ui,
                    selected_repository: &mut Option<String>,
                    remote_tag_list_status: &mut ListStatus,
                    display_name: String,
                    repo: String,
                ) {
                    if ui
                        .selectable_value(
                            selected_repository,
                            Some(repo.clone()), // Full repo
                            display_name,       // Can be a partial name
                        )
                        .on_hover_text(format!("Seleccionar el repositorio {repo}"))
                        .clicked()
                    {
                        debug!("New selected repo: {selected_repository:?}");
                        *remote_tag_list_status = ListStatus::NeedsReload;
                    }
                }

                // Search bar
                if image_table_options.repo_search_field.is_some() {
                    let filter = ui
                        .text_edit_singleline(
                            image_table_options.repo_search_field.as_mut().unwrap(),
                        )
                        .on_hover_text("Nombre de repositorio para filtrar");
                    if search_focus {
                        filter.request_focus();
                    }
                    if filter.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        image_table_options.repo_search_field = None;
                    }
                    filter.context_menu(|ui| {
                        crate::ui::widgets::textedit_context_menu(
                            ui,
                            image_table_options.repo_search_field.as_mut().unwrap(),
                        );
                        filter.request_focus();
                    });
                }
                // Repo list
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if *remote_repository_list_status == ListStatus::Loading {
                        ui.set_cursor_icon(egui::CursorIcon::Wait);
                    }
                    match repositories_display_mode {
                        RepositoriesDisplayMode::Nested => {
                            for repo_item in remote_repository_nested_list {
                                match repo_item {
                                    NestedRepoListItem::Repo(repo) => {
                                        repo_ui_selection_item(
                                            ui,
                                            selected_repository,
                                            remote_tag_list_status,
                                            repo.to_string(),
                                            repo.to_string(),
                                        );
                                    }
                                    NestedRepoListItem::Directory(dir_item) => {
                                        egui::CollapsingHeader::new(dir_item.0.clone())
                                            .id_salt(dir_item.clone())
                                            .show(ui, |ui| {
                                                for repo in dir_item.1.clone() {
                                                    let full_repo_name =
                                                        format!("{}/{repo}", dir_item.0);
                                                    repo_ui_selection_item(
                                                        ui,
                                                        selected_repository,
                                                        remote_tag_list_status,
                                                        repo.to_string(),
                                                        full_repo_name,
                                                    );
                                                }
                                            });
                                    }
                                }
                            }
                        }
                        RepositoriesDisplayMode::Unnested => {
                            for repo in remote_repository_list.clone() {
                                if let Some(search_filter) =
                                    image_table_options.repo_search_field.clone()
                                {
                                    if !repo.contains(&search_filter) {
                                        continue;
                                    }
                                }
                                repo_ui_selection_item(
                                    ui,
                                    selected_repository,
                                    remote_tag_list_status,
                                    repo.to_string(),
                                    repo.to_string(),
                                );
                            }
                        }
                    }
                    ui.allocate_space(ui.available_size());
                });
            });
        });

    egui::CentralPanel::default().show(ui, |ui| {
        if *remote_tag_list_status == ListStatus::Loading {
            ui.set_cursor_icon(egui::CursorIcon::Wait);
        }

        if let Some(repo) = selected_repository {
            ui.horizontal(|ui| {
                //ui.label("Repositorio:");
                ui.label("🗀");
                ui.code(repo).on_hover_text("El repositorio seleccionado");
                if *remote_tag_list_status == ListStatus::Loading {
                    ui.spinner();
                }
            });
        }
        // Tag table
        ui.group(|ui| {
            if *remote_tag_list_status == ListStatus::Loading {
                ui.disable();
            }
            tag_list::table_ui(
                ui,
                image_table_options,
                remote_tag_list,
                &app_config,
                pull_modal_data,
                delete_modal_data,
            );
        });
    });
}

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
        .auto_shrink(false)
        .drag_to_scroll(DragScroll::Always)
        .cell_layout(egui::Layout::left_to_right(egui::Align::LEFT))
        .column(
            Column::remainder()
                .clip(true)
                .resizable(true)
                .at_least(200.0)
                .at_most(500.0),
        )
        .column(Column::auto().resizable(false).at_least(65.0))
        .column(Column::auto().resizable(false).at_least(90.0))
        .column(Column::auto().resizable(false))
        .min_scrolled_height(0.0)
        .max_scroll_height(available_height)
        .header(20.0, |mut header| {
            header.col(|ui| {
                ui.horizontal(|ui| {
                    ui.strong("Tag");
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
            let image_repo_info = image.repository_info.clone().unwrap();
            let mut image_name = image.get_image_name();
            if !image_name.contains(':') {
                image_name.push_str(" (Sin nombre)");
            }
            ui.label(image_repo_info.tag)
                .on_hover_text(image_name.clone());

            ui.with_layout(egui::Layout::right_to_left(egui::Align::LEFT), |ui| {
                if ui
                    .button("⬇")
                    .on_hover_text("Descargar de Registro")
                    .clicked()
                {
                    debug!("Pull modal for {image_name}");
                    trace!("{image:#?}");
                    *pull_modal_data = Some(PullModalData::new(image.clone(), config.clone()));
                }
                if ui
                    .button("🗑")
                    .on_hover_text(format!("Eliminar la imagen {image_name}"))
                    .clicked()
                {
                    debug!("Delete modal for: {image_name}");
                    *delete_modal_data = Some(DeleteModalData::new(image_name, config.clone()));
                }
                /*if ui
                    .button("ℹ")
                    .on_hover_text("Información de la imagen")
                    .clicked()
                {
                    debug!("Inspect {image_name}");
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
    debug!("Sorting image table: {options:#?}");
    match options.sort_type {
        TableSortType::None => {}
        TableSortType::Date => {
            list.sort_by_key(|x| x.date); // Newest image first
            if options.ui_date_rev {
                list.reverse();
            }
        }
        TableSortType::Name => {
            list.sort_by_key(|x| x.repository_info.as_ref().unwrap().tag.clone());
            if !options.ui_name_rev {
                list.reverse();
            }
        }
    }
}
