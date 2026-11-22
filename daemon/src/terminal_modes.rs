use std::collections::BTreeMap;

const MAX_SEQUENCE_LEN: usize = 64;
const CURSOR_VISIBLE: u16 = 25;

/// DEC private modes that change how the screen or input behaves:
/// cursor keys, cursor visibility, alternate screen, mouse, focus and
/// bracketed paste.
const TRACKED_PRIVATE_MODES: &[u16] = &[
    1, 25, 47, 1000, 1002, 1003, 1004, 1005, 1006, 1015, 1047, 1049, 2004,
];

#[derive(Default)]
enum ParserState {
    #[default]
    Ground,
    Escape,
    Csi(String),
}

/// Follows the terminal modes a program turns on and off, so a replay that
/// starts in the middle of the output can restore them first.
#[derive(Default)]
pub struct TerminalModes {
    private_modes: BTreeMap<u16, bool>,
    keyboard_flags: Vec<u32>,
    modify_other_keys: Option<u32>,
    state: ParserState,
}

impl TerminalModes {
    pub fn observe(&mut self, text: &str) {
        for character in text.chars() {
            self.state = match std::mem::take(&mut self.state) {
                ParserState::Ground if character == '\x1b' => ParserState::Escape,
                ParserState::Ground => ParserState::Ground,
                ParserState::Escape if character == '[' => ParserState::Csi(String::new()),
                ParserState::Escape if character == '\x1b' => ParserState::Escape,
                ParserState::Escape => ParserState::Ground,
                ParserState::Csi(mut body) => match character {
                    '\x20'..='\x3f' if body.len() < MAX_SEQUENCE_LEN => {
                        body.push(character);
                        ParserState::Csi(body)
                    }
                    '\x40'..='\x7e' => {
                        self.dispatch(&body, character);
                        ParserState::Ground
                    }
                    '\x1b' => ParserState::Escape,
                    _ => ParserState::Ground,
                },
            };
        }
    }

    /// Escape sequences that put a fresh terminal into the observed modes.
    pub fn replay(&self) -> String {
        let mut sequence = String::new();
        for (&mode, &enabled) in &self.private_modes {
            let default = mode == CURSOR_VISIBLE;
            if enabled != default {
                let action = if enabled { 'h' } else { 'l' };
                sequence.push_str(&format!("\x1b[?{mode}{action}"));
            }
        }
        for flags in &self.keyboard_flags {
            sequence.push_str(&format!("\x1b[>{flags}u"));
        }
        if let Some(level) = self.modify_other_keys {
            sequence.push_str(&format!("\x1b[>4;{level}m"));
        }
        sequence
    }

    fn dispatch(&mut self, body: &str, action: char) {
        match (body.chars().next(), action) {
            (Some('?'), 'h' | 'l') => self.set_private_modes(&body[1..], action == 'h'),
            (Some('>'), 'u') => self
                .keyboard_flags
                .push(first_param(&body[1..]).unwrap_or(0)),
            (Some('<'), 'u') => {
                let count = first_param(&body[1..]).unwrap_or(1) as usize;
                let keep = self.keyboard_flags.len().saturating_sub(count);
                self.keyboard_flags.truncate(keep);
            }
            (Some('='), 'u') => {
                let flags = first_param(&body[1..]).unwrap_or(0);
                match self.keyboard_flags.last_mut() {
                    Some(top) => *top = flags,
                    None => self.keyboard_flags.push(flags),
                }
            }
            (Some('>'), 'm') => self.set_modify_other_keys(&body[1..]),
            _ => {}
        }
    }

    fn set_private_modes(&mut self, params: &str, enabled: bool) {
        for mode in params
            .split(';')
            .filter_map(|param| param.parse::<u16>().ok())
        {
            if TRACKED_PRIVATE_MODES.contains(&mode) {
                self.private_modes.insert(mode, enabled);
            }
        }
    }

    fn set_modify_other_keys(&mut self, params: &str) {
        let mut values = params.split(';');
        if values.next() != Some("4") {
            return;
        }
        self.modify_other_keys = values
            .next()
            .and_then(|level| level.parse().ok())
            .filter(|&level| level > 0);
    }
}

fn first_param(params: &str) -> Option<u32> {
    params
        .split(';')
        .next()
        .and_then(|value| value.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn replay_of(chunks: &[&str]) -> String {
        let mut modes = TerminalModes::default();
        for chunk in chunks {
            modes.observe(chunk);
        }
        modes.replay()
    }

    #[test]
    fn fresh_terminal_needs_no_replay() {
        assert_eq!(replay_of(&["plain text\r\n"]), "");
    }

    #[test]
    fn restores_alternate_screen_and_bracketed_paste() {
        assert_eq!(
            replay_of(&["\x1b[?1049h\x1b[?2004hhello"]),
            "\x1b[?1049h\x1b[?2004h"
        );
    }

    #[test]
    fn mode_turned_off_again_is_not_replayed() {
        assert_eq!(replay_of(&["\x1b[?1049hx\x1b[?1049l"]), "");
    }

    #[test]
    fn handles_sequences_split_across_chunks() {
        assert_eq!(replay_of(&["\x1b[?10", "49h"]), "\x1b[?1049h");
    }

    #[test]
    fn handles_multiple_modes_in_one_sequence() {
        assert_eq!(replay_of(&["\x1b[?1000;1006h"]), "\x1b[?1000h\x1b[?1006h");
    }

    #[test]
    fn restores_hidden_cursor() {
        assert_eq!(replay_of(&["\x1b[?25l"]), "\x1b[?25l");
        assert_eq!(replay_of(&["\x1b[?25l\x1b[?25h"]), "");
    }

    #[test]
    fn ignores_untracked_modes() {
        assert_eq!(replay_of(&["\x1b[?2026h"]), "");
    }

    #[test]
    fn tracks_keyboard_protocol_stack() {
        assert_eq!(replay_of(&["\x1b[>1u\x1b[>5u"]), "\x1b[>1u\x1b[>5u");
        assert_eq!(replay_of(&["\x1b[>1u\x1b[>5u\x1b[<u"]), "\x1b[>1u");
        assert_eq!(replay_of(&["\x1b[>1u\x1b[<5u"]), "");
    }

    #[test]
    fn tracks_modify_other_keys() {
        assert_eq!(replay_of(&["\x1b[>4;2m"]), "\x1b[>4;2m");
        assert_eq!(replay_of(&["\x1b[>4;2m\x1b[>4m"]), "");
    }

    #[test]
    fn ignores_regular_sgr() {
        assert_eq!(replay_of(&["\x1b[1;31mred\x1b[0m"]), "");
    }
}
