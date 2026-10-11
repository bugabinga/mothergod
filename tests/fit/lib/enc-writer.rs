use std::io::Write;

pub fn idiom(sink: Vec<u8>) -> std::io::Result<()> {
    let mut encoder = mothergod::write::Encoder::new(sink);
    encoder.write_all(b"payload")
}
