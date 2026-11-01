use std::collections::VecDeque;

const HISTORY_LIMIT: usize = 1_000;
const QUEUE_LIMIT: usize = 1_000;

#[derive(Debug, Clone, Copy)]
pub enum Step {
    Next,
    Prev,
    Current,
}

pub struct Playlist {
    original: Vec<usize>,
    queue: Vec<usize>,
    history: VecDeque<usize>,
    pos: Option<usize>,
    step: Step,
    shuffle: bool,
    repeat: bool,
}

impl Playlist {
    pub fn new() -> Self {
        Self {
            original: Vec::new(),
            queue: Vec::new(),
            history: VecDeque::with_capacity(HISTORY_LIMIT),
            pos: None,
            step: Step::Next,
            shuffle: false,
            repeat: false,
        }
    }

    pub fn current(&self) -> Option<usize> {
        self.pos.and_then(|p| self.queue.get(p)).copied()
    }

    pub fn upcoming(&self) -> &[usize] {
        &self.queue[self.upcoming_start()..]
    }

    pub fn history(&self) -> &VecDeque<usize> {
        &self.history
    }

    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    pub fn repeat(&self) -> bool {
        self.repeat
    }

    pub fn has_prev(&self) -> bool {
        self.pos.is_some_and(|p| p > 0)
    }

    pub fn set_step(&mut self, step: Step) {
        self.step = step;
    }

    pub fn toggle_repeat(&mut self) {
        self.repeat = !self.repeat;
    }

    pub fn advance(&mut self, mut play: impl FnMut(usize) -> bool) {
        let mut target = match self.step {
            Step::Next => self.next_cursor(),
            Step::Prev => self.pos.and_then(|p| p.checked_sub(1)),
            Step::Current => self.pos,
        };

        self.step = Step::Next;

        for _ in 0..=self.original.len() {
            let Some(pos) = target else { break };
            if let Some(&song) = self.queue.get(pos)
                && play(song)
            {
                self.record(song);
                self.pos = Some(pos);
                break;
            }
            self.pos = Some(pos);
            target = self.next_cursor();
        }
    }

    pub fn start(&mut self, songs: Vec<usize>) {
        self.original = songs;

        self.queue.truncate(self.upcoming_start());

        let mut ctx = self.original.clone();
        if self.shuffle {
            fastrand::shuffle(&mut ctx[1..]);
        }

        if self.queue.last() == ctx.first() {
            self.queue.extend_from_slice(&ctx[1..]);
            self.step = Step::Current;
        } else {
            self.queue.extend(ctx);
            self.step = Step::Next;
        }
    }

    pub fn skip_upcoming(&mut self, count: usize) -> bool {
        let start = self.upcoming_start();
        if start + count >= self.queue.len() {
            return false;
        }

        self.queue.drain(start..start + count);
        true
    }

    pub fn toggle_shuffle(&mut self) {
        self.shuffle = !self.shuffle;
        let rest = self.rest_of_original();
        self.queue.truncate(self.upcoming_start());
        self.queue.extend(rest);
    }

    fn next_cursor(&mut self) -> Option<usize> {
        let mut next = self.upcoming_start();
        if next < self.queue.len() {
            return Some(next);
        }

        if !self.repeat || self.original.is_empty() {
            return None;
        }

        let mut lap = self.original.clone();

        if self.shuffle {
            fastrand::shuffle(&mut lap);

            if lap.len() > 1 && lap.first() == self.queue.last() {
                let i = fastrand::usize(1..lap.len());
                lap.swap(0, i);
            }
        }

        self.queue.extend(lap);

        if self.queue.len() > QUEUE_LIMIT {
            let cut = (self.queue.len() - QUEUE_LIMIT).min(self.pos.unwrap_or(0));
            self.queue.drain(..cut);
            next -= cut;
            self.pos = self.pos.map(|c| c - cut);
        }
        Some(next)
    }

    fn upcoming_start(&self) -> usize {
        self.pos.map_or(0, |p| p + 1).min(self.queue.len())
    }

    fn record(&mut self, song: usize) {
        if self.history.back() != Some(&song) {
            if self.history.len() >= HISTORY_LIMIT {
                self.history.pop_front();
            }
            self.history.push_back(song);
        }
    }

    fn rest_of_original(&self) -> Vec<usize> {
        let k = self
            .current()
            .and_then(|song| self.original.iter().position(|&i| i == song))
            .map_or(0, |k| k + 1);

        let mut rest = self.original[k..].to_vec();
        if self.shuffle {
            fastrand::shuffle(&mut rest);
        }

        rest
    }
}

#[cfg(test)]
mod tests {
    use super::{HISTORY_LIMIT, Playlist, QUEUE_LIMIT, Step};

    fn playlist(songs: &[usize]) -> Playlist {
        let mut playlist = Playlist::new();
        playlist.start(songs.to_vec());
        playlist
    }

    fn play(playlist: &mut Playlist) {
        playlist.advance(|_| true);
    }

    #[test]
    fn plays_in_order_and_stops_at_end() {
        let mut p = playlist(&[1, 2]);
        play(&mut p);
        assert_eq!(p.current(), Some(1));
        assert_eq!(p.upcoming(), [2]);
        play(&mut p);
        assert_eq!(p.current(), Some(2));
        play(&mut p);
        assert_eq!(p.current(), Some(2));
        assert_eq!(p.history(), &[1, 2]);
    }

    #[test]
    fn skips_songs_that_fail_to_play() {
        let mut p = playlist(&[1, 2, 3]);
        p.advance(|song| song == 3);
        assert_eq!(p.current(), Some(3));
        assert_eq!(p.history(), &[3]);
    }

    #[test]
    fn stops_when_every_song_fails() {
        let mut p = playlist(&[1, 2]);
        p.toggle_repeat();
        let mut attempts = 0;
        p.advance(|_| {
            attempts += 1;
            false
        });
        assert_eq!(attempts, 3);
        assert!(p.history().is_empty());
    }

    #[test]
    fn prev_replays_previous_song() {
        let mut p = playlist(&[1, 2]);
        play(&mut p);
        assert!(!p.has_prev());
        play(&mut p);
        assert!(p.has_prev());
        p.set_step(Step::Prev);
        play(&mut p);
        assert_eq!(p.current(), Some(1));
        assert_eq!(p.upcoming(), [2]);
    }

    #[test]
    fn current_step_replays_same_song() {
        let mut p = playlist(&[1, 2]);
        play(&mut p);
        p.set_step(Step::Current);
        play(&mut p);
        assert_eq!(p.current(), Some(1));
        assert_eq!(p.history(), &[1]);
    }

    #[test]
    fn replaying_current_song_keeps_it_once() {
        let mut p = playlist(&[1, 2]);
        play(&mut p);
        p.start(vec![1, 2, 3]);
        assert_eq!(p.upcoming(), [2, 3]);
        play(&mut p);
        assert_eq!(p.current(), Some(1));
        assert_eq!(p.upcoming(), [2, 3]);
    }

    #[test]
    fn starting_other_song_replaces_upcoming() {
        let mut p = playlist(&[1, 2]);
        play(&mut p);
        p.start(vec![3, 4]);
        play(&mut p);
        assert_eq!(p.current(), Some(3));
        assert_eq!(p.upcoming(), [4]);
    }

    #[test]
    fn repeat_starts_another_lap() {
        let mut p = playlist(&[1, 2]);
        p.toggle_repeat();
        for _ in 0..3 {
            play(&mut p);
        }
        assert_eq!(p.current(), Some(1));
    }

    #[test]
    fn shuffled_laps_do_not_repeat_a_song_back_to_back() {
        let mut p = playlist(&[1, 2]);
        p.toggle_repeat();
        p.toggle_shuffle();
        play(&mut p);
        for _ in 0..100 {
            let last = p.current();
            play(&mut p);
            assert_ne!(p.current(), last);
        }
    }

    #[test]
    fn skip_upcoming_drops_songs_before_selection() {
        let mut p = playlist(&[1, 2, 3, 4]);
        play(&mut p);
        assert!(!p.skip_upcoming(3));
        assert!(p.skip_upcoming(1));
        assert_eq!(p.upcoming(), [3, 4]);
    }

    #[test]
    fn toggling_shuffle_keeps_remaining_songs() {
        let mut p = playlist(&[1, 2, 3, 4]);
        play(&mut p);

        p.toggle_shuffle();
        let mut shuffled = p.upcoming().to_vec();
        shuffled.sort();
        assert_eq!(shuffled, [2, 3, 4]);

        p.toggle_shuffle();
        assert_eq!(p.current(), Some(1));
        assert_eq!(p.upcoming(), [2, 3, 4]);
    }

    #[test]
    fn long_repeat_stays_within_limits() {
        let songs: Vec<usize> = (0..600).collect();
        let mut p = playlist(&songs);
        p.toggle_repeat();

        for expected in (0..600).chain(0..600).chain(0..600) {
            play(&mut p);
            assert_eq!(p.current(), Some(expected));
            assert!(p.queue.len() <= QUEUE_LIMIT + songs.len());
        }
        assert_eq!(p.history().len(), HISTORY_LIMIT);
    }
}
