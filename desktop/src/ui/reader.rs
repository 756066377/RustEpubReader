use std::sync::Arc;

use eframe::egui;
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontFamily, FontId, UiBuilder};

use crate::app::ReaderApp;
use reader_core::epub::{ContentBlock, InlineStyle, TextSpan};

// Layout constants
const DUAL_COLUMN_THRESHOLD: f32 = 1050.0;
const MAX_TEXT_WIDTH_SINGLE: f32 = 850.0;
const DUAL_COLUMN_GAP: f32 = 30.0;
const DUAL_COLUMN_PADDING: f32 = 64.0;
const MAX_COLUMN_WIDTH: f32 = 600.0;
const MIN_COLUMN_MARGIN: f32 = 28.0;
const SINGLE_MIN_MARGIN: f32 = 40.0;
const SINGLE_TEXT_PADDING: f32 = 80.0;
const TITLE_SPACING: f32 = 40.0;
const FRAME_MARGIN: f32 = 104.0;

impl ReaderApp {
    pub fn recalculate_pages(&mut self, available_height: f32, max_width: f32) {
        self.page_block_ranges.clear();
        if let Some(book) = &self.book {
            if let Some(chapter) = book.chapters.get(self.current_chapter) {
                let blocks = &chapter.blocks;
                let line_height = self.font_size * 1.8;
                let mut page_start = 0;
                let mut current_h: f32 = 0.0;
                let first_is_heading = matches!(blocks.first(), Some(ContentBlock::Heading { .. }));
                let title_height = if first_is_heading {
                    TITLE_SPACING
                } else {
                    self.font_size * 2.0 + TITLE_SPACING
                };
                let usable = (available_height - FRAME_MARGIN).max(100.0);
                let mut first_page = true;
                for (i, block) in blocks.iter().enumerate() {
                    let bh = estimate_block_height(block, self.font_size, line_height, max_width);
                    let page_budget = if first_page {
                        usable - title_height
                    } else {
                        usable
                    };
                    if current_h + bh > page_budget && i > page_start {
                        self.page_block_ranges.push((page_start, i));
                        page_start = i;
                        current_h = 0.0;
                        first_page = false;
                    }
                    current_h += bh;
                }
                if page_start < blocks.len() {
                    self.page_block_ranges.push((page_start, blocks.len()));
                }
            }
        }
        self.total_pages = self.page_block_ranges.len().max(1);
        if self.current_page >= self.total_pages {
            self.current_page = self.total_pages.saturating_sub(1);
        }
        self.pages_dirty = false;
    }

    pub fn render_reader(&mut self, ui: &mut egui::Ui) {
        if self.page_anim_progress < 1.0 {
            self.page_anim_progress =
                (self.page_anim_progress + self.reader_page_animation_speed).min(1.0);
            // Request repaint after a short delay to cap animation frame rate (~60fps)
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(16));
        }
        if self.page_anim_progress >= 1.0 && self.page_anim_cross_chapter {
            self.page_anim_cross_chapter = false;
            self.page_anim_cross_chapter_snapshot = None;
        }

        let effective_font_family = if self.defer_custom_font_for_frame
            && !matches!(
                self.reader_font_family.as_str(),
                "Sans" | "Serif" | "Monospace"
            ) {
            "Sans".to_string()
        } else {
            self.reader_font_family.clone()
        };

        let full_rect = ui.available_rect_before_wrap();
        if let Some(tex) = &self.reader_bg_texture {
            ui.painter().image(
                tex.id(),
                full_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::from_white_alpha((self.reader_bg_image_alpha * 255.0) as u8),
            );
        }

        let mut action_prev_chapter = false;
        let mut action_next_chapter = false;
        let mut action_go_back = false;
        let mut action_prev_page = false;
        let mut action_next_page = false;
        let mut clicked_link: Option<String> = None;
        let has_previous_chapter = self.previous_chapter.is_some();
        let mut is_dual_column = false;

        if let Some(book) = &self.book {
            if let Some(chapter) = book.chapters.get(self.current_chapter) {
                let available_width = ui.available_width();
                let available_height = ui.available_height();
                if (self.last_avail_width - available_width).abs() > 1.0
                    || (self.last_avail_height - available_height).abs() > 1.0
                {
                    self.pages_dirty = true;
                    self.last_avail_width = available_width;
                    self.last_avail_height = available_height;
                }
                let dual_column = !self.scroll_mode && available_width > DUAL_COLUMN_THRESHOLD;
                is_dual_column = dual_column;
                self.is_dual_column = dual_column;
                let (text_width, h_margin) = if dual_column {
                    let col_w = (available_width - DUAL_COLUMN_GAP) / 2.0;
                    let tw = (col_w - DUAL_COLUMN_PADDING).min(MAX_COLUMN_WIDTH);
                    let hm = ((col_w - tw) / 2.0).max(MIN_COLUMN_MARGIN);
                    (tw, hm)
                } else {
                    let hm = if available_width > MAX_TEXT_WIDTH_SINGLE {
                        (available_width - MAX_TEXT_WIDTH_SINGLE) / 2.0
                    } else {
                        SINGLE_MIN_MARGIN
                    };
                    let tw = MAX_TEXT_WIDTH_SINGLE.min(available_width - SINGLE_TEXT_PADDING);
                    (tw, hm)
                };
                let title = chapter.title.clone();
                let blocks = chapter.blocks.clone();
                let total_ch = book.chapters.len();
                if !self.scroll_mode && self.pages_dirty {
                    self.recalculate_pages(ui.available_height(), text_width);
                }
                if !self.scroll_mode
                    && self.total_pages > 0
                    && self.current_page >= self.total_pages
                {
                    self.current_page = self.total_pages - 1;
                }
                if dual_column && !self.current_page.is_multiple_of(2) {
                    self.current_page = self.current_page.saturating_sub(1);
                }
                let (block_start, block_end) = if self.scroll_mode {
                    (0, blocks.len())
                } else if let Some(&(s, e)) = self.page_block_ranges.get(self.current_page) {
                    (s.min(blocks.len()), e.min(blocks.len()))
                } else {
                    (0, blocks.len())
                };
                let show_title = self.scroll_mode || self.current_page == 0;

                if self.scroll_mode {
                    let mut scroll_area = egui::ScrollArea::vertical().auto_shrink([false; 2]);
                    if self.scroll_to_top {
                        scroll_area = scroll_area.vertical_scroll_offset(0.0);
                        self.scroll_to_top = false;
                    }
                    scroll_area.show(ui, |ui| {
                        Self::render_content_layout(
                            ui,
                            h_margin,
                            text_width,
                            &title,
                            &blocks,
                            block_start,
                            block_end,
                            show_title,
                            self.font_size,
                            self.reader_bg_color,
                            self.current_chapter,
                            total_ch,
                            &mut action_prev_chapter,
                            &mut action_next_chapter,
                            &mut action_go_back,
                            true,
                            has_previous_chapter,
                            self.reader_font_color,
                            &effective_font_family,
                            &self.i18n,
                            &mut clicked_link,
                        );
                    });
                } else {
                    let page_rect = ui.available_rect_before_wrap();
                    if dual_column {
                        let col_w = (page_rect.width() - DUAL_COLUMN_GAP) / 2.0;
                        let left_rect = egui::Rect::from_min_size(
                            page_rect.min,
                            egui::vec2(col_w, page_rect.height()),
                        );
                        let right_rect = egui::Rect::from_min_size(
                            egui::pos2(page_rect.min.x + col_w + DUAL_COLUMN_GAP, page_rect.min.y),
                            egui::vec2(col_w, page_rect.height()),
                        );
                        let right_page = self.current_page + 1;
                        let is_anim_dual = self.reader_page_animation != "None"
                            && self.page_anim_progress < 1.0
                            && (self.page_anim_from != self.page_anim_to
                                || self.page_anim_cross_chapter);
                        if is_anim_dual {
                            let t = self.page_anim_progress;
                            let w = page_rect.width();
                            let dir = self.page_anim_direction;
                            let to_offset = egui::vec2(dir * (1.0 - t) * w, 0.0);
                            // "from" spread (sliding out, or static for Cover)
                            {
                                let from_offset = if self.reader_page_animation == "Cover" {
                                    egui::vec2(0.0, 0.0)
                                } else {
                                    egui::vec2(-dir * t * w, 0.0)
                                };
                                if let Some(snap) = &self.page_anim_cross_chapter_snapshot {
                                    let snap_blocks = Arc::clone(&snap.blocks);
                                    let snap_ranges = snap.block_ranges.clone();
                                    let snap_total = snap.total_pages;
                                    let snap_from = snap.from_page;
                                    let snap_title = snap.title.clone();
                                    let from_raw = snap_from.min(snap_total.saturating_sub(1));
                                    let from_left = (from_raw / 2) * 2;
                                    let (fls, fle) = snap_ranges
                                        .get(from_left)
                                        .copied()
                                        .map(|(s, e)| {
                                            (s.min(snap_blocks.len()), e.min(snap_blocks.len()))
                                        })
                                        .unwrap_or((0, snap_blocks.len()));
                                    let left_from_rect = left_rect.translate(from_offset);
                                    ui.allocate_new_ui(
                                        UiBuilder::new().max_rect(left_from_rect),
                                        |ui| {
                                            let clip = left_from_rect.intersect(page_rect);
                                            ui.set_clip_rect(clip);
                                            ui.painter().rect_filled(
                                                clip,
                                                0.0,
                                                self.reader_bg_color,
                                            );
                                            Self::render_content_layout(
                                                ui,
                                                h_margin,
                                                text_width,
                                                &snap_title,
                                                &snap_blocks,
                                                fls,
                                                fle,
                                                from_left == 0,
                                                self.font_size,
                                                self.reader_bg_color,
                                                self.current_chapter,
                                                total_ch,
                                                &mut action_prev_chapter,
                                                &mut action_next_chapter,
                                                &mut action_go_back,
                                                false,
                                                has_previous_chapter,
                                                self.reader_font_color,
                                                &effective_font_family,
                                                &self.i18n,
                                                &mut clicked_link,
                                            );
                                        },
                                    );
                                    let from_right = from_left + 1;
                                    if from_right < snap_total {
                                        let (frs, fre) = snap_ranges
                                            .get(from_right)
                                            .copied()
                                            .map(|(s, e)| {
                                                (s.min(snap_blocks.len()), e.min(snap_blocks.len()))
                                            })
                                            .unwrap_or((0, 0));
                                        let right_from_rect = right_rect.translate(from_offset);
                                        ui.allocate_new_ui(
                                            UiBuilder::new().max_rect(right_from_rect),
                                            |ui| {
                                                let clip = right_from_rect.intersect(page_rect);
                                                ui.set_clip_rect(clip);
                                                ui.painter().rect_filled(
                                                    clip,
                                                    0.0,
                                                    self.reader_bg_color,
                                                );
                                                Self::render_content_layout(
                                                    ui,
                                                    h_margin,
                                                    text_width,
                                                    &snap_title,
                                                    &snap_blocks,
                                                    frs,
                                                    fre,
                                                    false,
                                                    self.font_size,
                                                    self.reader_bg_color,
                                                    self.current_chapter,
                                                    total_ch,
                                                    &mut action_prev_chapter,
                                                    &mut action_next_chapter,
                                                    &mut action_go_back,
                                                    false,
                                                    has_previous_chapter,
                                                    self.reader_font_color,
                                                    &effective_font_family,
                                                    &self.i18n,
                                                    &mut clicked_link,
                                                );
                                            },
                                        );
                                    }
                                } else {
                                    let from_raw =
                                        self.page_anim_from.min(self.total_pages.saturating_sub(1));
                                    let from_left = (from_raw / 2) * 2;
                                    let (fls, fle) = self
                                        .page_block_ranges
                                        .get(from_left)
                                        .copied()
                                        .map(|(s, e)| (s.min(blocks.len()), e.min(blocks.len())))
                                        .unwrap_or((0, blocks.len()));
                                    let left_from_rect = left_rect.translate(from_offset);
                                    ui.allocate_new_ui(
                                        UiBuilder::new().max_rect(left_from_rect),
                                        |ui| {
                                            let clip = left_from_rect.intersect(page_rect);
                                            ui.set_clip_rect(clip);
                                            ui.painter().rect_filled(
                                                clip,
                                                0.0,
                                                self.reader_bg_color,
                                            );
                                            Self::render_content_layout(
                                                ui,
                                                h_margin,
                                                text_width,
                                                &title,
                                                &blocks,
                                                fls,
                                                fle,
                                                from_left == 0,
                                                self.font_size,
                                                self.reader_bg_color,
                                                self.current_chapter,
                                                total_ch,
                                                &mut action_prev_chapter,
                                                &mut action_next_chapter,
                                                &mut action_go_back,
                                                false,
                                                has_previous_chapter,
                                                self.reader_font_color,
                                                &effective_font_family,
                                                &self.i18n,
                                                &mut clicked_link,
                                            );
                                        },
                                    );
                                    let from_right = from_left + 1;
                                    if from_right < self.total_pages {
                                        let (frs, fre) = self
                                            .page_block_ranges
                                            .get(from_right)
                                            .copied()
                                            .map(|(s, e)| {
                                                (s.min(blocks.len()), e.min(blocks.len()))
                                            })
                                            .unwrap_or((0, 0));
                                        let right_from_rect = right_rect.translate(from_offset);
                                        ui.allocate_new_ui(
                                            UiBuilder::new().max_rect(right_from_rect),
                                            |ui| {
                                                let clip = right_from_rect.intersect(page_rect);
                                                ui.set_clip_rect(clip);
                                                ui.painter().rect_filled(
                                                    clip,
                                                    0.0,
                                                    self.reader_bg_color,
                                                );
                                                Self::render_content_layout(
                                                    ui,
                                                    h_margin,
                                                    text_width,
                                                    &title,
                                                    &blocks,
                                                    frs,
                                                    fre,
                                                    false,
                                                    self.font_size,
                                                    self.reader_bg_color,
                                                    self.current_chapter,
                                                    total_ch,
                                                    &mut action_prev_chapter,
                                                    &mut action_next_chapter,
                                                    &mut action_go_back,
                                                    false,
                                                    has_previous_chapter,
                                                    self.reader_font_color,
                                                    &effective_font_family,
                                                    &self.i18n,
                                                    &mut clicked_link,
                                                );
                                            },
                                        );
                                    }
                                }

                                // Cover animation: shadow on leading edge of incoming spread
                                if self.reader_page_animation == "Cover" {
                                    let to_rect_pos = left_rect.translate(to_offset);
                                    let shadow_w = 28.0f32;
                                    let steps = 8u32;
                                    for i in 0..steps {
                                        let sub_w = shadow_w / steps as f32;
                                        let (sub_x, alpha_val) = if dir > 0.0 {
                                            let x =
                                                to_rect_pos.left() - shadow_w + i as f32 * sub_w;
                                            let a = ((i + 1) as f32 * 70.0 / steps as f32) as u8;
                                            (x, a)
                                        } else {
                                            let x = to_rect_pos.right()
                                                + (page_rect.width() - left_rect.width())
                                                + i as f32 * sub_w;
                                            let a =
                                                ((steps - i) as f32 * 70.0 / steps as f32) as u8;
                                            (x, a)
                                        };
                                        let sub_rect = egui::Rect::from_min_size(
                                            egui::pos2(sub_x, page_rect.top()),
                                            egui::vec2(sub_w, page_rect.height()),
                                        );
                                        ui.painter().rect_filled(
                                            sub_rect,
                                            0.0,
                                            Color32::from_black_alpha(alpha_val),
                                        );
                                    }
                                }
                            }
                            // "to" spread (sliding in)
                            let to_raw = self.page_anim_to.min(self.total_pages.saturating_sub(1));
                            let to_left = (to_raw / 2) * 2;
                            let (tls, tle) = self
                                .page_block_ranges
                                .get(to_left)
                                .copied()
                                .map(|(s, e)| (s.min(blocks.len()), e.min(blocks.len())))
                                .unwrap_or((0, blocks.len()));
                            let left_to_rect = left_rect.translate(to_offset);
                            ui.allocate_new_ui(UiBuilder::new().max_rect(left_to_rect), |ui| {
                                let clip = left_to_rect.intersect(page_rect);
                                ui.set_clip_rect(clip);
                                ui.painter().rect_filled(clip, 0.0, self.reader_bg_color);
                                Self::render_content_layout(
                                    ui,
                                    h_margin,
                                    text_width,
                                    &title,
                                    &blocks,
                                    tls,
                                    tle,
                                    to_left == 0,
                                    self.font_size,
                                    self.reader_bg_color,
                                    self.current_chapter,
                                    total_ch,
                                    &mut action_prev_chapter,
                                    &mut action_next_chapter,
                                    &mut action_go_back,
                                    false,
                                    has_previous_chapter,
                                    self.reader_font_color,
                                    &effective_font_family,
                                    &self.i18n,
                                    &mut clicked_link,
                                );
                            });
                            let to_right = to_left + 1;
                            if to_right < self.total_pages {
                                let (trs, tre) = self
                                    .page_block_ranges
                                    .get(to_right)
                                    .copied()
                                    .map(|(s, e)| (s.min(blocks.len()), e.min(blocks.len())))
                                    .unwrap_or((0, 0));
                                let right_to_rect = right_rect.translate(to_offset);
                                ui.allocate_new_ui(
                                    UiBuilder::new().max_rect(right_to_rect),
                                    |ui| {
                                        let clip = right_to_rect.intersect(page_rect);
                                        ui.set_clip_rect(clip);
                                        ui.painter().rect_filled(clip, 0.0, self.reader_bg_color);
                                        Self::render_content_layout(
                                            ui,
                                            h_margin,
                                            text_width,
                                            &title,
                                            &blocks,
                                            trs,
                                            tre,
                                            false,
                                            self.font_size,
                                            self.reader_bg_color,
                                            self.current_chapter,
                                            total_ch,
                                            &mut action_prev_chapter,
                                            &mut action_next_chapter,
                                            &mut action_go_back,
                                            false,
                                            has_previous_chapter,
                                            self.reader_font_color,
                                            &effective_font_family,
                                            &self.i18n,
                                            &mut clicked_link,
                                        );
                                    },
                                );
                            }
                        } else {
                            ui.allocate_new_ui(UiBuilder::new().max_rect(left_rect), |ui| {
                                Self::render_content_layout(
                                    ui,
                                    h_margin,
                                    text_width,
                                    &title,
                                    &blocks,
                                    block_start,
                                    block_end,
                                    show_title,
                                    self.font_size,
                                    self.reader_bg_color,
                                    self.current_chapter,
                                    total_ch,
                                    &mut action_prev_chapter,
                                    &mut action_next_chapter,
                                    &mut action_go_back,
                                    false,
                                    has_previous_chapter,
                                    self.reader_font_color,
                                    &effective_font_family,
                                    &self.i18n,
                                    &mut clicked_link,
                                );
                            });
                            if right_page < self.total_pages {
                                let (rs, re) =
                                    if let Some(&(s, e)) = self.page_block_ranges.get(right_page) {
                                        (s.min(blocks.len()), e.min(blocks.len()))
                                    } else {
                                        (0, 0)
                                    };
                                ui.allocate_new_ui(UiBuilder::new().max_rect(right_rect), |ui| {
                                    Self::render_content_layout(
                                        ui,
                                        h_margin,
                                        text_width,
                                        &title,
                                        &blocks,
                                        rs,
                                        re,
                                        right_page == 0,
                                        self.font_size,
                                        self.reader_bg_color,
                                        self.current_chapter,
                                        total_ch,
                                        &mut action_prev_chapter,
                                        &mut action_next_chapter,
                                        &mut action_go_back,
                                        false,
                                        has_previous_chapter,
                                        self.reader_font_color,
                                        &effective_font_family,
                                        &self.i18n,
                                        &mut clicked_link,
                                    );
                                });
                            }
                        }
                        if !is_anim_dual {
                            let sep_x = page_rect.min.x + col_w + DUAL_COLUMN_GAP / 2.0;
                            ui.painter().line_segment(
                                [
                                    egui::pos2(sep_x, page_rect.top() + 20.0),
                                    egui::pos2(sep_x, page_rect.bottom() - 20.0),
                                ],
                                egui::Stroke::new(1.0, Color32::from_gray(80)),
                            );
                        }
                        let page_info = if right_page < self.total_pages {
                            format!(
                                "{}-{} / {}",
                                self.current_page + 1,
                                right_page + 1,
                                self.total_pages
                            )
                        } else {
                            format!("{} / {}", self.current_page + 1, self.total_pages)
                        };
                        ui.painter().text(
                            egui::pos2(page_rect.right() - 20.0, page_rect.top() + 8.0),
                            egui::Align2::RIGHT_TOP,
                            page_info,
                            FontId::proportional(13.0),
                            Color32::GRAY,
                        );
                        ui.painter().text(
                            egui::pos2(page_rect.right() - 20.0, page_rect.bottom() - 8.0),
                            egui::Align2::RIGHT_BOTTOM,
                            self.i18n.tf2(
                                "reader.chapter_indicator",
                                &(self.current_chapter + 1).to_string(),
                                &total_ch.to_string(),
                            ),
                            FontId::proportional(13.0),
                            Color32::GRAY,
                        );
                    } else {
                        let is_animating = self.reader_page_animation != "None"
                            && self.page_anim_progress < 1.0
                            && (self.page_anim_from != self.page_anim_to
                                || self.page_anim_cross_chapter);

                        if is_animating {
                            let t = self.page_anim_progress;
                            let w = page_rect.width();
                            let dir = self.page_anim_direction;
                            let to_offset = egui::vec2(dir * (1.0 - t) * w, 0.0);

                            let to_idx = self.page_anim_to.min(self.total_pages.saturating_sub(1));
                            let (ts, te) = self
                                .page_block_ranges
                                .get(to_idx)
                                .copied()
                                .unwrap_or((0, blocks.len()));

                            {
                                let from_offset = if self.reader_page_animation == "Cover" {
                                    egui::vec2(0.0, 0.0)
                                } else {
                                    egui::vec2(-dir * t * w, 0.0)
                                };
                                if let Some(snap) = &self.page_anim_cross_chapter_snapshot {
                                    let snap_blocks = Arc::clone(&snap.blocks);
                                    let snap_ranges = snap.block_ranges.clone();
                                    let snap_total = snap.total_pages;
                                    let snap_from = snap.from_page;
                                    let snap_title = snap.title.clone();
                                    let from_idx = snap_from.min(snap_total.saturating_sub(1));
                                    let (fs, fe) = snap_ranges
                                        .get(from_idx)
                                        .copied()
                                        .unwrap_or((0, snap_blocks.len()));
                                    let from_rect = page_rect.translate(from_offset);
                                    ui.allocate_new_ui(
                                        UiBuilder::new().max_rect(from_rect),
                                        |ui| {
                                            let clip = from_rect.intersect(page_rect);
                                            ui.set_clip_rect(clip);
                                            ui.painter().rect_filled(
                                                clip,
                                                0.0,
                                                self.reader_bg_color,
                                            );
                                            Self::render_content_layout(
                                                ui,
                                                h_margin,
                                                text_width,
                                                &snap_title,
                                                &snap_blocks,
                                                fs.min(snap_blocks.len()),
                                                fe.min(snap_blocks.len()),
                                                from_idx == 0,
                                                self.font_size,
                                                self.reader_bg_color,
                                                self.current_chapter,
                                                total_ch,
                                                &mut action_prev_chapter,
                                                &mut action_next_chapter,
                                                &mut action_go_back,
                                                false,
                                                has_previous_chapter,
                                                self.reader_font_color,
                                                &effective_font_family,
                                                &self.i18n,
                                                &mut clicked_link,
                                            );
                                        },
                                    );
                                } else {
                                    let from_idx =
                                        self.page_anim_from.min(self.total_pages.saturating_sub(1));
                                    let (fs, fe) = self
                                        .page_block_ranges
                                        .get(from_idx)
                                        .copied()
                                        .unwrap_or((0, blocks.len()));
                                    let from_rect = page_rect.translate(from_offset);
                                    ui.allocate_new_ui(
                                        UiBuilder::new().max_rect(from_rect),
                                        |ui| {
                                            let clip = from_rect.intersect(page_rect);
                                            ui.set_clip_rect(clip);
                                            ui.painter().rect_filled(
                                                clip,
                                                0.0,
                                                self.reader_bg_color,
                                            );
                                            Self::render_content_layout(
                                                ui,
                                                h_margin,
                                                text_width,
                                                &title,
                                                &blocks,
                                                fs.min(blocks.len()),
                                                fe.min(blocks.len()),
                                                from_idx == 0,
                                                self.font_size,
                                                self.reader_bg_color,
                                                self.current_chapter,
                                                total_ch,
                                                &mut action_prev_chapter,
                                                &mut action_next_chapter,
                                                &mut action_go_back,
                                                false,
                                                has_previous_chapter,
                                                self.reader_font_color,
                                                &effective_font_family,
                                                &self.i18n,
                                                &mut clicked_link,
                                            );
                                        },
                                    );
                                }

                                // Cover animation: draw shadow on leading edge of incoming page
                                if self.reader_page_animation == "Cover" {
                                    let to_rect_pos = page_rect.translate(to_offset);
                                    let shadow_w = 28.0f32;
                                    let steps = 8u32;
                                    for i in 0..steps {
                                        let sub_w = shadow_w / steps as f32;
                                        let (sub_x, alpha_val) = if dir > 0.0 {
                                            let x =
                                                to_rect_pos.left() - shadow_w + i as f32 * sub_w;
                                            let a = ((i + 1) as f32 * 70.0 / steps as f32) as u8;
                                            (x, a)
                                        } else {
                                            let x = to_rect_pos.right() + i as f32 * sub_w;
                                            let a =
                                                ((steps - i) as f32 * 70.0 / steps as f32) as u8;
                                            (x, a)
                                        };
                                        let sub_rect = egui::Rect::from_min_size(
                                            egui::pos2(sub_x, page_rect.top()),
                                            egui::vec2(sub_w, page_rect.height()),
                                        );
                                        ui.painter().rect_filled(
                                            sub_rect,
                                            0.0,
                                            Color32::from_black_alpha(alpha_val),
                                        );
                                    }
                                }
                            }

                            let to_rect = page_rect.translate(to_offset);

                            ui.allocate_new_ui(UiBuilder::new().max_rect(to_rect), |ui| {
                                let clip = to_rect.intersect(page_rect);
                                ui.set_clip_rect(clip);
                                ui.painter().rect_filled(clip, 0.0, self.reader_bg_color);
                                Self::render_content_layout(
                                    ui,
                                    h_margin,
                                    text_width,
                                    &title,
                                    &blocks,
                                    ts.min(blocks.len()),
                                    te.min(blocks.len()),
                                    to_idx == 0,
                                    self.font_size,
                                    self.reader_bg_color,
                                    self.current_chapter,
                                    total_ch,
                                    &mut action_prev_chapter,
                                    &mut action_next_chapter,
                                    &mut action_go_back,
                                    false,
                                    has_previous_chapter,
                                    self.reader_font_color,
                                    &effective_font_family,
                                    &self.i18n,
                                    &mut clicked_link,
                                );
                            });
                        } else {
                            Self::render_content_layout(
                                ui,
                                h_margin,
                                text_width,
                                &title,
                                &blocks,
                                block_start,
                                block_end,
                                show_title,
                                self.font_size,
                                self.reader_bg_color,
                                self.current_chapter,
                                total_ch,
                                &mut action_prev_chapter,
                                &mut action_next_chapter,
                                &mut action_go_back,
                                false,
                                has_previous_chapter,
                                self.reader_font_color,
                                &effective_font_family,
                                &self.i18n,
                                &mut clicked_link,
                            );
                        }
                        ui.painter().text(
                            egui::pos2(page_rect.right() - 20.0, page_rect.top() + 8.0),
                            egui::Align2::RIGHT_TOP,
                            format!("{} / {}", self.current_page + 1, self.total_pages),
                            FontId::proportional(13.0),
                            Color32::GRAY,
                        );
                        ui.painter().text(
                            egui::pos2(page_rect.right() - 20.0, page_rect.bottom() - 8.0),
                            egui::Align2::RIGHT_BOTTOM,
                            self.i18n.tf2(
                                "reader.chapter_indicator",
                                &(self.current_chapter + 1).to_string(),
                                &total_ch.to_string(),
                            ),
                            FontId::proportional(13.0),
                            Color32::GRAY,
                        );
                    }
                    if !self.show_reader_settings && !self.show_sharing_panel {
                        let pointer_in_page = ui.input(|i| {
                            i.pointer
                                .hover_pos()
                                .map(|pos| page_rect.contains(pos))
                                .unwrap_or(false)
                        });
                        if pointer_in_page {
                            let scroll = ui.input(|i| i.raw_scroll_delta.y);
                            if scroll < -30.0 {
                                action_next_page = true;
                            } else if scroll > 30.0 {
                                action_prev_page = true;
                            }
                        }
                        if clicked_link.is_none() && ui.input(|i| i.pointer.primary_clicked()) {
                            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                                if page_rect.contains(pos) {
                                    if pos.x < page_rect.center().x {
                                        action_prev_page = true;
                                    } else {
                                        action_next_page = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new(self.i18n.t("reader.select_book_hint"))
                        .size(24.0)
                        .color(Color32::from_gray(128)),
                );
            });
        }

        if action_prev_chapter {
            self.prev_chapter();
        }
        if action_next_chapter {
            self.next_chapter();
        }
        if action_go_back {
            if let Some(prev) = self.previous_chapter.take() {
                let total = self.total_chapters();
                if total > 0 {
                    self.current_chapter = prev.min(total - 1);
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
        }
        if let Some(url) = clicked_link {
            let url = url.trim().to_string();
            let lowered = url.to_lowercase();
            if lowered.starts_with("http://")
                || lowered.starts_with("https://")
                || lowered.starts_with("mailto:")
                || lowered.starts_with("tel:")
            {
                ui.ctx().open_url(egui::OpenUrl::new_tab(url));
            } else if !url.starts_with('#') {
                let normalized = normalize_epub_href(&url);
                let target_idx = if !normalized.is_empty() {
                    self.book.as_ref().and_then(|book| {
                        book.chapters.iter().position(|ch| {
                            let Some(ref src) = ch.source_href else {
                                return false;
                            };
                            let src_norm = normalize_epub_href(src);
                            src_norm == normalized
                                || src_norm.ends_with(&format!("/{normalized}"))
                                || normalized.ends_with(&format!("/{src_norm}"))
                        })
                    })
                } else {
                    None
                };
                if let Some(idx) = target_idx {
                    self.current_chapter = idx;
                    self.current_page = 0;
                    self.scroll_to_top = true;
                    self.pages_dirty = true;
                } else {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                }
            }
        }
        if action_prev_page {
            if is_dual_column {
                if self.current_page >= 2 {
                    self.trigger_page_animation_to(self.current_page - 2, -1.0);
                } else if self.current_chapter > 0 {
                    self.capture_cross_chapter_snapshot();
                    self.prev_chapter();
                    self.current_page = usize::MAX;
                    self.start_cross_chapter_animation(-1.0);
                }
            } else {
                self.prev_page();
            }
        }
        if action_next_page {
            if is_dual_column {
                if self.current_page + 2 < self.total_pages {
                    self.trigger_page_animation_to(self.current_page + 2, 1.0);
                } else {
                    self.capture_cross_chapter_snapshot();
                    self.next_chapter();
                    self.start_cross_chapter_animation(1.0);
                }
            } else {
                self.next_page();
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_content_layout(
        ui: &mut egui::Ui,
        h_margin: f32,
        text_width: f32,
        title: &str,
        blocks: &[ContentBlock],
        block_start: usize,
        block_end: usize,
        show_title: bool,
        font_size: f32,
        bg_color: Color32,
        current_chapter: usize,
        total_ch: usize,
        action_prev: &mut bool,
        action_next: &mut bool,
        action_go_back: &mut bool,
        show_chapter_nav: bool,
        has_previous_chapter: bool,
        font_color: Option<Color32>,
        font_family_name: &str,
        i18n: &reader_core::i18n::I18n,
        clicked_link: &mut Option<String>,
    ) {
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 0,
                right: 0,
                top: 48,
                bottom: 56,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(h_margin);
                    ui.vertical(|ui| {
                        ui.set_max_width(text_width);
                        if show_title {
                            let title_color = effective_text_color(bg_color, font_color);
                            let title_family = match font_family_name {
                                "Monospace" => FontFamily::Monospace,
                                "Serif" => FontFamily::Name("Serif".into()),
                                "Sans" => FontFamily::Proportional,
                                other => FontFamily::Name(other.into()),
                            };
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new(title)
                                        .size(font_size * 1.8)
                                        .strong()
                                        .color(title_color)
                                        .family(title_family),
                                );
                            });
                            ui.add_space(TITLE_SPACING);
                        }
                        let effective_start = if show_title && block_start == 0 {
                            if matches!(blocks.first(), Some(ContentBlock::Heading { .. })) {
                                1
                            } else {
                                0
                            }
                        } else {
                            block_start
                        };
                        for block in &blocks[effective_start..block_end] {
                            render_block(
                                ui,
                                block,
                                font_size,
                                bg_color,
                                text_width,
                                font_color,
                                font_family_name,
                                i18n,
                                clicked_link,
                            );
                        }
                        if show_chapter_nav {
                            ui.add_space(60.0);
                            if has_previous_chapter {
                                ui.vertical_centered(|ui| {
                                    if ui
                                        .button(
                                            egui::RichText::new(i18n.t("reader.go_back_chapter"))
                                                .size(15.0),
                                        )
                                        .clicked()
                                    {
                                        *action_go_back = true;
                                    }
                                });
                                ui.add_space(8.0);
                            }
                            ui.separator();
                            ui.add_space(30.0);
                            ui.horizontal(|ui| {
                                if current_chapter > 0
                                    && ui
                                        .button(
                                            egui::RichText::new(i18n.t("reader.prev_chapter"))
                                                .size(16.0),
                                        )
                                        .clicked()
                                {
                                    *action_prev = true;
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if current_chapter + 1 < total_ch
                                            && ui
                                                .button(
                                                    egui::RichText::new(
                                                        i18n.t("reader.next_chapter"),
                                                    )
                                                    .size(16.0),
                                                )
                                                .clicked()
                                        {
                                            *action_next = true;
                                        }
                                    },
                                );
                            });
                        }
                    });
                    ui.add_space(h_margin);
                });
            });
    }
}

fn effective_text_color(bg_color: Color32, font_color: Option<Color32>) -> Color32 {
    let bg_lum = {
        let [r, g, b, _] = bg_color.to_array();
        (r as f32 * 0.299 + g as f32 * 0.587 + b as f32 * 0.114) / 255.0
    };
    font_color.unwrap_or_else(|| {
        if bg_lum < 0.45 {
            Color32::from_gray(220)
        } else {
            Color32::from_gray(30)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn render_block(
    ui: &mut egui::Ui,
    block: &ContentBlock,
    font_size: f32,
    bg_color: Color32,
    max_width: f32,
    font_color: Option<Color32>,
    font_family_name: &str,
    i18n: &reader_core::i18n::I18n,
    clicked_link: &mut Option<String>,
) {
    match block {
        ContentBlock::Heading { level, spans } => {
            let scale = match level {
                1 => 2.0,
                2 => 1.6,
                3 => 1.3,
                _ => 1.2,
            };
            let job = build_layout_job(
                spans,
                font_size * scale,
                bg_color,
                true,
                max_width,
                font_color,
                font_family_name,
            );
            let first_link = spans.iter().find_map(|s| s.link_url.as_deref());
            ui.add_space(font_size * 0.8);
            if let Some(url) = first_link {
                let response = ui.add(egui::Label::new(job).sense(egui::Sense::click()));
                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if response.clicked() {
                    *clicked_link = Some(url.to_string());
                }
            } else {
                ui.label(job);
            }
            ui.add_space(font_size * 0.4);
        }
        ContentBlock::Paragraph { spans } => {
            let job = build_layout_job(
                spans,
                font_size,
                bg_color,
                false,
                max_width,
                font_color,
                font_family_name,
            );
            let first_link = spans.iter().find_map(|s| s.link_url.as_deref());
            if let Some(url) = first_link {
                let response = ui.add(egui::Label::new(job).sense(egui::Sense::click()));
                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if response.clicked() {
                    *clicked_link = Some(url.to_string());
                }
            } else {
                ui.label(job);
            }
            ui.add_space(font_size * 0.6);
        }
        ContentBlock::Separator => {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
        }
        ContentBlock::BlankLine => {
            ui.add_space(font_size * 0.5);
        }
        ContentBlock::Image { alt, .. } => {
            ui.add_space(font_size * 0.6);
            let text = alt
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(|s| i18n.tf1("reader.image_with_alt", s))
                .unwrap_or_else(|| i18n.t("reader.image").to_string());
            ui.label(egui::RichText::new(text).italics().color(Color32::GRAY));
            ui.add_space(font_size * 0.6);
        }
    }
}

fn build_layout_job(
    spans: &[TextSpan],
    font_size: f32,
    bg_color: Color32,
    is_heading: bool,
    max_width: f32,
    font_color: Option<Color32>,
    font_family_name: &str,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = max_width;
    let bg_lum = {
        let [r, g, b, _] = bg_color.to_array();
        (r as f32 * 0.299 + g as f32 * 0.587 + b as f32 * 0.114) / 255.0
    };
    let base_color = effective_text_color(bg_color, font_color);
    let link_color = if bg_lum < 0.45 {
        Color32::from_rgb(100, 160, 255)
    } else {
        Color32::from_rgb(30, 80, 200)
    };
    for (i, span) in spans.iter().enumerate() {
        let is_bold =
            matches!(span.style, InlineStyle::Bold | InlineStyle::BoldItalic) || is_heading;
        let is_italic = matches!(span.style, InlineStyle::Italic | InlineStyle::BoldItalic);
        let is_link = span.link_url.is_some();
        let family = if is_bold {
            FontFamily::Name("Bold".into())
        } else {
            match font_family_name {
                "Monospace" => FontFamily::Monospace,
                "Serif" => FontFamily::Name("Serif".into()),
                "Sans" => FontFamily::Proportional,
                other => FontFamily::Name(other.into()),
            }
        };
        let color = if is_link { link_color } else { base_color };
        let format = TextFormat {
            font_id: FontId::new(font_size, family),
            color,
            italics: is_italic,
            underline: if is_link {
                egui::Stroke::new(1.0, link_color)
            } else {
                egui::Stroke::NONE
            },
            line_height: Some(font_size * 1.8),
            ..Default::default()
        };
        let leading = if i == 0 && !is_heading {
            font_size * 2.0
        } else {
            0.0
        };
        let wrapped = wrap_cjk_text(
            &span.text,
            font_size,
            max_width,
            if i == 0 && !is_heading { leading } else { 0.0 },
        );
        job.append(&wrapped, leading, format);
    }
    if job.sections.is_empty() {
        job.append(
            " ",
            0.0,
            TextFormat {
                font_id: FontId::new(font_size, FontFamily::Proportional),
                color: Color32::TRANSPARENT,
                line_height: Some(font_size * 1.0),
                ..Default::default()
            },
        );
    }
    job
}

fn estimate_block_height(
    block: &ContentBlock,
    font_size: f32,
    line_height: f32,
    max_width: f32,
) -> f32 {
    match block {
        ContentBlock::Heading { level, spans } => {
            let scale = match level {
                1 => 2.0,
                2 => 1.6,
                3 => 1.3,
                _ => 1.2,
            };
            let sz = font_size * scale;
            let text_len: f32 = spans.iter().map(|s| estimate_text_width(&s.text, sz)).sum();
            (text_len / max_width).ceil().max(1.0) * sz * 1.8 + font_size * 1.2
        }
        ContentBlock::Paragraph { spans } => {
            let text_len: f32 = spans
                .iter()
                .map(|s| estimate_text_width(&s.text, font_size))
                .sum();
            ((text_len + font_size * 2.0) / max_width).ceil().max(1.0) * line_height
                + font_size * 0.6
        }
        ContentBlock::Separator => 24.0,
        ContentBlock::BlankLine => font_size * 0.5,
        ContentBlock::Image { .. } => font_size * 3.0,
    }
}

fn estimate_text_width(text: &str, font_size: f32) -> f32 {
    text.chars()
        .map(|c| {
            if c.is_ascii() {
                font_size * 0.55
            } else {
                font_size
            }
        })
        .sum()
}

fn wrap_cjk_text(text: &str, font_size: f32, max_width: f32, first_line_indent: f32) -> String {
    const NO_BREAK_BEFORE: &[char] = &[
        '\u{3002}', '\u{FF0C}', '\u{FF01}', '\u{FF1F}', '\u{FF1B}', '\u{FF1A}', '\u{3001}',
        '\u{FF09}', '\u{300B}', '\u{300D}', '\u{300F}', '\u{3011}', '\u{3015}', '\u{3009}',
        '\u{3017}', '\u{FF5E}', '\u{2026}', ',', '.', '!', '?', ';', ':', ')', ']', '}',
        '\u{2014}', '\u{2013}', '\u{201C}', '\u{201D}', '\u{2018}', '\u{2019}',
    ];
    const NO_BREAK_AFTER: &[char] = &[
        '\u{FF08}', '\u{300A}', '\u{300C}', '\u{300E}', '\u{3010}', '\u{3014}', '\u{3008}',
        '\u{3016}', '(', '[', '{', '\u{201C}', '\u{201D}', '\u{2018}', '\u{2019}',
    ];
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    let effective_max = max_width - font_size * 0.5;
    // Use a Vec<char> buffer to track the result, avoiding repeated String/Vec<char> conversions
    let mut buf: Vec<char> = Vec::with_capacity(chars.len() + chars.len() / 8);
    let mut line_width: f32 = first_line_indent;
    let char_width = |c: char| -> f32 {
        if c.is_ascii() {
            font_size * 0.55
        } else {
            font_size
        }
    };
    for (i, &ch) in chars.iter().enumerate() {
        let cw = char_width(ch);
        if line_width + cw > effective_max && i > 0 && ch != '\n' {
            if NO_BREAK_BEFORE.contains(&ch) {
                // Backtrack: find a good break point before the no-break-before char
                let mut backtrack = 0;
                let mut pos = buf.len();
                while pos > 0 && NO_BREAK_BEFORE.contains(&buf[pos - 1]) {
                    pos -= 1;
                    backtrack += 1;
                    if backtrack > 5 {
                        break;
                    }
                }
                if pos == buf.len() && pos > 0 {
                    pos -= 1;
                }
                if pos > 0 && NO_BREAK_AFTER.contains(&buf[pos - 1]) && pos > 1 {
                    pos -= 1;
                }
                if pos > 0 && pos < buf.len() {
                    buf.insert(pos, '\n');
                    line_width = buf[pos + 1..].iter().map(|&c| char_width(c)).sum();
                } else {
                    buf.push('\n');
                    line_width = 0.0;
                }
            } else if i > 0 && NO_BREAK_AFTER.contains(&chars[i - 1]) {
                let pos = buf.len().saturating_sub(1);
                if pos > 0 {
                    buf.insert(pos, '\n');
                    line_width = buf[pos + 1..].iter().map(|&c| char_width(c)).sum();
                } else {
                    buf.push('\n');
                    line_width = 0.0;
                }
            } else {
                buf.push('\n');
                line_width = 0.0;
            }
        }
        if ch == '\n' {
            buf.push(ch);
            line_width = 0.0;
        } else {
            buf.push(ch);
            line_width += cw;
        }
    }
    buf.into_iter().collect()
}

fn normalize_epub_href(href: &str) -> String {
    let s = href.trim().split('#').next().unwrap_or("").trim();
    if s.is_empty() {
        return String::new();
    }
    s.trim_start_matches("./").trim_matches('/').to_string()
}
