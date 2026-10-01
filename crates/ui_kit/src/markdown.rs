//! Markdown drawn with the design's fonts and colours: paragraphs,
//! headings, emphasis, inline code, links, lists (nested, numbered, task
//! lists), block quotes, fenced code with a copy button, tables and rules.
//!
//! The text is parsed into a small tree of blocks first, then drawn. Every
//! run of text is one selectable label, so a selection runs across its
//! styles; a link inside one is found from where the click lands in the
//! laid-out text.

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Galley, Stroke, Ui};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::theme::{mono, sans, sans_semibold};
use crate::tokens::*;
use crate::widgets::icon_button;

/// How a piece of markdown reads: its body size and colour.
#[derive(Debug, Clone, Copy)]
pub struct Style {
    pub size: f32,
    pub color: Color32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            size: FONT_SM,
            color: TEXT1,
        }
    }
}

/// Draw `text` as markdown. `id` keeps its code blocks' scroll apart from
/// other messages'.
pub fn show(ui: &mut Ui, id: egui::Id, text: &str, style: Style) {
    let blocks = parse(text);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = SPACE_1 + 2.0;
        draw_blocks(ui, id, &blocks, style);
    });
}

// ------------------------------------------------------------------ the tree

/// A run of text in one style.
#[derive(Debug, Clone, Default, PartialEq)]
struct Span {
    text: String,
    strong: bool,
    emphasis: bool,
    strike: bool,
    code: bool,
    link: Option<String>,
}

type Inline = Vec<Span>;

#[derive(Debug, Clone, PartialEq)]
enum Block {
    Paragraph(Inline),
    Heading(u8, Inline),
    Code {
        language: String,
        text: String,
    },
    Quote(Vec<Block>),
    List {
        start: Option<u64>,
        items: Vec<Vec<Block>>,
    },
    Table {
        head: Vec<Inline>,
        rows: Vec<Vec<Inline>>,
    },
    Rule,
}

/// A container being filled while the events come.
enum Open {
    Root(Vec<Block>),
    Quote(Vec<Block>),
    List {
        start: Option<u64>,
        items: Vec<Vec<Block>>,
    },
    Item(Vec<Block>),
}

impl Open {
    fn blocks(&mut self) -> &mut Vec<Block> {
        match self {
            Open::Root(b) | Open::Quote(b) | Open::Item(b) => b,
            // A block straight inside a list (never from the parser) goes
            // into its last item.
            Open::List { items, .. } => {
                if items.is_empty() {
                    items.push(Vec::new());
                }
                items.last_mut().expect("an item")
            }
        }
    }
}

#[derive(Default)]
struct Table {
    head: Vec<Inline>,
    rows: Vec<Vec<Inline>>,
    row: Vec<Inline>,
}

struct Builder {
    open: Vec<Open>,
    /// The text being gathered: a paragraph, a heading, a table cell, or
    /// a tight list item's text, which comes with no paragraph around it.
    inline: Option<Inline>,
    heading: Option<u8>,
    code: Option<(String, String)>,
    table: Option<Table>,
    strong: u32,
    emphasis: u32,
    strike: u32,
    link: Option<String>,
}

fn parse(text: &str) -> Vec<Block> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let mut b = Builder {
        open: vec![Open::Root(Vec::new())],
        inline: None,
        heading: None,
        code: None,
        table: None,
        strong: 0,
        emphasis: 0,
        strike: 0,
        link: None,
    };
    for event in Parser::new_ext(text, options) {
        b.event(event);
    }
    b.flush_inline();
    while b.open.len() > 1 {
        b.close();
    }
    match b.open.pop() {
        Some(Open::Root(blocks)) => blocks,
        _ => Vec::new(),
    }
}

impl Builder {
    fn push_block(&mut self, block: Block) {
        self.open.last_mut().expect("the root").blocks().push(block);
    }

    /// Text gathered with no paragraph around it becomes one.
    fn flush_inline(&mut self) {
        if self.table.is_some() {
            return;
        }
        if let Some(inline) = self.inline.take()
            && inline.iter().any(|s| !s.text.trim().is_empty())
        {
            let block = match self.heading.take() {
                Some(level) => Block::Heading(level, inline),
                None => Block::Paragraph(inline),
            };
            self.push_block(block);
        }
    }

    /// Close the innermost container into the one around it.
    fn close(&mut self) {
        self.flush_inline();
        let Some(done) = self.open.pop() else {
            return;
        };
        match done {
            Open::Quote(blocks) => self.push_block(Block::Quote(blocks)),
            Open::List { start, items } => self.push_block(Block::List { start, items }),
            Open::Item(blocks) => match self.open.last_mut() {
                Some(Open::List { items, .. }) => items.push(blocks),
                Some(other) => other.blocks().extend(blocks),
                None => self.open.push(Open::Root(blocks)),
            },
            Open::Root(blocks) => self.open.push(Open::Root(blocks)),
        }
    }

    fn text(&mut self, text: &str, code: bool) {
        if let Some((_, body)) = &mut self.code {
            body.push_str(text);
            return;
        }
        let span = Span {
            text: text.to_string(),
            strong: self.strong > 0,
            emphasis: self.emphasis > 0,
            strike: self.strike > 0,
            code,
            link: self.link.clone(),
        };
        self.inline.get_or_insert_with(Vec::new).push(span);
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.text(&text, false),
            Event::Code(text) => self.text(&text, true),
            Event::Html(text) | Event::InlineHtml(text) => self.text(&text, false),
            Event::InlineMath(text) | Event::DisplayMath(text) => self.text(&text, true),
            Event::SoftBreak => self.text(" ", false),
            Event::HardBreak => self.text("\n", false),
            Event::Rule => {
                self.flush_inline();
                self.push_block(Block::Rule);
            }
            Event::TaskListMarker(done) => self.text(if done { "[x] " } else { "[ ] " }, true),
            Event::FootnoteReference(name) => self.text(&format!("[{name}]"), false),
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {
                self.flush_inline();
                self.inline = Some(Vec::new());
            }
            Tag::Heading { level, .. } => {
                self.flush_inline();
                self.heading = Some(level as u8);
                self.inline = Some(Vec::new());
            }
            Tag::BlockQuote(_) => {
                self.flush_inline();
                self.open.push(Open::Quote(Vec::new()));
            }
            Tag::CodeBlock(kind) => {
                self.flush_inline();
                let language = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(info) => {
                        info.split_whitespace().next().unwrap_or("").to_string()
                    }
                    pulldown_cmark::CodeBlockKind::Indented => String::new(),
                };
                self.code = Some((language, String::new()));
            }
            Tag::List(start) => {
                self.flush_inline();
                self.open.push(Open::List {
                    start,
                    items: Vec::new(),
                });
            }
            Tag::Item => {
                self.flush_inline();
                self.open.push(Open::Item(Vec::new()));
            }
            Tag::Table(_) => {
                self.flush_inline();
                self.table = Some(Table::default());
            }
            Tag::TableCell => self.inline = Some(Vec::new()),
            Tag::Emphasis => self.emphasis += 1,
            Tag::Strong => self.strong += 1,
            Tag::Strikethrough => self.strike += 1,
            Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. } => {
                self.link = Some(dest_url.to_string());
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) => self.flush_inline(),
            TagEnd::BlockQuote(_) | TagEnd::List(_) | TagEnd::Item => self.close(),
            TagEnd::CodeBlock => {
                if let Some((language, mut text)) = self.code.take() {
                    if text.ends_with('\n') {
                        text.pop();
                    }
                    self.push_block(Block::Code { language, text });
                }
            }
            TagEnd::TableCell => {
                let cell = self.inline.take().unwrap_or_default();
                if let Some(table) = &mut self.table {
                    table.row.push(cell);
                }
            }
            TagEnd::TableHead => {
                if let Some(table) = &mut self.table {
                    table.head = std::mem::take(&mut table.row);
                }
            }
            TagEnd::TableRow => {
                if let Some(table) = &mut self.table {
                    let row = std::mem::take(&mut table.row);
                    table.rows.push(row);
                }
            }
            TagEnd::Table => {
                if let Some(table) = self.table.take() {
                    self.push_block(Block::Table {
                        head: table.head,
                        rows: table.rows,
                    });
                }
            }
            TagEnd::Emphasis => self.emphasis = self.emphasis.saturating_sub(1),
            TagEnd::Strong => self.strong = self.strong.saturating_sub(1),
            TagEnd::Strikethrough => self.strike = self.strike.saturating_sub(1),
            TagEnd::Link | TagEnd::Image => self.link = None,
            _ => {}
        }
    }
}

// ------------------------------------------------------------------ drawing

fn draw_blocks(ui: &mut Ui, id: egui::Id, blocks: &[Block], style: Style) {
    for (index, block) in blocks.iter().enumerate() {
        let id = id.with(index);
        match block {
            Block::Paragraph(inline) => text_run(ui, inline, style, false),
            Block::Heading(level, inline) => {
                let size = match level {
                    1 => FONT_LG,
                    2 => FONT_MD,
                    _ => style.size,
                };
                if index > 0 {
                    ui.add_space(SPACE_1);
                }
                text_run(ui, inline, Style { size, ..style }, true);
            }
            Block::Code { language, text } => code_block(ui, id, language, text, style),
            Block::Quote(inner) => {
                let response = ui
                    .horizontal(|ui| {
                        ui.add_space(SPACE_3);
                        ui.vertical(|ui| {
                            draw_blocks(
                                ui,
                                id,
                                inner,
                                Style {
                                    color: TEXT2,
                                    ..style
                                },
                            )
                        });
                    })
                    .response;
                let rect = response.rect;
                ui.painter().vline(
                    rect.left() + 2.0,
                    rect.y_range(),
                    Stroke::new(2.0, BORDER_STRONG),
                );
            }
            Block::List { start, items } => {
                for (n, item) in items.iter().enumerate() {
                    let marker = match start {
                        Some(first) => format!("{}.", first + n as u64),
                        None => "•".to_string(),
                    };
                    ui.horizontal_top(|ui| {
                        let width = if start.is_some() { 22.0 } else { 14.0 };
                        ui.allocate_ui_with_layout(
                            egui::vec2(width, 0.0),
                            egui::Layout::right_to_left(egui::Align::Min),
                            |ui| {
                                ui.label(
                                    egui::RichText::new(marker)
                                        .font(sans(style.size))
                                        .color(TEXT2),
                                );
                            },
                        );
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = SPACE_1;
                            draw_blocks(ui, id.with(n), item, style);
                        });
                    });
                }
            }
            Block::Table { head, rows } => table(ui, id, head, rows, style),
            Block::Rule => {
                ui.separator();
            }
        }
    }
}

/// One run of styled text, selectable, its links clickable.
fn text_run(ui: &mut Ui, inline: &Inline, style: Style, heading: bool) {
    let (job, links) = layout_job(inline, style, heading, ui.available_width());
    let (pos, galley, response) = egui::Label::new(job)
        .selectable(true)
        .wrap()
        .layout_in_ui(ui);
    if !ui.is_rect_visible(response.rect) {
        return;
    }
    let over_link = response
        .hover_pos()
        .and_then(|p| link_at(&galley, &links, p - pos));
    egui::text_selection::LabelSelectionState::label_text_selection(
        ui,
        &response,
        pos,
        galley.clone(),
        style.color,
        Stroke::NONE,
    );
    if let Some(url) = over_link {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        if response.clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(url));
        }
        response.on_hover_text(url);
    }
}

/// The link under `at` (relative to the galley's corner), if any.
fn link_at<'a>(
    galley: &Arc<Galley>,
    links: &'a [(usize, usize, String)],
    at: egui::Vec2,
) -> Option<&'a str> {
    if !galley.rect.contains(egui::pos2(at.x, at.y)) {
        return None;
    }
    let index = galley.cursor_from_pos(at).index.0;
    links
        .iter()
        .find(|(from, to, _)| (*from..*to).contains(&index))
        .map(|(_, _, url)| url.as_str())
}

/// The run as a layout job, and where its links sit in it (char ranges).
fn layout_job(
    inline: &Inline,
    style: Style,
    heading: bool,
    width: f32,
) -> (LayoutJob, Vec<(usize, usize, String)>) {
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    let mut links = Vec::new();
    let mut chars = 0usize;
    for span in inline {
        let font: FontId = if span.code {
            mono(style.size - 1.0)
        } else if span.strong || heading {
            sans_semibold(style.size)
        } else {
            sans(style.size)
        };
        let color = if span.link.is_some() {
            ACCENT
        } else {
            style.color
        };
        let format = TextFormat {
            font_id: font,
            color,
            background: if span.code { BG3 } else { Color32::TRANSPARENT },
            italics: span.emphasis,
            strikethrough: if span.strike {
                Stroke::new(1.0, color)
            } else {
                Stroke::NONE
            },
            underline: if span.link.is_some() {
                Stroke::new(1.0, ACCENT)
            } else {
                Stroke::NONE
            },
            ..Default::default()
        };
        let count = span.text.chars().count();
        if let Some(url) = &span.link {
            links.push((chars, chars + count, url.clone()));
        }
        chars += count;
        job.append(&span.text, 0.0, format);
    }
    (job, links)
}

fn code_block(ui: &mut Ui, id: egui::Id, language: &str, text: &str, style: Style) {
    egui::Frame::new()
        .fill(BG2)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(RADIUS_SM as u8)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(language)
                        .font(mono(FONT_XS))
                        .color(TEXT3),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(ui, "copy", "Copy the code").clicked() {
                        ui.ctx().copy_text(text.to_string());
                    }
                });
            });
            egui::ScrollArea::horizontal()
                .id_salt(id)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(text)
                                .font(mono(style.size - 1.0))
                                .color(TEXT1),
                        )
                        .selectable(true)
                        .extend(),
                    );
                });
        });
}

fn table(ui: &mut Ui, id: egui::Id, head: &[Inline], rows: &[Vec<Inline>], style: Style) {
    egui::ScrollArea::horizontal()
        .id_salt(id.with("table"))
        .auto_shrink([false, true])
        .show(ui, |ui| {
            egui::Grid::new(id.with("grid"))
                .striped(true)
                .spacing(egui::vec2(SPACE_3, SPACE_1))
                .show(ui, |ui| {
                    for cell in head {
                        cell_label(ui, cell, style, true);
                    }
                    ui.end_row();
                    for row in rows {
                        for cell in row {
                            cell_label(ui, cell, style, false);
                        }
                        ui.end_row();
                    }
                });
        });
}

fn cell_label(ui: &mut Ui, cell: &Inline, style: Style, head: bool) {
    let (job, _) = layout_job(cell, style, head, f32::INFINITY);
    ui.add(egui::Label::new(job).selectable(true).extend());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(inline: &Inline) -> String {
        inline.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn paragraphs_headings_and_styles() {
        let blocks = parse("# Title\n\nSome **bold** and *soft* and `code`.");
        assert_eq!(blocks.len(), 2);
        let Block::Heading(1, title) = &blocks[0] else {
            panic!("{blocks:?}")
        };
        assert_eq!(plain(title), "Title");
        let Block::Paragraph(spans) = &blocks[1] else {
            panic!("{blocks:?}")
        };
        assert_eq!(plain(spans), "Some bold and soft and code.");
        assert!(spans.iter().any(|s| s.strong && s.text == "bold"));
        assert!(spans.iter().any(|s| s.emphasis && s.text == "soft"));
        assert!(spans.iter().any(|s| s.code && s.text == "code"));
    }

    #[test]
    fn tight_and_nested_lists_keep_their_items() {
        let blocks = parse("1. one\n2. two\n   - inner\n3. three\n");
        let Block::List { start, items } = &blocks[0] else {
            panic!("{blocks:?}")
        };
        assert_eq!(*start, Some(1));
        assert_eq!(items.len(), 3);
        let Block::Paragraph(two) = &items[1][0] else {
            panic!("{items:?}")
        };
        assert_eq!(plain(two), "two");
        assert!(matches!(&items[1][1], Block::List { start: None, items } if items.len() == 1));
    }

    #[test]
    fn a_fence_keeps_its_text_and_language() {
        let blocks = parse("```rust\nfn main() {}\n```\nafter");
        assert_eq!(
            blocks[0],
            Block::Code {
                language: "rust".into(),
                text: "fn main() {}".into()
            }
        );
        assert!(matches!(&blocks[1], Block::Paragraph(_)));
    }

    #[test]
    fn an_unclosed_fence_while_streaming_is_still_code() {
        let blocks = parse("Here:\n```\nlet x = 1;\n");
        assert!(matches!(&blocks[1], Block::Code { text, .. } if text == "let x = 1;"));
    }

    #[test]
    fn links_tables_quotes_and_rules() {
        let blocks = parse(
            "See [the docs](https://x.y).\n\n> quoted\n\n---\n\n| a | b |\n|---|---|\n| 1 | 2 |\n",
        );
        let Block::Paragraph(spans) = &blocks[0] else {
            panic!("{blocks:?}")
        };
        assert!(
            spans
                .iter()
                .any(|s| s.text == "the docs" && s.link.as_deref() == Some("https://x.y"))
        );
        assert!(matches!(&blocks[1], Block::Quote(inner) if inner.len() == 1));
        assert_eq!(blocks[2], Block::Rule);
        let Block::Table { head, rows } = &blocks[3] else {
            panic!("{blocks:?}")
        };
        assert_eq!(head.iter().map(plain).collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(rows[0].iter().map(plain).collect::<Vec<_>>(), ["1", "2"]);
    }

    /// Everything the renderer draws, through a real frame with the
    /// design's fonts, at a width that wraps and one that barely fits.
    #[test]
    fn a_long_mixed_message_draws() {
        let text = "# Plan\n\nSome **bold**, *soft*, ~~gone~~, `code` and [a link](https://x.y).\n\n\
                    1. first\n2. second\n   - inner with `code`\n   - [x] done\n\n\
                    > a quote\n> over two lines\n\n---\n\n\
                    | col | other |\n|---|---|\n| 1 | a long cell that runs |\n\n\
                    ```rust\nfn main() { println!(\"a very long line that has to scroll sideways\"); }\n```\n\n\
                    ```\nunclosed";
        let ctx = egui::Context::default();
        crate::theme::apply_theme(&ctx);
        for width in [600.0, 40.0] {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 800.0),
                )),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                show(ui, egui::Id::new("m"), text, Style::default());
            });
            output.textures_delta.clear();
        }
    }

    #[test]
    fn a_link_s_chars_are_where_the_layout_puts_them() {
        let inline = vec![
            Span {
                text: "go ".into(),
                ..Span::default()
            },
            Span {
                text: "here".into(),
                link: Some("u".into()),
                ..Span::default()
            },
        ];
        let (job, links) = layout_job(&inline, Style::default(), false, 100.0);
        assert_eq!(job.text, "go here");
        assert_eq!(links, vec![(3, 7, "u".to_string())]);
    }
}
