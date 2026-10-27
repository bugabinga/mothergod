pub fn idiom(input: &[u8]) -> Vec<u8> {
    mothergod::compress_level(input, mothergod::Level::Fast)
}
