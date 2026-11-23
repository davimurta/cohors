//! Identity of this daemon binary: a hash of the executable file. The app
//! runs its bundled daemon with [`FLAG`] and compares the answer with the
//! one the running daemon reports, so it notices a daemon left over from an
//! older build even when both speak the same protocol.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::sync::OnceLock;

/// `cohors-daemon --build-id` prints the id and exits.
pub const FLAG: &str = "--build-id";
const UNKNOWN: &str = "unknown";
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Hash of the running executable, computed once.
pub fn current() -> &'static str {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        std::env::current_exe()
            .and_then(|path| of_file(&path))
            .unwrap_or_else(|error| {
                eprintln!("failed to hash the daemon binary: {error}");
                UNKNOWN.to_owned()
            })
    })
}

/// FNV-1a over the file: not cryptographic, only a change detector.
pub fn of_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut hash = FNV_OFFSET;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        for byte in &buffer[..read] {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    Ok(format!("{hash:016x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_bytes_same_id() {
        let dir = std::env::temp_dir().join(format!("cohors-build-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("dir");
        let (a, b, c) = (dir.join("a"), dir.join("b"), dir.join("c"));
        std::fs::write(&a, b"daemon v1").expect("a");
        std::fs::write(&b, b"daemon v1").expect("b");
        std::fs::write(&c, b"daemon v2").expect("c");
        let id = of_file(&a).expect("hash");
        assert_eq!(id.len(), 16);
        assert_eq!(id, of_file(&b).expect("hash"));
        assert_ne!(id, of_file(&c).expect("hash"));
        // Empty input hashes to the FNV offset basis.
        std::fs::write(&a, b"").expect("empty");
        assert_eq!(of_file(&a).expect("hash"), "cbf29ce484222325");
        std::fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn running_binary_has_an_id() {
        assert_ne!(current(), UNKNOWN);
    }
}
