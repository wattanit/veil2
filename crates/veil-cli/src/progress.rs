//! Progress on standard error, and cancellation from an interrupt
//! (Design §3.4; A-3, FR-15, FR-20).

use std::io::{IsTerminal, Write};
use std::time::{Duration, Instant};

use veil_core::{Cancel, Progress, ProgressReport, Unit};

use crate::output::{count, human_size};

/// How often a line is written when nothing is watching. Initial; tune with
/// use.
const OFF_TERMINAL_INTERVAL: Duration = Duration::from_secs(2);

/// Writes progress to standard error, in place on a terminal and as periodic
/// lines off one.
///
/// Off a terminal it emits no control characters at all: a log full of
/// carriage returns is the reason Design §3.4 has this rule.
pub struct Stderr {
    verb: &'static str,
    terminal: bool,
    last: Option<Instant>,
    drawn: usize,
}

impl Stderr {
    /// A sink labelled with what is happening.
    #[must_use]
    pub fn new(verb: &'static str) -> Self {
        Self {
            verb,
            terminal: std::io::stderr().is_terminal(),
            // Starts the clock now rather than at the first report, so an
            // operation shorter than the interval says nothing at all. A line
            // of progress for an eleven-byte file is noise in a log.
            last: Some(Instant::now()),
            drawn: 0,
        }
    }

    /// Clears the in-place line, so a result never lands on top of progress.
    pub fn finish(&mut self) {
        if self.terminal && self.drawn > 0 {
            let mut err = std::io::stderr().lock();
            let _ = write!(err, "\r{:width$}\r", "", width = self.drawn);
            let _ = err.flush();
        }
    }
}

impl Progress for Stderr {
    fn report(&mut self, report: ProgressReport) {
        let line = match (report.unit, report.total) {
            (Unit::Bytes, Some(total)) => format!(
                "{} {} of {}",
                self.verb,
                human_size(report.done),
                human_size(total)
            ),
            (Unit::Bytes, None) => format!("{} {}", self.verb, human_size(report.done)),
            (Unit::Entries, Some(total)) => format!(
                "{} {} of {} files",
                self.verb,
                count(report.done),
                count(total)
            ),
            (Unit::Entries, None) => {
                format!("{} {} files", self.verb, count(report.done))
            }
        };

        let mut err = std::io::stderr().lock();
        if self.terminal {
            let _ = write!(err, "\r{line:width$}", width = self.drawn);
            let _ = err.flush();
            self.drawn = line.chars().count();
        } else {
            let now = Instant::now();
            if self
                .last
                .is_some_and(|t| now.duration_since(t) < OFF_TERMINAL_INTERVAL)
            {
                return;
            }
            self.last = Some(now);
            let _ = writeln!(err, "{line}");
        }
    }
}

/// A cancellation token that an interrupt sets.
///
/// Without this the core's cancellation is unreachable from the command line:
/// an interrupt would kill the process, which HC-4 makes safe but which leaves
/// the stronger guarantee — FR-15's *as though it had not been started* —
/// unavailable to the only frontend that exists.
///
/// Returns an uncancellable token if the handler cannot be installed. Failing
/// the command over that would be worse: the operation is still safe to
/// interrupt, it just stops less tidily.
#[must_use]
pub fn cancel_on_interrupt() -> Cancel {
    let cancel = Cancel::new();
    let handler = cancel.clone();
    let _ = ctrlc::set_handler(move || handler.cancel());
    cancel
}
