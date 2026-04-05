fn is_unreserved(byte: u8) -> bool {
    matches!(
        byte,
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~'
    )
}

fn decode_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn percent_encode_segment(segment: &str) -> String {
    let mut encoded = String::with_capacity(segment.len());

    for byte in segment.bytes() {
        if is_unreserved(byte) {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02X}"));
        }
    }

    encoded
}

fn percent_decode_segment(segment: &str) -> Option<String> {
    let bytes = segment.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut idx = 0;

    while idx < bytes.len() {
        match bytes[idx] {
            b'%' => {
                let high = *bytes.get(idx + 1)?;
                let low = *bytes.get(idx + 2)?;
                decoded.push((decode_hex(high)? << 4) | decode_hex(low)?);
                idx += 3;
            }
            byte => {
                decoded.push(byte);
                idx += 1;
            }
        }
    }

    String::from_utf8(decoded).ok()
}

pub fn encode_url_path(path: &str) -> String {
    path.replace('\\', "/")
        .split('/')
        .map(percent_encode_segment)
        .collect::<Vec<_>>()
        .join("/")
}

pub fn decode_url_path(path: &str) -> Option<String> {
    path.split('/')
        .map(percent_decode_segment)
        .collect::<Option<Vec<_>>>()
        .map(|segments| segments.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_reserved_path_characters() {
        assert_eq!(
            encode_url_path("./images/hero image #1%.png"),
            "./images/hero%20image%20%231%25.png"
        );
    }

    #[test]
    fn decodes_percent_encoded_paths() {
        assert_eq!(
            decode_url_path("./images/hero%20image%20%231%25.png"),
            Some("./images/hero image #1%.png".to_string())
        );
    }

    #[test]
    fn rejects_invalid_percent_encoding() {
        assert_eq!(decode_url_path("./hero%2Gimage.png"), None);
        assert_eq!(decode_url_path("./hero%"), None);
    }
}
