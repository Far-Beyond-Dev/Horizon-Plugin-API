#[derive(Clone, Debug)]
pub struct Plugin {}

#[derive(PartialEq, Eq, Hash, Clone)]
pub enum Pluginstate {
    ACTIVE,
    INACTIVE,
    CRASH,
}

#[derive(Clone, Debug)]
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
            .map(|p| &p.instance as &dyn $name::PluginAPI)
            .expect(&format!("Plugin {} not found", stringify!($name)))
    };
}

#[macro_export]
macro_rules! get_type_from_plugin {
    ($name:ident, $plugins:expr, $api:ty) => {
        $plugins
            .get(stringify!($name))
            .map(|p| &p.instance as $name::&$api)
            .expect(&format!("Plugin {} not found", stringify!($name)))
    };
}