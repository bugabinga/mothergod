use super::*;

#[test]
fn push_then_get_reads_back_recent_bytes() {
    let mut window = Window::try_new(Reach::Standard, usize::MAX).unwrap();
    for byte in 0..10u8 {
        window.push(byte);
    }
    assert_eq!(window.written_len(), 10);
    // The byte pushed most recently (9) is 1 back; the first (0) is 10
    // back.
    assert_eq!(window.get(1), 9);
    assert_eq!(window.get(10), 0);
    assert_eq!(window.get(5), 5);
}

#[test]
fn get_matches_copy_checked_over_an_overlapping_run() {
    // Same shape as `copy_checked`'s own doc comment: a distance
    // shorter than the run length must reproduce a repeating pattern,
    // reading bytes this same loop just wrote.
    let mut window = Window::try_new(Reach::Standard, usize::MAX).unwrap();
    for byte in *b"ab" {
        window.push(byte);
    }
    let distance = 2usize;
    let mut produced = Vec::new();
    for _ in 0..7 {
        let byte = window.get(distance);
        window.push(byte);
        produced.push(byte);
    }
    assert_eq!(produced, b"abababa");
}

#[test]
fn wraps_past_capacity_without_disturbing_still_in_range_bytes() {
    for reach in [Reach::Standard, Reach::Wide, Reach::Far] {
        let window_len = reach.window();
        let mut window = Window::try_new(reach, usize::MAX).unwrap();
        for i in 0..(window_len + 5) {
            // Pack i into a byte so wrap-around is visible in the pattern.
            window.push(u8::try_from(i % 256).unwrap());
        }
        assert_eq!(window.written_len(), window_len + 5);
        // The 5 most recent bytes are still exactly what was pushed.
        for back in 1..=5usize {
            let pushed_index = window_len + 5 - back;
            assert_eq!(window.get(back), u8::try_from(pushed_index % 256).unwrap());
        }
        // The widest legal distance still reads the byte pushed that far
        // back: the ring's capacity is exactly the reach.
        assert_eq!(window.get(window_len), 5, "{reach:?}");
    }
}

#[test]
fn capacity_is_the_reach_or_the_declared_length_whichever_is_shorter() {
    // A hostile 10-byte frame declaring the far reach costs 10 bytes of
    // ring, not 16 MiB; an empty one still gets a nonzero modulus.
    for (reach, declared_len, capacity) in [
        (Reach::Far, 10, 10),
        (Reach::Far, 0, 1),
        (Reach::Standard, WINDOW + 1, WINDOW),
        (Reach::Far, usize::MAX, FAR_WINDOW),
    ] {
        let window = Window::try_new(reach, declared_len).unwrap();
        assert_eq!(window.buf.len(), capacity, "{reach:?} at {declared_len}");
    }
}

#[test]
fn a_ring_sized_by_the_declared_length_reads_back_its_oldest_byte() {
    // The ring is exactly the output, so the widest distance a sink lets
    // through (every byte written so far) still reads the first byte.
    let mut window = Window::try_new(Reach::Far, 6).unwrap();
    for byte in *b"abcdef" {
        window.push(byte);
    }
    assert_eq!(window.get(6), b'a');
    assert_eq!(window.get(1), b'f');
}
