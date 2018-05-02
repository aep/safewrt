extern crate chrono;
extern crate iptables;
extern crate sentry;

use std::fs::File;
use std::io::Write;

use chrono::Duration;
use chrono::offset::Utc;

const IPT_TABLE: &str = "nat";
const IPT_CHAIN: &str = "prerouting_public_rule";

fn prepare_iptables(duration: Duration, valid_mac: &str, expired_mac: &str) {
    let ipt = iptables::new(false).unwrap();

    ipt.flush_table(IPT_TABLE)
        .expect("Could not flush the table");
    ipt.new_chain(IPT_TABLE, IPT_CHAIN)
        .expect("Could not create new chain");

    let ctime = Utc::now().timestamp();

    ipt.append(
        IPT_TABLE,
        IPT_CHAIN,
        &format!(
            r#"-m mac --mac-source {} -m comment --comment timestamp={} -j ACCEPT"#,
            valid_mac,
            ctime
        ),
    ).expect("Error adding rule 1");

    ipt.append(
        IPT_TABLE,
        IPT_CHAIN,
        &format!(
            r#"-m mac --mac-source {} -m comment --comment timestamp={} -j ACCEPT"#,
            expired_mac,
            ctime - duration.num_seconds() - 10
        ),
    ).expect("Error adding rule 2");
}

fn check_iptables_output(valid_mac: &str, expired_mac: &str) {
    let ipt = iptables::new(false).unwrap();

    let rules = ipt.list(IPT_TABLE, IPT_CHAIN)
        .expect("Could not list rules")
        .iter()
        .flat_map(|s| s.chars())
        .collect::<String>();

    assert!(rules.contains(&valid_mac.to_string()));
    assert!(!rules.contains(&expired_mac.to_string()));
}

#[test]
fn test_access_control() {
    let valid_mac = "DE:AD:BE:DE:AD:DE";
    let expired_mac = "DE:AD:BE:DE:FF:DE";
    let duration = Duration::hours(1);

    prepare_iptables(duration, valid_mac, expired_mac);

    sentry::check_for_expired(Some(duration)).expect("Error calling zealot main");

    check_iptables_output(valid_mac, expired_mac);
}

#[test]
fn test_access_control_duration_from_file() {
    let valid_mac = "DE:AD:BE:DE:AD:DF";
    let expired_mac = "DE:AD:BE:DE:FF:DF";
    let duration = Duration::hours(1);

    prepare_iptables(duration, valid_mac, expired_mac);

    let mut conf_file =
        File::create("/etc/zealot_rule_valid_time").expect("Error creating config file!");
    write!(conf_file, "{}", duration.num_seconds()).expect("Error writing to config file!");

    sentry::check_for_expired(None).expect("Error calling zealot main");

    check_iptables_output(valid_mac, expired_mac);
}
