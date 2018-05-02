use errors::*;

use std::fs::File;
use std::io::Read;

use iptables;

use regex::Regex;

use chrono::Duration;
use chrono::offset::Utc;

const IPT_CHAIN: &str = "prerouting_public_rule";
const IPT_TABLE: &str = "nat";
const CONFIG_FILE: &str = "/etc/zealot_rule_valid_time";

lazy_static! {
    static ref MAC_SOURCE_REGEX: Regex = Regex::new(
        r"--mac-source\s([a-fA-F0-9:]{17})").unwrap();
    static ref TIMESTAMP_REGEX: Regex = Regex::new(
        r#""timestamp=(\d+)""#).unwrap();
}

#[derive(PartialEq, Debug)]
struct Rule<'a> {
    mac_source: &'a str,
    timestamp: i64,
}

impl<'rule> Rule<'rule> {
    fn parse<'a>(rule: &'a str) -> Option<Rule<'a>> {
        let mac_source_capt = MAC_SOURCE_REGEX.captures(rule);
        let timestamp_capt = TIMESTAMP_REGEX.captures(rule);

        if mac_source_capt.is_none() || timestamp_capt.is_none() {
            return None;
        }

        let timestamp_capt = timestamp_capt.and_then(|t| t.get(1));
        if let Some(Ok(timestamp)) = timestamp_capt.map(|t| t.as_str().parse::<i64>()) {
            Some(Rule {
                mac_source: mac_source_capt.unwrap().get(1).unwrap().as_str(),
                timestamp: timestamp,
            })
        } else {
            None
        }
    }

    fn to_string(&self) -> String {
        format!(
            r#"-m mac --mac-source {} -m comment --comment timestamp={} -j ACCEPT"#,
            self.mac_source,
            self.timestamp
        )
    }

    fn is_expired(&self, valid_time: Duration) -> bool {
        self.timestamp + valid_time.num_seconds() < Utc::now().timestamp()
    }
}

fn read_valid_time() -> Duration {
    if let Ok(mut file) = File::open(CONFIG_FILE) {
        let mut time_str = String::new();

        if file.read_to_string(&mut time_str).is_ok() {
            if let Ok(time) = time_str.parse::<i64>() {
                return Duration::seconds(time);
            }
        }
    }

    // default to 24 hours
    Duration::hours(24)
}

/// Checks for expired accesses in iptables
///
/// # Arguments
///
/// `valid_time` - The time it takes until an access is expired.
pub fn check_for_expired(valid_time: Option<Duration>) -> Result<()> {
    let valid_time = valid_time.unwrap_or_else(read_valid_time);

    let ipt = iptables::new(false).unwrap();

    let rules = ipt.list(IPT_TABLE, IPT_CHAIN)
        .chain_err(|| "Could not list the chain rules!")?;

    for rule in rules {
        if let Some(rule) = Rule::parse(&rule) {
            if rule.is_expired(valid_time) {
                ipt.delete(IPT_TABLE, IPT_CHAIN, &rule.to_string())
                    .chain_err(|| format!("Error deleting rule: {}", rule.to_string()))?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_parse() {
        let expected_rule = Rule {
            mac_source: "DE:AD:BE:EF:DE:AD",
            timestamp: 233445,
        };

        let rule = Rule::parse(
            "-A prerouting_public_rule -m mac --mac-source DE:AD:BE:EF:DE:AD
                     -m comment --comment \"timestamp=233445\" -j ACCEPT",
        ).expect("Error parsing the rule");

        assert_eq!(expected_rule, rule);
    }

    #[test]
    fn test_rule_parse_fail() {
        assert!(
            Rule::parse(
                "-A prerouting_public_rule -m mac
                     -m comment --comment \"timestamp=233445\" -j ACCEPT"
            ).is_none()
        );
    }

    #[test]
    fn test_rule_parse_fail_mac_wrong() {
        assert!(
            Rule::parse(
                "-A prerouting_public_rule -m mac  --mac-source DE:AD:BE:EG:DE:AD
                     -m comment --comment \"timestamp=233445\" -j ACCEPT"
            ).is_none()
        );

        assert!(
            Rule::parse(
                "-A prerouting_public_rule -m mac  --mac-source DE:AD:BE:DE:AD
                     -m comment --comment \"timestamp=233445\" -j ACCEPT"
            ).is_none()
        );
    }

    #[test]
    fn test_rule_parse_fail_timestamp_wrong() {
        assert!(
            Rule::parse(
                "-A prerouting_public_rule -m mac  --mac-source DE:AD:BE:DE:AD:DE
                     -m comment --comment \"timestamp=hi\" -j ACCEPT"
            ).is_none()
        );
    }

    #[test]
    fn test_rule_expired() {
        let duration = Duration::hours(1);
        let time = Utc::now().timestamp();
        let mut rule = Rule {
            mac_source: "",
            timestamp: time - duration.num_seconds() - 10,
        };

        assert!(rule.is_expired(duration));

        rule = Rule {
            mac_source: "",
            timestamp: time,
        };

        assert!(!rule.is_expired(duration));
    }

    #[test]
    fn test_rule_to_string() {
        let rule = Rule {
            mac_source: "DE:AD:BE:DE:AD:DE",
            timestamp: 3456,
        };

        let expected_result = "-m mac --mac-source DE:AD:BE:DE:AD:DE -m comment --comment \
                               timestamp=3456 -j ACCEPT";

        assert_eq!(expected_result, rule.to_string());
    }

}
