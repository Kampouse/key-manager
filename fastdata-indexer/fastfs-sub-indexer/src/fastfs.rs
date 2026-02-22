use borsh::{BorshDeserialize, BorshSerialize};

const MAX_RELATIVE_PATH_LENGTH: usize = 1024;
const MAX_MIME_TYPE_LENGTH: usize = 256;
const OFFSET_ALIGNMENT: u32 = 1 << 20; // 1Mb
const MAX_FULL_SIZE: u32 = 32 * OFFSET_ALIGNMENT; // 32Mb

#[derive(Clone, BorshSerialize, BorshDeserialize)]
pub enum FastfsData {
    Simple(Box<SimpleFastfs>),
    Partial(Box<PartialFastfs>),
}

#[derive(Clone, BorshSerialize, BorshDeserialize)]
pub struct SimpleFastfs {
    pub relative_path: String,
    pub content: Option<FastfsFileContent>,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct FastfsFileContent {
    pub mime_type: String,
    pub content: Vec<u8>,
}

#[derive(Clone, BorshSerialize, BorshDeserialize)]
pub struct PartialFastfs {
    pub relative_path: String,
    /// The offset has to be aligned to 1Mb (1048576 bytes).
    pub offset: u32,
    /// The full content size in bytes up to 32Mb (33554432 bytes).
    pub full_size: u32,
    pub mime_type: String,
    /// The content chunk up to 1Mb (1048576 bytes). We assume zero bytes tail if the chunk is
    /// smaller than 1Mb, but inside the file.
    pub content_chunk: Vec<u8>,
    /// A nonce to differentiate different uploads at the same relative path.
    /// The nonce should match for all parts of the same file.
    /// Max value is i32::MAX.
    /// Min value should be 1, to differentiate from simple fastfs entries.
    pub nonce: u32,
}

fn is_valid_relative_path(path: &str) -> bool {
    if path.is_empty()
        || path.len() > MAX_RELATIVE_PATH_LENGTH
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\0')
        || path.ends_with('/')
        || path.ends_with('\\')
        || path.split('/').any(|c| c.is_empty() || c == "." || c == "..")
        || path.split('\\').any(|c| c.is_empty() || c == "." || c == "..")
        || path.contains('%') // Block URL-encoded traversal (e.g. %2e%2e)
        || !path.is_ascii() // Block non-ASCII/Unicode bypass characters
        || path.chars().any(|c| c.is_ascii_control()) // Block control characters
    {
        return false;
    }
    true
}

impl PartialFastfs {
    pub fn is_valid(&self) -> bool {
        if !is_valid_relative_path(&self.relative_path) {
            return false;
        }
        if !self.offset.is_multiple_of(OFFSET_ALIGNMENT) || self.offset >= MAX_FULL_SIZE {
            return false;
        }
        if self.full_size == 0 || self.full_size > MAX_FULL_SIZE {
            return false;
        }
        if self.mime_type.is_empty()
            || self.mime_type.len() > MAX_MIME_TYPE_LENGTH
            || !self.mime_type.is_ascii()
            || self.mime_type.chars().any(|c| c.is_ascii_control())
        {
            return false;
        }
        if self.content_chunk.len() > OFFSET_ALIGNMENT as usize {
            return false;
        }
        if self.offset + self.content_chunk.len() as u32 > self.full_size {
            return false;
        }
        if self.nonce == 0 || self.nonce > i32::MAX as u32 {
            return false;
        }
        true
    }
}

impl SimpleFastfs {
    pub fn is_valid(&self) -> bool {
        if !is_valid_relative_path(&self.relative_path) {
            return false;
        }
        if let Some(content) = &self.content {
            if content.mime_type.is_empty()
                || content.mime_type.len() > MAX_MIME_TYPE_LENGTH
                || !content.mime_type.is_ascii()
                || content.mime_type.chars().any(|c| c.is_ascii_control())
            {
                return false;
            }
            if content.content.len() > MAX_FULL_SIZE as usize {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- is_valid_relative_path tests ---

    #[test]
    fn test_valid_simple_path() {
        assert!(is_valid_relative_path("foo/bar.txt"));
    }

    #[test]
    fn test_valid_single_file() {
        assert!(is_valid_relative_path("file.txt"));
    }

    #[test]
    fn test_valid_deep_path() {
        assert!(is_valid_relative_path("a/b/c/d/e.json"));
    }

    #[test]
    fn test_empty_path() {
        assert!(!is_valid_relative_path(""));
    }

    #[test]
    fn test_leading_slash() {
        assert!(!is_valid_relative_path("/foo/bar"));
    }

    #[test]
    fn test_trailing_slash() {
        assert!(!is_valid_relative_path("foo/bar/"));
    }

    #[test]
    fn test_dot_segment() {
        assert!(!is_valid_relative_path("foo/./bar"));
    }

    #[test]
    fn test_dotdot_segment() {
        assert!(!is_valid_relative_path("foo/../bar"));
    }

    #[test]
    fn test_empty_segment() {
        assert!(!is_valid_relative_path("foo//bar"));
    }

    #[test]
    fn test_null_byte() {
        assert!(!is_valid_relative_path("foo\0bar"));
    }

    #[test]
    fn test_percent_encoding() {
        assert!(!is_valid_relative_path("foo/%2e%2e/bar"));
    }

    #[test]
    fn test_non_ascii() {
        assert!(!is_valid_relative_path("foo/b\u{00e4}r"));
    }

    #[test]
    fn test_control_char() {
        assert!(!is_valid_relative_path("foo/\x01bar"));
    }

    #[test]
    fn test_backslash_start() {
        assert!(!is_valid_relative_path("\\foo\\bar"));
    }

    #[test]
    fn test_too_long_path() {
        let long = "a".repeat(MAX_RELATIVE_PATH_LENGTH + 1);
        assert!(!is_valid_relative_path(&long));
    }

    #[test]
    fn test_max_length_path() {
        let exact = "a".repeat(MAX_RELATIVE_PATH_LENGTH);
        assert!(is_valid_relative_path(&exact));
    }

    // --- SimpleFastfs::is_valid tests ---

    #[test]
    fn test_simple_valid_with_content() {
        let s = SimpleFastfs {
            relative_path: "file.txt".into(),
            content: Some(FastfsFileContent {
                mime_type: "text/plain".into(),
                content: vec![1, 2, 3],
            }),
        };
        assert!(s.is_valid());
    }

    #[test]
    fn test_simple_valid_deletion() {
        let s = SimpleFastfs {
            relative_path: "file.txt".into(),
            content: None,
        };
        assert!(s.is_valid());
    }

    #[test]
    fn test_simple_invalid_path() {
        let s = SimpleFastfs {
            relative_path: "../escape.txt".into(),
            content: None,
        };
        assert!(!s.is_valid());
    }

    #[test]
    fn test_simple_empty_mime() {
        let s = SimpleFastfs {
            relative_path: "file.txt".into(),
            content: Some(FastfsFileContent {
                mime_type: "".into(),
                content: vec![1],
            }),
        };
        assert!(!s.is_valid());
    }

    #[test]
    fn test_simple_control_char_mime() {
        let s = SimpleFastfs {
            relative_path: "file.txt".into(),
            content: Some(FastfsFileContent {
                mime_type: "text/\x00plain".into(),
                content: vec![1],
            }),
        };
        assert!(!s.is_valid());
    }

    // --- PartialFastfs::is_valid tests ---

    #[test]
    fn test_partial_valid() {
        let p = PartialFastfs {
            relative_path: "file.bin".into(),
            offset: 0,
            full_size: OFFSET_ALIGNMENT,
            mime_type: "application/octet-stream".into(),
            content_chunk: vec![0u8; 100],
            nonce: 1,
        };
        assert!(p.is_valid());
    }

    #[test]
    fn test_partial_unaligned_offset() {
        let p = PartialFastfs {
            relative_path: "file.bin".into(),
            offset: 500,
            full_size: OFFSET_ALIGNMENT,
            mime_type: "application/octet-stream".into(),
            content_chunk: vec![0u8; 100],
            nonce: 1,
        };
        assert!(!p.is_valid());
    }

    #[test]
    fn test_partial_zero_full_size() {
        let p = PartialFastfs {
            relative_path: "file.bin".into(),
            offset: 0,
            full_size: 0,
            mime_type: "application/octet-stream".into(),
            content_chunk: vec![],
            nonce: 1,
        };
        assert!(!p.is_valid());
    }

    #[test]
    fn test_partial_nonce_zero() {
        let p = PartialFastfs {
            relative_path: "file.bin".into(),
            offset: 0,
            full_size: OFFSET_ALIGNMENT,
            mime_type: "application/octet-stream".into(),
            content_chunk: vec![0u8; 100],
            nonce: 0,
        };
        assert!(!p.is_valid());
    }

    #[test]
    fn test_partial_chunk_exceeds_full_size() {
        let p = PartialFastfs {
            relative_path: "file.bin".into(),
            offset: 0,
            full_size: 50,
            mime_type: "application/octet-stream".into(),
            content_chunk: vec![0u8; 100],
            nonce: 1,
        };
        assert!(!p.is_valid());
    }

    #[test]
    fn test_partial_invalid_path() {
        let p = PartialFastfs {
            relative_path: "/absolute/path".into(),
            offset: 0,
            full_size: OFFSET_ALIGNMENT,
            mime_type: "text/plain".into(),
            content_chunk: vec![0u8; 10],
            nonce: 1,
        };
        assert!(!p.is_valid());
    }
}
