use crate::{
    backend::models::{Group, REPOS, Role, Setup},
    frontend::{
        stores::wizard::Wizard,
        types::{Focus, Row, Tab},
    },
};

pub fn menu_rows(setup: Setup, checks: usize) -> Vec<Row> {
    let mut rows = vec![Row::Header("Setup")];
    rows.extend(Setup::ALL.iter().map(|s| Row::Setup(*s)));
    rows.push(Row::Note(setup.about()));
    rows.push(Row::Gap);
    match setup {
        Setup::Developer => {
            rows.push(Row::Header("What will this machine work on?"));
            rows.extend(Role::ALL.iter().map(|r| Row::Role(*r)));
            rows.push(Row::Gap);
            for group in Group::ALL {
                rows.push(Row::Header(group.title()));
                rows.push(Row::Note(group.about()));
                rows.extend(REPOS.iter().filter(|r| r.group == group).map(Row::Repo));
                rows.push(Row::Gap);
            }
        }
        Setup::Server => {
            rows.push(Row::Header("Checks"));
            rows.extend((0..checks).map(Row::Check));
            rows.push(Row::Gap);
        }
    }
    rows.push(Row::Start);
    rows
}

pub fn selectable(row: &Row) -> bool {
    !matches!(
        row,
        Row::Header(_) | Row::Note(_) | Row::Check(_) | Row::Gap
    )
}

pub fn row_label(row: &Row) -> String {
    match row {
        Row::Setup(s) => s.to_string(),
        Row::Role(r) => r.to_string(),
        Row::Repo(r) => r.name.to_string(),
        Row::Start => "start".into(),
        Row::Header(t) => (*t).to_string(),
        Row::Note(_) | Row::Check(_) | Row::Gap => String::new(),
    }
}

impl Wizard {
    pub fn step(&mut self, down: bool) {
        let mut i = self.cursor;
        loop {
            let next = if down { i + 1 } else { i.wrapping_sub(1) };
            if next >= self.rows.len() {
                return;
            }
            i = next;
            if selectable(&self.rows[i]) {
                self.cursor = i;
                return;
            }
        }
    }

    pub fn activate(&mut self) {
        self.status.clear();
        match self.rows[self.cursor] {
            Row::Setup(Setup::Server)
                if self.sel.setup != Setup::Server && !self.server_unlocked() =>
            {
                self.ask_for_passphrase();
            }
            Row::Setup(s) => self.choose_setup(s),
            Row::Role(r) => self.sel.choose(r),
            Row::Repo(r) if self.machine.is_cloned(r) && self.sel.role == Role::Custom => {
                self.status = format!("{} is already cloned, so it stays", r.name);
            }
            Row::Repo(r) => self.sel.toggle(r),
            Row::Start => self.try_start(),
            Row::Header(_) | Row::Note(_) | Row::Check(_) | Row::Gap => {}
        }
        self.scroll = 0;
    }

    // The rest of the menu belongs to the setup, so it is rebuilt around the cursor.
    pub fn choose_setup(&mut self, setup: Setup) {
        if self.sel.setup == setup {
            return;
        }
        self.sel.setup = setup;
        self.rows = menu_rows(setup, self.checks.len());
        self.cursor = self
            .rows
            .iter()
            .position(|r| *r == Row::Setup(setup))
            .unwrap_or(0);
        self.tab = Tab::for_setup(setup)[0];
    }

    // A section starts at the first selectable row under a header.
    fn section_starts(&self) -> Vec<usize> {
        (0..self.rows.len())
            .filter(|&i| {
                selectable(&self.rows[i])
                    && (self.rows[i] == Row::Start
                        || self.rows[..i]
                            .iter()
                            .rev()
                            .take_while(|r| !selectable(r))
                            .any(|r| matches!(r, Row::Header(_))))
            })
            .collect()
    }

    pub fn section(&mut self, forward: bool) {
        if self.focus != Focus::Menu {
            return;
        }
        let starts = self.section_starts();
        let target = if forward {
            starts.iter().find(|&&i| i > self.cursor)
        } else {
            starts.iter().rev().find(|&&i| i < self.cursor)
        };
        if let Some(&i) = target {
            self.cursor = i;
        }
    }

    pub fn repeat_search(&mut self, forward: bool) {
        let Some(needle) = self.last_search.clone() else {
            self.status = "no previous search".into();
            return;
        };
        let needle = needle.to_lowercase();
        let len = self.rows.len();
        for step in 1..=len {
            let i = if forward {
                (self.cursor + step) % len
            } else {
                (self.cursor + len - step % len) % len
            };
            if selectable(&self.rows[i])
                && row_label(&self.rows[i]).to_lowercase().contains(&needle)
            {
                self.cursor = i;
                self.focus = Focus::Menu;
                return;
            }
        }
        self.status = format!("pattern not found: {needle}");
    }
}
