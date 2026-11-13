//! The print engine: what a printer does with the commands it read, over time.
//!
//! A real printer has three parts, and this is the middle one:
//!
//! 1. The *receive buffer* takes the bytes a program sends, as many as it has room for. A program
//!    that sends more has to wait.
//! 2. The *engine* works through the commands in the order they came. Each takes the time its paper
//!    needs to come out, and the bytes of a command leave the buffer when it is done. So the buffer
//!    empties at the speed of the printing, and the program is let go at that speed.
//! 3. The *paper* shows what was printed so far, and a receipt is finished by its cut.
//!
//! Nothing here reads a socket, a clock or the hub. The engine is told how much time passed, and
//! says what was done in it, so every result can be tested exactly. The layers around it do the
//! rest: [`crate::session`] decodes the bytes and plans the work, [`crate::printer`] holds the
//! engine of one printer, and [`crate::hub`] turns what was printed into receipts and events.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use stubprint_core::{Event, Profile, Pushed, Receipt, Step};
use stubprint_render::{blocks, Options};

/// How long the cutter takes to cut, when the printer takes time to print.
const CUT_TIME: Duration = Duration::from_millis(150);

/// How fast the paper moves, and how many dots an inch of it has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pacing {
    /// Millimetres of paper a second. 0 takes no time.
    pub speed: usize,
    /// Dots to the inch: most thermal printers print 203.
    pub dpi: u32,
}

impl Pacing {
    /// The pacing of a printer that prints at `speed` millimetres a second, when it is of this model.
    pub fn of(profile: &Profile, speed: usize) -> Self {
        Self {
            speed,
            dpi: profile.dpi.filter(|dpi| *dpi > 0).unwrap_or(203),
        }
    }

    /// How long it takes to bring out this many dots of paper.
    pub fn time_for(&self, dots: u32) -> Duration {
        if self.speed == 0 {
            return Duration::ZERO;
        }
        let millimetres = f64::from(dots) / f64::from(self.dpi) * 25.4;
        Duration::try_from_secs_f64(millimetres / speed_as_f64(self.speed)).unwrap_or_default()
    }
}

fn speed_as_f64(speed: usize) -> f64 {
    f64::from(u32::try_from(speed).unwrap_or(u32::MAX))
}

/// One thing for the engine to do.
#[derive(Debug, Clone)]
pub struct Work {
    /// The connection whose bytes it came from.
    pub connection: u64,
    /// How long it takes.
    pub cost: Duration,
    /// How many bytes leave the receive buffer when it is done.
    pub freed: usize,
    pub what: What,
}

#[derive(Debug, Clone)]
pub enum What {
    /// A command is carried out: the paper moves, the cutter cuts, the drawer is kicked.
    Command {
        event: Arc<Event>,
        step: Step,
        /// The bytes the command was made of.
        bytes: Vec<u8>,
        /// The model that read it, which is also the model that draws it.
        profile: Arc<Profile>,
    },
    /// A receipt is finished and goes to the hub. It takes no time of its own.
    Finished {
        receipt: Receipt,
        profile: Arc<Profile>,
    },
}

/// What a job has printed so far: the commands that are done, with their bytes. It is the receipt
/// as it is on the paper while the printer is still printing it.
#[derive(Debug, Default)]
pub struct Job {
    events: Vec<Event>,
    steps: Vec<Step>,
    raw: Vec<u8>,
    /// Where the first command of the job started, in what the connection sent.
    origin: Option<usize>,
    prints: bool,
}

impl Job {
    /// A command is done: the paper has what it asked for.
    pub fn add(&mut self, event: &Event, step: &Step, bytes: &[u8]) {
        let origin = *self
            .origin
            .get_or_insert_with(|| step.end.saturating_sub(bytes.len()));
        self.prints |= event.prints();
        self.events.push(event.clone());
        // A receipt counts the ends of its commands from its own start.
        self.steps.push(Step {
            end: step.end.saturating_sub(origin),
            state: step.state.clone(),
        });
        self.raw.extend_from_slice(bytes);
    }

    /// Whether anything is on the paper yet. A job of commands that print nothing is not a receipt.
    pub fn has_printed(&self) -> bool {
        self.prints
    }

    /// The receipt as it is now.
    pub fn receipt(&self) -> Receipt {
        Receipt::from_parts(self.events.clone(), self.steps.clone(), self.raw.clone())
    }
}

/// The work of one printer, in the order it came, with the bytes it still holds.
#[derive(Debug, Default)]
pub struct Engine {
    queue: VecDeque<Work>,
    /// How much of the time of the first one is already spent.
    spent: Duration,
    /// How many bytes the receive buffer holds.
    buffered: usize,
}

impl Engine {
    /// Bytes came into the receive buffer.
    pub fn receive(&mut self, bytes: usize) {
        self.buffered += bytes;
    }

    /// Bytes that no command will ever free, such as the half of a command a program sent before
    /// it went away, leave the buffer.
    pub fn give_back(&mut self, bytes: usize) {
        self.buffered = self.buffered.saturating_sub(bytes);
    }

    /// How many bytes the receive buffer holds.
    pub fn buffered(&self) -> usize {
        self.buffered
    }

    /// How many more bytes fit in a buffer of `capacity`.
    pub fn room(&self, capacity: usize) -> usize {
        capacity.saturating_sub(self.buffered)
    }

    /// Adds work after what is there.
    pub fn push(&mut self, work: impl IntoIterator<Item = Work>) {
        self.queue.extend(work);
    }

    /// Whether there is nothing to do.
    pub fn is_idle(&self) -> bool {
        self.queue.is_empty()
    }

    /// How long until the work it is on is done, or `None` when there is none.
    pub fn due_in(&self) -> Option<Duration> {
        self.queue
            .front()
            .map(|work| work.cost.saturating_sub(self.spent))
    }

    /// Lets `elapsed` time pass and returns what was done in it, in order. Work that takes no time
    /// is done at once. What a printer cannot print, it is not given time for: the caller does not
    /// call this while the printer is off line.
    pub fn advance(&mut self, mut elapsed: Duration) -> Vec<Work> {
        let mut done = Vec::new();
        while let Some(front) = self.queue.front() {
            let left = front.cost.saturating_sub(self.spent);
            if elapsed < left {
                self.spent += elapsed;
                break;
            }
            elapsed -= left;
            self.spent = Duration::ZERO;
            let Some(work) = self.queue.pop_front() else {
                break;
            };
            self.buffered = self.buffered.saturating_sub(work.freed);
            done.push(work);
        }
        done
    }
}

/// Works out what a connection's commands ask of the engine: how long each takes, and which bytes
/// it frees. It keeps the job that is being read, since the paper a command moves depends on the
/// lines before it.
#[derive(Debug, Default)]
pub struct Planner {
    /// The commands of the job being read, which has no cut yet.
    job: Vec<Arc<Event>>,
    /// Where the last command ended in what the connection sent.
    last_end: usize,
}

impl Planner {
    /// Plans what some bytes made. The commands and the receipts they finished come in the order
    /// the printer does them.
    pub fn plan(
        &mut self,
        connection: u64,
        pushed: Pushed,
        profile: &Arc<Profile>,
        speed: usize,
    ) -> Vec<Work> {
        let options = Options::for_profile(profile);
        let pacing = Pacing::of(profile, speed);
        let Pushed {
            traced,
            receipts,
            finished_after,
            chunks,
        } = pushed;
        let mut finished = receipts.into_iter().zip(finished_after).peekable();
        let mut traced = traced.into_iter().zip(chunks).peekable();
        let mut works = Vec::new();
        let mut taken = 0;
        loop {
            // The commands up to where the next receipt is finished, or the rest of them.
            let until = finished.peek().map_or(usize::MAX, |(_, place)| *place);
            let mut segment = Vec::new();
            while taken < until {
                let Some(item) = traced.next() else {
                    break;
                };
                segment.push(item);
                taken += 1;
            }
            self.plan_segment(connection, segment, profile, &options, pacing, &mut works);
            let Some((receipt, _)) = finished.next() else {
                break;
            };
            works.push(Work {
                connection,
                cost: Duration::ZERO,
                freed: 0,
                what: What::Finished {
                    receipt,
                    profile: profile.clone(),
                },
            });
            self.job.clear();
        }
        works
    }

    fn plan_segment(
        &mut self,
        connection: u64,
        segment: Vec<(stubprint_core::Traced, Vec<u8>)>,
        profile: &Arc<Profile>,
        options: &Options,
        pacing: Pacing,
        works: &mut Vec<Work>,
    ) {
        if segment.is_empty() {
            return;
        }
        let before = self.job.len();
        let events: Vec<Arc<Event>> = segment
            .iter()
            .map(|(item, _)| Arc::new(item.event.clone()))
            .collect();
        self.job.extend(events.iter().cloned());
        let marks = blocks::marks(self.job.iter().map(|event| &**event), options);
        let mut mark = before.checked_sub(1).map_or(0, |last| marks[last]);
        for (offset, ((item, bytes), event)) in segment.into_iter().zip(events).enumerate() {
            let next = marks[before + offset];
            let mut cost = pacing.time_for(next - mark);
            if pacing.speed > 0 && matches!(*event, Event::Cut(_)) {
                cost += CUT_TIME;
            }
            mark = next;
            let freed = item.end.saturating_sub(self.last_end);
            self.last_end = item.end;
            works.push(Work {
                connection,
                cost,
                freed,
                what: What::Command {
                    event,
                    step: Step {
                        end: item.end,
                        state: item.state,
                    },
                    bytes,
                    profile: profile.clone(),
                },
            });
        }
    }

    /// Where the last command ended: the bytes after it are not part of a command yet.
    pub fn last_end(&self) -> usize {
        self.last_end
    }

    /// The stream ended, and what was left of the job goes with it.
    pub fn ended(&mut self) {
        self.job.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stubprint_core::Printer;

    const CUT: [u8; 4] = [0x1d, b'V', 0x42, 0x00];

    fn profile() -> Arc<Profile> {
        Arc::new(Profile::generic())
    }

    fn receipt_bytes(lines: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        for number in 0..lines {
            bytes.extend(format!("line {number}\n").bytes());
        }
        bytes.extend(CUT);
        bytes
    }

    /// What a printer read from the bytes, planned at a speed.
    fn planned(bytes: &[u8], speed: usize) -> (Vec<Work>, Planner) {
        let mut printer =
            Printer::with_dialect(stubprint_core::Dialect::EscPos, (*profile()).clone());
        let mut planner = Planner::default();
        let pushed = printer.push_traced(bytes);
        let works = planner.plan(1, pushed, &profile(), speed);
        (works, planner)
    }

    fn total(works: &[Work]) -> Duration {
        works.iter().map(|work| work.cost).sum()
    }

    #[test]
    fn a_printer_that_takes_no_time_does_everything_at_once() {
        let (works, _) = planned(&receipt_bytes(5), 0);
        assert!(works.iter().all(|work| work.cost.is_zero()));
        let mut engine = Engine::default();
        engine.receive(100);
        engine.push(works);
        let done = engine.advance(Duration::ZERO);
        assert!(matches!(done.last().unwrap().what, What::Finished { .. }));
        assert!(engine.is_idle());
    }

    #[test]
    fn each_line_takes_the_time_its_paper_needs_and_the_receipt_takes_the_sum() {
        let (works, _) = planned(&receipt_bytes(10), 100);
        // A line of paper is 30 dots at 203 to the inch: 3.75 mm, which takes 37.5 ms at 100 mm/s.
        let feeds: Vec<Duration> = works
            .iter()
            .filter(|work| matches!(&work.what, What::Command { event, .. } if matches!(**event, Event::LineFeed)))
            .map(|work| work.cost)
            .collect();
        assert_eq!(feeds.len(), 10);
        for cost in &feeds {
            let millis = cost.as_secs_f64() * 1000.0;
            assert!((millis - 37.5).abs() < 8.0, "a line took {millis} ms");
        }
        // The cut takes its own time on top of the paper.
        let expected = feeds.iter().sum::<Duration>() + CUT_TIME;
        assert_eq!(total(&works), expected);
        // The text itself takes no time: the line comes out when it ends.
        assert!(works
            .iter()
            .filter(|work| matches!(&work.what, What::Command { event, .. } if matches!(**event, Event::Text { .. })))
            .all(|work| work.cost.is_zero()));
    }

    #[test]
    fn the_buffer_empties_as_the_printing_goes_and_the_receipt_comes_out_at_the_end() {
        let bytes = receipt_bytes(10);
        let (works, _) = planned(&bytes, 100);
        let whole = total(&works);
        let mut engine = Engine::default();
        engine.receive(bytes.len());
        engine.push(works);

        // Half of the time: some of the bytes are free, the rest are not, and nothing is finished.
        let first = engine.advance(whole / 2);
        assert!(!first.is_empty());
        assert!(first
            .iter()
            .all(|work| matches!(work.what, What::Command { .. })));
        assert!(engine.buffered() > 0 && engine.buffered() < bytes.len());
        assert!(!engine.is_idle());

        // The paper has what came out so far, and not the rest.
        let mut job = Job::default();
        for work in &first {
            if let What::Command {
                event, step, bytes, ..
            } = &work.what
            {
                job.add(event, step, bytes);
            }
        }
        assert_eq!(job.receipt().events().len(), first.len());
        assert!(job.has_printed());

        let rest = engine.advance(whole);
        assert!(matches!(rest.last().unwrap().what, What::Finished { .. }));
        assert_eq!(engine.buffered(), 0);
        assert!(engine.is_idle());
    }

    #[test]
    fn a_job_is_the_receipt_so_far_with_the_bytes_of_its_commands() {
        let bytes = receipt_bytes(4);
        let (works, _) = planned(&bytes, 100);
        let mut job = Job::default();
        let mut done = Vec::new();
        for work in &works {
            if let What::Command {
                event, step, bytes, ..
            } = &work.what
            {
                job.add(event, step, bytes);
                done.push(event.clone());
            }
        }
        let receipt = job.receipt();
        // Once every command is done it is the receipt the printer read.
        let mut reader =
            Printer::with_dialect(stubprint_core::Dialect::EscPos, (*profile()).clone());
        let mut read = reader.push(&bytes);
        let whole = read.remove(0);
        assert_eq!(receipt.events(), whole.events());
        assert_eq!(receipt.raw(), whole.raw());
        assert_eq!(receipt.steps(), whole.steps());
    }

    #[test]
    fn a_job_of_commands_that_print_nothing_is_not_a_receipt_yet() {
        let (works, _) = planned(&CUT, 100);
        let mut job = Job::default();
        for work in &works {
            if let What::Command {
                event, step, bytes, ..
            } = &work.what
            {
                job.add(event, step, bytes);
            }
        }
        assert!(!job.has_printed());
    }

    #[test]
    fn a_command_that_is_not_done_yet_keeps_its_bytes_and_says_how_long_it_has_to_go() {
        let (works, _) = planned(&receipt_bytes(3), 100);
        let mut engine = Engine::default();
        engine.push(works);
        // The text before the first line feed takes no time, so it is done at once.
        let done = engine.advance(Duration::ZERO);
        assert_eq!(done.len(), 1);
        let due = engine.due_in().unwrap();
        assert!(due > Duration::ZERO);
        // A little time does not finish it, and the rest of it is what is due.
        let little = due / 4;
        assert!(engine.advance(little).is_empty());
        assert_eq!(engine.due_in().unwrap(), due - little);
        assert!(!engine.advance(due).is_empty());
    }

    #[test]
    fn what_a_stream_made_in_pieces_costs_the_same_as_what_it_made_whole() {
        let bytes = receipt_bytes(12);
        let (whole, _) = planned(&bytes, 150);

        let mut printer =
            Printer::with_dialect(stubprint_core::Dialect::EscPos, (*profile()).clone());
        let mut planner = Planner::default();
        let mut pieces = Vec::new();
        for chunk in bytes.chunks(7) {
            let pushed = printer.push_traced(chunk);
            pieces.extend(planner.plan(1, pushed, &profile(), 150));
        }
        assert_eq!(pieces.len(), whole.len());
        assert_eq!(total(&pieces), total(&whole));
        let freed = |works: &[Work]| works.iter().map(|work| work.freed).sum::<usize>();
        assert_eq!(freed(&pieces), freed(&whole));
        assert_eq!(freed(&whole), bytes.len());
    }

    #[test]
    fn a_cut_that_prints_nothing_does_not_finish_a_receipt_and_the_next_one_goes_on_from_it() {
        let mut bytes = CUT.to_vec();
        bytes.extend(receipt_bytes(2));
        let (works, planner) = planned(&bytes, 100);
        let finished = works
            .iter()
            .filter(|work| matches!(work.what, What::Finished { .. }))
            .count();
        assert_eq!(finished, 1);
        assert_eq!(planner.last_end(), bytes.len());
    }

    #[test]
    fn a_receipt_finished_by_the_end_of_the_stream_comes_after_its_last_command() {
        let mut printer =
            Printer::with_dialect(stubprint_core::Dialect::EscPos, (*profile()).clone());
        let mut planner = Planner::default();
        let mut works = planner.plan(1, printer.push_traced(b"no cut\n"), &profile(), 100);
        assert!(works
            .iter()
            .all(|work| matches!(work.what, What::Command { .. })));
        works.extend(planner.plan(1, printer.finish_traced(), &profile(), 100));
        assert!(matches!(works.last().unwrap().what, What::Finished { .. }));
    }

    #[test]
    fn the_bytes_a_program_sent_that_never_made_a_command_are_given_back() {
        let mut engine = Engine::default();
        engine.receive(10);
        engine.give_back(3);
        assert_eq!(engine.buffered(), 7);
        engine.give_back(100);
        assert_eq!(engine.buffered(), 0);
        assert_eq!(engine.room(4096), 4096);
    }
}
