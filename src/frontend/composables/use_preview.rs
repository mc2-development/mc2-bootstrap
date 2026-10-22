use crate::frontend::{stores::wizard::Wizard, types::Tab};

impl Wizard {
    pub fn scroll_by(&mut self, delta: isize) {
        let max = self.max_scroll.get() as isize;
        self.scroll = (self.scroll as isize + delta).clamp(0, max) as u16;
    }

    pub fn switch_tab(&mut self) {
        let tabs = Tab::for_setup(self.sel.setup);
        let at = tabs.iter().position(|t| *t == self.tab).unwrap_or(0);
        self.tab = tabs[(at + 1) % tabs.len()];
        self.scroll = 0;
    }
}
