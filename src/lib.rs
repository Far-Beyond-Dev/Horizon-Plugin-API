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

pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub hotfix: u16,
}

#[macro_export]
macro_rules! get_plugin {
    ($name:ident, $plugins:expr) => {
        $plugins
            .get(stringify!($name))
            .map(|p| &p as &dyn $name::PluginAPI)
            .expect(&format!("Plugin {} not found", stringify!($name)))
    };
}