use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
};

use crate::frontend::{
    components::{command_line, status_bar},
    stores::wizard::Wizard,
};

pub fn render(f: &mut Frame, w: &Wizard, page: impl FnOnce(&mut Frame, Rect)) {
    let [body, footer] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).areas(f.area());
    let [top, bottom] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(footer);
    page(f, body);
    status_bar::render(f, top, w);
    command_line::render(f, bottom, w);
}
