use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::use_menu::selectable;
use crate::frontend::{
    stores::wizard::Wizard,
    types::{Focus, Mode, Outcome, Page},
};

impl Wizard {
    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && key.code == KeyCode::Char('c') {
            self.outcome = Outcome::Cancel;
            return;
        }
        match self.mode {
            Mode::Help => self.mode = Mode::Normal,
            Mode::Password => self.password_key(key),
            Mode::Command => self.command_key(key),
            Mode::Search => self.search_key(key),
            Mode::Normal if self.page == Page::SignIn => self.sign_in_key(key),
            Mode::Normal => self.normal_key(key, ctrl),
        }
    }

    fn normal_key(&mut self, key: KeyEvent, ctrl: bool) {
        self.status.clear();
        if key.code == KeyCode::Esc {
            self.seq.clear();
            self.count.clear();
            return;
        }
        // Mid-sequence: `g` waits for a second `g`.
        if !self.seq.is_empty() {
            match key.code {
                KeyCode::Char(c) => {
                    self.seq.push(c);
                    self.run_sequence();
                }
                _ => self.seq.clear(),
            }
            return;
        }
        if let KeyCode::Char(c) = key.code
            && !ctrl
            && (c.is_ascii_digit() && (c != '0' || !self.count.is_empty()))
        {
            self.count.push(c);
            return;
        }
        let n = self.take_count();
        match (key.code, ctrl) {
            // Window navigation, as in LazyVim: <C-h> left, <C-l> right.
            (KeyCode::Char('h'), true) | (KeyCode::Left, _) | (KeyCode::Char('h'), false) => {
                self.focus = Focus::Menu;
            }
            (KeyCode::Char('l'), true) | (KeyCode::Right, _) | (KeyCode::Char('l'), false) => {
                self.focus = Focus::Preview;
            }
            (KeyCode::Char('j'), false) | (KeyCode::Down, _) => self.down(n),
            (KeyCode::Char('k'), false) | (KeyCode::Up, _) => self.up(n),
            (KeyCode::Char('d'), true) | (KeyCode::PageDown, _) => self.half_page(true, n),
            (KeyCode::Char('u'), true) | (KeyCode::PageUp, _) => self.half_page(false, n),
            // Sections (`}` / `{` are vim's paragraph motions), and <C-j>/<C-k> where the
            // terminal can tell them apart from Enter.
            (KeyCode::Char('}'), _) | (KeyCode::Char('j'), true) => self.section(true),
            (KeyCode::Char('{'), _) | (KeyCode::Char('k'), true) => self.section(false),
            (KeyCode::Char('g'), false) => self.seq.push('g'),
            (KeyCode::Char('G'), _) => self.bottom(),
            (KeyCode::Enter, _) | (KeyCode::Char('x' | ' '), false) => {
                if self.focus == Focus::Menu {
                    self.activate();
                }
            }
            // <S-h>/<S-l> cycle buffers in LazyVim; here, the preview tabs.
            (KeyCode::Char('H'), _)
            | (KeyCode::Char('L'), _)
            | (KeyCode::Tab, _)
            | (KeyCode::BackTab, _) => self.switch_tab(),
            (KeyCode::Char('/'), false) => {
                self.mode = Mode::Search;
                self.input.clear();
            }
            (KeyCode::Char(':'), _) => {
                self.mode = Mode::Command;
                self.input.clear();
            }
            (KeyCode::Char('n'), false) => self.repeat_search(true),
            (KeyCode::Char('N'), _) => self.repeat_search(false),
            (KeyCode::Char('?'), _) => self.mode = Mode::Help,
            (KeyCode::Char('q'), false) => self.outcome = Outcome::Cancel,
            _ => {}
        }
    }

    fn take_count(&mut self) -> usize {
        let n = self.count.parse().unwrap_or(1).max(1);
        self.count.clear();
        n
    }

    // The only multi-key sequence is `gg`.
    fn run_sequence(&mut self) {
        if self.seq == "gg" {
            self.top();
        }
        self.seq.clear();
    }

    fn down(&mut self, n: usize) {
        match self.focus {
            Focus::Menu => (0..n).for_each(|_| self.step(true)),
            Focus::Preview => self.scroll_by(n as isize),
        }
    }

    fn up(&mut self, n: usize) {
        match self.focus {
            Focus::Menu => (0..n).for_each(|_| self.step(false)),
            Focus::Preview => self.scroll_by(-(n as isize)),
        }
    }

    fn half_page(&mut self, down: bool, n: usize) {
        match self.focus {
            Focus::Menu => (0..5 * n).for_each(|_| self.step(down)),
            Focus::Preview => {
                let by = 10 * n as isize;
                self.scroll_by(if down { by } else { -by });
            }
        }
    }

    fn top(&mut self) {
        match self.focus {
            Focus::Menu => self.cursor = self.rows.iter().position(selectable).unwrap_or(0),
            Focus::Preview => self.scroll = 0,
        }
    }

    fn bottom(&mut self) {
        match self.focus {
            Focus::Menu => self.cursor = self.rows.iter().rposition(selectable).unwrap_or(0),
            Focus::Preview => self.scroll = self.max_scroll.get(),
        }
    }

    fn search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Enter => {
                self.mode = Mode::Normal;
                if !self.input.is_empty() {
                    self.last_search = Some(self.input.clone());
                    self.repeat_search(true);
                }
            }
            KeyCode::Backspace => {
                if self.input.pop().is_none() {
                    self.mode = Mode::Normal;
                }
            }
            KeyCode::Char(c) => self.input.push(c),
            _ => {}
        }
    }

    fn command_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Backspace => {
                if self.input.pop().is_none() {
                    self.mode = Mode::Normal;
                }
            }
            KeyCode::Char(c) => self.input.push(c),
            KeyCode::Enter => {
                self.mode = Mode::Normal;
                let cmd = std::mem::take(&mut self.input);
                match cmd.trim() {
                    "q" | "q!" | "qa" | "quit" => self.outcome = Outcome::Cancel,
                    "w" | "wq" | "wq!" | "x" | "write" | "start" => self.try_start(),
                    "h" | "help" => self.mode = Mode::Help,
                    "bn" | "bp" | "tabn" => self.switch_tab(),
                    "" => {}
                    other => self.status = format!("E492: not an editor command: {other}"),
                }
            }
            _ => {}
        }
    }
}
