use crate::Action;

pub struct SafeInterval {
    pub s_time: u64,
    pub e_time: u64,
    pub agent_id: u64,
    pub action: Action,
}

impl SafeInterval {
    pub fn new(s_time: u64, e_time: u64, agent_id: u64, action: Action) -> SafeInterval {
        SafeInterval {
            s_time,
            e_time,
            agent_id,
            action,
        }
    }
}
