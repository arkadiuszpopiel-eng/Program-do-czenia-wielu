//! Komendy IPC (COMMANDS.md) jako metody `AppCore::<przestrzeń>_<nazwa>`, po pliku na przestrzeń.

pub(crate) mod account_keys;
mod accounts;
mod agents;
pub(crate) mod app;
mod bridges;
pub(crate) mod cameras;
mod costs;
pub(crate) mod device;
pub(crate) mod files;
mod memory;
mod models;
mod permissions;
mod queue;
mod quick;
mod sessions;
pub(crate) mod settings;
mod model_routing;
mod shared_queue;
mod system;
mod tasks;
mod timeline;
mod transfer;
mod turn_intents;
mod turns;
mod voice;
mod work;
mod workdir;
