use std::io::Read;

pub fn idiom(source: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    mothergod::read::Encoder::new(source).read_to_end(&mut out)?;
    Ok(out)
}
