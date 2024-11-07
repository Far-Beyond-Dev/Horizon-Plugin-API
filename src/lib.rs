pub struct Plugin {}

#[derive(PartialEq, Eq,Hash)]
pub enum Pluginstate {
    ACTIVE,
    INACTIVE,
    CRASH,
}
