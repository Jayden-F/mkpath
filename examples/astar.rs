use std::f64::consts::SQRT_2;
use std::path::PathBuf;

use mkpath::grid::{eight_connected, GridPool};
use mkpath::{NodeBuilder, PriorityQueueFactory};
use structopt::StructOpt;

mod movingai;

#[derive(StructOpt)]
struct Options {
    scen: PathBuf,
}

fn main() {
    let opt = Options::from_args();
    let scen = movingai::read_scenario(&opt.scen).unwrap();
    let map = movingai::read_bitgrid(&scen.map).unwrap();

    let mut builder = NodeBuilder::new();
    let state = builder.add_field((-1, -1));
    let g = builder.add_field(f64::INFINITY);
    let h = builder.add_field(f64::NAN);
    let f = builder.add_field(f64::INFINITY);
    let mut open_list_factory = PriorityQueueFactory::new(&mut builder);

    let mut pool = GridPool::new(builder.build(), state, map.width(), map.height());

    for problem in &scen.instances {
        pool.reset();

        let mut open_list = open_list_factory.new_queue((f, h));
        let mut expander = eight_connected::SimpleExpander::new(&map, &pool);
        let mut edges = vec![];

        // start node
        let start = pool.generate(problem.start);
        start.set(g, 0.0);
        start.set(h, octile(problem.start, problem.target));
        start.set(f, start.get(g) + start.get(h));
        open_list.push(start);

        // target node
        let target = pool.generate(problem.target);

        while let Some(node) = open_list.pop() {
            if node.same_ptr(target) {
                break;
            }

            edges.clear();
            expander.expand(node, &mut edges);

            for &(successor, cost) in &edges {
                if successor.get(h).is_nan() {
                    successor.set(h, octile(successor.get(state), problem.target))
                }
                let new_g = node.get(g) + cost;
                if new_g < successor.get(g) {
                    successor.set_parent(Some(node));
                    successor.set(g, new_g);
                    successor.set(f, new_g + successor.get(h));
                    open_list.push(successor);
                }
            }
        }

        if target.get(g) < f64::INFINITY {
            let mut node = target;
            let mut path = vec![target.get(state)];
            while let Some(parent) = node.get_parent() {
                node = parent;
                path.push(node.get(state));
            }
            path.reverse();
            println!("{:.2} {path:?}", target.get(g));
        } else {
            println!("failed to find path");
        }
    }
}

fn octile((x1, y1): (i32, i32), (x2, y2): (i32, i32)) -> f64 {
    let dx = (x1 - x2).abs();
    let dy = (y1 - y2).abs();
    let diagonals = dx.min(dy);
    let orthos = dx.max(dy) - diagonals;
    orthos as f64 + diagonals as f64 * SQRT_2
}
