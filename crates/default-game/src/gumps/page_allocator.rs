use yewoh_server::gump_builder::{GumpBoxLayout, GumpRect, GumpRectLayout};

const ROW_HEIGHT: i32 = 20;

pub struct GumpPageBoxAllocator<'a> {
    layout: Option<GumpBoxLayout<'a>>,
    page_index: usize,
    page_rect: GumpRect,
}

impl<'a> GumpPageBoxAllocator<'a> {
    pub fn new(mut layout: GumpRectLayout<'a>, first_page: usize) -> GumpPageBoxAllocator<'a> {
        layout.builder().add_page(first_page);

        let page_rect = layout.rect();
        let layout = Some(layout.into_vbox());
        GumpPageBoxAllocator {
            layout,
            page_index: first_page,
            page_rect,
        }
    }

    pub fn add_page(&mut self) -> &mut Self {
        self.page_index += 1;
        let rest = self.layout.take().unwrap().rest();
        let (builder, text) = rest.into_builder();
        builder.add_page(self.page_index);
        self.layout = Some(GumpRectLayout::new(builder, text, self.page_rect).into_vbox());
        self
    }

    pub fn allocate(&mut self, amount: i32, f: impl FnOnce(GumpRectLayout)) -> &mut Self {
        let layout = self.layout.as_mut().unwrap();
        let avail = layout.available();
        if avail < (amount + ROW_HEIGHT * 2) {
            // Not enough space, allocate a new page
            layout.allocate_end(ROW_HEIGHT * 2, |builder| builder
                .background(|b| b.html("<center>Next Page</center>"))
                .right(16)
                .page_button(0x15e1, 0x15e5, self.page_index + 1));
            self.add_page();
            self.layout.as_mut().unwrap().allocate(ROW_HEIGHT * 2, |builder| builder
                .background(|b| b.html("<center>Previous Page</center>"))
                .left(16)
                .page_button(0x15e3, 0x15e7, self.page_index - 1));
        }

        self.layout.as_mut().unwrap().allocate(amount, f);
        self
    }
}
