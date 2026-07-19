pub const DEFAULT_SCROLLOFF: usize = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScrollAction {
    LineUp,
    LineDown,
    PageUp,
    PageDown,
    Top,
    Bottom,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewportState {
    scroll: usize,
    page_size: usize,
    content_len: usize,
}

impl Default for ViewportState {
    fn default() -> Self {
        Self {
            scroll: 0,
            page_size: 1,
            content_len: 1,
        }
    }
}

impl ViewportState {
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    pub fn page_size(&self) -> usize {
        self.page_size
    }

    #[cfg(test)]
    pub fn content_len(&self) -> usize {
        self.content_len
    }

    pub fn set_page_size(&mut self, page_size: usize) {
        self.page_size = page_size.max(1);
        self.clamp();
    }

    pub fn set_content_len(&mut self, content_len: usize) {
        self.content_len = content_len.max(1);
        self.clamp();
    }

    pub fn set_metrics(&mut self, page_size: usize, content_len: usize) {
        self.page_size = page_size.max(1);
        self.content_len = content_len.max(1);
        self.clamp();
    }

    pub fn set_scroll(&mut self, scroll: usize) {
        self.scroll = scroll;
        self.clamp();
    }

    pub fn reset(&mut self) {
        self.scroll = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll = max_scroll(self.content_len, self.page_size);
    }

    pub fn ensure_visible(&mut self, selected: usize) {
        self.scroll = ensure_visible(self.scroll, selected, self.content_len, self.page_size);
    }

    pub fn ensure_range_visible(&mut self, start: usize, len: usize) {
        self.scroll =
            ensure_range_visible(self.scroll, start, len, self.content_len, self.page_size);
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.scroll = up(self.scroll, amount);
    }

    pub fn scroll_down(&mut self, amount: usize) {
        self.scroll = down(self.scroll, amount, self.content_len, self.page_size);
    }

    pub fn page_up(&mut self) {
        self.scroll_up(self.page_size);
    }

    pub fn page_down(&mut self) {
        self.scroll_down(self.page_size);
    }

    pub fn apply_scroll(&mut self, action: ScrollAction) {
        match action {
            ScrollAction::LineUp => self.scroll_up(1),
            ScrollAction::LineDown => self.scroll_down(1),
            ScrollAction::PageUp => self.page_up(),
            ScrollAction::PageDown => self.page_down(),
            ScrollAction::Top => self.reset(),
            ScrollAction::Bottom => self.scroll_to_bottom(),
        }
    }

    pub fn clamp(&mut self) {
        self.scroll = clamp(self.scroll, self.content_len, self.page_size);
    }

    pub fn render_scroll(&self) -> u16 {
        self.scroll.min(u16::MAX as usize) as u16
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FollowViewportState {
    viewport: ViewportState,
    follow: bool,
}

impl Default for FollowViewportState {
    fn default() -> Self {
        Self {
            viewport: ViewportState::default(),
            follow: true,
        }
    }
}

impl FollowViewportState {
    pub fn page_size(&self) -> usize {
        self.viewport.page_size()
    }

    pub fn follow(&self) -> bool {
        self.follow
    }

    pub fn viewport(&self) -> &ViewportState {
        &self.viewport
    }

    pub fn set_page_size(&mut self, page_size: usize) {
        self.viewport.set_page_size(page_size);
        if self.follow {
            self.viewport.scroll_to_bottom();
        }
    }

    pub fn set_content_len(&mut self, content_len: usize) {
        self.viewport.set_content_len(content_len);
        if self.follow {
            self.viewport.scroll_to_bottom();
        }
    }

    pub fn set_metrics(&mut self, page_size: usize, content_len: usize) {
        self.viewport.set_metrics(page_size, content_len);
        if self.follow {
            self.viewport.scroll_to_bottom();
        }
    }

    pub fn set_scroll(&mut self, scroll: usize) {
        self.viewport.set_scroll(scroll);
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.viewport.scroll_up(amount);
        self.disable_follow();
    }

    pub fn scroll_down(&mut self, amount: usize) {
        self.viewport.scroll_down(amount);
        self.disable_follow();
    }

    pub fn apply_scroll(&mut self, action: ScrollAction) {
        match action {
            ScrollAction::LineUp => self.scroll_up(1),
            ScrollAction::LineDown => self.scroll_down(1),
            ScrollAction::PageUp => self.scroll_up(self.viewport.page_size()),
            ScrollAction::PageDown => self.scroll_down(self.viewport.page_size()),
            ScrollAction::Top => {
                self.viewport.reset();
                self.disable_follow();
            }
            ScrollAction::Bottom => {
                self.follow = true;
                self.viewport.scroll_to_bottom();
            }
        }
    }

    pub fn disable_follow(&mut self) {
        self.follow = false;
    }

    #[cfg(test)]
    pub fn set_follow(&mut self, follow: bool) {
        self.follow = follow;
        if self.follow {
            self.viewport.scroll_to_bottom();
        }
    }

    pub fn snap_to_bottom(&mut self, content_len: usize) {
        self.follow = true;
        self.viewport.set_content_len(content_len);
        self.viewport.scroll_to_bottom();
    }

    pub fn toggle_follow(&mut self, content_len: usize) -> bool {
        if self.follow {
            self.follow = false;
        } else {
            self.snap_to_bottom(content_len);
        }
        self.follow
    }

    pub fn reset_for_source_change(&mut self) {
        self.viewport.reset();
        self.follow = true;
    }

    pub fn reset_to_start(&mut self) {
        self.viewport.reset();
        self.follow = false;
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

pub fn ensure_range_visible(
    scroll: usize,
    start: usize,
    len: usize,
    item_count: usize,
    viewport_size: usize,
) -> usize {
    if item_count == 0 {
        return 0;
    }

    let viewport_size = viewport_size.max(1);
    let start = start.min(item_count.saturating_sub(1));
    let len = len.max(1);
    let end = start
        .saturating_add(len - 1)
        .min(item_count.saturating_sub(1));
    let scroll = clamp(scroll, item_count, viewport_size);

    if len >= viewport_size {
        return clamp(start, item_count, viewport_size);
    }

    if start < scroll {
        start
    } else if end >= scroll.saturating_add(viewport_size) {
        end.saturating_add(1).saturating_sub(viewport_size)
    } else {
        scroll
    }
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
