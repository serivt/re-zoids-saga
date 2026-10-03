//! The enhanced mode's autosave queue, a port feature: each autosave the
//! game asks for waits its turn and a worker thread writes them in the
//! order asked, so the field never waits for the disk and quick changes of
//! map are all kept, the last one last. Where there are no threads to
//! start (WebAssembly in a browser), each is written as it is queued
//! instead, and reported the same way; an image is a few kilobytes.
//!
//! Source of knowledge: this project's own design; the image written is
//! the save memory the pause menu writes (see `docs/formats/save.md`).

use std::cell::{Cell, RefCell};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::SystemTime;

use platform::{SaveStorage, StorageError};

use crate::save::SaveFile;
use crate::stats::Stats;

/// Where the autosave is kept; the worker writes it while the game plays.
pub type SharedStorage = Arc<Mutex<Box<dyn SaveStorage + Send>>>;

/// A game to write: the game-state block, the player's name and the map
/// it was saved on.
struct Job {
    state: Vec<u8>,
    name: String,
    stats: Stats,
    map: usize,
}

/// What became of a job: the map it was saved on, and why it could not be
/// written if it was not.
pub type Written = (usize, Result<(), String>);

/// Whether the platform starts threads: WebAssembly in a browser does not.
const THREADS: bool = !cfg!(target_family = "wasm");

/// The queue and the worker that empties it, or the layout each job is
/// written with at once where there is no worker.
pub struct Autosaver {
    storage: SharedStorage,
    jobs: Option<Sender<Job>>,
    written: Receiver<Written>,
    worker: Option<JoinHandle<()>>,
    /// The layout to write with as each job is queued, without a worker.
    inline: Option<SaveFile>,
    /// Jobs queued and not yet reported written.
    pending: Cell<usize>,
    /// Jobs written while waiting for the queue, until they are polled.
    waited: RefCell<Vec<Written>>,
}

impl Autosaver {
    /// Starts the worker that writes into `storage` with `save`'s layout;
    /// where the platform starts no threads, writes as each job is queued
    /// (see [`Self::inline`]).
    #[must_use]
    pub fn new(storage: Box<dyn SaveStorage + Send>, save: SaveFile) -> Self {
        if !THREADS {
            return Self::inline(storage, save);
        }
        let storage: SharedStorage = Arc::new(Mutex::new(storage));
        let (jobs, queue) = channel::<Job>();
        let (report, written) = channel();
        let shared = Arc::clone(&storage);
        let worker = std::thread::spawn(move || {
            for job in queue {
                let result = write(&shared, &save, &job);
                if report.send((job.map, result)).is_err() {
                    break;
                }
            }
        });
        Self {
            storage,
            jobs: Some(jobs),
            written,
            worker: Some(worker),
            inline: None,
            pending: Cell::new(0),
            waited: RefCell::new(Vec::new()),
        }
    }

    /// Writes into `storage` with `save`'s layout as each job is queued,
    /// without a thread; [`Self::poll`] reports them as the worker's are.
    #[must_use]
    pub fn inline(storage: Box<dyn SaveStorage + Send>, save: SaveFile) -> Self {
        let (_, written) = channel();
        Self {
            storage: Arc::new(Mutex::new(storage)),
            jobs: None,
            written,
            worker: None,
            inline: Some(save),
            pending: Cell::new(0),
            waited: RefCell::new(Vec::new()),
        }
    }

    /// Queues the game `state` of `name`, with its `statistics`, saved on
    /// `map`, after the ones already waiting.
    pub fn queue(&self, state: Vec<u8>, name: String, statistics: Stats, map: usize) {
        let job = Job {
            state,
            name,
            stats: statistics,
            map,
        };
        if let Some(save) = &self.inline {
            let result = write(&self.storage, save, &job);
            self.waited.borrow_mut().push((job.map, result));
            return;
        }
        if self
            .jobs
            .as_ref()
            .is_some_and(|jobs| jobs.send(job).is_ok())
        {
            self.pending.set(self.pending.get() + 1);
        }
    }

    /// The jobs written since the last call, in the order queued.
    pub fn poll(&self) -> Vec<Written> {
        let mut done = self.waited.take();
        let fresh: Vec<_> = self.written.try_iter().collect();
        self.pending
            .set(self.pending.get().saturating_sub(fresh.len()));
        done.extend(fresh);
        done
    }

    /// Whether some job still waits or is being written.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.pending.get() > 0
    }

    /// Waits until every job queued is written; [`Self::poll`] reports
    /// them.
    pub fn flush(&self) {
        while self.pending.get() > 0 {
            let Ok(written) = self.written.recv() else {
                self.pending.set(0);
                break;
            };
            self.pending.set(self.pending.get() - 1);
            self.waited.borrow_mut().push(written);
        }
    }

    /// The image stored, once every job queued is written.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Read`] when the image cannot be read.
    pub fn load(&self) -> Result<Option<Vec<u8>>, StorageError> {
        self.flush();
        let storage = self.storage.lock().unwrap_or_else(PoisonError::into_inner);
        storage.load()
    }

    /// When the image was last stored, once every job queued is written,
    /// if the storage keeps track.
    #[must_use]
    pub fn modified(&self) -> Option<SystemTime> {
        self.flush();
        let storage = self.storage.lock().unwrap_or_else(PoisonError::into_inner);
        storage.modified()
    }
}

/// The frames the notice shows at least, those it takes to fade in and
/// out, those each of its dots lasts, and its most dots.
const NOTICE_LEAST_FRAMES: u32 = 60;
const NOTICE_FADE_FRAMES: u32 = 16;
const NOTICE_DOT_FRAMES: u32 = 12;
const NOTICE_DOTS: u32 = 3;

/// The notice the field shows while autosaves are written: it fades in
/// when one is queued, its dots count up while the queue is busy, and once
/// the queue is empty and it has shown long enough to read, it fades out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Notice {
    /// Frames since it came.
    shown: u32,
    /// Frames since it began to leave, once it has.
    leaving: Option<u32>,
}

impl Notice {
    /// The notice for a save just queued: a new one, or `current` staying.
    #[must_use]
    pub fn queued(current: Option<Self>) -> Self {
        Self {
            shown: current.map_or(0, |notice| notice.shown),
            leaving: None,
        }
    }

    /// A frame of the notice, the queue `busy` or not; `None` once it has
    /// faded out.
    #[must_use]
    pub fn step(self, busy: bool) -> Option<Self> {
        let shown = self.shown.saturating_add(1);
        let leaving = match self.leaving {
            _ if busy => None,
            Some(frames) => Some(frames + 1),
            None if shown >= NOTICE_LEAST_FRAMES => Some(0),
            None => None,
        };
        match leaving {
            Some(frames) if frames >= NOTICE_FADE_FRAMES => None,
            _ => Some(Self { shown, leaving }),
        }
    }

    /// How strongly it shows, 0 to `levels`.
    #[must_use]
    pub fn level(self, levels: u8) -> u8 {
        let coming = (self.shown + 1).min(NOTICE_FADE_FRAMES);
        let going = NOTICE_FADE_FRAMES - self.leaving.unwrap_or(0).min(NOTICE_FADE_FRAMES);
        let level = coming.min(going) * u32::from(levels) / NOTICE_FADE_FRAMES;
        u8::try_from(level).unwrap_or(levels)
    }

    /// The dots after its text, one to three, counting up as it shows.
    #[must_use]
    pub fn dots(self) -> usize {
        let dots = self.shown / NOTICE_DOT_FRAMES % NOTICE_DOTS + 1;
        usize::try_from(dots).unwrap_or(1)
    }
}

impl Drop for Autosaver {
    /// Lets the worker write what is queued before the game goes.
    fn drop(&mut self) {
        self.jobs = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Writes `job` into `storage`, keeping what the image holds besides the
/// game-state block; an image that cannot be read is written anew, as the
/// pause menu's save does.
fn write(storage: &SharedStorage, save: &SaveFile, job: &Job) -> Result<(), String> {
    let mut storage = storage.lock().unwrap_or_else(PoisonError::into_inner);
    let previous = storage.load().unwrap_or(None);
    let image = save
        .write(previous, &job.state, &job.name, &job.stats)
        .map_err(|error| error.to_string())?;
    storage.store(&image).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::save::Found;
    use formats::progress::STATE_LEN;
    use formats::{Progress, SaveLayout};

    /// A storage that keeps each image stored, in order.
    #[derive(Clone, Default)]
    struct Recorder(Arc<Mutex<Vec<Vec<u8>>>>);

    impl SaveStorage for Recorder {
        fn load(&self) -> Result<Option<Vec<u8>>, StorageError> {
            Ok(self.0.lock().expect("lock").last().cloned())
        }
        fn store(&mut self, bytes: &[u8]) -> Result<(), StorageError> {
            self.0.lock().expect("lock").push(bytes.to_vec());
            Ok(())
        }
    }

    fn save() -> SaveFile {
        SaveFile::new(SaveLayout {
            magic: b"TEST\0".to_vec(),
            blocks: vec![STATE_LEN, 4],
            copies: 2,
            memory_size: 2 * (STATE_LEN + 12) + 5 + 64,
        })
    }

    fn state(map: u16) -> Vec<u8> {
        let mut state = vec![0; STATE_LEN];
        let mut progress = Progress::read(&state).expect("block");
        progress.map = map;
        progress.write(&mut state).expect("block");
        state
    }

    fn map_of(image: &[u8]) -> u16 {
        match save().read(Some(image.to_vec())) {
            Found::Saved(game) => game.progress().expect("block").map,
            other => panic!("not a game: {other:?}"),
        }
    }

    #[test]
    fn writes_the_games_queued_in_order_and_reports_each() {
        let recorder = Recorder::default();
        let autosaver = Autosaver::new(Box::new(recorder.clone()), save());
        for map in [3, 7, 5] {
            autosaver.queue(
                state(map),
                "アトレー".to_owned(),
                Stats::default(),
                usize::from(map),
            );
        }
        assert!(autosaver.is_busy());
        autosaver.flush();
        assert!(!autosaver.is_busy());
        let written = autosaver.poll();
        assert_eq!(written, [(3, Ok(())), (7, Ok(())), (5, Ok(()))]);
        let images = recorder.0.lock().expect("lock").clone();
        assert_eq!(
            images.iter().map(|image| map_of(image)).collect::<Vec<_>>(),
            [3, 7, 5]
        );
        let image = autosaver.load().expect("readable").expect("stored");
        assert_eq!(map_of(&image), 5);
        assert!(autosaver.poll().is_empty());
    }

    #[test]
    fn without_a_worker_each_game_is_written_as_it_is_queued() {
        let recorder = Recorder::default();
        let autosaver = Autosaver::inline(Box::new(recorder.clone()), save());
        for map in [3, 7] {
            autosaver.queue(
                state(map),
                "Atory".to_owned(),
                Stats::default(),
                usize::from(map),
            );
            assert!(!autosaver.is_busy(), "nothing waits");
        }
        assert_eq!(recorder.0.lock().expect("lock").len(), 2);
        assert_eq!(autosaver.poll(), [(3, Ok(())), (7, Ok(()))]);
        assert!(autosaver.poll().is_empty());
        let image = autosaver.load().expect("readable").expect("stored");
        assert_eq!(map_of(&image), 7);
    }

    #[test]
    fn the_notice_stays_while_the_queue_is_busy_then_fades_out() {
        let mut notice = Notice::queued(None);
        assert_eq!((notice.level(16), notice.dots()), (1, 1));
        for _ in 0..200 {
            notice = notice.step(true).expect("busy");
        }
        assert_eq!(notice.level(16), 16);
        notice = Notice::queued(Some(notice));
        for _ in 0..=NOTICE_FADE_FRAMES / 2 {
            notice = notice.step(false).expect("fading");
        }
        assert_eq!(notice.level(16), 8);
        assert_eq!(Notice::queued(Some(notice)).level(16), 16);
        for _ in 1..NOTICE_FADE_FRAMES / 2 {
            notice = notice.step(false).expect("fading");
        }
        assert_eq!(notice.step(false), None);
    }

    #[test]
    fn a_quick_save_shows_its_notice_long_enough_to_read() {
        let mut notice = Some(Notice::queued(None));
        let mut frames = 0;
        while let Some(shown) = notice {
            notice = shown.step(false);
            frames += 1;
        }
        assert_eq!(frames, NOTICE_LEAST_FRAMES + NOTICE_FADE_FRAMES);
        let dots: Vec<_> = (0..4)
            .map(|step| {
                Notice {
                    shown: step * NOTICE_DOT_FRAMES,
                    leaving: None,
                }
                .dots()
            })
            .collect();
        assert_eq!(dots, [1, 2, 3, 1]);
    }

    #[test]
    fn going_away_waits_for_the_games_queued() {
        let recorder = Recorder::default();
        let autosaver = Autosaver::new(Box::new(recorder.clone()), save());
        autosaver.queue(state(9), "アトレー".to_owned(), Stats::default(), 9);
        autosaver.queue(state(2), "アトレー".to_owned(), Stats::default(), 2);
        drop(autosaver);
        assert_eq!(recorder.0.lock().expect("lock").len(), 2);
    }
}
