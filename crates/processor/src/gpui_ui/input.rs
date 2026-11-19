use std::borrow::Cow;
use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, ContentMask, Context, CursorStyle, Element, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, FocusHandle, Focusable, GlobalElementId,
    InspectorElementId, KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels,
    Point, ScrollWheelEvent, ShapedLine, SharedString, Style, TextRun, UTF16Selection, Window,
    actions, div, fill, point, prelude::*, px, relative, size,
};
use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};
use zeroize::Zeroizing;

use super::Palette;

const INPUT: &str = "ScriptHubInput";
const MULTILINE: &str = "ScriptHubMultiline";
const EDIT_LIMIT: usize = 1024 * 1024;
const BULLET: &str = "•";

actions!(
    script_hub_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        SelectHome,
        SelectEnd,
        Up,
        Down,
        SelectUp,
        SelectDown,
        Paste,
        Cut,
        Copy,
        Newline,
        NextField,
        PreviousField,
    ]
);

pub(super) struct TextInput {
    value: Zeroizing<String>,
    placeholder: SharedString,
    masked: bool,
    rows: usize,
    starts: Vec<usize>,
    focus: FocusHandle,
    anchor: usize,
    cursor: usize,
    marked: Option<Range<usize>>,
    selecting: bool,
    reveal_cursor: bool,
    scroll_carry: f32,
    layout: Option<InputLayout>,
}

impl TextInput {
    pub(super) fn bind_keys(cx: &mut App) {
        let context = Some(INPUT);
        let multiline = Some(MULTILINE);
        let command = if cfg!(target_os = "macos") {
            "cmd"
        } else {
            "ctrl"
        };
        cx.bind_keys([
            KeyBinding::new("backspace", Backspace, context),
            KeyBinding::new("delete", Delete, context),
            KeyBinding::new("left", Left, context),
            KeyBinding::new("right", Right, context),
            KeyBinding::new("shift-left", SelectLeft, context),
            KeyBinding::new("shift-right", SelectRight, context),
            KeyBinding::new("home", Home, context),
            KeyBinding::new("end", End, context),
            KeyBinding::new("shift-home", SelectHome, context),
            KeyBinding::new("shift-end", SelectEnd, context),
            KeyBinding::new("up", Up, multiline),
            KeyBinding::new("down", Down, multiline),
            KeyBinding::new("shift-up", SelectUp, multiline),
            KeyBinding::new("shift-down", SelectDown, multiline),
            KeyBinding::new("enter", Newline, multiline),
            KeyBinding::new("tab", NextField, context),
            KeyBinding::new("shift-tab", PreviousField, context),
            KeyBinding::new(&format!("{command}-a"), SelectAll, context),
            KeyBinding::new(&format!("{command}-v"), Paste, context),
            KeyBinding::new(&format!("{command}-c"), Copy, context),
            KeyBinding::new(&format!("{command}-x"), Cut, context),
        ]);
    }

    pub(super) fn new(
        placeholder: impl Into<SharedString>,
        masked: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            value: Zeroizing::new(String::new()),
            placeholder: placeholder.into(),
            masked,
            rows: 1,
            starts: vec![0],
            focus: cx.focus_handle(),
            anchor: 0,
            cursor: 0,
            marked: None,
            selecting: false,
            reveal_cursor: true,
            scroll_carry: 0.0,
            layout: None,
        }
    }

    pub(super) fn multiline(
        placeholder: impl Into<SharedString>,
        visible_lines: usize,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            rows: visible_lines.max(2),
            ..Self::new(placeholder, false, cx)
        }
    }

    pub(super) fn value(&self) -> &str {
        &self.value
    }

    pub(super) fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.placeholder = placeholder.into();
        cx.notify();
    }

    pub(super) fn set_value(&mut self, value: impl Into<String>, cx: &mut Context<Self>) {
        let value = Zeroizing::new(value.into());
        self.value = match self.normalize(&value) {
            Cow::Borrowed(_) => value,
            Cow::Owned(text) => Zeroizing::new(text),
        };
        self.marked = None;
        self.layout = None;
        self.scroll_carry = 0.0;
        self.reindex_lines();
        self.move_to(self.value.len(), false, cx);
    }

    pub(super) fn clear(&mut self, cx: &mut Context<Self>) {
        self.set_value(String::new(), cx);
    }

    fn normalize<'a>(&self, value: &'a str) -> Cow<'a, str> {
        if self.rows == 1 && value.contains(['\r', '\n', '\t']) {
            return Cow::Owned(value.replace("\r\n", " ").replace(['\r', '\n', '\t'], " "));
        }
        Cow::Borrowed(value)
    }

    fn selected(&self) -> Range<usize> {
        self.anchor.min(self.cursor)..self.anchor.max(self.cursor)
    }

    fn move_to(&mut self, offset: usize, extend: bool, cx: &mut Context<Self>) {
        self.cursor = offset;
        if !extend {
            self.anchor = offset;
        }
        self.reveal_cursor = true;
        cx.notify();
    }

    fn previous(&self, offset: usize) -> usize {
        GraphemeCursor::new(offset, self.value.len(), true)
            .prev_boundary(&self.value, 0)
            .ok()
            .flatten()
            .unwrap_or(0)
    }

    fn next(&self, offset: usize) -> usize {
        GraphemeCursor::new(offset, self.value.len(), true)
            .next_boundary(&self.value, 0)
            .ok()
            .flatten()
            .unwrap_or(self.value.len())
    }

    fn horizontal(&mut self, right: bool, extend: bool, cx: &mut Context<Self>) {
        let selected = self.selected();
        let next = match (right, extend || selected.is_empty()) {
            (true, true) => self.next(self.cursor),
            (false, true) => self.previous(self.cursor),
            (true, false) => selected.end,
            (false, false) => selected.start,
        };
        self.move_to(next, extend, cx);
    }

    fn row_at(&self, offset: usize) -> usize {
        self.starts.partition_point(|start| *start <= offset) - 1
    }

    fn row_range(&self, row: usize) -> Range<usize> {
        let start = self.starts[row];
        let end = self
            .starts
            .get(row + 1)
            .map_or(self.value.len(), |next| next - 1);
        let end = if end > start && self.value.as_bytes()[end - 1] == b'\r' {
            end - 1
        } else {
            end
        };
        start..end
    }

    fn reindex_lines(&mut self) {
        self.starts.truncate(1);
        self.starts
            .extend(self.value.match_indices('\n').map(|(index, _)| index + 1));
    }

    fn line_edge(&mut self, end: bool, extend: bool, cx: &mut Context<Self>) {
        let range = self.row_range(self.row_at(self.cursor));
        self.move_to(if end { range.end } else { range.start }, extend, cx);
    }

    fn vertical(&mut self, delta: isize, extend: bool, cx: &mut Context<Self>) {
        let row = self.row_at(self.cursor);
        let target = row.saturating_add_signed(delta).min(self.starts.len() - 1);
        let from = self.row_range(row);
        let to = self.row_range(target);
        let column = self.value[from.start..self.cursor.min(from.end)]
            .graphemes(true)
            .count();
        let offset = self.value[to.clone()]
            .grapheme_indices(true)
            .nth(column)
            .map_or(to.end, |(index, _)| to.start + index);
        self.move_to(offset, extend, cx);
    }

    fn mouse_offset(&self, at: Point<Pixels>) -> usize {
        let Some(layout) = &self.layout else {
            return 0;
        };
        layout.offset_at(at)
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.selecting = true;
        self.move_to(self.mouse_offset(event.position), event.modifiers.shift, cx);
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.selecting {
            self.move_to(self.mouse_offset(event.position), true, cx);
        }
    }

    fn scroll(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(layout) = &mut self.layout else {
            return;
        };
        let delta = event.delta.pixel_delta(window.line_height()).y / window.line_height();
        let max_first = self.starts.len().saturating_sub(self.rows);
        if delta == 0.0
            || (delta > 0.0 && layout.first == 0)
            || (delta < 0.0 && layout.first == max_first)
        {
            self.scroll_carry = 0.0;
            return;
        }
        self.scroll_carry += delta;
        let rows = self.scroll_carry.trunc();
        self.scroll_carry -= rows;
        let next = layout
            .first
            .saturating_add_signed(-(rows as isize))
            .min(max_first);
        if layout.first != next {
            layout.first = next;
            self.reveal_cursor = false;
            cx.notify();
        }
        cx.stop_propagation();
    }

    fn paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let text = Zeroizing::new(text);
            self.replace_text_in_range(None, &text, window, cx);
        }
    }

    fn copy(&self, cx: &mut Context<Self>) {
        let selection = self.selected();
        if !self.masked && !selection.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(self.value[selection].to_owned()));
        }
    }

    fn delete(&mut self, right: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.anchor == self.cursor && self.marked.is_none() {
            self.horizontal(right, true, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn changed(&mut self, range: Range<usize>, value: &str) -> Option<Range<usize>> {
        let value = self.normalize(value);
        let final_len = self.value.len() - range.len() + value.len();
        if final_len > EDIT_LIMIT && final_len >= self.value.len() {
            return None;
        }
        let mut edited = Zeroizing::new(String::with_capacity(final_len));
        edited.push_str(&self.value[..range.start]);
        edited.push_str(&value);
        edited.push_str(&self.value[range.end..]);
        let inserted = range.start..range.start + value.len();
        self.value = edited;
        self.reindex_lines();
        Some(inserted)
    }

    fn replacement(&self, range: Option<Range<usize>>) -> Range<usize> {
        range
            .map(|range| bytes_for_utf16(&self.value, range))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selected())
    }
}

fn bytes_for_utf16(value: &str, range: Range<usize>) -> Range<usize> {
    let byte = |limit| {
        let mut units = 0;
        for (index, character) in value.char_indices() {
            if units >= limit {
                return index;
            }
            units += character.len_utf16();
        }
        value.len()
    };
    byte(range.start)..byte(range.end)
}

fn utf16_for_bytes(value: &str, range: Range<usize>) -> Range<usize> {
    let units = |index| value[..index].encode_utf16().count();
    units(range.start)..units(range.end)
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let bytes = bytes_for_utf16(&self.value, range);
        let adjusted = utf16_for_bytes(&self.value, bytes.clone());
        let text = if self.masked {
            BULLET.repeat(adjusted.len())
        } else {
            self.value[bytes].to_owned()
        };
        *actual = Some(adjusted);
        Some(text)
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: utf16_for_bytes(&self.value, self.selected()),
            reversed: self.anchor > self.cursor,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked
            .clone()
            .map(|range| utf16_for_bytes(&self.value, range))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        value: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(inserted) = self.changed(self.replacement(range), value) {
            self.marked = None;
            self.move_to(inserted.end, false, cx);
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        value: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(inserted) = self.changed(self.replacement(range), value) {
            let selection = selected.map_or(inserted.len()..inserted.len(), |range| {
                bytes_for_utf16(&self.value[inserted.clone()], range)
            });
            self.marked = (!inserted.is_empty()).then_some(inserted.clone());
            self.anchor = inserted.start + selection.start;
            self.move_to(inserted.start + selection.end, true, cx);
        }
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = bytes_for_utf16(&self.value, range);
        let layout = self.layout.as_ref()?;
        let row = self.row_at(range.start);
        let line = layout.lines.get(row.checked_sub(layout.first)?)?;
        Some(layout.highlight(line, range, row - layout.first))
    }

    fn character_index_for_point(
        &mut self,
        at: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let layout = self.layout.as_ref()?;
        if !layout.bounds.contains(&at) {
            return None;
        }
        Some(self.value[..layout.offset_at(at)].encode_utf16().count())
    }
}

fn mask_text(source: &str) -> (SharedString, Vec<usize>) {
    let offsets = source
        .grapheme_indices(true)
        .map(|(index, _)| index)
        .chain([source.len()])
        .collect::<Vec<_>>();
    (BULLET.repeat(offsets.len() - 1).into(), offsets)
}

struct VisualLine {
    bytes: Range<usize>,
    shaped: ShapedLine,
    original_offsets: Option<Vec<usize>>,
}

impl VisualLine {
    fn x(&self, offset: usize) -> Pixels {
        let offset = offset.clamp(self.bytes.start, self.bytes.end) - self.bytes.start;
        let display = self.original_offsets.as_ref().map_or(offset, |offsets| {
            offsets
                .partition_point(|index| *index <= offset)
                .saturating_sub(1)
                * BULLET.len()
        });
        self.shaped.x_for_index(display)
    }

    fn offset(&self, x: Pixels) -> usize {
        if self.bytes.is_empty() {
            return self.bytes.start;
        }
        let display = self.shaped.closest_index_for_x(x);
        let offset = self
            .original_offsets
            .as_ref()
            .map_or(display, |offsets| offsets[display / BULLET.len()]);
        self.bytes.start + offset
    }
}

struct InputLayout {
    bounds: Bounds<Pixels>,
    lines: Vec<VisualLine>,
    first: usize,
    offset_x: Pixels,
    row_height: Pixels,
}

impl InputLayout {
    fn origin(&self, row: usize) -> Point<Pixels> {
        point(
            self.bounds.left() - self.offset_x,
            self.bounds.top() + self.row_height * row as f32,
        )
    }

    fn offset_at(&self, at: Point<Pixels>) -> usize {
        let row = ((at.y - self.bounds.top()) / self.row_height) as usize;
        let row = row.min(self.lines.len() - 1);
        self.lines[row].offset(at.x - self.origin(row).x)
    }

    fn highlight(&self, line: &VisualLine, range: Range<usize>, row: usize) -> Bounds<Pixels> {
        let origin = self.origin(row);
        let start = line.x(range.start);
        let end = line.x(range.end);
        Bounds::new(
            point(origin.x + start.min(end), origin.y + px(2.)),
            size((end - start).abs().max(px(2.)), self.row_height - px(4.)),
        )
    }
}

struct InputSurface(Entity<TextInput>);

impl IntoElement for InputSurface {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for InputSurface {
    type RequestLayoutState = ();
    type PrepaintState = Option<InputLayout>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = (window.line_height() * self.0.read(cx).rows as f32).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Option<InputLayout> {
        let input = self.0.read(cx);
        let cursor_row = input.row_at(input.cursor);
        let mut first = input.layout.as_ref().map_or(0, |layout| layout.first);
        if input.reveal_cursor {
            first = first
                .min(cursor_row)
                .max((cursor_row + 1).saturating_sub(input.rows));
        }
        first = first.min(input.starts.len().saturating_sub(input.rows));
        let style = window.text_style();
        let colors = Palette::for_window(window, cx);
        let mut lines = Vec::new();
        let end = (first + input.rows).min(input.starts.len());
        for row in first..end {
            let bytes = input.row_range(row);
            let source = &input.value[bytes.clone()];
            let (visible, original_offsets) = if input.value.is_empty() {
                (input.placeholder.clone(), None)
            } else if input.masked {
                let (encoded, offsets) = mask_text(source);
                (encoded, Some(offsets))
            } else {
                (source.to_owned().into(), None)
            };
            let color = if input.value.is_empty() {
                colors.muted.into()
            } else {
                style.color
            };
            let run = TextRun {
                len: visible.len(),
                color,
                font: style.font(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let mut line = VisualLine {
                bytes,
                shaped: window.text_system().shape_line(
                    visible,
                    style.font_size.to_pixels(window.rem_size()),
                    &[run],
                    None,
                ),
                original_offsets,
            };
            // Empty source uses placeholder glyphs while any hit still maps to zero.
            if line.bytes.is_empty() && input.masked {
                line.original_offsets = None;
            }
            lines.push(line);
        }
        let mut offset_x = input
            .layout
            .as_ref()
            .map_or(px(0.), |layout| layout.offset_x);
        if input.reveal_cursor
            && let Some(line) = cursor_row
                .checked_sub(first)
                .and_then(|index| lines.get(index))
        {
            let cursor_x = line.x(input.cursor);
            let visible_edge = (bounds.size.width - px(8.)).max(px(1.));
            offset_x = offset_x
                .min(cursor_x)
                .max(cursor_x - visible_edge)
                .max(px(0.));
        }
        Some(InputLayout {
            bounds,
            lines,
            first,
            offset_x,
            row_height: window.line_height(),
        })
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        layout: &mut Option<InputLayout>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(layout) = layout.take() else {
            return;
        };
        let input = self.0.read(cx);
        let selection = input.selected();
        let marked_range = input.marked.clone();
        let caret = input.cursor;
        let cursor_row = input.row_at(caret);
        let focus = input.focus.clone();
        let focused = focus.is_focused(window);
        let colors = Palette::for_window(window, cx);
        window.handle_input(&focus, ElementInputHandler::new(bounds, self.0.clone()), cx);
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for (row, line) in layout.lines.iter().enumerate() {
                let active_row = row + layout.first;
                if focused && selection.start <= line.bytes.end && selection.end > line.bytes.start
                {
                    let quad = layout.highlight(line, selection.clone(), row);
                    window.paint_quad(fill(quad, gpui::Hsla::from(colors.accent).opacity(0.2)));
                }
                let origin = layout.origin(row);
                let _ = line.shaped.paint(origin, layout.row_height, window, cx);
                if let Some(marked) = &marked_range
                    && marked.start <= line.bytes.end
                    && marked.end > line.bytes.start
                {
                    let underline = layout.highlight(line, marked.clone(), row);
                    window.paint_quad(fill(
                        Bounds::new(
                            point(underline.left(), underline.bottom() - px(2.)),
                            size(underline.size.width, px(1.)),
                        ),
                        colors.accent,
                    ));
                }
                if focused && selection.is_empty() && active_row == cursor_row {
                    let caret = layout.highlight(line, caret..caret, row);
                    window.paint_quad(fill(caret, colors.accent));
                }
            }
        });
        self.0.update(cx, |input, _| {
            input.layout = Some(layout);
            input.reveal_cursor = false;
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = Palette::for_window(window, cx);
        let focus = self.focus.is_focused(window);
        div()
            .w_full()
            .track_focus(&self.focus)
            .tab_index(0)
            .key_context(if self.rows > 1 {
                "ScriptHubInput ScriptHubMultiline"
            } else {
                INPUT
            })
            .cursor(CursorStyle::IBeam)
            .bg(colors.surface)
            .text_color(colors.ink)
            .border_1()
            .border_color(if focus { colors.accent } else { colors.border })
            .rounded_md()
            .px_3()
            .py_2()
            .line_height(px(22.))
            .text_size(px(14.))
            .on_action(cx.listener(|this, _: &Backspace, w, cx| this.delete(false, w, cx)))
            .on_action(cx.listener(|this, _: &Delete, w, cx| this.delete(true, w, cx)))
            .on_action(cx.listener(|this, _: &Left, _, cx| this.horizontal(false, false, cx)))
            .on_action(cx.listener(|this, _: &Right, _, cx| this.horizontal(true, false, cx)))
            .on_action(cx.listener(|this, _: &SelectLeft, _, cx| this.horizontal(false, true, cx)))
            .on_action(cx.listener(|this, _: &SelectRight, _, cx| this.horizontal(true, true, cx)))
            .on_action(cx.listener(|this, _: &Home, _, cx| this.line_edge(false, false, cx)))
            .on_action(cx.listener(|this, _: &End, _, cx| this.line_edge(true, false, cx)))
            .on_action(cx.listener(|this, _: &SelectHome, _, cx| this.line_edge(false, true, cx)))
            .on_action(cx.listener(|this, _: &SelectEnd, _, cx| this.line_edge(true, true, cx)))
            .on_action(cx.listener(|this, _: &Up, _, cx| this.vertical(-1, false, cx)))
            .on_action(cx.listener(|this, _: &Down, _, cx| this.vertical(1, false, cx)))
            .on_action(cx.listener(|this, _: &SelectUp, _, cx| this.vertical(-1, true, cx)))
            .on_action(cx.listener(|this, _: &SelectDown, _, cx| this.vertical(1, true, cx)))
            .on_action(cx.listener(|this, _: &SelectAll, _, cx| {
                this.anchor = 0;
                this.move_to(this.value.len(), true, cx);
            }))
            .on_action(cx.listener(|this, _: &Newline, window, cx| {
                this.replace_text_in_range(None, "\n", window, cx);
            }))
            .on_action(cx.listener(|this, _: &Paste, window, cx| this.paste(window, cx)))
            .on_action(cx.listener(|this, _: &Copy, _, cx| this.copy(cx)))
            .on_action(cx.listener(|this, _: &Cut, window, cx| {
                if !this.selected().is_empty() {
                    this.copy(cx);
                    this.replace_text_in_range(None, "", window, cx);
                }
            }))
            .on_action(cx.listener(|_, _: &NextField, window, _| window.focus_next()))
            .on_action(cx.listener(|_, _: &PreviousField, window, _| window.focus_prev()))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.selecting = false),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.selecting = false),
            )
            .on_scroll_wheel(cx.listener(Self::scroll))
            .child(InputSurface(cx.entity()))
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_display_contains_only_mask_glyphs() {
        let value = "e\u{301}🔐文";
        let (display, offsets) = mask_text(value);
        assert_eq!(display.as_ref(), "•••");
        assert_eq!(offsets, [0, 3, 7, value.len()]);
    }

    #[test]
    fn ime_offsets_round_trip_surrogate_pairs() {
        let value = "a🔐文";
        assert_eq!(bytes_for_utf16(value, 1..3), 1..5);
        assert_eq!(utf16_for_bytes(value, 1..5), 1..3);
        assert_eq!(bytes_for_utf16(value, 3..8), 5..value.len());
    }
}
