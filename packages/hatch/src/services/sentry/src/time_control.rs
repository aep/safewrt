use errors::*;

use std::fs::File;
use std::process::Command;

use chrono::{Datelike, TimeZone, Timelike, Weekday};
use chrono::offset::Utc;
use chrono_tz::Tz;

use serde_json;

pub const PUBLIC_WIFI_RADIOS: &[&str] = &["a", "g"];
pub const PUBLIC_WIFI_TIME_CONTROL_PATH: &str = "/etc/zealot.pub.tc";

/// Stores information about the wifi up times.
#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct TimeControl {
    /// The vector of vectors that contain the wifi up times.
    /// The up time is given as hour in 24 hour format.
    up_time: Vec<Vec<u8>>,
    /// The timezone of the given up times.
    timezone: String,
}

impl Default for TimeControl {
    fn default() -> TimeControl {
        TimeControl {
            up_time: vec![],
            timezone: "Europe/Berlin".to_string(),
        }
    }
}

/// Checks if the current status of the public wifi corresponds to the configured
/// up times.
/// If the status does not match, the public wifi is activated/deactivated.
pub fn check_public_wifi() -> Result<()> {
    let wifi_status = is_pub_wifi_enabled().unwrap_or(false);
    let req_wifi_status = get_current_requested_wifi_status().unwrap_or(true);

    if wifi_status != req_wifi_status {
        for standard in PUBLIC_WIFI_RADIOS {
            change_wifi_status(req_wifi_status, standard);
        }

        // activate the changes
        let _ = Command::new("wifi").output();
    }

    Ok(())
}

fn change_wifi_status(enable: bool, standard: &str) {
    let disabled = if enable { 0 } else { 1 };
    let _ = Command::new("uci")
        .args(&[
            "set",
            &format!("wireless.wpublic{}.disabled={}", standard, disabled),
        ])
        .output();
}

/// Checks, based on the time control, if the wifi should be on or off at the time
/// this function is running.
///
/// # Return value
///
/// True => wifi on
/// False => wifi off
fn get_current_requested_wifi_status() -> Result<bool> {
    let time_control =
        File::open(PUBLIC_WIFI_TIME_CONTROL_PATH).chain_err(|| "error reading time control file")?;
    let time_control: TimeControl =
        serde_json::from_reader(time_control).chain_err(|| "error parsing time control file")?;

    let timezone: Tz = time_control.timezone.parse()?;

    let now = timezone.from_utc_datetime(&Utc::now().naive_utc());

    let day_times = time_control
        .up_time
        .get(weekday_to_index(now.date().weekday()));

    Ok(day_times
            .map(|day_times| {
                if day_times.is_empty() {
                    // if no up times are given, the wifi should be activated the whole day
                    true
                } else {
                    day_times.iter().any(|t| *t == now.time().hour() as u8)
                }
            })
            // If None, enable wifi
            .unwrap_or(true))
}

fn weekday_to_index(wday: Weekday) -> usize {
    match wday {
        Weekday::Mon => 0,
        Weekday::Tue => 1,
        Weekday::Wed => 2,
        Weekday::Thu => 3,
        Weekday::Fri => 4,
        Weekday::Sat => 5,
        Weekday::Sun => 6,
    }
}

/// Returns if the public wifi is currently enabled
fn is_pub_wifi_enabled() -> Result<bool> {
    let uci_show = Command::new("uci")
        .args(&["show", "wireless"])
        .output()
        .chain_err(|| "error running uci show wireless")?;

    let uci_show = String::from_utf8(uci_show.stdout)
        .chain_err(|| "error parsing uci show wireless as utf8 string")?;

    Ok(is_pub_wifi_enabled_impl(&uci_show))
}

fn is_pub_wifi_enabled_impl(output: &str) -> bool {
    for standard in PUBLIC_WIFI_RADIOS {
        if output.contains(&format!("wireless.wpublic{}.disabled='1'", standard)) {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const UCI_SHOW_NO_DISABLED: &str = r#"
        wireless.wpublicg=wifi-iface
        wireless.wpublicg.device='radio1'
        wireless.wpublicg.ifname='w-public-g'
        wireless.wpublicg.mode='ap'
        wireless.wpublicg.network='public'
        wireless.wpublicg.country='DE'
        wireless.wpublicg.ssid='spm-test Free'
        wireless.wpublicg.encryption='none'
    "#;

    #[test]
    fn uci_show_no_disabled_parse() {
        assert!(is_pub_wifi_enabled_impl(UCI_SHOW_NO_DISABLED));
    }

    const UCI_SHOW_ENABLED: &str = r#"
        wireless.wpublicg=wifi-iface
        wireless.wpublicg.device='radio1'
        wireless.wpublicg.ifname='w-public-g'
        wireless.wpublicg.mode='ap'
        wireless.wpublicg.network='public'
        wireless.wpublicg.country='DE'
        wireless.wpublicg.ssid='spm-test Free'
        wireless.wpublicg.encryption='none'
        wireless.wpublicg.disabled='0'
    "#;

    #[test]
    fn uci_show_enabled_parse() {
        assert!(is_pub_wifi_enabled_impl(UCI_SHOW_ENABLED));
    }

    const UCI_SHOW_DISABLED: &str = r#"
        wireless.wpublicg=wifi-iface
        wireless.wpublicg.device='radio1'
        wireless.wpublicg.ifname='w-public-g'
        wireless.wpublicg.mode='ap'
        wireless.wpublicg.network='public'
        wireless.wpublicg.country='DE'
        wireless.wpublicg.ssid='spm-test Free'
        wireless.wpublicg.encryption='none'
        wireless.wpublicg.disabled='1'
    "#;

    #[test]
    fn uci_show_disabled_parse() {
        assert!(!is_pub_wifi_enabled_impl(UCI_SHOW_DISABLED));
    }
}
