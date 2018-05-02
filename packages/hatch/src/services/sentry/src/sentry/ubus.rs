//FIXME: Use the ubus rust implementation!

use std::process::Command;

use serde_json;
use std::collections::HashMap;

fn get_ipleases(leases: &str) -> Option<String> {
    if let Ok(output) = Command::new("ubus")
        .args(&["call", "dhcp", leases])
        .output()
    {
        if let Ok(string) = String::from_utf8(output.stdout) {
            return Some(string);
        }
    }

    None
}

fn parse_ipleases(output: &str) -> Vec<(String, String)> {
    let mut result: Vec<(String, String)> = Vec::new();

    let json_result: serde_json::Result<serde_json::Value> = serde_json::from_str(output);

    if let Ok(json) = json_result {
        if let Some(leases) = json["device"]["br-public"]["leases"].as_array() {
            for lease in leases {
                if let Some(hostname) = lease["hostname"].as_str() {
                    if let Some(ip) = lease["ip"].as_str() {
                        result.push((ip.to_owned(), hostname.to_owned()))
                    }
                }
            }
        }
    }

    result
}

pub fn get_hostname_for_ip(ip: &str) -> Option<String> {
    for leases in &["ipv4leases", "ipv6leases"] {
        if let Some(output) = get_ipleases(leases) {
            for &(ref iph, ref hostname) in &parse_ipleases(&output) {
                if iph == ip {
                    return Some(hostname.to_owned());
                }
            }
        }
    }

    None
}

pub fn send_message(channel: &str, data: &HashMap<&str, &str>) {
    if let Ok(data) = serde_json::to_string(&data) {
        if Command::new("ubus")
            .args(&["send", channel, &data])
            .spawn()
            .map(|mut c| c.wait())
            .is_err()
        {
            //FIXME: Add logging
            println!("Error calling ubus!");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    const UBUS_IPLEASES_OUTPUT: &'static str = r#"
    {
        "device": {
                "br-private": {
                        "leases": [

                        ]
                },
                "br-public": {
                        "leases": [
                                {
                                        "mac": "macmacmac",
                                        "hostname": "nixos",
                                        "ip": "192.168.44.200",
                                        "valid": -43175
                                },
                                {
                                        "mac": "macmacmac",
                                        "hostname": "android-b4283b7e2ffccd8",
                                        "ip": "192.168.44.230",
                                        "valid": -42406
                                }
                        ]
                }
        }
   }"#;

    #[test]
    fn test_parse_ipleaeases() {
        let hostnames = parse_ipleases(&UBUS_IPLEASES_OUTPUT);
        assert!(hostnames.contains(&(String::from("192.168.44.200"), String::from("nixos"))));
        assert!(hostnames.contains(&(
            String::from("192.168.44.230"),
            String::from("android-b4283b7e2ffccd8")
        )));
    }

    #[test]
    fn test_command_args_order() {
        let output = Command::new("echo")
            .args(&["call", "dhcp", "ipv4leases"])
            .output()
            .unwrap()
            .stdout;

        assert_eq!(
            &String::from_utf8(output).unwrap(),
            "call dhcp ipv4leases\n"
        );
    }
}
