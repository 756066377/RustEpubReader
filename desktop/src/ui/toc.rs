use crate::app::ReaderApp;
use eframe::egui;

impl ReaderApp {
    pub fn render_toc(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.heading(
            egui::RichText::new(self.i18n.t("toc.title"))
                .size(18.0)
                .strong(),
        );
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // "Locate current chapter" button
        if ui
            .button(egui::RichText::new(self.i18n.t("toc.locate_current")).size(13.0))
            .clicked()
        {
            self.scroll_toc_to_current = true;
        }
        ui.add_space(4.0);

        if let Some(book) = &self.book {
            let toc = book.toc.clone();
            let should_scroll = self.scroll_toc_to_current;
            self.scroll_toc_to_current = false;
            egui::ScrollArea::vertical()
                .id_salt("toc_scroll")
                .show(ui, |ui| {
                    for entry in &toc {
                        let is_current = entry.chapter_index == self.current_chapter;
                        let text = egui::RichText::new(&entry.title).size(14.0);
                        let label = ui.selectable_label(is_current, text);
                        if is_current && should_scroll {
                            label.scroll_to_me(Some(egui::Align::Center));
                        }
                        if label.clicked() && entry.chapter_index != self.current_chapter {
                            self.previous_chapter = Some(self.current_chapter);
                            self.current_chapter = entry.chapter_index;
                            self.scroll_to_top = true;
                            self.pages_dirty = true;
                            self.current_page = 0;
                            if let Some(p) = &self.book_path {
                                let chap_title = self
                                    .book
                                    .as_ref()
                                    .and_then(|b| b.chapters.get(self.current_chapter))
                                    .map(|c| c.title.clone());
                                self.library.update_chapter(
                                    &self.data_dir,
                                    p,
                                    self.current_chapter,
                                    chap_title,
                                );
                            }
                        }
                    }
                });
        }
    }
}
