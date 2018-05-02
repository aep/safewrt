use std::process::Command;

fn execute(args: &[&str]) -> Option<String> {
    if let Ok(output) = Command::new("ip").args(args).output() {
        if let Ok(string) = String::from_utf8(output.stdout) {
            return Some(string);
        }
    }

    None
}

fn get_mac_impl(ip: &str, output: &str) -> Option<String> {
    for line in output.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();

        if cols.len() < 6 {
            continue;
        }

        if cols[0] == ip {
            return Some(cols[4].to_owned());
        }
    }

    None
}

pub fn ip_to_mac(ip: &str) -> Option<String> {
    if let Some(output) = execute(&["n"]) {
        get_mac_impl(ip, &output)
    } else {
        None
    }
}



#[cfg(test)]
mod tests {
    use super::get_mac_impl;

    const TEST_IP_OUTPUT: &'static str = "192.168.8.1 dev enp0s20u1 lladdr \
                                          DE:AD:BE:EF:00:11 REACHABLE\n\
                                          192.168.8.2 dev enp0s20u2 lladdr \
                                          DE:AD:BE:EF:00:22 REACHABLE";

    const TEST_INVALID_IP_OUTPUT: &'static str = "192.168.8.1 dev enp0s20u1 lladdr \
                                                  DE:AD:BE:EF:00:11";

    #[test]
    fn test_get_mac() {
        let test_ip_addresses = [
            ("192.168.8.1", Some(String::from("DE:AD:BE:EF:00:11"))),
            ("192.168.8.2", Some(String::from("DE:AD:BE:EF:00:22"))),
            ("192.168.8.3", None),
        ];

        for &(ref test_ip, ref test_result) in &test_ip_addresses {
            assert_eq!(*test_result, get_mac_impl(&test_ip, &TEST_IP_OUTPUT));
        }
    }

    #[test]
    fn test_get_mac_invalid_output() {
        assert_eq!(None, get_mac_impl("192.168.8.1", &TEST_INVALID_IP_OUTPUT));
    }

}
