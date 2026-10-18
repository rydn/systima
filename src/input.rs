//! Keyboard → `Action`, aware of the current UI mode.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Cpu,
    Mem,
    Pid,
    Name,
    User,
    Time,
}

impl SortKey {
    pub fn label(self) -> &'static str {
        match self {
            SortKey::Cpu => "CPU",
            SortKey::Mem => "MEM",
            SortKey::Pid => "PID",
            SortKey::Name => "NAME",
            SortKey::User => "USER",
            SortKey::Time => "TIME",
        }
    }

    /// Whether the natural order for this key is descending.
    pub fn default_desc(self) -> bool {
        matches!(self, SortKey::Cpu | SortKey::Mem | SortKey::Time)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Filter,
    ConfirmKill,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    Quit,
    Up,
    Down,
    PageUp,
    PageDown,
    Top,
    Bottom,
    Sort(SortKey),
    ReverseSort,
    FilterStart,
    FilterChar(char),
    FilterBackspace,
    FilterApply,
    FilterClear,
    KillPrompt,
    ConfirmKill { force: bool },
    Cancel,
    FocusNext,
    IfacePrev,
    IfaceNext,
    Pause,
    CycleTheme,
    NormalizeCpu,
    IntervalUp,
    IntervalDown,
    Help,
}

pub fn map(mode: Mode, key: KeyEvent) -> Action {
    if key.kind == KeyEventKind::Release {
        return Action::None;
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C')) {
        return Action::Quit;
    }
    match mode {
        Mode::Filter => match key.code {
            KeyCode::Esc => Action::FilterClear,
            KeyCode::Enter => Action::FilterApply,
            KeyCode::Backspace => Action::FilterBackspace,
            KeyCode::Up => Action::Up,
            KeyCode::Down => Action::Down,
            KeyCode::Char(c) if !ctrl => Action::FilterChar(c),
            _ => Action::None,
        },
        Mode::ConfirmKill => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                Action::ConfirmKill { force: false }
            }
            KeyCode::Char('K') => Action::ConfirmKill { force: true },
            _ => Action::Cancel,
        },
        Mode::Help => match key.code {
            KeyCode::Char('q') => Action::Quit,
            _ => Action::Cancel,
        },
        Mode::Normal => match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
            KeyCode::Up | KeyCode::Char('k') => Action::Up,
            KeyCode::Down | KeyCode::Char('j') => Action::Down,
            KeyCode::PageUp => Action::PageUp,
            KeyCode::PageDown => Action::PageDown,
            KeyCode::Home | KeyCode::Char('g') => Action::Top,
            KeyCode::End | KeyCode::Char('G') => Action::Bottom,
            KeyCode::Char('c') => Action::Sort(SortKey::Cpu),
            KeyCode::Char('m') => Action::Sort(SortKey::Mem),
            KeyCode::Char('p') => Action::Sort(SortKey::Pid),
            KeyCode::Char('n') => Action::Sort(SortKey::Name),
            KeyCode::Char('u') => Action::Sort(SortKey::User),
            KeyCode::Char('T') => Action::Sort(SortKey::Time),
            KeyCode::Char('r') => Action::ReverseSort,
            KeyCode::Char('/') => Action::FilterStart,
            KeyCode::Char('x') | KeyCode::F(9) => Action::KillPrompt,
            KeyCode::Tab => Action::FocusNext,
            KeyCode::Char('[') => Action::IfacePrev,
            KeyCode::Char(']') => Action::IfaceNext,
            KeyCode::Char(' ') => Action::Pause,
            KeyCode::Char('t') => Action::CycleTheme,
            KeyCode::Char('e') => Action::NormalizeCpu,
            KeyCode::Char('+') | KeyCode::Char('=') => Action::IntervalUp,
            KeyCode::Char('-') | KeyCode::Char('_') => Action::IntervalDown,
            KeyCode::Char('?') | KeyCode::F(1) | KeyCode::Char('h') => Action::Help,
            _ => Action::None,
        },
    }
}

pub const HELP_LINES: &[(&str, &str)] = &[
    ("↑↓ j k", "move selection"),
    ("PgUp PgDn g G", "page / top / bottom"),
    (
        "c m p n u T",
        "sort by CPU / MEM / PID / NAME / USER / TIME",
    ),
    ("r", "reverse sort order"),
    ("/", "filter processes (Enter apply, Esc clear)"),
    ("x  F9", "kill selected (y = TERM, K = KILL)"),
    ("Tab", "cycle panel focus"),
    ("[ ]", "previous / next network interface"),
    ("space", "pause sampling"),
    ("+ -", "faster / slower sampling"),
    ("e", "normalise process CPU by core count"),
    ("t", "cycle colour theme"),
    ("? h F1", "this help"),
    ("q Esc ^C", "quit"),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn normal_mode_bindings() {
        assert_eq!(map(Mode::Normal, key(KeyCode::Char('q'))), Action::Quit);
        assert_eq!(
            map(Mode::Normal, key(KeyCode::Char('c'))),
            Action::Sort(SortKey::Cpu)
        );
        assert_eq!(map(Mode::Normal, key(KeyCode::Char('j'))), Action::Down);
        assert_eq!(map(Mode::Normal, key(KeyCode::F(9))), Action::KillPrompt);
        assert_eq!(map(Mode::Normal, key(KeyCode::Char('z'))), Action::None);
    }

    #[test]
    fn ctrl_c_quits_in_every_mode() {
        let k = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        for m in [Mode::Normal, Mode::Filter, Mode::ConfirmKill, Mode::Help] {
            assert_eq!(map(m, k), Action::Quit);
        }
    }

    #[test]
    fn filter_mode_collects_text() {
        assert_eq!(
            map(Mode::Filter, key(KeyCode::Char('q'))),
            Action::FilterChar('q')
        );
        assert_eq!(map(Mode::Filter, key(KeyCode::Enter)), Action::FilterApply);
        assert_eq!(map(Mode::Filter, key(KeyCode::Esc)), Action::FilterClear);
    }

    #[test]
    fn confirm_kill_requires_explicit_yes() {
        assert_eq!(
            map(Mode::ConfirmKill, key(KeyCode::Char('y'))),
            Action::ConfirmKill { force: false }
        );
        assert_eq!(
            map(Mode::ConfirmKill, key(KeyCode::Char('K'))),
            Action::ConfirmKill { force: true }
        );
        assert_eq!(
            map(Mode::ConfirmKill, key(KeyCode::Char('n'))),
            Action::Cancel
        );
    }

    #[test]
    fn release_events_ignored() {
        let mut k = key(KeyCode::Char('q'));
        k.kind = KeyEventKind::Release;
        assert_eq!(map(Mode::Normal, k), Action::None);
    }
}
