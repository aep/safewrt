#[macro_use] extern crate bart_derive;
extern crate serde;
extern crate toml;
#[macro_use] extern crate serde_derive;

fn maketrue() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
pub struct ConfigWifiRadio {
    #[serde(default = "maketrue")]
    on: bool,
    channel: Option<u32>
}

impl Default for ConfigWifiRadio {
    fn default() -> Self {
        Self {
            on: true,
            channel: None,
        }
    }
}

#[derive(Serialize, Default, Deserialize)]
pub struct ConfigWifiRadios {
    #[serde(default)]
    a: ConfigWifiRadio,
    #[serde(default)]
    g: ConfigWifiRadio,
}

#[derive(Serialize, Default, Deserialize)]
pub struct ConfigWifiAuth{
    encryption: String,
    key: String,
}

#[derive(Serialize, Default, Deserialize)]
pub struct ConfigWifiAp {
    ssid: String,
    #[serde(default)]
    auth: Option<ConfigWifiAuth>,
}

#[derive(Serialize, Default, Deserialize)]
pub struct ConfigWifiAps {
    #[serde(default)]
    public: ConfigWifiAp,
}

#[derive(Serialize, Deserialize)]
pub struct ConfigWifi {
    #[serde(default)]
    radio: ConfigWifiRadios,
    #[serde(default)]
    ap: ConfigWifiAps,
}


#[derive(Serialize, Deserialize)]
pub struct Config {
    wifi: ConfigWifi,
}

#[derive(BartDisplay)]
#[template = "templates/wireless.mustache"]
pub struct TplConfigWireless<'a> {
    c: &'a Config,
}

#[derive(BartDisplay)]
#[template = "templates/network.mustache"]
pub struct TplConfigNetwork<'a> {
    c: &'a Config,
}

#[derive(BartDisplay)]
#[template = "templates/firewall.mustache"]
pub struct TplConfigFirewall<'a> {
    c: &'a Config,
}

#[derive(BartDisplay)]
#[template = "templates/dhcp.mustache"]
pub struct TplConfigDhcp<'a> {
    c: &'a Config,
}

#[derive(BartDisplay)]
#[template = "templates/system.mustache"]
pub struct TplConfigSystem<'a> {
    c: &'a Config,
}

use std::fs::File;
use std::io::Write;
use std::io::Read;
use std::env::args;

fn main() {

    let mut cf = File::open(args().nth(1).unwrap()).unwrap();
    let mut buf = Vec::new();
    cf.read_to_end(&mut buf).unwrap();

    let config: Config = toml::from_slice(&buf).unwrap();

    {
        let mut f = File::create("/etc/config/wireless").unwrap();
        let s = format!("{}", TplConfigWireless{c: &config});
        f.write_all(&s.as_bytes()).unwrap();
    }

    {
        let mut f = File::create("/etc/config/network").unwrap();
        let s = format!("{}", TplConfigNetwork{c: &config});
        f.write_all(&s.as_bytes()).unwrap();
    }

    {
        let mut f = File::create("/etc/config/firewall").unwrap();
        let s = format!("{}", TplConfigFirewall{c: &config});
        f.write_all(&s.as_bytes()).unwrap();
    }

    {
        let mut f = File::create("/etc/config/dhcp").unwrap();
        let s = format!("{}", TplConfigDhcp{c: &config});
        f.write_all(&s.as_bytes()).unwrap();
    }

    {
        let mut f = File::create("/etc/config/system").unwrap();
        let s = format!("{}", TplConfigSystem{c: &config});
        f.write_all(&s.as_bytes()).unwrap();
    }
}
