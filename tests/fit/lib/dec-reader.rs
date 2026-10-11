use std::io::Read;

pub fn idiom(frame: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    mothergod::read::Decoder::new(frame).read_to_end(&mut out)?;
    Ok(out)
}
