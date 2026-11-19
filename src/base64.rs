//! Base64 decoding: a JSON Web Token's expiry and the AppImage's environment
//! snapshot. Two calls don't justify a dependency.

/// Decodes base64 (`+`, `/`; padding optional), as `base64` prints it.
pub fn decode(text: &str) -> Option<Vec<u8>> {
    decode_with(text.trim_end_matches('='), b'+', b'/')
}

/// Decodes unpadded base64url (a JSON Web Token's parts).
pub fn decode_url(text: &str) -> Option<Vec<u8>> {
    decode_with(text, b'-', b'_')
}

fn decode_with(text: &str, sixty_two: u8, sixty_three: u8) -> Option<Vec<u8>> {
    let value = |byte: u8| match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        _ if byte == sixty_two => Some(62),
        _ if byte == sixty_three => Some(63),
        _ => None,
    };
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut bits, mut held) = (0u32, 0);
    for byte in text.bytes() {
        bits = bits << 6 | u32::from(value(byte)?);
        held += 6;
        if held >= 8 {
            held -= 8;
            out.push((bits >> held) as u8);
        }
    }
    Some(out)
}
