use super::*;

#[test]
fn each_host_failure_reads_as_one_plain_sentence() {
    let sentences = [
        (HostFailureCode::UiBusy, "herdr is busy, try again"),
        (HostFailureCode::Rejected, "herdr refused the request"),
        (HostFailureCode::Spawn, "could not start herdr"),
        (HostFailureCode::Io, "could not talk to herdr"),
        (HostFailureCode::Exit, "herdr exited with an error"),
        (
            HostFailureCode::Malformed,
            "herdr sent a reply that could not be read",
        ),
        (
            HostFailureCode::Oversized,
            "herdr sent a reply that is too large",
        ),
        (HostFailureCode::Timeout, "herdr did not answer in time"),
    ];
    for (code, sentence) in sentences {
        assert_eq!(HostFailure::new(code).to_string(), sentence);
    }
}
