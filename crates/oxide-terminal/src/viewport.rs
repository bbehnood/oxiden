use oxide_buffer::Position;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Viewport {
    pub top: usize,
    pub left: usize,
    pub width: usize,
    pub height: usize,
}

impl Viewport {
    pub fn new(width: usize, height: usize) -> Self {
        Self { top: 0, left: 0, width, height }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
    }

    pub fn scroll_to(&mut self, cursor: Position) {
        if cursor.line < self.top {
            self.top = cursor.line;
        } else if self.height > 0 && cursor.line >= self.top + self.height {
            self.top = cursor.line + 1 - self.height;
        }

        if cursor.column < self.left {
            self.left = cursor.column;
        } else if self.width > 0 && cursor.column >= self.left + self.width {
            self.left = cursor.column + 1 - self.width;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_within_view_does_not_scroll() {
        let mut viewport = Viewport::new(80, 24);

        viewport.scroll_to(Position::new(5, 5));

        assert_eq!(viewport.top, 0);
        assert_eq!(viewport.left, 0);
    }

    #[test]
    fn cursor_below_view_scrolls_down() {
        let mut viewport = Viewport::new(80, 24);

        viewport.scroll_to(Position::new(30, 0));

        assert_eq!(viewport.top, 7);
    }

    #[test]
    fn cursor_above_view_scrolls_up() {
        let mut viewport = Viewport { top: 10, ..Viewport::new(80, 24) };

        viewport.scroll_to(Position::new(2, 0));

        assert_eq!(viewport.top, 2);
    }

    #[test]
    fn cursor_right_of_view_scrolls_right() {
        let mut viewport = Viewport::new(10, 24);

        viewport.scroll_to(Position::new(0, 25));

        assert_eq!(viewport.left, 16);
    }
}
