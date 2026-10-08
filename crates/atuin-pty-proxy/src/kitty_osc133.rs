//! Streaming OSC 133 compatibility filtering for Kitty.
//!
//! Only the terminal-forwarding stream is filtered. Command capture receives the
//! original PTY bytes. A malformed or oversized OSC 133 sequence is discarded
//! rather than forwarded with potentially incompatible parameters.

const PREFIX: &[u8] = b"\x1b]133;";
const MAX_SEQUENCE_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy)]
enum State {
    Ground,
    Prefix(usize),
    Osc,
    OscEsc,
}

/// Filters Atuin-specific OSC 133 command-finished parameters before forwarding
/// output to Kitty. Streaming across arbitrary PTY read boundaries.
pub(crate) struct KittyOsc133Filter {
    state: State,
    pending: Vec<u8>,
}

impl KittyOsc133Filter {
    pub(crate) fn new() -> Self {
        Self { state: State::Ground, pending: Vec::new() }
    }

    pub(crate) fn push(&mut self, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len());
        for &byte in data {
            self.accept(byte, &mut out);
        }
        out
    }

    fn accept(&mut self, byte: u8, out: &mut Vec<u8>) {
        match self.state {
            State::Ground => {
                if byte == PREFIX[0] {
                    self.pending.push(byte);
                    self.state = State::Prefix(1);
                } else {
                    out.push(byte);
                }
            }
            State::Prefix(pos) => {
                if byte == PREFIX[pos] {
                    self.pending.push(byte);
                    if pos + 1 == PREFIX.len() {
                        self.state = State::Osc;
                    } else {
                        self.state = State::Prefix(pos + 1);
                    }
                } else {
                    out.append(&mut self.pending);
                    self.state = State::Ground;
                    self.accept(byte, out);
                }
            }
            State::Osc => {
                self.pending.push(byte);
                if byte == b'\x07' || byte == b'\x9c' {
                    self.complete(out);
                } else if byte == b'\x1b' {
                    self.state = State::OscEsc;
                } else {
                    self.enforce_limit();
                }
            }
            State::OscEsc => {
                if byte == b'\\' {
                    self.pending.push(byte);
                    self.complete(out);
                } else {
                    // Unexpected ESC inside OSC: abort the malformed sequence.
                    // Its bytes are deliberately not forwarded to Kitty.
                    self.pending.clear();
                    self.state = State::Ground;
                    self.accept(b'\x1b', out);
                    self.accept(byte, out);
                }
            }
        }
    }

    fn enforce_limit(&mut self) {
        if self.pending.len() > MAX_SEQUENCE_BYTES {
            self.pending.clear();
            self.state = State::Ground;
        }
    }

    fn complete(&mut self, out: &mut Vec<u8>) {
        let marker = std::mem::take(&mut self.pending);
        self.state = State::Ground;
        let terminator_len = if marker.ends_with(b"\x1b\\") { 2 } else { 1 };
        let body = &marker[PREFIX.len()..marker.len() - terminator_len];

        if let Some(rest) = body.strip_prefix(b"D;") {
            if let Some(status_end) = rest.iter().position(|&b| b == b';') {
                let status = &rest[..status_end];
                let params = &rest[status_end + 1..];
                if params.split(|&b| b == b';').any(|p| p.starts_with(b"history_id=")) {
                    // Kitty expects the exit code without Atuin's metadata.
                    // Drop all marker parameters, since other arbitrary fields
                    // are not guaranteed to be understood by Kitty.
                    out.extend_from_slice(PREFIX);
                    out.extend_from_slice(b"D;");
                    out.extend_from_slice(status);
                    out.extend_from_slice(&marker[marker.len() - terminator_len..]);
                    return;
                }
            }
        }

        out.extend_from_slice(&marker);
    }

    pub(crate) fn finish(&mut self) -> Vec<u8> {
        let bytes = if matches!(self.state, State::Osc | State::OscEsc) {
            // Do not leak an incomplete OSC 133 sequence on shutdown.
            Vec::new()
        } else {
            std::mem::take(&mut self.pending)
        };
        self.pending.clear();
        self.state = State::Ground;
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filtered(chunks: &[&[u8]]) -> Vec<u8> {
        let mut f = KittyOsc133Filter::new();
        let mut output = Vec::new();
        for chunk in chunks {
            output.extend(f.push(chunk));
        }
        output.extend(f.finish());
        output
    }

    #[test]
    fn strips_atuin_finished_parameters() {
        assert_eq!(
            filtered(&[b"\x1b]133;D;42;history_id=018f;session_id=abcd\x07"]),
            b"\x1b]133;D;42\x07"
        );
    }

    #[test]
    fn preserves_other_markers_and_terminal_bytes() {
        let src = b"before\x1b[31mred\x1b[0m\x1b]133;C;cmdline=false\x07\x1b]133;D;42\x07after";
        assert_eq!(filtered(&[src]), src);
    }

    #[test]
    fn split_markers_and_st_terminators() {
        assert_eq!(
            filtered(&[b"hello\x1b]133;D", b";7;history_id=abc\x1b", b"\\world"]),
            b"hello\x1b]133;D;7\x1b\\world"
        );
    }

    #[test]
    fn c1_st_terminator() {
        assert_eq!(filtered(&[b"\x1b]133;D;0;history_id=x\x9c"]), b"\x1b]133;D;0\x9c");
    }

    #[test]
    fn preserves_partial_prefix_but_drops_partial_osc() {
        assert_eq!(filtered(&[b"hi\x1b]13"]), b"hi\x1b]13");
        assert_eq!(filtered(&[b"hi\x1b]133;D;0;history_id=x"]), b"hi");
    }

    #[test]
    fn recovers_from_nested_osc() {
        assert_eq!(
            filtered(&[b"a\x1b]133;D;0;history_id=bad\x1b]133;D;1;history_id=good\x07z"]),
            b"a\x1b]133;D;1\x07z"
        );
    }

    #[test]
    fn oversized_sequence_is_not_forwarded() {
        let mut src = b"a\x1b]133;D;0;history_id=".to_vec();
        src.extend(std::iter::repeat_n(b'x', MAX_SEQUENCE_BYTES + 1));
        src.push(b'\x07');
        src.push(b'z');
        let got = filtered(&[&src]);
        assert!(got.ends_with(b"z"));
        assert!(!got.windows(b"history_id=".len()).any(|w| w == b"history_id="));
    }

    #[test]
    fn every_split_produces_identical_output() {
        let examples: &[&[u8]] = &[
            b"hello\x1b]133;D;42;history_id=018f\x07world",
            b"x\x1b]133;D;1;history_id=bad\x1b]133;D;2;history_id=ok\x1b\\y",
            b"\x1b[31mred\x1b[0m\x1b]133;C\x07",
            b"hello\x1b]13",
        ];
        for &input in examples {
            let whole = filtered(&[input]);
            for at in 0..=input.len() {
                assert_eq!(filtered(&[&input[..at], &input[at..]]), whole, "split at {at}");
            }
        }
    }

    #[test]
    fn randomized_chunk_partitions_preserve_result() {
        // Deterministic pseudo-random chunk sizes; no additional dependencies.
        let data = b"x\x1b]133;D;0;history_id=abc;session_id=def\x07\x1b[31mred\x1b[0my";
        let expected = filtered(&[data]);
        for seed in 1..128_u64 {
            let mut n = seed;
            let mut offset = 0;
            let mut f = KittyOsc133Filter::new();
            let mut actual = Vec::new();
            while offset < data.len() {
                n ^= n << 13;
                n ^= n >> 7;
                n ^= n << 17;
                let end = (offset + (n as usize % 11) + 1).min(data.len());
                actual.extend(f.push(&data[offset..end]));
                offset = end;
            }
            actual.extend(f.finish());
            assert_eq!(actual, expected, "seed {seed}");
        }
    }
}
