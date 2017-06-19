
use std::path::Path;
use config as libconfig;
use config::{Value, Config};

static CONFIG_FILE: &'static str = "/etc/ipset_listener.conf";

/// Store an IPSet set
pub struct SetIpset {
    pub name: String,
    pub type_name: String,
    pub maxelem: u64,
}

/// Store an IPSet set
pub struct GlobalConfig {
    /// Limit the server to a certain number of threads
    pub limit_threads: u32,
    /// Binary to call when spawning ipset
    pub ipset_bin: String,
    /// Address to listen on
    pub listen_addr: Vec<String>,
    /// IPSet set to use
    pub registered_users_set: SetIpset
}


fn config_from_file() -> Config{
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

    setup_default_values(conf)
}

fn setup_default_values(mut conf: Config) -> Config{
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


/// Return a vector of strings from an array defined in the config file
fn config_array_str_to_vec(config_array: Vec<Value>) -> Vec<String> {
    let mut str_array: Vec<String> = vec![];
    for i in config_array {
        str_array.push(match i.into_str() {
            Ok(s) => s,
            Err(_) => {
                panic!("Configuration file not defined correctly")
            }
        })
    }

    str_array
}


lazy_static! {
    static ref CONFIG: Config = config_from_file();

    pub static ref GLOBAL_CONFIG: GlobalConfig = GlobalConfig {
        limit_threads: CONFIG.get_int("threads").unwrap() as u32,
        ipset_bin: CONFIG.get_str("ipset_bin").unwrap(),
        listen_addr: config_array_str_to_vec(
            CONFIG.get_array("listen_addr").unwrap()
        ),
        registered_users_set: SetIpset {
            name: CONFIG.get_str(
                "registered_users_set.name"
            ).unwrap(),
            type_name: CONFIG.get_str(
                "registered_users_set.type_name"
            ).unwrap(),
            maxelem: CONFIG.get_int(
                "registered_users_set.maxelem"
            ).unwrap() as u64
        },
    };
}
