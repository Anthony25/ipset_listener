#[macro_use]
extern crate log;
extern crate crossbeam;
extern crate config;
extern crate regex;
extern crate serde;
#[macro_use]
extern crate serde_derive;

mod conf;
mod daemon;
mod multisocketaddr;

use conf::Configuration;
use daemon::IPSetListenerDaemon;


fn main() {
    extern crate env_logger;
    let _ = env_logger::init();

    let conf = Configuration::new();
    let daemon = IPSetListenerDaemon::new(conf);
    daemon.start();
}
