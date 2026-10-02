use super::*;

#[test]
fn push_then_get_reads_back_recent_bytes() {
    let mut window = Window::try_new().unwrap();
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
    let mut window = Window::try_new().unwrap();
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
    let mut window = Window::try_new().unwrap();
    for i in 0..(WINDOW + 5) {
        // Pack i into a byte so wrap-around is visible in the pattern.
        window.push(u8::try_from(i % 256).unwrap());
    }
    assert_eq!(window.written_len(), WINDOW + 5);
    // The 5 most recent bytes are still exactly what was pushed.
    for back in 1..=5usize {
        let pushed_index = WINDOW + 5 - back;
        assert_eq!(window.get(back), u8::try_from(pushed_index % 256).unwrap());
    }
}
