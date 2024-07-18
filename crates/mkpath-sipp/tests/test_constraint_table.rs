use mkpath_sipp::{Action, ConstraintTable, UnsafeInterval};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_constraint() {
        let mut constraint_table = ConstraintTable::new(1);

        constraint_table.add_constraint(0, UnsafeInterval::new(5, 10, 0, Action::Up));

        assert_eq!(
            constraint_table.get_unsafe_interval(0, 0),
            UnsafeInterval::new(5, 10, 0, Action::Up)
        );
    }

    #[test]
    fn find_constraint() {
        let mut constraint_table = ConstraintTable::new(1);
        let location = 0;
        let agent_id = 0;
        let action = Action::Up;

        constraint_table.add_constraint(
            location,
            UnsafeInterval::new(5, 10, agent_id, action.clone()),
        );

        assert_eq!(constraint_table.find_interval_index(location, 0), Err(0));

        assert_eq!(constraint_table.find_interval_index(location, 5), Ok(0));

        assert_eq!(constraint_table.find_interval_index(location, 15), Err(1));

        constraint_table.add_constraint(
            location,
            UnsafeInterval::new(15, 20, agent_id, action.clone()),
        );

        assert_eq!(constraint_table.find_interval_index(location, 15), Ok(1));
    }
}
