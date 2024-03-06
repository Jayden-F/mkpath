use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use ahash::{HashMap, HashSet};
use enumset::EnumSet;
use mkpath_core::NodeBuilder;
use mkpath_cpd::{BucketQueueFactory, CpdRow, FirstMoveSearcher, StateIdMapper};
use mkpath_grid::{BitGrid, Direction, EightConnectedExpander, Grid, GridPool};
use mkpath_jps::{canonical_successors, CanonicalGridExpander, JumpDatabase};
use rayon::prelude::*;

mod tops_expander;

pub use self::tops_expander::*;

pub struct ToppingPlusOracle {
    mapper: GridMapper,
    jump_db: JumpDatabase,
    partial_cpd: HashMap<(i32, i32), CpdRow>,
}

impl ToppingPlusOracle {
    pub fn compute(
        map: BitGrid,
        progress_callback: impl Fn(usize, usize, Duration) + Sync,
    ) -> Self {
        use Direction::*;

        let jump_db = JumpDatabase::new(map);
        let map = jump_db.map();
        let mapper = GridMapper::dfs_preorder(map);

        let diagonals = NorthWest | SouthWest | NorthEast | SouthEast;

        let mut jump_points = HashSet::default();
        for y in 0..map.height() {
            for x in 0..map.width() {
                if !map.get(x, y) {
                    continue;
                }

                let nb = map.get_neighborhood(x, y);
                let mut diagonal_jumps = EnumSet::empty();
                let mut is_jp = false;

                for dir in [North, South, East, West] {
                    if !nb.contains(dir.backwards()) {
                        continue;
                    }
                    let dirs = canonical_successors(nb, Some(dir));
                    if dirs & dir != dirs {
                        is_jp = true;
                        diagonal_jumps |= dirs & diagonals;
                    }
                }

                if is_jp {
                    jump_points.insert((x, y));

                    if diagonal_jumps.contains(NorthWest) {
                        collect_diagonal_jps(&mut jump_points, &jump_db, x, y, NorthWest);
                    }
                    if diagonal_jumps.contains(SouthWest) {
                        collect_diagonal_jps(&mut jump_points, &jump_db, x, y, SouthWest);
                    }
                    if diagonal_jumps.contains(SouthEast) {
                        collect_diagonal_jps(&mut jump_points, &jump_db, x, y, SouthEast);
                    }
                    if diagonal_jumps.contains(NorthEast) {
                        collect_diagonal_jps(&mut jump_points, &jump_db, x, y, NorthEast);
                    }
                }
            }
        }

        let progress = AtomicUsize::new(0);
        let start = std::time::Instant::now();
        let num_jps = jump_points.len();

        let partial_cpd: HashMap<_, _> = jump_points
            .par_iter()
            .map_init(
                || {
                    let mut builder = NodeBuilder::new();
                    let state = builder.add_field((-1, -1));
                    let searcher = FirstMoveSearcher::new(&mut builder);
                    let pqueue = BucketQueueFactory::new(&mut builder);
                    let pool = GridPool::new(
                        builder.build_with_capacity(mapper.array.len()),
                        state,
                        map.width(),
                        map.height(),
                    );
                    (state, searcher, pqueue, pool)
                },
                |(state, searcher, pqueue, pool), &source| {
                    pool.reset();
                    let result = CpdRow::compute(
                        &mapper,
                        searcher,
                        CanonicalGridExpander::new(jump_db.map(), pool),
                        pqueue.new_queue(searcher.g(), 0.999),
                        pool.generate(source),
                        *state,
                    );
                    let progress = progress.fetch_add(1, Ordering::Relaxed) + 1;
                    progress_callback(progress, num_jps, start.elapsed());
                    (source, result)
                },
            )
            .collect();

        ToppingPlusOracle {
            mapper,
            jump_db,
            partial_cpd,
        }
    }

    pub fn load(map: BitGrid, from: &mut impl Read) -> std::io::Result<Self> {
        let jump_db = JumpDatabase::new(map);
        let mapper = GridMapper::load(from)?;

        let mut bytes = [0; 4];
        from.read_exact(&mut bytes)?;
        let num_jps = u32::from_le_bytes(bytes) as usize;

        let mut partial_cpd = HashMap::default();
        for _ in 0..num_jps {
            from.read_exact(&mut bytes)?;
            let x = i32::from_le_bytes(bytes);
            from.read_exact(&mut bytes)?;
            let y = i32::from_le_bytes(bytes);

            assert!(x >= 0);
            assert!(y >= 0);
            assert!(x < jump_db.map().width());
            assert!(y < jump_db.map().height());

            partial_cpd.insert((x, y), CpdRow::load(from)?);
        }

        Ok(ToppingPlusOracle {
            mapper,
            jump_db,
            partial_cpd,
        })
    }

    pub fn save(&self, to: &mut impl Write) -> std::io::Result<()> {
        self.mapper.save(to)?;
        to.write_all(&u32::to_le_bytes(self.partial_cpd.len() as u32))?;
        for ((x, y), row) in &self.partial_cpd {
            to.write_all(&x.to_le_bytes())?;
            to.write_all(&y.to_le_bytes())?;
            row.save(to)?;
        }
        Ok(())
    }

    pub fn query(&self, pos: (i32, i32), target: (i32, i32)) -> Option<Direction> {
        self.partial_cpd
            .get(&pos)
            .and_then(|row| row.lookup(self.mapper.state_to_id(target)).try_into().ok())
    }
}

fn collect_diagonal_jps(
    jump_points: &mut HashSet<(i32, i32)>,
    jump_db: &JumpDatabase,
    mut x: i32,
    mut y: i32,
    dir: Direction,
) {
    let (dx, dy) = match dir {
        Direction::NorthWest => (-1, -1),
        Direction::SouthWest => (-1, 1),
        Direction::SouthEast => (1, 1),
        Direction::NorthEast => (1, -1),
        _ => unreachable!(),
    };

    while let (dist, true) = jump_db.get(x, y, dir) {
        x += dx * dist;
        y += dy * dist;
        jump_points.insert((x, y));
    }
}

pub struct GridMapper {
    grid: Grid<usize>,
    array: Box<[(i32, i32)]>,
}

impl GridMapper {
    pub fn dfs_preorder(map: &BitGrid) -> Self {
        let mut grid = Grid::new(map.width(), map.height(), |_, _| usize::MAX);
        let mut array = vec![];

        let mut builder = NodeBuilder::new();
        let state = builder.add_field((-1, -1));
        let mut pool = GridPool::new(builder.build(), state, map.width(), map.height());

        for y in 0..map.height() {
            for x in 0..map.width() {
                if !map.get(x, y) || grid[(x, y)] != usize::MAX {
                    continue;
                }

                pool.reset();
                mkpath_cpd::dfs_traversal(
                    pool.generate((x, y)),
                    EightConnectedExpander::new(&map, &pool),
                    |node| {
                        if grid[node.get(state)] == usize::MAX {
                            grid[node.get(state)] = array.len();
                            array.push(node.get(state));
                            true
                        } else {
                            false
                        }
                    },
                );
            }
        }

        GridMapper {
            grid,
            array: array.into_boxed_slice(),
        }
    }

    pub fn load(from: &mut impl Read) -> std::io::Result<Self> {
        let mut bytes = [0; 4];
        from.read_exact(&mut bytes)?;
        let len = u32::from_le_bytes(bytes) as usize;

        from.read_exact(&mut bytes)?;
        let width = i32::from_le_bytes(bytes);
        from.read_exact(&mut bytes)?;
        let height = i32::from_le_bytes(bytes);

        let mut grid = Grid::new(width, height, |_, _| usize::MAX);
        let mut array = vec![(0, 0); len].into_boxed_slice();
        for id in 0..len {
            from.read_exact(&mut bytes)?;
            let x = i32::from_le_bytes(bytes);
            from.read_exact(&mut bytes)?;
            let y = i32::from_le_bytes(bytes);
            grid[(x, y)] = id;
            array[id] = (x, y);
        }

        Ok(GridMapper { grid, array })
    }

    pub fn save(&self, to: &mut impl Write) -> std::io::Result<()> {
        to.write_all(&(self.array.len() as u32).to_le_bytes())?;
        to.write_all(&self.grid.width().to_le_bytes())?;
        to.write_all(&self.grid.height().to_le_bytes())?;
        for (x, y) in self.array.iter() {
            to.write_all(&x.to_le_bytes())?;
            to.write_all(&y.to_le_bytes())?;
        }
        Ok(())
    }
}

impl StateIdMapper for GridMapper {
    type State = (i32, i32);

    fn num_ids(&self) -> usize {
        self.array.len()
    }

    fn state_to_id(&self, state: Self::State) -> usize {
        self.grid[state]
    }

    fn id_to_state(&self, id: usize) -> Self::State {
        self.array[id]
    }
}
