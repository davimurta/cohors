const TOKEN_BYTES: usize = 32;

/// Origins of the only clients allowed to connect: the Tauri web view
/// (macOS and Linux, then Windows) and the Vite dev server.
const ALLOWED_ORIGINS: &[&str] = &[
    "tauri://localhost",
    "http://tauri.localhost",
    "http://localhost:5173",
    "http://127.0.0.1:5173",
];

pub fn generate_token() -> Result<String, getrandom::Error> {
    let mut bytes = [0u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Constant-time comparison so response timing does not leak the token.
pub fn token_matches(expected: &str, given: &str) -> bool {
    let expected = expected.as_bytes();
    let given = given.as_bytes();
    if expected.len() != given.len() {
        return false;
    }
    expected
        .iter()
        .zip(given)
        .fold(0u8, |diff, (a, b)| diff | (a ^ b))
        == 0
}

pub fn origin_allowed(origin: Option<&str>) -> bool {
    origin.is_some_and(|origin| ALLOWED_ORIGINS.contains(&origin))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_token_is_64_hex_chars() {
        let token = generate_token().expect("token");
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn generated_tokens_differ() {
        assert_ne!(generate_token().ok(), generate_token().ok());
    }

    #[test]
    fn token_matches_only_exact_value() {
        assert!(token_matches("abc123", "abc123"));
        assert!(!token_matches("abc123", "abc124"));
        assert!(!token_matches("abc123", "abc12"));
        assert!(!token_matches("abc123", ""));
    }

    #[test]
    fn origin_requires_allowlisted_value() {
        assert!(origin_allowed(Some("tauri://localhost")));
        assert!(origin_allowed(Some("http://localhost:5173")));
        assert!(!origin_allowed(Some("https://evil.example")));
        assert!(!origin_allowed(Some("http://localhost:5174")));
        assert!(!origin_allowed(None));
    }
}
