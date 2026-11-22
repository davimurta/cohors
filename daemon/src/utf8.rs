/// Turns a byte stream into UTF-8 text, carrying incomplete trailing
/// sequences over to the next call so characters split across PTY reads
/// are not corrupted.
#[derive(Default)]
pub struct Utf8Decoder {
    pending: Vec<u8>,
}

impl Utf8Decoder {
    pub fn decode(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let mut output = String::with_capacity(self.pending.len());
        let mut rest: &[u8] = &self.pending;
        loop {
            match std::str::from_utf8(rest) {
                Ok(valid) => {
                    output.push_str(valid);
                    rest = &[];
                    break;
                }
                Err(error) => {
                    let (valid, after) = rest.split_at(error.valid_up_to());
                    // valid_up_to guarantees this prefix is valid UTF-8.
                    output.push_str(&String::from_utf8_lossy(valid));
                    match error.error_len() {
                        Some(invalid_len) => {
                            output.push(char::REPLACEMENT_CHARACTER);
                            rest = &after[invalid_len..];
                        }
                        None => {
                            rest = after;
                            break;
                        }
                    }
                }
            }
        }
        self.pending = rest.to_vec();
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_complete_text() {
        let mut decoder = Utf8Decoder::default();
        assert_eq!(decoder.decode("olá".as_bytes()), "olá");
    }

    #[test]
    fn carries_split_character_to_next_chunk() {
        let mut decoder = Utf8Decoder::default();
        let bytes = "ação".as_bytes();
        assert_eq!(decoder.decode(&bytes[..2]), "a");
        assert_eq!(decoder.decode(&bytes[2..]), "ção");
    }

    #[test]
    fn replaces_invalid_bytes() {
        let mut decoder = Utf8Decoder::default();
        assert_eq!(decoder.decode(&[b'a', 0xff, b'b']), "a\u{fffd}b");
    }
}
