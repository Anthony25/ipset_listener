
use std::path::Path;
use config as libconfig;
use config::{Value, Config};

static CONFIG_FILE: &'static str = "/etc/ipset_listener.conf";

/// Store an IPSet set
#[derive(Deserialize)]
pub struct SetIpset {
    pub name: String,
    pub type_name: String,
    pub maxelem: u64,
}


/// Global configuration
#[derive(Deserialize)]
pub struct Configuration {
    /// Limit the server to a certain number of threads
    pub threads: u32,
    /// Binary to call when spawning ipset
    pub ipset_bin: String,
    /// Address to listen on
    pub listen_addr: Vec<String>,
    /// IPSet set to use
    pub registered_users_set: SetIpset
}


impl Configuration {
    pub fn new() -> Self {
        let mut c = Self::config_from_file_if_exists();
        c = Self::setup_default_values_to_config(c);

        c.deserialize().unwrap()
    }

    fn config_from_file_if_exists() -> Config{
        let mut conf = Config::new();
        if Path::new(CONFIG_FILE).is_file() {
            // Not using `.required(false)`, as it does returns an error if parsing
            // failed
            conf.merge(
                libconfig::File::new(
                    CONFIG_FILE, libconfig::FileFormat::Yaml
                )
            ).unwrap();
        }
        conf
    }

    fn setup_default_values_to_config(mut conf: Config) -> Config{
        conf.set_default("threads", 100).unwrap();
        conf.set_default("ipset_bin", "ipset").unwrap();
        conf.set_default(
            "listen_addr", vec![ "127.0.0.1:8000", "[::1]:8000" ]
        ).unwrap();
        conf.set_default(
            "registered_users_set.name", "registered_users"
        ).unwrap();
        conf.set_default(
            "registered_users_set.type_name", "hash:mac"
        ).unwrap();
        conf.set_default(
            "registered_users_set.maxelem", 65536
        ).unwrap();

        conf
    }
}
