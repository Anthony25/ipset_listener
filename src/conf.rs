
use std::path::Path;
use config as libconfig;
use config::types::{Value, ScalarValue};

static CONFIG_FILE: &'static str = "/etc/ipset_listener.conf";

/// Store an IPSet set
pub struct SetIpset<'a> {
    pub name: &'a str,
    pub type_name: &'a str,
    pub maxelem: u64,
}


fn config_from_file() -> libconfig::types::Config{
    let my_conf = libconfig::reader::from_file(Path::new(CONFIG_FILE));
    my_conf.is_ok();

    let configuration = my_conf.expect("Error in configuration");
    configuration
}


/// Return a vector of strings from an array defined in the config file
fn config_array_str_to_vec<'a>(config_array: &'a Value) -> Vec<&'a str> {
    match config_array {
        &Value::Array(ref array) => {
            let mut str_array: Vec<&'a str> = vec![];
            for i in array.iter() {
                str_array.push(match i {
                    &Value::Svalue(ScalarValue::Str(ref s)) => { s }
                    _ => { panic!("Configuration file not defined correctly") }
                })
            }
            str_array
        },
        _ => vec![]
    }
}


lazy_static! {
    static ref CONFIG: libconfig::types::Config = config_from_file();

    /// Limit the server to a certain number of threads
    pub static ref LIMIT_THREADS: u32 = (
        CONFIG.lookup_integer32_or("threads", 1000) as u32
    );

    /// Binary to call when spawning ipset
    pub static ref IPSET_BIN: &'static str = CONFIG.lookup_str_or(
        "ipset_bin", "ipset"
    );

    pub static ref LISTEN_ADDR: Vec<&'static str> = (
        config_array_str_to_vec(
            CONFIG.lookup("listen_addr").unwrap()
        )
    );

    /// IPSet set to use
    pub static ref REGISTERED_USERS_SET: SetIpset<'static> = SetIpset {
        name: CONFIG.lookup_str_or(
            "registered_users_set.name", "registered_users"
        ),
        type_name: CONFIG.lookup_str_or(
            "registered_users_set.type_name", "hash:mac"
        ),
        maxelem: CONFIG.lookup_integer64_or(
            "registered_users_set.maxelem", 65536
        ) as u64
    };
}
