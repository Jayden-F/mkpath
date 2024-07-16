use crate::Action;

#[derive(Clone, Debug, PartialEq)]
pub struct UnsafeInterval {
    pub s_time: u64,
    pub e_time: u64,
    pub agent_id: u64,
    pub action: Action,
}

impl UnsafeInterval {
    pub fn new(s_time: u64, e_time: u64, agent_id: u64, action: Action) -> UnsafeInterval {
        UnsafeInterval {
            s_time,
            e_time,
            agent_id,
            action,
        }
    }
}
