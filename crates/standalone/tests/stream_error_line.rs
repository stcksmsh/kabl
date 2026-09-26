//! D03-R2: the stream-error log line gives the age of the last delivered message, so it is
//! never read as belonging to the interval whose counts it follows.

/// D03-R2: the last delivered message is printed with its age, never as if it belonged to
/// the interval whose counts it follows.
#[test]
fn the_last_delivered_message_carries_its_age() {
    let line = kabl_standalone::stream_error_line(10.0, "BackendError=3", Some(("gone", 42.04)), 3);
    assert_eq!(
        line,
        "stream errors in 10.0s: BackendError=3; last delivered message (taken 42.0s ago, \
         possibly before this interval): gone; not delivered (queue full): 3"
    );
    let line = kabl_standalone::stream_error_line(10.0, "Other=1", None, 1);
    assert!(line.contains("no message delivered yet"), "{line}");
}
