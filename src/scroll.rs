pub const DEFAULT_SCROLLOFF: usize = 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionViewport {
    scroll: usize,
    page_size: usize,
}

impl Default for SelectionViewport {
    fn default() -> Self {
        Self {
            scroll: 0,
            page_size: 1,
        }
    }
}

impl SelectionViewport {
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    pub fn page_size(&self) -> usize {
        self.page_size
    }

    pub fn set_page_size(&mut self, page_size: usize) {
        self.page_size = page_size.max(1);
    }

    pub fn reset(&mut self) {
        self.scroll = 0;
    }

    pub fn ensure_visible(&mut self, selected: usize, item_count: usize) {
        self.scroll = ensure_visible(self.scroll, selected, item_count, self.page_size);
    }
}

pub fn max_scroll(item_count: usize, viewport_size: usize) -> usize {
    item_count.saturating_sub(viewport_size.max(1))
}

pub fn clamp(scroll: usize, item_count: usize, viewport_size: usize) -> usize {
    scroll.min(max_scroll(item_count, viewport_size))
}

pub fn ensure_visible(
    scroll: usize,
    selected: usize,
    item_count: usize,
    viewport_size: usize,
) -> usize {
    ensure_visible_with_scrolloff(
        scroll,
        selected,
        item_count,
        viewport_size,
        DEFAULT_SCROLLOFF,
    )
}

pub fn ensure_visible_with_scrolloff(
    scroll: usize,
    selected: usize,
    item_count: usize,
    viewport_size: usize,
    scrolloff: usize,
) -> usize {
    if item_count == 0 {
        return 0;
    }

    let viewport_size = viewport_size.max(1);
    let selected = selected.min(item_count.saturating_sub(1));
    let scroll = clamp(scroll, item_count, viewport_size);
    let scrolloff = scrolloff.min(viewport_size.saturating_sub(1) / 2);
    let top_threshold = scroll.saturating_add(scrolloff);
    let bottom_threshold = scroll.saturating_add(viewport_size.saturating_sub(scrolloff + 1));
    let scroll = if selected < top_threshold {
        selected.saturating_sub(scrolloff)
    } else if selected > bottom_threshold {
        selected
            .saturating_add(scrolloff + 1)
            .saturating_sub(viewport_size)
    } else {
        scroll
    };
    clamp(scroll, item_count, viewport_size)
}

pub fn up(scroll: usize, amount: usize) -> usize {
    scroll.saturating_sub(amount.max(1))
}

pub fn down(scroll: usize, amount: usize, item_count: usize, viewport_size: usize) -> usize {
    clamp(
        scroll.saturating_add(amount.max(1)),
        item_count,
        viewport_size,
    )
}

#[cfg(test)]
mod tests;
