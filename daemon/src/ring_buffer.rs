use std::collections::VecDeque;

use crate::terminal_modes::TerminalModes;

/// Recent terminal output of a session, stored as whole chunks so that
/// eviction never splits a UTF-8 character. Modes set by evicted output are
/// kept so the snapshot still starts in the right terminal state.
pub struct RingBuffer {
    chunks: VecDeque<String>,
    evicted_modes: TerminalModes,
    bytes: usize,
    capacity: usize,
    last_seq: u64,
}

pub struct Snapshot {
    pub seq: u64,
    pub data: String,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            chunks: VecDeque::new(),
            evicted_modes: TerminalModes::default(),
            bytes: 0,
            capacity,
            last_seq: 0,
        }
    }

    /// Appends a chunk and returns its sequence number.
    pub fn push(&mut self, data: String) -> u64 {
        self.last_seq += 1;
        self.bytes += data.len();
        self.chunks.push_back(data);
        while self.bytes > self.capacity && self.chunks.len() > 1 {
            if let Some(evicted) = self.chunks.pop_front() {
                self.bytes -= evicted.len();
                self.evicted_modes.observe(&evicted);
            }
        }
        self.last_seq
    }

    pub fn snapshot(&self) -> Snapshot {
        let mut data = self.evicted_modes.replay();
        data.reserve(self.bytes);
        for chunk in &self.chunks {
            data.push_str(chunk);
        }
        Snapshot {
            seq: self.last_seq,
            data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_snapshot_has_seq_zero() {
        let buffer = RingBuffer::new(16);
        let snapshot = buffer.snapshot();
        assert_eq!(snapshot.seq, 0);
        assert_eq!(snapshot.data, "");
    }

    #[test]
    fn push_assigns_increasing_seq() {
        let mut buffer = RingBuffer::new(16);
        assert_eq!(buffer.push("a".into()), 1);
        assert_eq!(buffer.push("b".into()), 2);
        assert_eq!(buffer.snapshot().seq, 2);
    }

    #[test]
    fn evicts_oldest_chunks_over_capacity() {
        let mut buffer = RingBuffer::new(6);
        buffer.push("abc".into());
        buffer.push("def".into());
        buffer.push("ghi".into());
        let snapshot = buffer.snapshot();
        assert_eq!(snapshot.data, "defghi");
        assert_eq!(snapshot.seq, 3);
    }

    #[test]
    fn keeps_newest_chunk_even_if_larger_than_capacity() {
        let mut buffer = RingBuffer::new(2);
        buffer.push("ab".into());
        buffer.push("cdef".into());
        assert_eq!(buffer.snapshot().data, "cdef");
    }

    #[test]
    fn snapshot_restores_modes_from_evicted_output() {
        let mut buffer = RingBuffer::new(8);
        buffer.push("\x1b[?1049h".into());
        buffer.push("12345678".into());
        assert_eq!(buffer.snapshot().data, "\x1b[?1049h12345678");
    }

    #[test]
    fn eviction_keeps_multibyte_characters_intact() {
        let mut buffer = RingBuffer::new(4);
        buffer.push("é".into());
        buffer.push("ção".into());
        assert_eq!(buffer.snapshot().data, "ção");
    }
}
