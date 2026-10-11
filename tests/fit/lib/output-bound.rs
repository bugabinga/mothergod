pub fn idiom(frame: &[u8]) -> Result<Vec<u8>, mothergod::Error> {
    mothergod::decompress_bounded(frame, 1 << 20)
}
