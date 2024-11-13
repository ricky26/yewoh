use std::borrow::Cow;
use std::fmt::{Display, Formatter, Write};

use glam::{ivec2, IVec2};
use indexmap::IndexSet;
use yewoh::EntityId;
use yewoh::protocol::GumpLayout;

#[derive(Clone, Copy, Debug, Default)]
pub struct GumpPadding {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl From<i32> for GumpPadding {
    fn from(value: i32) -> Self {
        GumpPadding {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }
}

impl GumpPadding {
    pub fn left(left: i32) -> GumpPadding {
        GumpPadding { left, ..Default::default() }
    }

    pub fn top(top: i32) -> GumpPadding {
        GumpPadding { top, ..Default::default() }
    }

    pub fn right(right: i32) -> GumpPadding {
        GumpPadding { right, ..Default::default() }
    }

    pub fn bottom(bottom: i32) -> GumpPadding {
        GumpPadding { bottom, ..Default::default() }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GumpRect {
    pub min: IVec2,
    pub max: IVec2,
}

impl GumpRect {
    pub fn is_empty(&self) -> bool {
        let size = self.size();
        (size.x < 0) || (size.y < 0)
    }

    pub fn size(&self) -> IVec2 {
        self.max - self.min
    }

    pub fn new(min: IVec2, max: IVec2) -> GumpRect {
        GumpRect { min, max }
    }

    pub fn from_zero(max: IVec2) -> GumpRect {
        GumpRect { min: IVec2::ZERO, max }
    }

    pub fn with_coords(x0: i32, y0: i32, x1: i32, y1: i32) -> GumpRect {
        GumpRect {
            min: ivec2(x0, y0),
            max: ivec2(x1, y1),
        }.sanitise()
    }

    pub fn with_coords_size(x: i32, y: i32, w: i32, h: i32) -> GumpRect {
        let min = ivec2(x, y);
        let max = min + ivec2(w, h);
        GumpRect { min, max }
    }

    pub fn sanitise(self) -> Self {
        let min = self.min;
        let max = self.max.max(min);
        GumpRect { min, max }
    }

    pub fn take_top(&mut self, amount: i32) -> Self {
        let amount = amount.min(self.size().y);
        let result = GumpRect { min: self.min, max: ivec2(self.max.x, self.min.y + amount) };
        self.min.y += amount;
        result
    }

    pub fn take_left(&mut self, amount: i32) -> Self {
        let amount = amount.min(self.size().x);
        let result = GumpRect { min: self.min, max: ivec2(self.max.x + amount, self.min.y) };
        self.min.y += amount;
        result
    }

    pub fn take_bottom(&mut self, amount: i32) -> Self {
        let amount = amount.min(self.size().y);
        let result = GumpRect { min: ivec2(self.min.x, self.max.y - amount), max: self.max };
        self.max.y -= amount;
        result
    }

    pub fn take_right(&mut self, amount: i32) -> Self {
        let amount = amount.min(self.size().x);
        let result = GumpRect { min: ivec2(self.max.x - amount, self.min.y), max: self.max };
        self.max.x -= amount;
        result
    }

    pub fn with_padding(self, padding: impl Into<GumpPadding>) -> Self {
        let padding = padding.into();
        GumpRect {
            min: ivec2(self.min.x + padding.left, self.min.y + padding.top),
            max: ivec2(self.max.x - padding.right, self.max.y - padding.bottom),
        }.sanitise()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GumpTextId(pub u32);

impl Display for GumpTextId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Default)]
pub struct GumpText {
    text: IndexSet<String>,
}

impl GumpText {
    pub fn new() -> GumpText {
        Default::default()
    }

    pub fn intern<'a>(&mut self, text: impl Into<Cow<'a, str>>) -> GumpTextId {
        let text = text.into();
        let id = if let Some(existing) = self.text.get_index_of(text.as_ref()) {
            existing
        } else {
            self.text.insert_full(text.into_owned()).0
        };
        GumpTextId(id as u32)
    }
}

#[derive(Clone, Debug)]
pub enum GumpButtonAction {
    GoToPage(usize),
    Close(usize),
}

#[derive(Debug, Clone, Default)]
pub struct GumpBuilder {
    layout: String,
}

impl GumpBuilder {
    pub fn new() -> GumpBuilder {
        Self::default()
    }

    pub fn into_layout(self, text: GumpText) -> GumpLayout {
        GumpLayout {
            layout: self.layout,
            text: text.text.into_iter().collect()
        }
    }

    pub fn mark_no_close(&mut self) -> &mut Self {
        write!(&mut self.layout, "{{ noclose }}").unwrap();
        self
    }

    pub fn mark_no_dispose(&mut self) -> &mut Self {
        write!(&mut self.layout, "{{ nodispose }}").unwrap();
        self
    }

    pub fn mark_no_move(&mut self) -> &mut Self {
        write!(&mut self.layout, "{{ nomove }}").unwrap();
        self
    }

    pub fn override_gump_id(&mut self, id: u32) -> &mut Self {
        write!(&mut self.layout, "{{ mastergump {} }}", id).unwrap();
        self
    }

    pub fn add_tooltip_localised(&mut self, text_id: u32, args: &str) -> &mut Self {
        write!(&mut self.layout, "{{ tooltip {} @{}@ }}", text_id, args).unwrap();
        self
    }

    pub fn add_page(&mut self, page: usize) -> &mut Self {
        write!(&mut self.layout, "{{ page {} }}", page).unwrap();
        self
    }

    pub fn start_group(&mut self, group_id: usize) -> &mut Self {
        write!(&mut self.layout, "{{ group {} }}", group_id).unwrap();
        self
    }

    pub fn end_group(&mut self) -> &mut Self {
        write!(&mut self.layout, "{{ group }}").unwrap();
        self
    }

    pub fn add_alpha_cutout(&mut self, rect: GumpRect) -> &mut Self {
        let min = rect.min;
        let size = rect.size();
        write!(&mut self.layout, "{{ checkertrans {} {} {} {} }}", min.x, min.y, size.x, size.y).unwrap();
        self
    }

    pub fn add_image(&mut self, image_id: u16, position: IVec2) -> &mut Self {
        write!(&mut self.layout, "{{ gumppic {} {} {} }}", position.x, position.y, image_id)
            .unwrap();
        self
    }

    pub fn add_image_hue(&mut self, image_id: u16, hue: u16, position: IVec2) -> &mut Self {
        write!(&mut self.layout, "{{ gumppic {} {} {} hue={} }}", position.x, position.y, image_id, hue)
            .unwrap();
        self
    }

    pub fn add_image_sliced(&mut self, image_id: u16, rect: GumpRect) -> &mut Self {
        let min = rect.min;
        let size = rect.size();
        write!(&mut self.layout, "{{ resizepic {} {} {} {} {} }}", min.x, min.y, image_id, size.x, size.y)
            .unwrap();
        self
    }

    pub fn add_image_tiled(&mut self, image_id: u16, rect: GumpRect) -> &mut Self {
        let min = rect.min;
        let size = rect.size();
        write!(
            &mut self.layout,
            "{{ gumppictiled {} {} {} {} {} }}",
            min.x,
            min.y,
            size.x,
            size.y,
            image_id,
        ).unwrap();
        self
    }

    pub fn add_tile_image(&mut self, graphic_id: u16, position: IVec2) -> &mut Self {
        write!(&mut self.layout, "{{ tilepic {} {} {} }}", position.x, position.y, graphic_id)
            .unwrap();
        self
    }

    pub fn add_tile_image_hue(&mut self, graphic_id: u16, hue: u16, position: IVec2) -> &mut Self {
        write!(&mut self.layout, "{{ tilepic {} {} {} {} }}", position.x, position.y, graphic_id, hue)
            .unwrap();
        self
    }

    pub fn add_item_property(&mut self, entity_id: EntityId) -> &mut Self {
        write!(&mut self.layout, "{{ itemproperty {} }}", entity_id.as_u32())
            .unwrap();
        self
    }

    pub fn add_sprite(&mut self, image_id: u16, rect: GumpRect, sprite_offset: IVec2) -> &mut Self {
        let min = rect.min;
        let size = rect.size();
        write!(
            &mut self.layout,
            "{{ picinpic {} {} {} {} {} {} {} }}",
            min.x,
            min.y,
            image_id,
            sprite_offset.x,
            sprite_offset.y,
            size.x,
            size.y,
        ).unwrap();
        self
    }

    pub fn add_text(&mut self, intern_id: GumpTextId, hue: u16, position: IVec2) -> &mut Self {
        write!(&mut self.layout, "{{ text {} {} {} {} }}", position.x, position.y, hue, intern_id)
            .unwrap();
        self
    }

    pub fn add_text_cropped(&mut self, intern_id: GumpTextId, hue: u16, rect: GumpRect) -> &mut Self {
        let min = rect.min;
        let size = rect.size();
        write!(
            &mut self.layout,
            "{{ croppedtext {} {} {} {} {} {} }}",
            min.x,
            min.y,
            size.x,
            size.y,
            hue,
            intern_id,
        ).unwrap();
        self
    }

    pub fn add_text_entry(
        &mut self,
        id: u32,
        default_intern_id: GumpTextId,
        hue: u16,
        rect: GumpRect,
    ) -> &mut Self {
        let min = rect.min;
        let size = rect.size();
        write!(
            &mut self.layout,
            "{{ textentry {} {} {} {} {} {} {} }}",
            min.x,
            min.y,
            size.x,
            size.y,
            hue,
            id,
            default_intern_id,
        ).unwrap();
        self
    }

    pub fn add_text_entry_limited(
        &mut self,
        id: u32,
        max_length: usize,
        default_intern_id: GumpTextId,
        hue: u16,
        rect: GumpRect,
    ) -> &mut Self {
        let min = rect.min;
        let size = rect.size();
        write!(
            &mut self.layout,
            "{{ textentrylimited {} {} {} {} {} {} {} {} }}",
            min.x,
            min.y,
            size.x,
            size.y,
            hue,
            id,
            default_intern_id,
            max_length,
        ).unwrap();
        self
    }

    pub fn add_button(
        &mut self,
        up_texture_id: u16,
        down_texture_id: u16,
        action: GumpButtonAction,
        position: IVec2,
    ) -> &mut Self {
        let (kind, page_id, button_id) = match action {
            GumpButtonAction::GoToPage(index) => (0, index, 0),
            GumpButtonAction::Close(response) => (1, 0, response),
        };
        write!(
            &mut self.layout,
            "{{ button {} {} {} {} {} {} {} }}",
            position.x,
            position.y,
            up_texture_id,
            down_texture_id,
            kind,
            page_id,
            button_id,
        ).unwrap();
        self
    }

    pub fn add_page_button(
        &mut self,
        up_texture_id: u16,
        down_texture_id: u16,
        page_id: usize,
        position: IVec2,
    ) -> &mut Self {
        self.add_button(
            up_texture_id,
            down_texture_id,
            GumpButtonAction::GoToPage(page_id),
            position,
        )
    }

    pub fn add_close_button(
        &mut self,
        up_texture_id: u16,
        down_texture_id: u16,
        result: usize,
        position: IVec2,
    ) -> &mut Self {
        self.add_button(
            up_texture_id,
            down_texture_id,
            GumpButtonAction::Close(result),
            position,
        )
    }

    pub fn add_tile_button(
        &mut self,
        up_texture_id: u16,
        down_texture_id: u16,
        graphic_id: u16,
        hue: u16,
        action: GumpButtonAction,
        position: IVec2,
        tile_offset: IVec2,
    ) -> &mut Self {
        let (kind, page_id, button_id) = match action {
            GumpButtonAction::GoToPage(index) => (0, index, 0),
            GumpButtonAction::Close(response) => (1, 0, response),
        };
        write!(
            &mut self.layout,
            "{{ buttontileart {} {} {} {} {} {} {} {} {} {} {} }}",
            position.x,
            position.y,
            up_texture_id,
            down_texture_id,
            kind,
            page_id,
            button_id,
            graphic_id,
            hue,
            tile_offset.x,
            tile_offset.y,
        ).unwrap();
        self
    }

    pub fn add_page_tile_button(
        &mut self,
        up_texture_id: u16,
        down_texture_id: u16,
        graphic_id: u16,
        hue: u16,
        page_id: usize,
        position: IVec2,
        tile_offset: IVec2,
    ) -> &mut Self {
        self.add_tile_button(
            up_texture_id,
            down_texture_id,
            graphic_id,
            hue,
            GumpButtonAction::GoToPage(page_id),
            position,
            tile_offset,
        )
    }

    pub fn add_close_tile_button(
        &mut self,
        up_texture_id: u16,
        down_texture_id: u16,
        graphic_id: u16,
        hue: u16,
        result: usize,
        position: IVec2,
        tile_offset: IVec2,
    ) -> &mut Self {
        self.add_tile_button(
            up_texture_id,
            down_texture_id,
            graphic_id,
            hue,
            GumpButtonAction::Close(result),
            position,
            tile_offset,
        )
    }

    pub fn add_checkbox(
        &mut self,
        off_image_id: u16,
        on_image_id: u16,
        on: bool,
        switch_id: u32,
        position: IVec2,
    ) -> &mut Self {
        let state = if on { 1 } else { 0 };
        write!(
            &mut self.layout,
            "{{ checkbox {} {} {} {} {} {} }}",
            position.x,
            position.y,
            off_image_id,
            on_image_id,
            state,
            switch_id,
        ).unwrap();
        self
    }

    pub fn add_radio(
        &mut self,
        off_image_id: u16,
        on_image_id: u16,
        on: bool,
        switch_id: u32,
        position: IVec2,
    ) -> &mut Self {
        let state = if on { 1 } else { 0 };
        write!(
            &mut self.layout,
            "{{ radio {} {} {} {} {} {} }}",
            position.x,
            position.y,
            off_image_id,
            on_image_id,
            state,
            switch_id,
        ).unwrap();
        self
    }

    pub fn add_html(
        &mut self,
        intern_id: GumpTextId,
        background: bool,
        scrollbar: bool,
        rect: GumpRect,
    ) -> &mut Self {
        let background = if background { 1 } else { 0 };
        let scrollbar = if scrollbar { 1 } else { 0 };
        let min = rect.min;
        let size = rect.size();

        write!(
            &mut self.layout,
            "{{ htmlgump {} {} {} {} {} {} {} }}",
            min.x,
            min.y,
            size.x,
            size.y,
            intern_id,
            background,
            scrollbar,
        ).unwrap();
        self
    }

    pub fn add_html_localised(
        &mut self,
        text_id: u32,
        background: bool,
        scrollbar: bool,
        rect: GumpRect,
    ) -> &mut Self {
        let background = if background { 1 } else { 0 };
        let scrollbar = if scrollbar { 1 } else { 0 };
        let min = rect.min;
        let size = rect.size();

        write!(
            &mut self.layout,
            "{{ xmfhtmlgump {} {} {} {} {} {} {} }}",
            min.x,
            min.y,
            size.x,
            size.y,
            text_id,
            background,
            scrollbar,
        ).unwrap();
        self
    }

    pub fn add_html_localised_parametric(
        &mut self,
        text_id: u32,
        params: &str,
        background: bool,
        scrollbar: bool,
        rect: GumpRect,
    ) -> &mut Self {
        let background = if background { 1 } else { 0 };
        let scrollbar = if scrollbar { 1 } else { 0 };
        let min = rect.min;
        let size = rect.size();

        write!(
            &mut self.layout,
            "{{ xmfhtmltok {} {} {} {} {} {} {} @{}@ }}",
            min.x,
            min.y,
            size.x,
            size.y,
            text_id,
            background,
            scrollbar,
            params,
        ).unwrap();
        self
    }

    pub fn add_html_colour(
        &mut self,
        text_id: u32,
        colour: u32,
        background: bool,
        scrollbar: bool,
        rect: GumpRect,
    ) -> &mut Self {
        let background = if background { 1 } else { 0 };
        let scrollbar = if scrollbar { 1 } else { 0 };
        let min = rect.min;
        let size = rect.size();

        write!(
            &mut self.layout,
            "{{ xmfhtmlgumpcolor {} {} {} {} {} {} {} {} }}",
            min.x,
            min.y,
            size.x,
            size.y,
            text_id,
            background,
            scrollbar,
            colour,
        ).unwrap();
        self
    }
}

pub trait IntoGumpTextId {
    fn into_gump_text_id(self, text: &mut GumpText) -> GumpTextId;
}

impl IntoGumpTextId for GumpTextId {
    fn into_gump_text_id(self, _text: &mut GumpText) -> GumpTextId {
        self
    }
}

impl IntoGumpTextId for &str {
    fn into_gump_text_id(self, text: &mut GumpText) -> GumpTextId {
        text.intern(self)
    }
}

impl IntoGumpTextId for String {
    fn into_gump_text_id(self, text: &mut GumpText) -> GumpTextId {
        text.intern(self)
    }
}

pub struct GumpRectLayout<'a> {
    builder: &'a mut GumpBuilder,
    text: &'a mut GumpText,
    rect: GumpRect,
}

impl<'a> GumpRectLayout<'a> {
    pub fn new(
        builder: &'a mut GumpBuilder,
        text: &'a mut GumpText,
        rect: GumpRect,
    ) -> GumpRectLayout<'a> {
        GumpRectLayout { builder, text, rect }
    }

    pub fn builder(&mut self) -> &mut GumpBuilder {
        self.builder
    }

    pub fn into_builder(self) -> (&'a mut GumpBuilder, &'a mut GumpText) {
        (self.builder, self.text)
    }

    pub fn rect(&self) -> GumpRect {
        self.rect
    }

    pub fn add_item_property(self, entity_id: EntityId) -> Self {
        self.builder.add_item_property(entity_id);
        self
    }

    pub fn background(self, f: impl FnOnce(GumpRectLayout)) -> Self {
        let child = GumpRectLayout {
            builder: self.builder,
            text: self.text,
            rect: self.rect,
        };
        f(child);
        self
    }

    pub fn left(mut self, amount: i32) -> GumpRectLayout<'a> {
        self.rect = self.rect.take_left(amount);
        self
    }

    pub fn top(mut self, amount: i32) -> GumpRectLayout<'a> {
        self.rect = self.rect.take_top(amount);
        self
    }

    pub fn right(mut self, amount: i32) -> GumpRectLayout<'a> {
        self.rect = self.rect.take_right(amount);
        self
    }

    pub fn bottom(mut self, amount: i32) -> GumpRectLayout<'a> {
        self.rect = self.rect.take_bottom(amount);
        self
    }

    pub fn with_padding(mut self, padding: impl Into<GumpPadding>) -> GumpRectLayout<'a> {
        self.rect = self.rect.with_padding(padding.into());
        self
    }

    pub fn into_box_layout(self, axis: impl Into<GumpBoxLayoutAxis>) -> GumpBoxLayout<'a> {
        GumpBoxLayout {
            builder: self.builder,
            text: self.text,
            rect: self.rect,
            axis: axis.into(),
        }
    }

    pub fn into_hbox(self) -> GumpBoxLayout<'a> {
        self.into_box_layout(GumpBoxLayoutAxis::Horizontal)
    }

    pub fn into_vbox(self) -> GumpBoxLayout<'a> {
        self.into_box_layout(GumpBoxLayoutAxis::Vertical)
    }

    pub fn into_paged(self, first_page: usize) -> GumpPagedLayout<'a> {
        GumpPagedLayout {
            builder: self.builder,
            text: self.text,
            rect: self.rect,
            page_id: first_page,
        }
    }

    pub fn alpha_cutout(self) {
        self.builder.add_alpha_cutout(self.rect);
    }

    pub fn image(self, image_id: u16) {
        self.builder.add_image(image_id, self.rect.min);
    }

    pub fn image_hue(self, image_id: u16, hue: u16) {
        self.builder.add_image_hue(image_id, hue, self.rect.min);
    }

    pub fn image_sliced(self, image_id: u16) {
        self.builder.add_image_sliced(image_id, self.rect);
    }

    pub fn image_tiled(self, image_id: u16) {
        self.builder.add_image_tiled(image_id, self.rect);
    }

    pub fn tile_image(self, graphic_id: u16) {
        self.builder.add_tile_image(graphic_id, self.rect.min);
    }

    pub fn tile_image_hue(self, graphic_id: u16, hue: u16) {
        self.builder.add_tile_image_hue(graphic_id, hue, self.rect.min);
    }

    pub fn sprite(self, image_id: u16, sprite_offset: IVec2) {
        self.builder.add_sprite(image_id, self.rect, sprite_offset);
    }

    pub fn text(self, text: impl IntoGumpTextId, hue: u16) {
        let intern_id = text.into_gump_text_id(self.text);
        self.builder.add_text(intern_id, hue, self.rect.min);
    }

    pub fn text_cropped(self, text: impl IntoGumpTextId, hue: u16) {
        let intern_id = text.into_gump_text_id(self.text);
        self.builder.add_text_cropped(intern_id, hue, self.rect);
    }

    pub fn text_entry(self, id: u32, default_text: impl IntoGumpTextId, hue: u16) {
        let intern_id = default_text.into_gump_text_id(self.text);
        self.builder.add_text_entry(id, intern_id, hue, self.rect);
    }

    pub fn text_entry_limited(
        self, id: u32, max_length: usize, default_text: impl IntoGumpTextId, hue: u16,
    ) {
        let intern_id = default_text.into_gump_text_id(self.text);
        self.builder.add_text_entry_limited(id, max_length, intern_id, hue, self.rect);
    }

    pub fn button(
        self,
        up_texture_id: u16,
        down_texture_id: u16,
        action: GumpButtonAction,
    ) {
        self.builder.add_button(up_texture_id, down_texture_id, action, self.rect.min);
    }

    pub fn page_button(
        self,
        up_texture_id: u16,
        down_texture_id: u16,
        page_id: usize,
    ) {
        self.builder.add_page_button(up_texture_id, down_texture_id, page_id, self.rect.min);
    }

    pub fn close_button(
        self,
        up_texture_id: u16,
        down_texture_id: u16,
        result: usize,
    ) {
        self.builder.add_close_button(up_texture_id, down_texture_id, result, self.rect.min);
    }

    pub fn tile_button(
        self,
        up_texture_id: u16,
        down_texture_id: u16,
        graphic_id: u16,
        hue: u16,
        action: GumpButtonAction,
        tile_offset: IVec2,
    ) {
        self.builder.add_tile_button(
            up_texture_id,
            down_texture_id,
            graphic_id,
            hue,
            action,
            self.rect.min,
            tile_offset,
        );
    }

    pub fn page_tile_button(
        self,
        up_texture_id: u16,
        down_texture_id: u16,
        graphic_id: u16,
        hue: u16,
        page_id: usize,
        tile_offset: IVec2,
    ) {
        self.builder.add_tile_button(
            up_texture_id,
            down_texture_id,
            graphic_id,
            hue,
            GumpButtonAction::GoToPage(page_id),
            self.rect.min,
            tile_offset,
        );
    }

    pub fn close_tile_button(
        self,
        up_texture_id: u16,
        down_texture_id: u16,
        graphic_id: u16,
        hue: u16,
        result: usize,
        tile_offset: IVec2,
    ) {
        self.builder.add_tile_button(
            up_texture_id,
            down_texture_id,
            graphic_id,
            hue,
            GumpButtonAction::Close(result),
            self.rect.min,
            tile_offset,
        );
    }

    pub fn checkbox(
        self,
        off_image_id: u16,
        on_image_id: u16,
        on: bool,
        switch_id: u32,
    ) {
        self.builder.add_checkbox(off_image_id, on_image_id, on, switch_id, self.rect.min);
    }

    pub fn radio(
        self,
        off_image_id: u16,
        on_image_id: u16,
        on: bool,
        switch_id: u32,
    ) {
        self.builder.add_radio(off_image_id, on_image_id, on, switch_id, self.rect.min);
    }

    pub fn html(self, text: impl IntoGumpTextId) {
        self.html_ex(text, false, false);
    }

    pub fn html_ex(
        self,
        text: impl IntoGumpTextId,
        background: bool,
        scrollbar: bool,
    ) {
        let intern_id = text.into_gump_text_id(self.text);
        self.builder.add_html(intern_id, background, scrollbar, self.rect);
    }

    pub fn html_localised(
        self,
        text_id: u32,
        background: bool,
        scrollbar: bool,
    ) {
        self.builder.add_html_localised(text_id, background, scrollbar, self.rect);
    }

    pub fn html_localised_parametric(
        self,
        text_id: u32,
        params: &str,
        background: bool,
        scrollbar: bool,
    ) {
        self.builder.add_html_localised_parametric(
            text_id, params, background, scrollbar, self.rect);
    }

    pub fn html_colour(
        self,
        text_id: u32,
        colour: u32,
        background: bool,
        scrollbar: bool,
    ) {
        self.builder.add_html_colour(
            text_id, colour, background, scrollbar, self.rect);
    }
}

#[derive(Clone, Copy, Debug)]
pub enum GumpBoxLayoutAxis {
    Vertical,
    Horizontal,
    VerticalReverse,
    HorizontalReverse,
}

impl GumpBoxLayoutAxis {
    pub fn available(self, rect: &GumpRect) -> i32 {
        let size = rect.size();
        match self {
            GumpBoxLayoutAxis::Vertical | GumpBoxLayoutAxis::VerticalReverse => size.y,
            GumpBoxLayoutAxis::Horizontal | GumpBoxLayoutAxis::HorizontalReverse => size.x,
        }
    }

    pub fn allocate(self, rect: &mut GumpRect, amount: i32) -> GumpRect {
        match self {
            GumpBoxLayoutAxis::Vertical => rect.take_top(amount),
            GumpBoxLayoutAxis::Horizontal => rect.take_left(amount),
            GumpBoxLayoutAxis::VerticalReverse => rect.take_bottom(amount),
            GumpBoxLayoutAxis::HorizontalReverse => rect.take_right(amount),
        }
    }

    pub fn allocate_end(self, rect: &mut GumpRect, amount: i32) -> GumpRect {
        match self {
            GumpBoxLayoutAxis::Vertical => rect.take_bottom(amount),
            GumpBoxLayoutAxis::Horizontal => rect.take_right(amount),
            GumpBoxLayoutAxis::VerticalReverse => rect.take_top(amount),
            GumpBoxLayoutAxis::HorizontalReverse => rect.take_left(amount),
        }
    }
}

pub struct GumpBoxLayout<'a> {
    builder: &'a mut GumpBuilder,
    text: &'a mut GumpText,
    rect: GumpRect,
    axis: GumpBoxLayoutAxis,
}

impl<'a> GumpBoxLayout<'a> {
    pub fn axis(&self) -> GumpBoxLayoutAxis {
        self.axis
    }

    pub fn available(&self) -> i32 {
        self.axis.available(&self.rect)
    }

    pub fn gap(&mut self, amount: i32) -> &mut Self {
        self.axis.allocate(&mut self.rect, amount);
        self
    }

    pub fn gap_end(&mut self, amount: i32) -> &mut Self {
        self.axis.allocate_end(&mut self.rect, amount);
        self
    }

    pub fn allocate_ref(&mut self, amount: i32) -> GumpRectLayout<'_> {
        let child_rect = self.axis.allocate(&mut self.rect, amount);
        GumpRectLayout {
            builder: self.builder,
            text: self.text,
            rect: child_rect,
        }
    }

    pub fn allocate(&mut self, amount: i32, f: impl FnOnce(GumpRectLayout)) -> &mut Self {
        let child_layout = self.allocate_ref(amount);
        f(child_layout);
        self
    }

    pub fn allocate_end_ref(&mut self, amount: i32) -> GumpRectLayout<'_> {
        let child_rect = self.axis.allocate_end(&mut self.rect, amount);
        GumpRectLayout {
            builder: self.builder,
            text: self.text,
            rect: child_rect,
        }
    }

    pub fn allocate_end(&mut self, amount: i32, f: impl FnOnce(GumpRectLayout)) -> &mut Self {
        let child_layout = self.allocate_end_ref(amount);
        f(child_layout);
        self
    }

    pub fn rest(self) -> GumpRectLayout<'a> {
        GumpRectLayout {
            builder: self.builder,
            text: self.text,
            rect: self.rect,
        }
    }
}

pub struct GumpPagedLayout<'a> {
    builder: &'a mut GumpBuilder,
    text: &'a mut GumpText,
    rect: GumpRect,
    page_id: usize,
}

impl<'a> GumpPagedLayout<'a> {
    pub fn allocate_ref(&mut self) -> GumpRectLayout<'_> {
        self.builder.add_page(self.page_id);
        self.page_id += 1;
        GumpRectLayout {
            builder: self.builder,
            text: self.text,
            rect: self.rect,
        }
    }

    pub fn allocate(&mut self, f: impl FnOnce(GumpRectLayout)) -> &mut Self {
        let child = self.allocate_ref();
        f(child);
        self
    }

    pub fn finish(self) -> GumpRectLayout<'a> {
        self.builder.add_page(0);
        GumpRectLayout {
            builder: self.builder,
            text: self.text,
            rect: self.rect,
        }
    }
}
