use super::{Machine, Repo, Role, Tool};

pub struct Plan {
    pub role: Role,
    pub repos: Vec<&'static Repo>,
    pub tools: Vec<&'static Tool>,
}

impl Plan {
    pub fn missing(&self, machine: &Machine) -> Vec<&'static Tool> {
        self.tools
            .iter()
            .copied()
            .filter(|t| !machine.has(t))
            .collect()
    }

    pub fn to_clone(&self, machine: &Machine) -> Vec<&'static Repo> {
        self.repos
            .iter()
            .copied()
            .filter(|r| !machine.is_cloned(r))
            .collect()
    }
}
