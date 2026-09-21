//! Table of Contents (TOC) side-panel UI component.
use crate::app::ReaderApp;
use eframe::egui;
use std::collections::HashSet;

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

        let mut clicked_chapter = None;
        let mut bookmark_toggle = None;
        if let Some(book) = &self.book {
            let toc = &book.toc;
            let bookmarked: HashSet<usize> = self
                .book_config
                .as_ref()
                .map(|cfg| cfg.bookmarks.iter().map(|b| b.chapter).collect())
                .unwrap_or_default();
            let current_chapter = self.current_chapter;
            let should_scroll =
                self.scroll_toc_to_current || self.last_toc_chapter != Some(current_chapter);
            self.scroll_toc_to_current = false;
            self.last_toc_chapter = Some(current_chapter);
            const ROW_HEIGHT: f32 = 24.0;
            const SCROLLBAR_GUTTER: f32 = 18.0;
            let viewport_height = ui.available_height();
            let mut scroll_area = egui::ScrollArea::vertical()
                .id_salt("toc_scroll")
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible);
            if should_scroll {
                if let Some(row) = toc
                    .iter()
                    .position(|entry| entry.chapter_index == current_chapter)
                {
                    let centered =
                        row as f32 * ROW_HEIGHT - (viewport_height - ROW_HEIGHT).max(0.0) * 0.5;
                    scroll_area = scroll_area.vertical_scroll_offset(centered.max(0.0));
                }
            }
            scroll_area.show_rows(ui, ROW_HEIGHT, toc.len(), |ui, visible_rows| {
                for entry in &toc[visible_rows] {
                    let is_current = entry.chapter_index == current_chapter;
                    let ch_bookmarked = bookmarked.contains(&entry.chapter_index);

                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), ROW_HEIGHT),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            let text = egui::RichText::new(&entry.title).size(14.0);
                            let label_width =
                                (ui.available_width() - 28.0 - SCROLLBAR_GUTTER).max(1.0);
                            let label = ui.add_sized(
                                [label_width, ROW_HEIGHT],
                                egui::SelectableLabel::new(is_current, text),
                            );
                            if label.clicked() && entry.chapter_index != current_chapter {
                                clicked_chapter = Some(entry.chapter_index);
                            }

                            let bm_icon = if ch_bookmarked { "★" } else { "☆" };
                            let bm_color = if ch_bookmarked {
                                egui::Color32::from_rgb(255, 200, 0)
                            } else {
                                egui::Color32::GRAY
                            };
                            let chapter_idx = entry.chapter_index;
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(bm_icon).size(14.0).color(bm_color),
                                    )
                                    .frame(false),
                                )
                                .on_hover_text(if ch_bookmarked {
                                    "取消书签"
                                } else {
                                    "添加书签"
                                })
                                .clicked()
                            {
                                bookmark_toggle = Some((chapter_idx, ch_bookmarked));
                            }
                            ui.add_space(SCROLLBAR_GUTTER);
                        },
                    );
                }
            });
        }
        if let Some((chapter_idx, was_bookmarked)) = bookmark_toggle {
            if let Some(cfg) = &mut self.book_config {
                if was_bookmarked {
                    cfg.bookmarks.retain(|b| b.chapter != chapter_idx);
                } else {
                    cfg.bookmarks.push(reader_core::library::Bookmark {
                        chapter: chapter_idx,
                        block: 0,
                        created_at: reader_core::now_secs(),
                    });
                }
                cfg.save(&self.data_dir);
            }
        }
        if let Some(chapter_idx) = clicked_chapter {
            self.previous_chapter = Some(self.current_chapter);
            self.current_chapter = chapter_idx;
            self.current_block = 0;
            self.pending_restore_block = None;
            self.layout_reanchor_pending = false;
            if self.scroll_mode {
                self.pending_scroll_chapter = Some(chapter_idx);
                self.continuous_scroll
                    .reset(chapter_idx, self.total_chapters());
            } else {
                self.pending_scroll_chapter = None;
                self.scroll_to_top = true;
            }
            self.pages_dirty = true;
            self.current_page = 0;
            self.request_chapter_loads(&[chapter_idx]);
            if let Some(p) = &self.book_path {
                let chap_title = self
                    .book
                    .as_ref()
                    .and_then(|b| b.chapters.get(chapter_idx))
                    .map(|c| c.title.clone());
                self.library
                    .update_chapter(&self.data_dir, p, chapter_idx, chap_title);
            }
        }
    }
}
