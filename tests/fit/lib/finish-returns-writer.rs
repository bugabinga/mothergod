pub fn idiom() -> std::io::Result<Vec<u8>> {
    mothergod::write::Encoder::new(Vec::new()).finish()
}
