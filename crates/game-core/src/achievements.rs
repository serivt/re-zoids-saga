//! The achievements, a port feature of the enhanced mode: 31 goals, from
//! the chapters cleared to the collection and the battles' records, which
//! unlock once and for good, shared by every game played from the same
//! save folder.
//!
//! Most read the game state, so a game saved before they existed, here or
//! in an emulator, unlocks them as soon as it is continued: the flags the
//! chapters' openings set, the Zi data at `+0x33E2`, the chests' flags,
//! the character guide's bits at `+0x34A4`, the deck commands at
//! `+0x347B`, the units, the Zoid cores at `+0x330C`, the level and the
//! money (see `docs/formats/save.md`). A few read the player's statistics
//! (see [`crate::stats`]), and a few are met only as they happen: a
//! battle's end, a development in a lab, the staff roll. The game checks
//! them twice a second while the player walks freely, and when such a
//! moment comes; each one unlocked is announced by a window that slides
//! down at the field's top (several at once share one), and the pause
//! menu's 実績 lists them all.
//!
//! The totals the collection asks for are those the ROM can give: the Zi
//! data of the chests, of the roaming formations the map Zoids may stand
//! for and of the story's gifts (140 of the 153 the list has room for);
//! the chests on the maps (247); the characters the story's ten groups put
//! in the guide (63 of 87); and the 33 deck commands, which the teachers
//! and the story give every one of.
//!
//! The unlocked achievements are kept in a text file of their own, one key
//! a line, beside the saves.
//!
//! Source of knowledge: this project's own design; the fields and flags
//! each achievement reads are the game's (see `docs/achievements.md`).

use extraction::saga::{self, CHEST_FLAG_BASE, Reward};
use extraction::{saga_encounter, saga_party, saga_save};
use formats::Progress;
use formats::progress::{character_known, command_learned, zoid_seen};
use platform::{Frame, SaveStorage};

use crate::combat::{BattleTally, Outcome};
use crate::extension::SharedExtensions;
use crate::port_text::{ACHIEVEMENT_MANY, ACHIEVEMENT_UNLOCKED, PortText, fill, port_text};
use crate::stats::{FRAMES_PER_SECOND, Stats};
use crate::story;
use crate::text::{CELL_WIDTH, TextPainter};
use crate::window::WindowPainter;

/// The mark before an unlocked achievement's name, and before a locked
/// one's, in the ROM's font.
pub const UNLOCKED_MARK: char = '★';
/// See [`UNLOCKED_MARK`].
pub const LOCKED_MARK: char = '☆';
/// The groups of characters the story meets (`0x080099E4` and the like,
/// lists 0–9 of ROM `0x66C8D0`).
const STORY_GROUPS: usize = 10;
/// The deck commands there are (the records at ROM `0x683AC0`).
const DECK_COMMANDS: usize = 33;
/// Trinity Liger BA, which Blue Gem's Zi data lets the lab build.
const TRINITY_LIGER_BA: u16 = 0x90;
/// The hours the staff roll must start within for 時間との戦い.
const CLOCK_HOURS: u32 = 15;
/// Frames between two checks of the game state.
const CHECK_FRAMES: u32 = 30;
/// The sound a toast plays: the battle's spoils' jingle.
const TOAST_SOUND: u8 = 0x35;
/// Frames the toast takes to slide down, stays, and takes to slide up.
const TOAST_SLIDE: u32 = 8;
const TOAST_HOLD: u32 = 150;
/// The toast's lines, its window's border and margin in cells, and the
/// most cells it may span.
const TOAST_LINES: usize = 2;
const TOAST_FRAME_CELLS: usize = 3;
const TOAST_MOST_CELLS: usize = 30;
const LINE_PIXELS: usize = 16;
const SCREEN_WIDTH: usize = 240;
/// The first line of the file the unlocked achievements are kept in.
const FILE_HEADER: &str = "# re-zoids-saga achievements";

/// What an achievement asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Goal {
    /// Every one of the flags set.
    Flags(&'static [u16]),
    /// A story battle won, as the statistics count it.
    StoryBattle(u8),
    /// At least this many Zi data held.
    ZiData(usize),
    /// Every Zi data the ROM can give held.
    AllZiData,
    /// Every chest on the maps opened.
    AllChests,
    /// Every character the story meets in the character guide.
    AllCharacters,
    /// Every deck command learned.
    AllCommands,
    /// A unit of this Zoid owned.
    OwnZoid(u16),
    /// At least this many kinds of Zoid core held at once.
    CoreKinds(usize),
    /// At least this many units.
    Units(u8),
    /// The party's level at least this.
    Level(u8),
    /// At least this much money held at once.
    Money(u32),
    /// At least this many battles won.
    BattlesWon(u32),
    /// A blow of at least this much damage.
    BestHit(u32),
    /// A battle of at least this many rounds.
    LongestBattle(u32),
    /// A story battle won without losing a Zoid, as it ends.
    Flawless,
    /// A battle won in its first round, as it ends.
    Lightning,
    /// A Zoid developed in a lab.
    Develop,
    /// The staff roll started within [`CLOCK_HOURS`] of play.
    Clock,
    /// Every other achievement.
    All,
}

/// One achievement: its key in the file, what it asks for, its name and
/// what it asks for in the player's words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Achievement {
    /// The key the file keeps it under.
    pub key: &'static str,
    goal: Goal,
    /// Its name.
    pub name: PortText,
    /// What it asks for.
    pub help: PortText,
}

const NAME_PIXELS: usize = 184;
const HELP_PIXELS: usize = 192;

/// An achievement whose texts are `port/achievement/<key>` and
/// `port/achievement/<key>-help`.
macro_rules! achievement {
    ($key:literal, $goal:expr, $name:literal, $help:literal, $note:literal) => {
        Achievement {
            key: $key,
            goal: $goal,
            name: PortText {
                key: concat!("port/achievement/", $key),
                text: $name,
                note: concat!(
                    "The name of an achievement (a port feature): ",
                    $note,
                    "; in the pause menu's list after a star, and in the window that announces it; 184 pixels"
                ),
                pixels: NAME_PIXELS,
            },
            help: PortText {
                key: concat!("port/achievement/", $key, "-help"),
                text: $help,
                note: concat!(
                    "What the achievement '",
                    $note,
                    "' asks for, under its name in the pause menu's list; 192 pixels"
                ),
                pixels: HELP_PIXELS,
            },
        }
    };
}

const fn opening(chapter: usize) -> Goal {
    Goal::Flags(match chapter {
        1 => &[story::CHAPTER_OPENINGS[0]],
        2 => &[story::CHAPTER_OPENINGS[1]],
        3 => &[story::CHAPTER_OPENINGS[2]],
        4 => &[story::CHAPTER_OPENINGS[3]],
        5 => &[story::CHAPTER_OPENINGS[4]],
        6 => &[story::CHAPTER_OPENINGS[5]],
        7 => &[story::CHAPTER_OPENINGS[6]],
        8 => &[story::CHAPTER_OPENINGS[7]],
        _ => &[story::CHAPTER_OPENINGS[8]],
    })
}

/// Every achievement, in the menu's order.
pub const ACHIEVEMENTS: [Achievement; 31] = [
    achievement!(
        "chapter-1",
        opening(1),
        "赤い川の彼方へ",
        "第１章をクリアする",
        "Beyond the Red River, chapter 1 cleared"
    ),
    achievement!(
        "chapter-2",
        opening(2),
        "砂と反逆",
        "第２章をクリアする",
        "Sand and Rebellion, chapter 2 cleared"
    ),
    achievement!(
        "chapter-3",
        opening(3),
        "目覚めるクレーター",
        "第３章をクリアする",
        "The Crater Awakens, chapter 3 cleared"
    ),
    achievement!(
        "chapter-4",
        opening(4),
        "鋼と鋼",
        "第４章をクリアする",
        "Steel Against Steel, chapter 4 cleared"
    ),
    achievement!(
        "chapter-5",
        opening(5),
        "ドームの覇者",
        "第５章をクリアする",
        "Dome Champion, chapter 5 cleared"
    ),
    achievement!(
        "chapter-6",
        opening(6),
        "風が止むとき",
        "第６章をクリアする",
        "The Wind Falls Still, chapter 6 cleared"
    ),
    achievement!(
        "chapter-7",
        opening(7),
        "ブルージェムとの約束",
        "第７章をクリアする",
        "A Promise to Blue Gem, chapter 7 cleared"
    ),
    achievement!(
        "chapter-8",
        opening(8),
        "逃げ場なし",
        "第８章をクリアする",
        "Nowhere to Run, chapter 8 cleared"
    ),
    achievement!(
        "chapter-9",
        opening(9),
        "帝国の落日",
        "第９章をクリアしエンディングを見る",
        "Fall of the Empire, chapter 9 cleared and the ending seen"
    ),
    achievement!(
        "chapter-10",
        Goal::Flags(&[story::VEGA_BEATEN]),
        "解き放たれた怒り",
        "ベガのバーサークフューラーを倒す",
        "Fury Unleashed, Vega's Berserk Fury beaten in chapter 10"
    ),
    achievement!(
        "old-rivals",
        Goal::Flags(&[story::ROSSO_JOINED]),
        "かつての好敵手",
        "ロッソとヴィオラを仲間にする",
        "Old Rivals, Rosso and Viola taken along in the ruins"
    ),
    achievement!(
        "defiance",
        Goal::StoryBattle(story::ULTRASAURUS_BATTLE),
        "不遜",
        "ウルトラザウルスに挑んで勝つ",
        "Defiance, the Zoid Federation's Ultrasaurus challenged and beaten"
    ),
    achievement!(
        "lost-data",
        Goal::Flags(&story::RESEARCHERS_GAVE),
        "失われたデータ",
        "研究員から１６のＺｉデータを受け取る",
        "Lost Data, the 16 Zi data of chapter 10's researchers received"
    ),
    achievement!(
        "researcher",
        Goal::ZiData(50),
        "研究者",
        "Ｚｉデータを５０集める",
        "Researcher, 50 Zi data held"
    ),
    achievement!(
        "complete-archive",
        Goal::AllZiData,
        "完全なる記録",
        "手に入る１４０のＺｉデータを集める",
        "Complete Archive, the 140 Zi data the game gives all held"
    ),
    achievement!(
        "treasure-hunter",
        Goal::AllChests,
        "トレジャーハンター",
        "２４７の宝箱をすべて開ける",
        "Treasure Hunter, the 247 chests all opened"
    ),
    achievement!(
        "whos-who",
        Goal::AllCharacters,
        "人物名鑑",
        "６３人を人物図鑑に登録する",
        "Who's Who, the 63 characters the story meets all in the character guide"
    ),
    achievement!(
        "tactics-master",
        Goal::AllCommands,
        "戦術の達人",
        "３３のデッキコマンドを覚える",
        "Tactics Master, the 33 deck commands all learned"
    ),
    achievement!(
        "lion-reborn",
        Goal::OwnZoid(TRINITY_LIGER_BA),
        "よみがえる獅子",
        "トリニティライガーＢＡを手に入れる",
        "The Lion Reborn, a Trinity Liger BA owned"
    ),
    achievement!(
        "core-forger",
        Goal::CoreKinds(10),
        "コアの鍛冶師",
        "ゾイドコアを１０種類持つ",
        "Core Forger, 10 kinds of Zoid core held at once"
    ),
    achievement!(
        "full-hangar",
        Goal::Units(50),
        "満員の格納庫",
        "ゾイドを５０体持つ",
        "Full Hangar, 50 units owned"
    ),
    achievement!(
        "engineer",
        Goal::Develop,
        "技術者",
        "研究所でゾイドを開発する",
        "Engineer, a Zoid developed in a lab"
    ),
    achievement!(
        "summit",
        Goal::Level(99),
        "頂点",
        "レベル９９になる",
        "Summit, the party's level 99 reached"
    ),
    achievement!(
        "millionaire",
        Goal::Money(1_000_000),
        "大富豪",
        "１００万Ｇを持つ",
        "Millionaire, 1,000,000 G held at once"
    ),
    achievement!(
        "veteran",
        Goal::BattlesWon(300),
        "歴戦の勇士",
        "戦闘に３００回勝つ",
        "Veteran, 300 battles won"
    ),
    achievement!(
        "devastating-blow",
        Goal::BestHit(3000),
        "会心の一撃",
        "一撃で３０００ダメージを与える",
        "Devastating Blow, a blow of 3000 damage or more"
    ),
    achievement!(
        "war-of-attrition",
        Goal::LongestBattle(20),
        "消耗戦",
        "２０ターン以上戦い抜く",
        "War of Attrition, a battle of 20 rounds or more"
    ),
    achievement!(
        "flawless-victory",
        Goal::Flawless,
        "完全勝利",
        "味方を失わずにストーリー戦闘に勝つ",
        "Flawless Victory, a story battle won without losing a Zoid"
    ),
    achievement!(
        "lightning-strike",
        Goal::Lightning,
        "電光石火",
        "１ターン目で戦闘に勝つ",
        "Lightning Strike, a battle won in its first round"
    ),
    achievement!(
        "against-the-clock",
        Goal::Clock,
        "時間との戦い",
        "１５時間以内にエンディングを見る",
        "Against the Clock, the staff roll reached within 15 hours of play"
    ),
    achievement!(
        "legend-of-zi",
        Goal::All,
        "Ｚｉの伝説",
        "ほかの実績をすべて達成する",
        "Legend of Zi, every other achievement unlocked"
    ),
];

/// The texts of every achievement, names then what they ask for in turn.
pub fn texts() -> impl Iterator<Item = &'static PortText> {
    ACHIEVEMENTS
        .iter()
        .flat_map(|achievement| [&achievement.name, &achievement.help])
}

/// The achievements unlocked, a bit each by their place in
/// [`ACHIEVEMENTS`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Unlocked(u64);

impl Unlocked {
    /// Whether achievement `index` is unlocked.
    #[must_use]
    pub fn has(self, index: usize) -> bool {
        index < ACHIEVEMENTS.len() && self.0 & (1 << index) != 0
    }

    /// How many are unlocked.
    #[must_use]
    pub fn count(self) -> usize {
        usize::try_from(self.0.count_ones()).unwrap_or(0)
    }

    fn insert(&mut self, index: usize) -> bool {
        let new = !self.has(index) && index < ACHIEVEMENTS.len();
        if new {
            self.0 |= 1 << index;
        }
        new
    }

    /// The file's text: a header, then a key a line.
    #[must_use]
    pub fn to_text(self) -> String {
        let mut text = format!("{FILE_HEADER}\n");
        for (index, achievement) in ACHIEVEMENTS.iter().enumerate() {
            if self.has(index) {
                text.push_str(achievement.key);
                text.push('\n');
            }
        }
        text
    }

    /// The achievements a file's `text` names; lines it does not know are
    /// left out.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut unlocked = Self::default();
        for line in text.lines().map(str::trim) {
            if let Some(index) = ACHIEVEMENTS.iter().position(|a| a.key == line) {
                unlocked.insert(index);
            }
        }
        unlocked
    }
}

/// What the collection's achievements ask for, as the ROM gives it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Totals {
    zi_data: Vec<u8>,
    chests: Vec<u16>,
    characters: Vec<u8>,
}

impl Totals {
    fn read(rom: &[u8]) -> Self {
        let chests = saga::map_chests(rom);
        let mut zi_data: Vec<u8> = chests
            .iter()
            .filter_map(
                |&chest| match saga::treasure(rom, usize::from(chest))?.reward() {
                    Reward::ZiData(zoid) => Some(zoid),
                    _ => None,
                },
            )
            .chain(saga_encounter::roaming_zi_data(rom))
            .chain(story::zi_data_gifts())
            .collect();
        zi_data.sort_unstable();
        zi_data.dedup();
        Self {
            zi_data,
            chests,
            characters: saga_save::listed_characters(rom, STORY_GROUPS).unwrap_or_default(),
        }
    }
}

/// What a toast announces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum News {
    One(usize),
    Many(usize),
}

/// The window announcing what was unlocked, and the frames it has shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Toast {
    news: News,
    frames: u32,
}

impl Toast {
    /// The pixels of its window hidden above the screen, of `height`.
    fn hidden(self, height: usize) -> usize {
        let shown = if self.frames < TOAST_SLIDE {
            self.frames
        } else {
            (TOAST_SLIDE * 2 + TOAST_HOLD).saturating_sub(self.frames)
        }
        .min(TOAST_SLIDE);
        let shown = usize::try_from(shown).unwrap_or(0);
        height - height * shown / usize::try_from(TOAST_SLIDE).unwrap_or(1)
    }
}

/// What the tracker has to tell the game: an achievement unlocked, by its
/// key, or why the file could not be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    /// An achievement was unlocked.
    Unlocked(&'static str),
    /// The file could not be written.
    StorageFailed(String),
}

/// The achievements unlocked, where they are kept, and those to announce.
#[derive(Default)]
pub struct Tracker {
    unlocked: Unlocked,
    storage: Option<Box<dyn SaveStorage>>,
    totals: Option<Totals>,
    pending: Vec<usize>,
    toast: Option<Toast>,
    reports: Vec<Report>,
    since_check: u32,
    credits_seen: bool,
}

impl Tracker {
    /// Keeps the achievements in `storage`, taking those it holds.
    pub fn set_storage(&mut self, storage: Box<dyn SaveStorage>) {
        match storage.load() {
            Ok(Some(bytes)) => {
                self.unlocked = Unlocked::parse(&String::from_utf8_lossy(&bytes));
            }
            Ok(None) => {}
            Err(error) => self.reports.push(Report::StorageFailed(error.to_string())),
        }
        self.storage = Some(storage);
    }

    /// The achievements unlocked.
    #[must_use]
    pub fn unlocked(&self) -> Unlocked {
        self.unlocked
    }

    /// What happened since the last call.
    pub fn take_reports(&mut self) -> Vec<Report> {
        std::mem::take(&mut self.reports)
    }

    /// Counts a frame of free play; every [`CHECK_FRAMES`] the game state
    /// is checked. Returns whether it is due.
    pub fn due(&mut self) -> bool {
        self.since_check += 1;
        if self.since_check >= CHECK_FRAMES {
            self.since_check = 0;
            true
        } else {
            false
        }
    }

    /// Checks what `state` (the game state with its flags, level and money
    /// up to date) and `statistics` say against the achievements still locked.
    pub fn check(&mut self, rom: &[u8], state: &[u8], statistics: &Stats) {
        let Ok(progress) = Progress::read(state) else {
            return;
        };
        let totals = self.totals.get_or_insert_with(|| Totals::read(rom));
        let met: Vec<usize> = ACHIEVEMENTS
            .iter()
            .enumerate()
            .filter(|(index, _)| !self.unlocked.has(*index))
            .filter(|(_, achievement)| {
                reached(achievement.goal, state, &progress, statistics, totals)
            })
            .map(|(index, _)| index)
            .collect();
        self.unlock(&met);
    }

    /// A battle ended with `outcome`, `tally` seeing it; `story` for one of
    /// the story's.
    pub fn battle_ended(&mut self, outcome: Outcome, tally: &BattleTally, story: bool) {
        if outcome != Outcome::Won {
            return;
        }
        let met = goals_matching(|goal| match goal {
            Goal::Flawless => story && tally.party_destroyed == 0,
            Goal::Lightning => tally.rounds == 1,
            _ => false,
        });
        self.unlock(&met);
    }

    /// A Zoid was developed in a lab.
    pub fn developed(&mut self) {
        self.unlock(&goals_matching(|goal| goal == Goal::Develop));
    }

    /// Notes whether the staff roll runs, `stats` saying how long the game
    /// has been played: the first frame it runs, within [`CLOCK_HOURS`],
    /// unlocks 時間との戦い.
    pub fn credits_running(&mut self, running: bool, stats: &Stats) {
        if running && !self.credits_seen {
            let limit = CLOCK_HOURS * 3600 * FRAMES_PER_SECOND;
            if stats.play_frames < limit {
                self.unlock(&goals_matching(|goal| goal == Goal::Clock));
            }
        }
        self.credits_seen = running;
    }

    /// Unlocks the achievements `met` names, and then the one for all of
    /// them when it is due; announces and keeps them.
    fn unlock(&mut self, met: &[usize]) {
        let mut new: Vec<usize> = met
            .iter()
            .copied()
            .filter(|&index| self.unlocked.insert(index))
            .collect();
        if new.is_empty() {
            return;
        }
        let all = goals_matching(|goal| goal == Goal::All);
        let others = (0..ACHIEVEMENTS.len())
            .filter(|index| !all.contains(index))
            .all(|index| self.unlocked.has(index));
        if others {
            new.extend(all.into_iter().filter(|&index| self.unlocked.insert(index)));
        }
        for &index in &new {
            self.reports.push(Report::Unlocked(ACHIEVEMENTS[index].key));
        }
        self.pending.extend(new);
        if let Some(storage) = &mut self.storage
            && let Err(error) = storage.store(self.unlocked.to_text().as_bytes())
        {
            self.reports.push(Report::StorageFailed(error.to_string()));
        }
    }

    /// A frame of the toast while the field shows it (`showing`): the next
    /// one starts once the last is gone; returns the sound to play when one
    /// starts.
    pub fn step(&mut self, showing: bool) -> Option<u8> {
        if !showing {
            return None;
        }
        if let Some(toast) = &mut self.toast {
            toast.frames += 1;
            if toast.frames >= TOAST_SLIDE * 2 + TOAST_HOLD {
                self.toast = None;
            }
            return None;
        }
        let news = match self.pending.as_slice() {
            [] => return None,
            [index] => News::One(*index),
            many => News::Many(many.len()),
        };
        self.pending.clear();
        self.toast = Some(Toast { news, frames: 0 });
        Some(TOAST_SOUND)
    }

    /// Draws the toast over `frame`, when one shows: a window centered at
    /// the top, sliding down and back up.
    pub fn draw(
        &self,
        frame: &mut Frame,
        extensions: &SharedExtensions,
        painter: &TextPainter,
        skin: &WindowPainter,
    ) {
        let Some(toast) = self.toast else {
            return;
        };
        let second = match toast.news {
            News::One(index) => format!(
                "{UNLOCKED_MARK} {}",
                port_text(extensions, ACHIEVEMENTS[index].name.key)
            ),
            News::Many(count) => fill(
                &port_text(extensions, ACHIEVEMENT_MANY),
                &[("count", u32::try_from(count).unwrap_or(u32::MAX))],
            ),
        };
        let lines = [port_text(extensions, ACHIEVEMENT_UNLOCKED), second];
        let metrics = painter.metrics();
        let cells = lines
            .iter()
            .map(|line| metrics.width(line).div_ceil(CELL_WIDTH))
            .max()
            .unwrap_or(0);
        let columns = (cells + TOAST_FRAME_CELLS).min(TOAST_MOST_CELLS);
        let rows = TOAST_LINES * LINE_PIXELS / CELL_WIDTH + 2;
        let (width, height) = (columns * CELL_WIDTH, rows * CELL_WIDTH);
        let mut window = Frame::new(width, height, platform::Rgb::default());
        skin.draw_window(&mut window, 0, 0, columns, rows);
        for (row, line) in lines.iter().enumerate() {
            let y = CELL_WIDTH + row * LINE_PIXELS;
            painter.draw(
                &mut window,
                i32::try_from(CELL_WIDTH).unwrap_or(0),
                i32::try_from(y).unwrap_or(0),
                line,
                skin.palette(),
            );
        }
        let hidden = toast.hidden(height);
        let left = SCREEN_WIDTH.saturating_sub(width) / 2;
        for y in hidden..height {
            for x in 0..width {
                if let Some(color) = window.pixel(x, y) {
                    frame.set_pixel(left + x, y - hidden, color);
                }
            }
        }
    }
}

/// The achievements whose goal `test` accepts.
fn goals_matching(test: impl Fn(Goal) -> bool) -> Vec<usize> {
    ACHIEVEMENTS
        .iter()
        .enumerate()
        .filter(|(_, achievement)| test(achievement.goal))
        .map(|(index, _)| index)
        .collect()
}

/// Whether the game state, its fields and the statistics meet `goal`; the
/// goals met as they happen never are here.
fn reached(
    goal: Goal,
    state: &[u8],
    progress: &Progress,
    statistics: &Stats,
    totals: &Totals,
) -> bool {
    let all_of = |count: usize, test: &dyn Fn(usize) -> bool| count > 0 && (0..count).all(test);
    match goal {
        Goal::Flags(flags) => flags.iter().all(|&flag| progress.flag(flag)),
        Goal::StoryBattle(battle) => statistics.story_won & (1 << battle) != 0,
        Goal::ZiData(count) => saga_party::zi_data_count(state) >= count,
        Goal::AllZiData => all_of(totals.zi_data.len(), &|at| {
            zoid_seen(state, usize::from(totals.zi_data[at]))
        }),
        Goal::AllChests => all_of(totals.chests.len(), &|at| {
            progress.flag(CHEST_FLAG_BASE + totals.chests[at])
        }),
        Goal::AllCharacters => all_of(totals.characters.len(), &|at| {
            character_known(state, usize::from(totals.characters[at]))
        }),
        Goal::AllCommands => all_of(DECK_COMMANDS, &|command| command_learned(state, command)),
        Goal::OwnZoid(zoid) => saga_party::owns_zoid(state, zoid),
        Goal::CoreKinds(count) => saga_party::zi_item_kinds(state) >= count,
        Goal::Units(count) => saga_party::unit_count(state) >= count,
        Goal::Level(level) => progress.level >= level,
        Goal::Money(money) => progress.money >= money,
        Goal::BattlesWon(count) => statistics.battles_won >= count,
        Goal::BestHit(damage) => statistics.best_hit >= damage,
        Goal::LongestBattle(rounds) => statistics.longest_battle >= rounds,
        Goal::Flawless | Goal::Lightning | Goal::Develop | Goal::Clock | Goal::All => false,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use formats::progress::{STATE_LEN, learn_command};
    use platform::StorageError;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Clone, Default)]
    struct Memory(Rc<RefCell<Option<Vec<u8>>>>);

    impl SaveStorage for Memory {
        fn load(&self) -> Result<Option<Vec<u8>>, StorageError> {
            Ok(self.0.borrow().clone())
        }

        fn store(&mut self, bytes: &[u8]) -> Result<(), StorageError> {
            *self.0.borrow_mut() = Some(bytes.to_vec());
            Ok(())
        }
    }

    fn index(key: &str) -> usize {
        ACHIEVEMENTS.iter().position(|a| a.key == key).unwrap()
    }

    fn tracker() -> (Tracker, Memory) {
        let memory = Memory::default();
        let mut tracker = Tracker {
            totals: Some(Totals {
                zi_data: vec![3, 7],
                chests: vec![0, 2],
                characters: vec![1],
            }),
            ..Tracker::default()
        };
        tracker.set_storage(Box::new(memory.clone()));
        (tracker, memory)
    }

    fn state_with(edit: impl Fn(&mut Progress, &mut Vec<u8>)) -> Vec<u8> {
        let mut state = vec![0; STATE_LEN];
        let mut progress = Progress::read(&state).unwrap();
        edit(&mut progress, &mut state);
        progress.write(&mut state).unwrap();
        state
    }

    #[test]
    fn the_keys_are_distinct_and_the_file_gives_them_back() {
        let mut keys: Vec<&str> = ACHIEVEMENTS.iter().map(|a| a.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), ACHIEVEMENTS.len());
        let mut unlocked = Unlocked::default();
        unlocked.insert(index("chapter-3"));
        unlocked.insert(index("legend-of-zi"));
        let text = unlocked.to_text();
        assert!(text.starts_with(FILE_HEADER));
        assert_eq!(Unlocked::parse(&format!("{text}unknown\n")), unlocked);
        assert_eq!(unlocked.count(), 2);
    }

    #[test]
    fn the_state_unlocks_chapters_and_the_collection_retroactively() {
        let (mut tracker, memory) = tracker();
        let state = state_with(|progress, state| {
            progress.set_flag(story::CHAPTER_OPENINGS[0], true);
            progress.set_flag(story::CHAPTER_OPENINGS[1], true);
            progress.set_flag(CHEST_FLAG_BASE, true);
            progress.set_flag(CHEST_FLAG_BASE + 2, true);
            progress.level = 99;
            for command in 0..DECK_COMMANDS {
                learn_command(state, command);
            }
        });
        tracker.check(&[], &state, &Stats::default());
        let unlocked = tracker.unlocked();
        for key in [
            "chapter-1",
            "chapter-2",
            "treasure-hunter",
            "summit",
            "tactics-master",
        ] {
            assert!(unlocked.has(index(key)), "{key}");
        }
        assert!(!unlocked.has(index("chapter-3")));
        assert!(!unlocked.has(index("complete-archive")));
        assert_eq!(unlocked.count(), 5);
        let kept = memory.0.borrow().clone().unwrap();
        assert_eq!(Unlocked::parse(&String::from_utf8(kept).unwrap()), unlocked);
        assert_eq!(tracker.take_reports().len(), 5);
    }

    #[test]
    fn statistics_and_battles_unlock_the_records() {
        let (mut tracker, _) = tracker();
        let stats = Stats {
            battles_won: 300,
            best_hit: 3000,
            longest_battle: 19,
            story_won: 1 << story::ULTRASAURUS_BATTLE,
            ..Stats::default()
        };
        tracker.check(&[], &state_with(|_, _| {}), &stats);
        let unlocked = tracker.unlocked();
        assert!(unlocked.has(index("veteran")));
        assert!(unlocked.has(index("devastating-blow")));
        assert!(unlocked.has(index("defiance")));
        assert!(!unlocked.has(index("war-of-attrition")));
        let tally = BattleTally {
            party_destroyed: 1,
            rounds: 1,
            ..BattleTally::default()
        };
        tracker.battle_ended(Outcome::Lost, &tally, true);
        assert!(!tracker.unlocked().has(index("lightning-strike")));
        tracker.battle_ended(Outcome::Won, &tally, true);
        assert!(tracker.unlocked().has(index("lightning-strike")));
        assert!(!tracker.unlocked().has(index("flawless-victory")));
        tracker.battle_ended(Outcome::Won, &BattleTally::default(), true);
        assert!(tracker.unlocked().has(index("flawless-victory")));
    }

    #[test]
    fn the_staff_roll_counts_once_within_the_hours() {
        let (mut tracker, _) = tracker();
        let late = Stats {
            play_frames: CLOCK_HOURS * 3600 * FRAMES_PER_SECOND,
            ..Stats::default()
        };
        tracker.credits_running(true, &late);
        assert!(!tracker.unlocked().has(index("against-the-clock")));
        tracker.credits_running(true, &Stats::default());
        assert!(
            !tracker.unlocked().has(index("against-the-clock")),
            "the same roll"
        );
        tracker.credits_running(false, &Stats::default());
        tracker.credits_running(true, &Stats::default());
        assert!(tracker.unlocked().has(index("against-the-clock")));
    }

    #[test]
    fn the_last_one_unlocks_the_legend_and_one_toast_tells_of_many() {
        let (mut tracker, _) = tracker();
        let others: Vec<usize> = (0..ACHIEVEMENTS.len() - 1).collect();
        tracker.unlock(&others[..others.len() - 1]);
        assert!(!tracker.unlocked().has(index("legend-of-zi")));
        assert_eq!(tracker.step(true), Some(TOAST_SOUND));
        assert_eq!(
            tracker.toast.map(|toast| toast.news),
            Some(News::Many(ACHIEVEMENTS.len() - 2))
        );
        tracker.unlock(&others[others.len() - 1..]);
        assert!(tracker.unlocked().has(index("legend-of-zi")));
        assert_eq!(tracker.unlocked().count(), ACHIEVEMENTS.len());
        for _ in 0..TOAST_SLIDE * 2 + TOAST_HOLD {
            assert_eq!(tracker.step(true), None);
        }
        assert_eq!(tracker.step(false), None, "not while hidden");
        assert_eq!(tracker.step(true), Some(TOAST_SOUND));
        assert_eq!(tracker.toast.map(|toast| toast.news), Some(News::Many(2)));
    }

    #[test]
    fn an_empty_total_never_counts_as_complete() {
        let mut tracker = Tracker {
            totals: Some(Totals::default()),
            ..Tracker::default()
        };
        tracker.check(&[], &state_with(|_, _| {}), &Stats::default());
        assert!(!tracker.unlocked().has(index("complete-archive")));
        assert!(!tracker.unlocked().has(index("treasure-hunter")));
        assert!(!tracker.unlocked().has(index("whos-who")));
    }

    #[test]
    fn the_toast_slides_down_and_back_up() {
        let at = |frames| Toast {
            news: News::One(0),
            frames,
        };
        assert_eq!(at(0).hidden(48), 48);
        assert_eq!(at(4).hidden(48), 24);
        assert_eq!(at(TOAST_SLIDE).hidden(48), 0);
        assert_eq!(at(TOAST_SLIDE + TOAST_HOLD).hidden(48), 0);
        assert_eq!(at(TOAST_SLIDE * 2 + TOAST_HOLD - 2).hidden(48), 36);
    }
}
