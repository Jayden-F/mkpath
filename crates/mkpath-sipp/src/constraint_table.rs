use crate::{binary_search, Action, SafeInterval, SearchResult, UnsafeInterval};
use core::cmp::Ordering;

pub struct ConstraintTable {
    resources: Vec<Vec<UnsafeInterval>>,
}

impl ConstraintTable {
    pub fn new(size: usize) -> ConstraintTable {
        ConstraintTable {
            resources: vec![Vec::new(); size],
        }
    }

    pub fn add_constraint(&mut self, location: usize, unsafe_interval: UnsafeInterval) {
        let result = self.find_interval_index(location, unsafe_interval.s_time);
        match result {
            SearchResult::Hit(_) => panic!("Adding constraint that already exists"),
            SearchResult::Miss(index) => self.resources[location].insert(index, unsafe_interval),
        }
    }

    pub fn remove_constraint(
        &mut self,
        location: usize,
        s_time: u64,
        e_time: u64,
        agent_id: u64,
        action: Action,
    ) {
        let result = self.find_interval_index(location, s_time);
        match result {
            SearchResult::Hit(index) => {
                assert_eq!(self.resources[location][index].s_time, s_time);
                assert_eq!(self.resources[location][index].e_time, e_time);
                assert_eq!(self.resources[location][index].agent_id, agent_id);
                assert_eq!(self.resources[location][index].action, action);
                self.resources[location].remove(index);
            }
            SearchResult::Miss(_) => panic!("Adding constraint that does not exist"),
        }
    }

    pub fn get_unsafe_interval(&self, location: usize, index: usize) -> UnsafeInterval {
        self.resources[location][index].clone()
    }

    pub fn get_safe_interval(&self, location: usize, index: usize) -> Option<SafeInterval> {
        let location_constraints = &self.resources[location];
        let num_constraints = location_constraints.len();

        if index > num_constraints {
            return None;
        } else if num_constraints == 0 {
            return Some(SafeInterval::new(0, u64::MAX, 0, Action::Wait));
        }

        if index == 0 {
            Some(SafeInterval::new(
                0,
                location_constraints[0].s_time,
                0,
                Action::Wait,
            ))
        } else if index == num_constraints {
            Some(SafeInterval::new(
                location_constraints[index].e_time,
                u64::MAX,
                location_constraints[index].agent_id,
                location_constraints[index].action.clone(),
            ))
        } else {
            Some(SafeInterval::new(
                location_constraints[index - 1].e_time,
                location_constraints[index].s_time,
                location_constraints[index - 1].agent_id,
                location_constraints[index - 1].action.clone(),
            ))
        }
    }

    pub fn find_interval_index(&self, location: usize, timestep: u64) -> SearchResult {
        let cmp = |x: &UnsafeInterval| -> Ordering {
            if timestep < x.s_time {
                return Ordering::Less;
            } else if timestep >= x.e_time {
                return Ordering::Greater;
            } else {
                return Ordering::Equal;
            }
        };
        binary_search(&self.resources[location], cmp)
    }
}
