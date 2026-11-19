use super::*;

/// `window.copy_back(distance, len)`'s emitted runs, joined.
fn copy_back(window: &mut Window, distance: usize, len: usize) -> Result<Vec<u8>, Error> {
    let mut emitted = Vec::new();
    window.copy_back(distance, len, |run| {
        emitted.extend_from_slice(run);
        Ok::<(), Error>(())
    })?;
    Ok(emitted)
}

#[test]
fn push_then_copy_back_reads_back_recent_bytes() {
    let mut window = Window::try_new(Reach::Standard, usize::MAX).unwrap();
    for byte in 0..10u8 {
        window.push(byte);
    }
    assert_eq!(window.written_len(), 10);
    // The first byte pushed (0) is 10 back; the copy lands after the 9.
    assert_eq!(copy_back(&mut window, 10, 3), Ok(vec![0, 1, 2]));
    assert_eq!(window.written_len(), 13);
    // The byte pushed most recently is the copy's last, 1 back.
    assert_eq!(copy_back(&mut window, 1, 1), Ok(vec![2]));
}

#[test]
fn copy_back_matches_copy_checked_over_an_overlapping_run() {
    // Same shape as `copy_checked`'s own doc comment: a distance
    // shorter than the run length must reproduce a repeating pattern,
    // reading bytes this same copy just wrote.
    let mut window = Window::try_new(Reach::Standard, usize::MAX).unwrap();
    for byte in *b"ab" {
        window.push(byte);
    }
    assert_eq!(copy_back(&mut window, 2, 7), Ok(b"abababa".to_vec()));
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
        let recent: Vec<u8> = (window_len..window_len + 5)
            .map(|i| u8::try_from(i % 256).unwrap())
            .collect();
        assert_eq!(copy_back(&mut window, 5, 5), Ok(recent), "{reach:?}");
        // The widest legal distance still reads the byte pushed that far
        // back, the 11th: the ring's capacity is exactly the reach.
        assert_eq!(
            copy_back(&mut window, window_len, 1),
            Ok(vec![10]),
            "{reach:?}"
        );
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
    assert_eq!(copy_back(&mut window, 6, 1), Ok(b"a".to_vec()));
}

#[test]
fn copy_back_rejects_a_distance_with_no_source_and_writes_nothing() {
    // Capacity 4 over 6 bytes written: zero names no byte, 7 reaches
    // before the first, and 5 reaches a slot already overwritten.
    let mut window = Window::try_new(Reach::Far, 4).unwrap();
    for byte in *b"abcdef" {
        window.push(byte);
    }
    for distance in [0, 7, 5] {
        assert_eq!(
            copy_back(&mut window, distance, 1),
            Err(Error::Corrupt),
            "distance {distance}"
        );
        assert_eq!(window.written_len(), 6, "distance {distance}");
    }
    // The capacity itself is still legal.
    assert_eq!(copy_back(&mut window, 4, 1), Ok(b"c".to_vec()));
}

#[test]
fn copy_back_returns_the_first_emit_error_after_its_run_lands() {
    // A ring of 4 after "abcdef" writes its next byte at slot 2, so a
    // copy of 3 lands two bytes before the ring's end, then one; the first
    // run's error stops it there.
    let mut window = Window::try_new(Reach::Far, 4).unwrap();
    for byte in *b"abcdef" {
        window.push(byte);
    }
    let mut runs = Vec::new();
    let result = window.copy_back(4, 3, |run| {
        runs.push(run.to_vec());
        Err(Error::Truncated)
    });
    assert_eq!(result, Err(Error::Truncated));
    assert_eq!(runs, [b"cd".to_vec()]);
    assert_eq!(window.written_len(), 8);
}
