
use regex::Regex;
use crossbeam::scope;
use std::error::Error;
use std::io::prelude::{Read, Write};
use std::net::{self, IpAddr, TcpStream, TcpListener, ToSocketAddrs};
use std::sync::{Arc, Mutex, Condvar};
use std::process::Command;
use std::time::Duration;

use conf::Configuration;
use multisocketaddr::MultiSocketAddr;


static RE_MAC_PATTERN: &'static str = (
    r"(?P<mac>([a-f\d]{1,2}:){5}[a-f\d]{1,2})"
);

struct CompiledRegexes {
    daemon_proto: Regex,
    macaddr: Regex,
}

pub struct IPSetListenerDaemon {
    mutex_running_threads: Arc<(Mutex<u32>, Condvar)>,
    conf: Configuration,
    regexes: CompiledRegexes,
}


impl IPSetListenerDaemon {
    pub fn new(conf: Configuration) -> Self {
        IPSetListenerDaemon {
            mutex_running_threads: Arc::new(
                (Mutex::new(0u32), Condvar::new())
            ),
            conf: conf,
            regexes: CompiledRegexes {
                daemon_proto: (
                    Regex::new(
                        r"^(?P<action>[[:alpha:]]) *(?P<arg>.*)$"
                    ).unwrap()
                ),
                macaddr: Regex::new(RE_MAC_PATTERN).unwrap(),
            }
        }
    }

    pub fn start(&self) {
        let mut listeners = Vec::new();
        let mut multi = MultiSocketAddr::new();
        for addr in self.conf.listen_addr.iter() {
            multi.add(addr).unwrap();
        }

        for addr in multi.to_socket_addrs().unwrap() {
            listeners.push(scope(|scope| {
                scope.spawn(|| {
                    self.listen_on_addr(addr);
                })
            }));
        }
    }

    /// Create a TcpListener for the sent addr
    ///
    /// addr <SocketAddr>: Address to bind on
    fn listen_on_addr(&self, addr: net::SocketAddr) {
        let listener = TcpListener::bind(addr).unwrap();
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    // Requests should be snappy enough to never reach the 60
                    // seconds of timeout. If they reach it, we have another
                    // problem somewhere else…
                    {
                        let timeout = Some(Duration::new(60, 0));
                        let _ = stream.set_read_timeout(timeout);
                        let _ = stream.set_write_timeout(timeout);
                    }
                    self.wait_until_free_slot();
                    scope(|scope| {
                        scope.spawn(|| {
                            self.decrement_threads_and_handle_client(stream)
                        });
                    });
                },
                Err(_) => {
                    break
                }
            }
        }
        drop(listener);
    }


    fn wait_until_free_slot(&self) {
        let &(ref lock, ref cvar) = &*self.mutex_running_threads;
        let mut nb_threads = lock.lock().unwrap();
        // If we reached the limit, wait until any thread exits
        while *nb_threads >= self.conf.threads {
            nb_threads = cvar.wait(nb_threads).unwrap();
        }
        debug!("{}", *nb_threads);
        *nb_threads += 1;
    }


    fn decrement_threads_and_handle_client(&self, stream: TcpStream) {
        let &(ref lock, ref cvar) = &*self.mutex_running_threads;
        info!("New client…");
        self.handle_client(&stream);
        {
            let mut nb_threads = lock.lock().unwrap();
            *nb_threads -= 1;
            debug!("{}", *nb_threads);
        }
        // Notifies one waiting thread that the current one is
        // exiting
        cvar.notify_one();
    }


    /// Handle a new client and call to compute the response
    ///
    /// s <TcpStream>: client's stream
    fn handle_client(&self, s: &TcpStream) {
        let mut response: String = String::new();
        for b_result in s.bytes() {
            let b: u8 = b_result.unwrap();
            response.push(b as char);
            // End of line. Parse the received request.
            if b == 10 {
                response = String::from(response.trim());
                self.compute_response(&response, s);
                response.clear();
            }
        }

        if response.len() > 0 {
            response = String::from(response.trim());
            self.compute_response(&response, s);
        }
    }


    /// Checks if the response is correct and parse it
    fn compute_response(&self, response: &String, mut s: &TcpStream) {
        let send_error = |mut s: &TcpStream, err: &str| {
            s.write(&(format!("1 {}\r\n", err.trim_right())).into_bytes()).unwrap()
        };

        let mut bad_request: bool = false;
        match self.regexes.daemon_proto.captures(response.as_str()) {
            Some(capt) => {
                let action = capt.name("action").unwrap().as_str();
                let arg = capt.name("arg").unwrap().as_str();
                info!("{:?}", (action, arg));

                match action {
                    ipset_act @ "a" | ipset_act @ "d" => {
                        match self.add_or_delete_mac(ipset_act, arg) {
                            Ok(r) => { s.write(r.as_bytes()).unwrap(); },
                            Err(err) => { send_error(&s, &err); }
                        }
                    }, "m" => {
                        let ipaddr = arg;
                        match self.get_mac(ipaddr) {
                            Ok(mac) => {
                                let response = format!(
                                    "0 {}\r\n", mac
                                ).into_bytes();
                                s.write(&response).unwrap();
                            }
                            Err(err) => { send_error(&s, err.as_str()); },
                        };
                    }, _ => bad_request = true,
                }
            }, None => bad_request = true,
        }

        if bad_request {
            let msg: String = format!(
                "\"{}\": Request doesn't respect the protocol", response
            );
            error!("{}", msg.as_str());
            send_error(&s, msg.as_str());
        }
    }


    fn add_or_delete_mac(&self, ipset_act: &str,
                         ipset_arg: &str) -> Result<String, String> {
        let mac_addr = match self.regexes.macaddr.captures(ipset_arg) {
            Some(mac_capt) => mac_capt.name("mac").unwrap().as_str(),
            None => return Err(String::from("Missing mac address")),
        };
        let cmd = match ipset_act {
            "a" => "add",
            "d" => "del",
            _ => panic!("Action doesn't match"),
        };
        if mac_addr != "" {
            match self.spawn_ipset(
                &[
                    cmd, "-exist",
                    &self.conf.registered_users_set.name, mac_addr
                ]
            ) {
                Ok(()) =>  return Ok(String::from("0\r\n")),
                Err(err) => return Err(err),
            };
        }

        Err(String::from("Missing mac address"))
    }


    /// Interacts with ipset
    ///
    /// First creates the set, with -exist to avoid any error if the wanted set
    /// already exists, then executes ipset with arguments received in parameter
    ///
    /// ipset_args <&[&str]>: arguments for ipset
    fn spawn_ipset(&self, ipset_args: &[&str]) -> Result<(), String> {
        // Ensure that our set exists in ipset
        match self.create_ipset_set() {
            Ok(()) => {},
            Err(err) => return Err(err),
        }

        debug!("Launch \"{} {}\"", self.conf.ipset_bin, ipset_args.join(" "));
        let panic_err = |e: &str| {
            let msg: String = format!(
                "Failed to launch \"{} {}\"",
                self.conf.ipset_bin, ipset_args.join(" ")
            );
            error!("{}: {}", msg, e);
            msg
        };
        let launch_cmd = match Command::new(&self.conf.ipset_bin)
            .args(ipset_args)
            .output() {
                Ok(p) => p,
                Err(err) => return Err(panic_err(err.description().trim_right())),
            };
        if ! launch_cmd.status.success() {
            return Err(panic_err(
                &String::from_utf8(launch_cmd.stderr).unwrap().trim_right()
            ));
        }

        Ok(())
    }


    /// Create our set in ipset
    fn create_ipset_set(&self) -> Result<(), String> {
        debug!(
            "Creates set {} in ipset.", self.conf.registered_users_set.name
        );
        let panic_err = |e: &str| -> String {
            let msg: String = format!(
                "Failed to create {} in ipset",
                self.conf.registered_users_set.name
            );
            error!("{}: {}", msg, e);
            msg
        };
        let creation = match Command::new(&self.conf.ipset_bin)
            .arg("create").arg("-exist")
            .arg(&self.conf.registered_users_set.name)
            .arg(&self.conf.registered_users_set.type_name)
            .arg("maxelem")
            .arg(self.conf.registered_users_set.maxelem.to_string())
            .output() {
                Ok(p) => p,
                Err(err) => return Err(panic_err(err.description().trim_right())),
            };
        if ! creation.status.success() {
            return Err(panic_err(
                &String::from_utf8(creation.stderr).unwrap().trim_right()
            ));
        }

        Ok(())
    }


    /// Look for all mac addresses linked to the sent IP
    ///
    /// ip <&str>: arguments for ipset
    fn get_mac<'a>(&self, ip: &'a str) -> Result<String, String> {
        if !Self::is_ip_addr(ip) {
            return Err(String::from("Not an IP address"))
        }

        let ip_bin = "ip";
        let ip_args = ["n", "show", "to", ip];

        debug!("Launch \"{} {}\"", ip_bin, ip_args.join(" "));
        let panic_err = |e: &str| {
            let msg: String = format!(
                "Failed to launch \"{} {}\"", ip_bin, ip_args.join(" ")
            );
            error!("{}: {}", msg, e);
            msg
        };

        let launch_cmd = match Command::new(ip_bin).args(&ip_args)
            .output() {
                Ok(p) => p,
                Err(err) => return Err(panic_err(err.description().trim_right())),
            };
        if launch_cmd.status.success() {
            let mac_addr_result = self.filter_mac(
                String::from_utf8(launch_cmd.stdout).unwrap().trim_right()
            );
            return match mac_addr_result {
                Ok(m) => Ok(m),
                Err(e) => Err(panic_err(e.as_str())),
            }
        }
        else {
            return Err(panic_err(
                String::from_utf8(launch_cmd.stderr).unwrap().trim_right()
            ))
        }
    }


    /// Apply a regex on the "ip neigh" output to get the mac_address
    ///
    /// output <&str>: "ip neigh" output
    fn filter_mac(&self, output: &str) -> Result<String, String> {
        let mac_addr = match self.regexes.macaddr.captures(output) {
            Some(capt) => capt.name("mac").unwrap().as_str(),
            None => "",
        };
        match mac_addr {
            "" => Err(String::from("MAC cannot be found")),
            m => Ok(String::from(m)),
        }
    }


    /// Is an IP address ?
    ///
    /// returns bool
    fn is_ip_addr(s: &str) -> bool {
        match s.parse::<IpAddr>() {
            Ok(_) => return true,
            Err(_) => return false
        }
    }
}
