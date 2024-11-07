#[derive(Clone)]
pub struct Plugin {}

#[derive(PartialEq, Eq,Hash)]
pub enum Pluginstate {
    ACTIVE,
    INACTIVE,
    CRASH,
}

#[derive(Clone)]
pub struct LoadedPlugin {
    pub instance: Plugin,
}