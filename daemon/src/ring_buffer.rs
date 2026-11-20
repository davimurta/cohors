use std::collections::VecDeque;

/// Recent terminal output of a session, stored as whole chunks so that
/// eviction never splits a UTF-8 character.
pub struct RingBuffer {
    chunks: VecDeque<String>,
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
            }
        }
        self.last_seq
    }

    pub fn snapshot(&self) -> Snapshot {
        let mut data = String::with_capacity(self.bytes);
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
    fn eviction_keeps_multibyte_characters_intact() {
        let mut buffer = RingBuffer::new(4);
        buffer.push("é".into());
        buffer.push("ção".into());
        assert_eq!(buffer.snapshot().data, "ção");
    }
}
