use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
};

use crate::frontend::{
    components::{menu, preview},
    stores::wizard::Wizard,
};

pub fn render(f: &mut Frame, area: Rect, w: &Wizard) {
    let [left, right] =
        Layout::horizontal([Constraint::Length(48), Constraint::Min(30)]).areas(area);
    menu::render(f, left, w);
    preview::render(f, right, w);
}
