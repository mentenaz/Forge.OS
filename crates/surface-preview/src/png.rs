//! A PNG writer with no dependencies.
//!
//! It stores the pixels uncompressed, which makes files large and the code
//! small: the point is to look at a picture, not to ship one.

/// The largest block a stored deflate stream can hold.
const STORED_BLOCK: usize = 65_535;

/// Encode a picture of `width` by `height` pixels as an 8-bit RGB PNG.
///
/// `rgb` holds three bytes per pixel, row by row. Returns `None` if its
/// length is not `width * height * 3` or the size does not fit a PNG.
pub fn encode(width: u32, height: u32, rgb: &[u8]) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
        return None;
    }
    let row = width as usize * 3;
    if row.checked_mul(height as usize)? != rgb.len() {
        return None;
    }

    // Each row is a filter byte (0, none) and then its pixels.
    let mut raw = Vec::with_capacity((row + 1) * height as usize);
    for line in rgb.chunks_exact(row) {
        raw.push(0);
        raw.extend_from_slice(line);
    }

    let mut out = Vec::new();
    out.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, *b"IHDR", &header);
    chunk(&mut out, *b"IDAT", &zlib_stored(&raw));
    chunk(&mut out, *b"IEND", &[]);
    Some(out)
}

fn chunk(out: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(&kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn zlib_stored(data: &[u8]) -> Vec<u8> {
    // 0x78 0x01: deflate with a 32 KiB window, no preset dictionary, fastest.
    let mut out = vec![0x78, 0x01];
    let mut blocks = data.chunks(STORED_BLOCK).peekable();
    if blocks.peek().is_none() {
        out.extend_from_slice(&[1, 0, 0, 0xff, 0xff]);
    }
    while let Some(block) = blocks.next() {
        out.push(u8::from(blocks.peek().is_none()));
        let length = block.len() as u16;
        out.extend_from_slice(&length.to_le_bytes());
        out.extend_from_slice(&(!length).to_le_bytes());
        out.extend_from_slice(block);
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

/// The CRC-32 PNG chunks are protected with.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// The checksum a zlib stream ends with.
pub fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_checksums_match_their_published_check_values() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11e6_0398);
        assert_eq!(crc32(b""), 0);
        assert_eq!(adler32(b""), 1);
    }

    #[test]
    fn a_picture_with_the_wrong_amount_of_data_is_refused() {
        assert!(encode(2, 2, &[0; 11]).is_none());
        assert!(encode(2, 2, &[0; 13]).is_none());
        assert!(encode(0, 2, &[]).is_none());
        assert!(encode(2, 2, &[0; 12]).is_some());
    }

    #[test]
    fn the_file_starts_with_the_signature_and_ends_with_iend() {
        let png = encode(1, 1, &[1, 2, 3]).unwrap();
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        assert_eq!(&png[png.len() - 8..png.len() - 4], b"IEND");
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(&png[16..20], &1u32.to_be_bytes());
    }

    #[test]
    fn data_longer_than_one_stored_block_is_split_and_only_the_last_block_is_marked() {
        let stream = zlib_stored(&vec![7; STORED_BLOCK + 10]);
        assert_eq!(stream[2], 0, "first block is not the last");
        let second = 2 + 5 + STORED_BLOCK;
        assert_eq!(stream[second], 1, "second block is the last");
        assert_eq!(&stream[second + 1..second + 3], &10u16.to_le_bytes());
    }

    /// Read a PNG back the strict way and return its size and pixels.
    fn decode(png: &[u8]) -> (u32, u32, Vec<u8>) {
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        let mut at = 8;
        let mut header = None;
        let mut data = Vec::new();
        let mut kinds = Vec::new();
        while at < png.len() {
            let length = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
            let kind: [u8; 4] = png[at + 4..at + 8].try_into().unwrap();
            let body = &png[at + 8..at + 8 + length];
            let crc =
                u32::from_be_bytes(png[at + 8 + length..at + 12 + length].try_into().unwrap());
            assert_eq!(crc, crc32(&png[at + 4..at + 8 + length]), "crc of {kind:?}");
            match &kind {
                b"IHDR" => header = Some(body.to_vec()),
                b"IDAT" => data.extend_from_slice(body),
                _ => {}
            }
            kinds.push(kind);
            at += 12 + length;
        }
        assert_eq!(at, png.len());
        assert_eq!(kinds.first(), Some(b"IHDR"));
        assert_eq!(kinds.last(), Some(b"IEND"));
        let header = header.unwrap();
        let width = u32::from_be_bytes(header[0..4].try_into().unwrap());
        let height = u32::from_be_bytes(header[4..8].try_into().unwrap());
        assert_eq!(
            &header[8..],
            &[8, 2, 0, 0, 0],
            "8-bit truecolour, no interlace"
        );

        assert_eq!(&data[..2], &[0x78, 0x01]);
        let mut raw = Vec::new();
        let mut cursor = 2;
        loop {
            let last = data[cursor];
            let length = u16::from_le_bytes([data[cursor + 1], data[cursor + 2]]);
            let complement = u16::from_le_bytes([data[cursor + 3], data[cursor + 4]]);
            assert_eq!(complement, !length, "block length check");
            assert!(last <= 1);
            raw.extend_from_slice(&data[cursor + 5..cursor + 5 + length as usize]);
            cursor += 5 + length as usize;
            if last == 1 {
                break;
            }
        }
        let checksum = u32::from_be_bytes(data[cursor..cursor + 4].try_into().unwrap());
        assert_eq!(checksum, adler32(&raw), "adler of the decompressed data");
        assert_eq!(cursor + 4, data.len());

        let row = width as usize * 3;
        let mut pixels = Vec::new();
        for line in raw.chunks(row + 1) {
            assert_eq!(line[0], 0, "filter type none");
            pixels.extend_from_slice(&line[1..]);
        }
        (width, height, pixels)
    }

    #[test]
    fn what_goes_in_comes_out_for_several_shapes() {
        for (width, height) in [(1, 1), (3, 2), (7, 5), (300, 70), (200, 100)] {
            let rgb: Vec<u8> = (0..width * height * 3)
                .map(|i| (i * 31 + i / 7) as u8)
                .collect();
            let png = encode(width as u32, height as u32, &rgb).unwrap();
            assert_eq!(
                decode(&png),
                (width as u32, height as u32, rgb),
                "{width}x{height}"
            );
        }
    }

    #[test]
    fn adler_matches_the_definition_on_long_inputs() {
        let mut state = 1u64;
        let data: Vec<u8> = (0..200_000)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                (state >> 56) as u8
            })
            .collect();
        let n = data.len() as u64;
        let a = (1 + data.iter().map(|&d| u64::from(d)).sum::<u64>()) % 65_521;
        let b = (n + data
            .iter()
            .enumerate()
            .map(|(i, &d)| (n - i as u64) * u64::from(d))
            .sum::<u64>())
            % 65_521;
        assert_eq!(adler32(&data), ((b << 16) | a) as u32);
    }
}
