use std::io::{Read, Write};
use std::sync::Mutex;
use std::time::Duration;

use enumset::EnumSet;
use mkpath_grid::{BitGrid, Direction, Grid};
use mkpath_jps::JumpDatabase;
use rayon::prelude::*;

use crate::first_move::FirstMoveComputer;
use crate::independent_jump_points;
use crate::tiebreak::compute_tiebreak_table;

pub struct PartialCellBb {
    jump_db: JumpDatabase,
    partial_bb: Grid<[Rectangle; 8]>,
}

struct Rectangle {
    low_x: i32,
    low_y: i32,
    high_x: i32,
    high_y: i32,
}

impl PartialCellBb {
    pub fn compute(
        map: BitGrid,
        progress_callback: impl FnMut(usize, usize, Duration) + Send,
    ) -> Self {
        let jump_db = JumpDatabase::new(map);
        let map = jump_db.map();
        let jump_points = independent_jump_points(map, &jump_db);

        let start = std::time::Instant::now();
        let num_jps = jump_points.len();
        let progress = Mutex::new((0, progress_callback));

        let partial_bb = Mutex::new(Grid::new(
            jump_db.map().width(),
            jump_db.map().height(),
            |_, _| [(); 8].map(|_| Rectangle::empty()),
        ));
        jump_points.par_iter().for_each_init(
            || FirstMoveComputer::new(map),
            |fm_computer, (&source, &jps)| {
                let tiebreak_table =
                    compute_tiebreak_table(map.get_neighborhood(source.0, source.1), jps);

                let mut result = [(); 8].map(|_| Rectangle::empty());

                fm_computer.compute(source, |(x, y), fm| {
                    let fm = tiebreak_table[fm.as_usize()];
                    let best = fm
                        .iter()
                        .min_by_key(|&d| result[d as usize].area_increase_from_grow(x, y))
                        .unwrap();
                    result[best as usize].grow(x, y);
                });

                let mut progress = progress.lock().unwrap();
                let (progress, callback) = &mut *progress;
                *progress += 1;
                callback(*progress, num_jps, start.elapsed());

                partial_bb.lock().unwrap()[source] = result;
            },
        );

        PartialCellBb {
            jump_db,
            partial_bb: partial_bb.into_inner().unwrap(),
        }
    }

    pub fn load(map: BitGrid, from: &mut impl Read) -> std::io::Result<Self> {
        let jump_db = JumpDatabase::new(map);

        let mut bytes = [0; 4];
        from.read_exact(&mut bytes)?;
        let num_jps = u32::from_le_bytes(bytes) as usize;

        let mut read_i32 = || from.read(&mut bytes).map(|_| i32::from_le_bytes(bytes));

        let mut partial_bb = Grid::new(jump_db.map().width(), jump_db.map().height(), |_, _| {
            [(); 8].map(|_| Rectangle::empty())
        });
        for _ in 0..num_jps {
            let x = read_i32()?;
            let y = read_i32()?;

            assert!(x >= 0);
            assert!(y >= 0);
            assert!(x < jump_db.map().width());
            assert!(y < jump_db.map().height());

            let mut result = [(); 8].map(|_| Rectangle::empty());
            for dir in 0..8 {
                result[dir] = Rectangle {
                    low_x: read_i32()?,
                    low_y: read_i32()?,
                    high_x: read_i32()?,
                    high_y: read_i32()?,
                }
            }
            partial_bb[(x, y)] = result;
        }

        Ok(PartialCellBb {
            jump_db,
            partial_bb,
        })
    }

    pub fn save(&self, to: &mut impl Write) -> std::io::Result<()> {
        let num = self
            .partial_bb
            .storage()
            .iter()
            .filter(|rects| rects.iter().any(|r| !r.is_empty()))
            .count();
        to.write_all(&u32::to_le_bytes(num as u32))?;
        for y in 0..self.partial_bb.height() {
            for x in 0..self.partial_bb.width() {
                let rects = &self.partial_bb[(x, y)];
                if rects.iter().all(|r| r.is_empty()) {
                    continue;
                }
                to.write_all(&x.to_le_bytes())?;
                to.write_all(&y.to_le_bytes())?;
                for rect in rects {
                    to.write_all(&rect.low_x.to_le_bytes())?;
                    to.write_all(&rect.low_y.to_le_bytes())?;
                    to.write_all(&rect.high_x.to_le_bytes())?;
                    to.write_all(&rect.high_y.to_le_bytes())?;
                }
            }
        }
        Ok(())
    }

    pub fn query(&self, pos: (i32, i32), target: (i32, i32)) -> Option<EnumSet<Direction>> {
        let rects = &self.partial_bb[pos];
        let mut any_nonempty = false;
        let mut dirs = EnumSet::empty();
        for d in [
            Direction::North,
            Direction::West,
            Direction::South,
            Direction::East,
            Direction::NorthWest,
            Direction::SouthWest,
            Direction::SouthEast,
            Direction::NorthEast,
        ] {
            any_nonempty |= !rects[d as usize].is_empty();
            if rects[d as usize].contains(target.0, target.1) {
                dirs |= d;
            }
        }
        if any_nonempty {
            assert!(!dirs.is_empty());
            Some(dirs)
        } else {
            None
        }
    }

    pub fn map(&self) -> &BitGrid {
        self.jump_db.map()
    }

    pub fn jump_db(&self) -> &JumpDatabase {
        &self.jump_db
    }
}

impl Rectangle {
    fn empty() -> Self {
        Rectangle {
            low_x: 0,
            low_y: 0,
            high_x: 0,
            high_y: 0,
        }
    }

    fn is_empty(&self) -> bool {
        self.low_x == self.high_x && self.low_y == self.high_y
    }

    fn grow(&mut self, x: i32, y: i32) {
        if self.is_empty() {
            self.low_x = x;
            self.low_y = y;
            self.high_x = x + 1;
            self.high_y = y + 1;
        } else {
            self.low_x = self.low_x.min(x);
            self.low_y = self.low_y.min(y);
            self.high_x = self.high_x.max(x + 1);
            self.high_y = self.high_y.max(y + 1);
        }
    }

    fn area_increase_from_grow(&self, x: i32, y: i32) -> i32 {
        if self.is_empty() {
            return 1;
        }
        let growth_x = (self.low_x - x).max(x - self.high_x).max(0);
        let growth_y = (self.low_y - y).max(y - self.high_y).max(0);
        growth_x * (self.high_y - self.low_y)
            + growth_y * (self.high_x - self.low_x)
            + growth_x * growth_y
    }

    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.low_x && y >= self.low_y && x < self.high_x && y < self.high_y
    }
}
