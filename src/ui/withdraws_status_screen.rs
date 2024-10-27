use crate::app::{AppAction, DesiredAppAction};
use crate::context::AppContext;
use crate::platform::withdrawals::{
    WithdrawRecord, WithdrawStatusData, WithdrawStatusPartialData, WithdrawalsTask,
};
use crate::platform::{BackendTask, BackendTaskSuccessResult};
use crate::ui::components::left_panel::add_left_panel;
use crate::ui::components::top_panel::add_top_panel;
use crate::ui::{MessageType, RootScreenType, ScreenLike};
use dash_sdk::dpp::dash_to_credits;
use dash_sdk::dpp::data_contracts::withdrawals_contract::WithdrawalStatus;
use dash_sdk::dpp::document::DocumentV0Getters;
use dash_sdk::dpp::platform_value::Value;
use egui::{ComboBox, Context, Ui};
use egui_extras::{Column, TableBuilder};
use itertools::Itertools;
use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex, RwLock};

pub struct WithdrawsStatusScreen {
    pub app_context: Arc<AppContext>,
    data: Arc<RwLock<Option<WithdrawStatusData>>>,
    sort_column: Cell<Option<SortColumn>>,
    sort_ascending: Cell<bool>,
    filter_status_queued: Cell<bool>,
    filter_status_pooled: Cell<bool>,
    filter_status_broadcasted: Cell<bool>,
    filter_status_complete: Cell<bool>,
    filter_status_expired: Cell<bool>,
    filter_status_mix: Vec<WithdrawalStatus>,
    pagination_current_page: usize,
    pagination_items_per_page: PaginationItemsPerPage,
    error_message: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum SortColumn {
    DateTime,
    Status,
    Amount,
    OwnerId,
    Destination,
}

#[derive(Clone, Copy, PartialEq)]
enum PaginationItemsPerPage {
    Items10 = 10,
    Items15 = 15,
    Items20 = 20,
    Items30 = 30,
    Items50 = 50,
}

impl From<PaginationItemsPerPage> for u32 {
    fn from(item: PaginationItemsPerPage) -> Self {
        item as u32
    }
}

impl WithdrawsStatusScreen {
    pub fn new(app_context: &Arc<AppContext>) -> Self {
        Self {
            app_context: app_context.clone(),
            data: Arc::new(RwLock::new(None)),
            sort_ascending: Cell::from(false),
            sort_column: Cell::from(Some(SortColumn::DateTime)),
            error_message: None,
            filter_status_queued: Cell::new(true),
            filter_status_pooled: Cell::new(true),
            filter_status_broadcasted: Cell::new(true),
            filter_status_complete: Cell::new(true),
            filter_status_expired: Cell::new(false),
            filter_status_mix: vec![
                WithdrawalStatus::QUEUED,
                WithdrawalStatus::POOLED,
                WithdrawalStatus::BROADCASTED,
                WithdrawalStatus::COMPLETE,
                WithdrawalStatus::EXPIRED,
            ],
            pagination_current_page: 0,
            pagination_items_per_page: PaginationItemsPerPage::Items15,
        }
    }

    fn show_input_field(&mut self, ui: &mut Ui) {}

    fn show_output(&mut self, ui: &mut egui::Ui) {
        if self.error_message.is_some() {
            ui.centered_and_justified(|ui| {
                ui.heading(self.error_message.as_ref().unwrap());
            });
        } else {
            let lock_data = self.data.read().unwrap().clone();

            if let Some(mut data) = lock_data {
                let sorted_data = self.sort_withdraws_data(data.withdrawals.as_slice());
                data.withdrawals = sorted_data;
                self.show_withdraws_data(ui, &data);
            }
        }
    }

    fn sort_withdraws_data(&self, data: &[WithdrawRecord]) -> Vec<WithdrawRecord> {
        let mut result_data = data.to_vec();
        if let Some(column) = self.sort_column.get() {
            let compare = |a: &WithdrawRecord, b: &WithdrawRecord| -> std::cmp::Ordering {
                let ord = match column {
                    SortColumn::DateTime => a.date_time.cmp(&b.date_time),
                    SortColumn::Status => (a.status as u8).cmp(&(b.status as u8)),
                    SortColumn::Amount => a.amount.cmp(&b.amount),
                    SortColumn::OwnerId => a.owner_id.cmp(&b.owner_id),
                    SortColumn::Destination => a.address.cmp(&b.address),
                };
                if self.sort_ascending.get() {
                    ord
                } else {
                    ord.reverse()
                }
            };
            result_data.sort_by(compare);
        }
        result_data
    }

    fn handle_column_click(&self, current_sort: SortColumn) {
        if self.sort_column.get() == Some(current_sort) {
            self.sort_ascending.set(!self.sort_ascending.get());
        } else {
            self.sort_column.set(Some(current_sort));
            self.sort_ascending.set(true);
        }
    }

    fn show_withdraws_data(&mut self, ui: &mut egui::Ui, data: &WithdrawStatusData) {
        egui::Grid::new("general_info_grid")
            .num_columns(2)
            .spacing([20.0, 8.0]) // Adjust spacing as needed
            .show(ui, |ui| {
                ui.heading("General Information");
                ui.separator();
                ui.end_row();
                ui.label("Total withdrawals amount:");
                ui.label(format!(
                    "{:.2} DASH",
                    data.total_amount as f64 / (dash_to_credits!(1) as f64)
                ));
                ui.end_row();

                ui.label("Recent withdrawals amount:");
                ui.label(format!(
                    "{:.2} DASH",
                    data.recent_withdrawal_amounts as f64 / (dash_to_credits!(1) as f64)
                ));
                ui.end_row();

                ui.label("Daily withdrawals limit:");
                ui.label(format!(
                    "{:.2} DASH",
                    data.daily_withdrawal_limit as f64 / (dash_to_credits!(1) as f64)
                ));
                ui.end_row();

                ui.label("Total credits on Platform:");
                ui.label(format!(
                    "{:.2} DASH",
                    data.total_credits_on_platform as f64 / (dash_to_credits!(1) as f64)
                ));
                ui.end_row();
            });

        ui.add_space(30.0); // Optional spacing between the grids

        egui::Grid::new("filters_grid").show(ui, |ui| {
            ui.heading("Filters");
            ui.end_row();
            ui.horizontal(|ui| {
                ui.label("Filter by status:");
                ui.add_space(8.0); // Space after label
                let mut value = self.filter_status_queued.get();
                if ui.checkbox(&mut value, "Queued").changed() {
                    self.filter_status_queued.set(value);
                    self.util_build_combined_filter_status_mix();
                }
                ui.add_space(8.0);
                let mut value = self.filter_status_pooled.get();
                if ui.checkbox(&mut value, "Pooled").changed() {
                    self.filter_status_pooled.set(value);
                    self.util_build_combined_filter_status_mix();
                }
                ui.add_space(8.0);
                let mut value = self.filter_status_broadcasted.get();
                if ui.checkbox(&mut value, "Broadcasted").changed() {
                    self.filter_status_broadcasted.set(value);
                    self.util_build_combined_filter_status_mix();
                }
                ui.add_space(8.0);
                let mut value = self.filter_status_complete.get();
                if ui.checkbox(&mut value, "Complete").changed() {
                    self.filter_status_complete.set(value);
                    self.util_build_combined_filter_status_mix();
                }
                ui.add_space(8.0);
                let mut value = self.filter_status_expired.get();
                if ui.checkbox(&mut value, "Expired").changed() {
                    self.filter_status_expired.set(value);
                    self.util_build_combined_filter_status_mix();
                }
            });
        });
        ui.add_space(30.0);
        ui.heading(format!("Withdrawals ({})", data.withdrawals.len()));
        let mut selected = self.pagination_items_per_page;
        let old_selected = selected;
        ComboBox::from_label("Items per page")
            .selected_text(format!("{}", selected as usize))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut selected, PaginationItemsPerPage::Items10, "10");
                ui.selectable_value(&mut selected, PaginationItemsPerPage::Items15, "15");
                ui.selectable_value(&mut selected, PaginationItemsPerPage::Items20, "20");
                ui.selectable_value(&mut selected, PaginationItemsPerPage::Items30, "30");
                ui.selectable_value(&mut selected, PaginationItemsPerPage::Items50, "50");
            });
        if selected != old_selected {
            self.pagination_items_per_page = selected;
        }
        println!("computing with:{}", self.pagination_items_per_page as usize);
        let total_pages = (data.withdrawals.len() + (self.pagination_items_per_page as usize) - 1)
            / (self.pagination_items_per_page as usize);
        let mut current_page = self
            .pagination_current_page
            .min(total_pages.saturating_sub(1)); // Clamp to valid page range
                                                 // Calculate the slice of data for the current page
        let start_index = current_page * (self.pagination_items_per_page as usize);
        let end_index =
            (start_index + (self.pagination_items_per_page as usize)).min(data.withdrawals.len());
        ui.separator();
        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .column(Column::initial(150.0).resizable(true)) // Date / Time
            .column(Column::initial(80.0).resizable(true)) // Status
            .column(Column::initial(140.0).resizable(true)) // Amount
            .column(Column::initial(350.0).resizable(true)) // OwnerID
            .column(Column::initial(320.0).resizable(true)) // Destination
            .header(20.0, |mut header| {
                header.col(|ui| {
                    if ui.selectable_label(false, "Date / Time").clicked() {
                        self.handle_column_click(SortColumn::DateTime);
                    }
                });
                header.col(|ui| {
                    if ui.selectable_label(false, "Status").clicked() {
                        self.handle_column_click(SortColumn::Status);
                    }
                });
                header.col(|ui| {
                    if ui.selectable_label(false, "Amount").clicked() {
                        self.handle_column_click(SortColumn::Amount);
                    }
                });
                header.col(|ui| {
                    if ui.selectable_label(false, "Owner ID").clicked() {
                        self.handle_column_click(SortColumn::OwnerId);
                    }
                });
                header.col(|ui| {
                    if ui.selectable_label(false, "Destination").clicked() {
                        self.handle_column_click(SortColumn::Destination);
                    }
                });
            })
            .body(|mut body| {
                for record in &data.withdrawals[start_index..end_index] {
                    body.row(18.0, |mut row| {
                        row.col(|ui| {
                            ui.label(&record.date_time.format("%Y-%m-%d %H:%M:%S").to_string());
                        });
                        row.col(|ui| {
                            ui.label(format!("{}", &record.status));
                        });
                        row.col(|ui| {
                            ui.label(format!(
                                "{:.2} DASH",
                                record.amount as f64 / (dash_to_credits!(1) as f64)
                            ));
                        });
                        row.col(|ui| {
                            ui.label(format!("{}", &record.owner_id));
                        });
                        row.col(|ui| {
                            ui.label(format!("{}", &record.address));
                        });
                    });
                }
            });
        // Pagination controls at the bottom
        ui.horizontal(|ui| {
            if ui.button("Previous").clicked() && current_page > 0 {
                self.pagination_current_page = current_page - 1
            }

            ui.label(format!("Page {}/{}", current_page + 1, total_pages));

            if ui.button("Next").clicked() && current_page < total_pages - 1 {
                self.pagination_current_page = current_page + 1
            }
        });
    }

    fn util_build_combined_filter_status_mix(&mut self) {
        let mut res = vec![];
        if self.filter_status_queued.get() {
            res.push(WithdrawalStatus::QUEUED);
        }
        if self.filter_status_pooled.get() {
            res.push(WithdrawalStatus::POOLED);
        }
        if self.filter_status_broadcasted.get() {
            res.push(WithdrawalStatus::BROADCASTED);
        }
        if self.filter_status_complete.get() {
            res.push(WithdrawalStatus::COMPLETE);
        }
        if self.filter_status_expired.get() {
            res.push(WithdrawalStatus::EXPIRED);
        }
        self.filter_status_mix = res;
    }
}

impl ScreenLike for WithdrawsStatusScreen {
    fn refresh(&mut self) {
        let mut lock_data = self.data.write().unwrap();
        *lock_data = None;
        self.error_message = None;
    }

    fn display_message(&mut self, message: &str, message_type: MessageType) {
        self.error_message = Some(message.to_string());
    }
    fn display_task_result(&mut self, backend_task_success_result: BackendTaskSuccessResult) {
        if let BackendTaskSuccessResult::WithdrawalStatus(data) = backend_task_success_result {
            let mut lock_data = self.data.write().unwrap();
            if let Some(old_data) = lock_data.as_mut() {
                old_data.merge_with_data(data)
            } else {
                *lock_data = Some(data.try_into().expect("expected data to already exist"));
            }
            self.error_message = None;
        }
    }

    fn ui(&mut self, ctx: &Context) -> AppAction {
        let query = (
            "Refresh",
            DesiredAppAction::BackendTask(BackendTask::WithdrawalTask(
                WithdrawalsTask::QueryWithdrawals(
                    self.filter_status_mix.clone(),
                    self.pagination_items_per_page.into(),
                    None,
                    true,
                    true,
                ),
            )),
        );
        let mut action = add_top_panel(
            ctx,
            &self.app_context,
            vec![("Dash Evo Tool", AppAction::None)],
            vec![query],
        );

        action |= add_left_panel(
            ctx,
            &self.app_context,
            RootScreenType::RootScreenWithdrawsStatus,
        );

        egui::CentralPanel::default().show(ctx, |ui| {
            self.show_input_field(ui);
            self.show_output(ui);
        });

        action
    }
}
