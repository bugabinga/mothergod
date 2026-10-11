use std::io::Write;

pub fn idiom(sink: Vec<u8>, frame: &[u8]) -> std::io::Result<()> {
    let mut decoder = mothergod::write::Decoder::new(sink);
    decoder.write_all(frame)
}
