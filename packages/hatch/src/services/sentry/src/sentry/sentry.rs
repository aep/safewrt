use errors::*;
use sentry::ubus;
use sentry::portal;
use sentry::ip;
use sentry::proxy;

use std::collections::HashMap;

use tokio_core::reactor::Handle;

use hyper;
use hyper::header::Header;

use chrono::Local;

use iptables;

#[derive(Clone, new, Debug)]
pub struct Sentry {
    secret: String,
    pylon_name: String,
    evt_loop_handle: Handle,
}

impl Sentry {
    fn authorize_client_in_iptables(&self, mac: &str) -> Result<()> {
        let ipt = iptables::new(false).unwrap();

        ipt.append(
            "nat",
            "prerouting_public_rule",
            &format!(
                "-jACCEPT -mmac --mac-source {} -mcomment --comment timestamp={}",
                mac,
                Local::now().timestamp()
            ),
        ).chain_err(|| "Error authorizing client with iptables")
            .map(|_| ())
    }

    pub fn authorize_client(&self, ip: &str) {
        let mac = if let Some(mac) = ip::ip_to_mac(ip) {
            mac
        } else {
            return;
        };

        if self.authorize_client_in_iptables(&mac).is_ok() {
            let time = format!("{}", Local::now().timestamp());
            let mut map: HashMap<&str, &str> = HashMap::new();
            map.insert("ip", ip);
            map.insert("mac", mac.as_str());
            map.insert("timestamp", time.as_str());
            ubus::send_message("/sentry/accept", &map);
        }
    }

    pub fn fetch_portal(
        &self,
        ip_address: &str,
        inc_uri: &hyper::Uri,
        inc_method: &hyper::Method,
        inc_headers: &hyper::Headers,
    ) -> proxy::Result {
        let mac = ip::ip_to_mac(ip_address).expect(&format!(
            "Could not get mac address for the following ip address: {}",
            ip_address
        ));

        let hostname = ubus::get_hostname_for_ip(ip_address);

        portal::fetch(
            &self.evt_loop_handle,
            inc_uri,
            inc_method,
            inc_headers.clone(),
            &self.secret,
            &self.pylon_name,
            ip_address,
            &mac,
            &hostname,
        )
    }

    pub fn proxy_request(
        &self,
        inc_uri: &hyper::Uri,
        inc_method: &hyper::Method,
        inc_headers: &hyper::Headers,
    ) -> proxy::Result {
        proxy::request(
            &self.evt_loop_handle,
            inc_uri,
            inc_method,
            inc_headers,
            &[hyper::header::Referer::header_name()],
        )
    }

    pub fn contains_secret(&self, query: &str) -> bool {
        query.contains("tos_accepted=true")
    }
}
