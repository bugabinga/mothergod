pub fn idiom(frame: &[u8], sink: &mut Vec<u8>) -> Result<(), mothergod::WriteError> {
    mothergod::decompress_to_writer(frame, 1 << 20, sink)
}
