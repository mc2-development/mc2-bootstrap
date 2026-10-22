use std::io;

use ratatui::{
    DefaultTerminal,
    crossterm::{
        event::{
            KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
        },
        execute,
        terminal::supports_keyboard_enhancement,
    },
};

pub fn with_terminal<T>(body: impl FnOnce(&mut DefaultTerminal) -> io::Result<T>) -> io::Result<T> {
    let mut terminal = ratatui::init();
    // Where the terminal supports it (Ghostty, kitty, WezTerm), <C-j> and <C-k> arrive distinct
    // from Enter. Elsewhere they cannot, which is why `{` and `}` exist too.
    let enhanced = supports_keyboard_enhancement().unwrap_or(false);
    if enhanced {
        let _ = execute!(
            io::stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        );
    }
    let result = body(&mut terminal);
    if enhanced {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
    ratatui::restore();
    result
}
