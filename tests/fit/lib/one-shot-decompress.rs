pub fn idiom(frame: &[u8]) -> Result<Vec<u8>, mothergod::Error> {
    mothergod::decompress(frame)
}
