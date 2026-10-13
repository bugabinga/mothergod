//! The frame check value (ADR-0068): CRC-32 as gzip, zlib and xz define it
//! (reflected polynomial `0xEDB88320`, initial value and final XOR
//! `0xFFFF_FFFF`), so any tool that speaks it reproduces a frame's check
//! from the bytes before it. It detects every burst of 32 bits or fewer,
//! hence every single bit flip, which is the failure the frame guards
//! against; it is no defence against an adversary, who can forge a frame
//! with a matching check at will, and the decoder's bounds never lean on it.
//! `pub` for the sibling crates that build frames by hand (the fuzz targets
//! re-seal a mutated one, so the decoder behind the check stays reachable)
//! and for `tests/`, which share this one implementation.

/// The reflected polynomial of CRC-32/ISO-HDLC.
const POLYNOMIAL: u32 = 0xEDB8_8320;

/// Remainder of each byte value divided by [`POLYNOMIAL`], built at compile
/// time so there is no table to keep in step with it.
const TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut byte = 0;
    while byte < table.len() {
        #[allow(clippy::cast_possible_truncation, reason = "byte < 256")]
        let mut remainder = byte as u32;
        let mut bit = 0;
        while bit < 8 {
            remainder = if remainder % 2 == 1 {
                (remainder / 2) ^ POLYNOMIAL
            } else {
                remainder / 2
            };
            bit += 1;
        }
        table[byte] = remainder;
        byte += 1;
    }
    table
};

/// The CRC-32 of `bytes`.
#[must_use]
pub fn crc32(bytes: &[u8]) -> u32 {
    !bytes.iter().fold(!0u32, |crc, &byte| {
        TABLE[usize::from(crc.to_le_bytes()[0] ^ byte)] ^ (crc >> 8)
    })
}

#[cfg(test)]
mod tests;
