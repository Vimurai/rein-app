//! A tab's activity (status.md §Components 1, §Data Model `Activity`, D-062; E-76): from the
//! status script's `#{window_activity}` of the window the tab's client shows and the clock of
//! the machine tmux runs on — both read on that one machine, so on Windows both come from
//! inside WSL and no clock is compared across the boundary.
//!
//! E-76's read (b): a working Claude Code agent prints at least once a second (its spinner and
//! timer redraw, also through a tool run of minutes); idle at its prompt and at a permission
//! prompt it prints nothing. So `quiet_s` ≥ [`WORK_GAP_S`] separates working from not.
//! `window_activity` moves on pane output only — not on a status-line redraw, a resize or a
//! key that prints nothing (read (a), tmux 3.2a / 3.6 / 3.8).
//!
//! Pure: every sample carries its own clock; nothing here reads the time.

use serde::Serialize;

/// Gaps in output shorter than this are part of one stretch of work; at or over it the tab
/// is quiet (component 2: `working` below, `quiet` at or over).
pub const WORK_GAP_S: u64 = 15;

/// What the core keeps per tab.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tracker {
    /// The window's last output (tmux's `window_activity`), as last read.
    last_output: Option<u64>,
    /// When the stretch of output that ended at `last_output` began, as far as the samples
    /// can tell.
    began: Option<u64>,
    /// The clock of the previous sample.
    sampled: Option<u64>,
}

/// `Activity` of the data model, as the status and the notifications read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Activity {
    /// The window's last output, on the clock of the machine tmux runs on.
    pub last_output: Option<u64>,
    /// Seconds since the last output (0 when the clock is behind it).
    pub quiet_s: Option<u32>,
    /// How long output had been arriving, gaps under 15 s ignored, up to the last output.
    pub worked_s: u32,
    /// Whether an agent waits for the operator: reserved for rein's signal (D-062 addendum 1);
    /// nothing reads it yet.
    pub waiting: Option<bool>,
}

/// Where a stretch of work that a sample has just seen begins (status.md §Components 1 "Where
/// a stretch of work begins", D-062 addendum 5; E-96): its first output lies after the
/// previous sample, so the previous sample's clock + 1 s — not earlier than 14 s before the
/// output just read (one sighting never reads 15 s or more, whatever the gap between the two
/// samples) and not later than that output.
fn stretch_start(previous_sample: Option<u64>, output: u64) -> u64 {
    let earliest = output.saturating_sub(WORK_GAP_S - 1);
    previous_sample
        .map_or(output, |s| s.saturating_add(1))
        .clamp(earliest, output)
}

impl Tracker {
    /// One sample: `activity` is `#{window_activity}`, `clock` the same machine's time.
    pub fn sample(&mut self, activity: u64, clock: u64) {
        match self.last_output {
            // The first sample, or the client now shows another window whose output is
            // older: a new start — how long it had worked before is not known.
            None => self.restart(activity),
            Some(last) if activity < last => self.restart(activity),
            Some(last) if activity > last => {
                // New output since the previous sample. It continues the stretch unless the
                // window had already been quiet for the gap by the previous sample.
                let quiet_before = self.sampled.map_or(0, |s| s.saturating_sub(last));
                if quiet_before >= WORK_GAP_S {
                    self.began = Some(stretch_start(self.sampled, activity));
                }
                self.last_output = Some(activity);
            }
            Some(_) => {}
        }
        self.sampled = Some(clock);
    }

    fn restart(&mut self, activity: u64) {
        self.last_output = Some(activity);
        self.began = Some(activity);
    }

    /// The activity as of `clock`.
    pub fn at(&self, clock: u64) -> Activity {
        let quiet_s = self
            .last_output
            .map(|l| u32::try_from(clock.saturating_sub(l)).unwrap_or(u32::MAX));
        let worked_s = match (self.began, self.last_output) {
            (Some(b), Some(l)) => u32::try_from(l.saturating_sub(b)).unwrap_or(u32::MAX),
            _ => 0,
        };
        Activity {
            last_output: self.last_output,
            quiet_s,
            worked_s,
            waiting: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scripted series: (window_activity, clock) every 5 s, as the poll reads it.
    fn run(series: &[(u64, u64)]) -> Vec<(Option<u32>, u32)> {
        let mut t = Tracker::default();
        series
            .iter()
            .map(|&(a, c)| {
                t.sample(a, c);
                let x = t.at(c);
                (x.quiet_s, x.worked_s)
            })
            .collect()
    }

    #[test]
    fn quiet_and_worked_over_a_scripted_series() {
        assert_eq!(Tracker::default().at(100).quiet_s, None, "never read");
        assert_eq!(
            run(&[
                (1000, 1000), // first sample: output now, nothing known before
                (1004, 1005), // working: output every second
                (1009, 1010),
                (1014, 1015),
                (1019, 1020), // the last output
                (1019, 1025), // quiet 6 s: still part of the stretch if it resumes
                (1029, 1030), // resumes (the gap was 6..10 s, under 15)
                (1029, 1035),
                (1029, 1040),
                (1029, 1045), // quiet 16 s
                (1029, 1060), // quiet 31 s; worked 29 s, from 1000 to 1029
                (1062, 1065), // new output after quiet ≥ 15 s: a new stretch from 1061 (E-96)
                (1068, 1070),
            ]),
            [
                (Some(0), 0),
                (Some(1), 4),
                (Some(1), 9),
                (Some(1), 14),
                (Some(1), 19),
                (Some(6), 19),
                (Some(1), 29),
                (Some(6), 29),
                (Some(11), 29),
                (Some(16), 29),
                (Some(31), 29),
                (Some(3), 1),
                (Some(2), 7),
            ]
        );
    }

    /// A burst of `work_s` seconds of output (a line every second) starting `offset` s after a
    /// sample, read by samples every `period` s on a window quiet long before: the largest
    /// `worked_s` the tracker reports.
    fn burst(period: u64, offset: u64, work_s: u64) -> u32 {
        let t0 = 10_000;
        let start = t0 + offset;
        let last = start + work_s;
        let mut t = Tracker::default();
        let mut most = 0;
        let mut clock = t0;
        while clock <= last + 3 * period {
            // tmux's window_activity: the newest output at or before the sample.
            let activity = if clock < start {
                t0 - 600
            } else {
                clock.min(last)
            };
            t.sample(activity, clock);
            most = most.max(t.at(clock).worked_s);
            clock += period;
        }
        most
    }

    /// E-96 (D-062 addendum 5) (1): work of 15 s or more reaches 15 s at every start offset
    /// after a sample, at the hidden period (15 s) and the shown one (5 s).
    #[test]
    fn work_of_fifteen_seconds_or_more_reads_fifteen_at_every_offset() {
        for (period, work) in [(15, 20), (15, 15), (5, 20), (5, 15)] {
            for offset in 1..=period {
                let w = burst(period, offset, work);
                assert!(
                    w >= 15,
                    "period {period}, {work} s burst at +{offset} s: worked {w}"
                );
            }
        }
    }

    /// E-96 (2): output seen by one sample alone reads at most 14 s, whatever the gap
    /// between that sample and the one before (a late poll, a machine that slept).
    #[test]
    fn one_sighting_reads_at_most_fourteen_seconds() {
        for gap in [5, 15, 3600] {
            let mut t = Tracker::default();
            t.sample(1_000, 2_000); // quiet for 1000 s
            let out = 2_000 + gap - 1; // one line, just before the next sample
            t.sample(out, 2_000 + gap);
            t.sample(out, 2_000 + gap + 15);
            let w = t.at(2_000 + gap + 15).worked_s;
            assert!(w <= 14, "gap {gap}: worked {w}");
        }
    }

    #[test]
    fn another_window_or_a_clock_behind_the_output() {
        // The client now shows another window whose last output is older: a new start.
        assert_eq!(
            run(&[(2000, 2000), (2010, 2010), (1500, 2015)])[2],
            (Some(515), 0)
        );
        // A clock read just before tmux's own second ticked: quiet 0, never negative.
        assert_eq!(run(&[(3000, 2999)]), [(Some(0), 0)]);
        // Hidden-window polling every 15 s: a gap the samples cannot see is not invented.
        assert_eq!(run(&[(10, 10), (24, 25), (39, 40)])[2], (Some(1), 29));
    }
}
