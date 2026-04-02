use crate::app::{AppView, ReaderApp};
use eframe::egui;

impl ReaderApp {
    pub fn render_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let in_library = self.view == AppView::Library;
            if ui
                .selectable_label(
                    in_library,
                    egui::RichText::new(self.i18n.t("toolbar.library")).size(14.0),
                )
                .clicked()
            {
                self.view = AppView::Library;
            }
            ui.add_space(4.0);
            if ui
                .button(egui::RichText::new(self.i18n.t("toolbar.open")).size(14.0))
                .clicked()
            {
                self.open_file_dialog();
            }
            ui.add_space(4.0);
            if ui
                .selectable_label(
                    self.show_sharing_panel,
                    egui::RichText::new(self.i18n.t("share.toolbar")).size(14.0),
                )
                .clicked()
            {
                self.show_sharing_panel = !self.show_sharing_panel;
            }
            if self.book.is_some() {
                ui.add_space(4.0);
                let sidebar_icon = if self.show_toc {
                    self.i18n.t("toolbar.hide_toc")
                } else {
                    self.i18n.t("toolbar.show_toc")
                };
                if ui.button(sidebar_icon).clicked() {
                    self.show_toc = !self.show_toc;
                    if self.show_toc {
                        self.scroll_toc_to_current = true;
                    }
                }
                ui.add_space(4.0);
                let mode_label = if self.scroll_mode {
                    self.i18n.t("toolbar.scroll_mode")
                } else {
                    self.i18n.t("toolbar.page_mode")
                };
                if ui.button(mode_label).clicked() {
                    self.scroll_mode = !self.scroll_mode;
                    self.pages_dirty = true;
                }
                ui.add_space(4.0);
                if ui.button(self.i18n.t("toolbar.reading_settings")).clicked() {
                    self.show_reader_settings = true;
                }
                ui.add_space(12.0);
                ui.separator();
                ui.add_space(12.0);
                if let Some(book) = &self.book {
                    ui.label(egui::RichText::new(&book.title).strong().size(15.0).color(
                        if self.dark_mode {
                            egui::Color32::from_gray(200)
                        } else {
                            egui::Color32::from_gray(60)
                        },
                    ));
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(self.i18n.t("toolbar.shortcut_hint"))
                            .size(12.0)
                            .color(if self.dark_mode {
                                egui::Color32::from_gray(140)
                            } else {
                                egui::Color32::from_gray(115)
                            }),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let theme_icon = if self.dark_mode {
                        self.i18n.t("toolbar.light_mode")
                    } else {
                        self.i18n.t("toolbar.dark_mode")
                    };
                    if ui.button(theme_icon).clicked() {
                        self.dark_mode = !self.dark_mode;
                    }
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(12.0);
                    if ui.button("A+").clicked() {
                        self.font_size = (self.font_size + 2.0).min(40.0);
                        self.pages_dirty = true;
                    }
                    ui.label(format!("{:.0}", self.font_size));
                    if ui.button("A-").clicked() {
                        self.font_size = (self.font_size - 2.0).max(12.0);
                        self.pages_dirty = true;
                    }
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(12.0);
                    if ui.button("➡").clicked() {
                        self.next_chapter();
                    }
                    ui.label(format!(
                        " {} / {} ",
                        self.current_chapter + 1,
                        self.total_chapters()
                    ));
                    if ui.button("⬅").clicked() {
                        self.prev_chapter();
                    }
                });
            }
        });
    }
}
