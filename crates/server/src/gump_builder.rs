use std::borrow::Cow;
use std::fmt::{Display, Formatter, Write};

use glam::IVec2;
use indexmap::IndexSet;
use yewoh::EntityId;
use yewoh::protocol::GumpLayout;

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

    pub fn add_alpha_cutout(&mut self, position: IVec2, size: IVec2) -> &mut Self {
        write!(&mut self.layout, "{{ checkertrans {} {} {} {} }}", position.x, position.y, size.x, size.y).unwrap();
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

    pub fn add_image_sliced(&mut self, image_id: u16, position: IVec2, size: IVec2) -> &mut Self {
        write!(&mut self.layout, "{{ resizepic {} {} {} {} {} }}", position.x, position.y, image_id, size.x, size.y)
            .unwrap();
        self
    }

    pub fn add_image_tiled(&mut self, image_id: u16, position: IVec2, size: IVec2) -> &mut Self {
        write!(
            &mut self.layout,
            "{{ gumppictiled {} {} {} {} {} }}",
            position.x,
            position.y,
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

    pub fn add_sprite(&mut self, image_id: u16, position: IVec2, size: IVec2, sprite_offset: IVec2) -> &mut Self {
        write!(
            &mut self.layout,
            "{{ picinpic {} {} {} {} {} {} {} }}",
            position.x,
            position.y,
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

    pub fn add_text_cropped(&mut self, intern_id: GumpTextId, hue: u16, position: IVec2, size: IVec2) -> &mut Self {
        write!(
            &mut self.layout,
            "{{ croppedtext {} {} {} {} {} {} }}",
            position.x,
            position.y,
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
        position: IVec2,
        size: IVec2,
    ) -> &mut Self {
        write!(
            &mut self.layout,
            "{{ textentry {} {} {} {} {} {} {} }}",
            position.x,
            position.y,
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
        position: IVec2,
        size: IVec2,
    ) -> &mut Self {
        write!(
            &mut self.layout,
            "{{ textentrylimited {} {} {} {} {} {} {} {} }}",
            position.x,
            position.y,
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

    #[allow(clippy::too_many_arguments)]
    pub fn add_tile_button(
        &mut self,
        up_texture_id: u16,
        down_texture_id: u16,
        graphic_id: u16,
        hue: u16,
        button_id: u32,
        page_id: u16,
        close: bool,
        position: IVec2,
        tile_offset: IVec2,
    ) -> &mut Self {
        let kind = if close { 1 } else { 0 };
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
        position: IVec2,
        size: IVec2,
    ) -> &mut Self {
        let background = if background { 1 } else { 0 };
        let scrollbar = if scrollbar { 1 } else { 0 };

        write!(
            &mut self.layout,
            "{{ htmlgump {} {} {} {} {} {} {} }}",
            position.x,
            position.y,
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
        position: IVec2,
        size: IVec2,
    ) -> &mut Self {
        let background = if background { 1 } else { 0 };
        let scrollbar = if scrollbar { 1 } else { 0 };

        write!(
            &mut self.layout,
            "{{ xmfhtmlgump {} {} {} {} {} {} {} }}",
            position.x,
            position.y,
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
        position: IVec2,
        size: IVec2,
    ) -> &mut Self {
        let background = if background { 1 } else { 0 };
        let scrollbar = if scrollbar { 1 } else { 0 };

        write!(
            &mut self.layout,
            "{{ xmfhtmltok {} {} {} {} {} {} {} @{}@ }}",
            position.x,
            position.y,
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
        position: IVec2,
        size: IVec2,
    ) -> &mut Self {
        let background = if background { 1 } else { 0 };
        let scrollbar = if scrollbar { 1 } else { 0 };

        write!(
            &mut self.layout,
            "{{ xmfhtmlgumpcolor {} {} {} {} {} {} {} {} }}",
            position.x,
            position.y,
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

