use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Paragraph, Widget},
};

use crate::app::View;

pub(super) struct Footer {
    view: View,
}

impl Footer {
    pub(super) fn new(view: View) -> Self {
        Self { view }
    }
}

impl Widget for Footer {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let footer = match self.view {
            View::List => {
                "q/Esc quit  •  ↑↓ or j/k move  •  g/G home/end  •  Enter/click open details  •  v toggle range"
            }
            View::Details => {
                "q quit  •  Esc/←/h go back  •  ↑↓ or j/k move between events  •  s save current/range"
            }
        };

        Paragraph::new(footer).render(area, buf);
    }
}
