pub fn idiom(source: &[u8], sink: &mut Vec<u8>) -> std::io::Result<()> {
    mothergod::copy_encode(source, sink)
}
