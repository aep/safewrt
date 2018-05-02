#[macro_use] extern crate bart_derive;
extern crate serde;
extern crate toml;
#[macro_use] extern crate serde_derive;
extern crate ed25519_dalek;
extern crate bs58;
extern crate mtdparts;
extern crate sha2;


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
pub struct ConfigCaptif {
    url: String,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    wifi:   ConfigWifi,
    captif: ConfigCaptif,
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
    hostname: String,
}

#[derive(BartDisplay)]
#[template = "templates/dropbear.mustache"]
pub struct TplConfigDropbear <'a> {
    c: &'a Config,
}

#[derive(BartDisplay)]
#[template = "templates/rc.local.mustache"]
pub struct TplRcLocal <'a> {
    c: &'a Config,
}

use std::fs::File;
use std::fs;
use std::io::Write;
use std::io::Read;
use std::env::args;
use std::os::unix::fs::OpenOptionsExt;

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
        let s = format!("{}", TplConfigSystem{
            c: &config,
            hostname: get_identity().unwrap_or(String::from("unidentified.gf")),
        });
        f.write_all(&s.as_bytes()).unwrap();
    }

    {
        let mut f = File::create("/etc/config/dropbear").unwrap();
        let s = format!("{}", TplConfigDropbear{
            c: &config,
        });
        f.write_all(&s.as_bytes()).unwrap();
    }

    {
        let mut f = File::create("/etc/sentry.url").unwrap();
        f.write_all(config.captif.url.as_bytes()).unwrap();
    }

    {
        let mut f = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .mode(0o770)
            .open("/etc/rc.local")
            .unwrap();
        let s = format!("{}", TplRcLocal {
            c: &config,
        });
        f.write_all(&s.as_bytes()).unwrap();
    }

    {
        let mut f = File::create("/etc/dropbear/authorized_keys").unwrap();
        f.write_all(b"ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQC55PtSpE2uexmJ05bxtdcI13skU4rYwQWpJIIpZ6++TivIPKR0V++VbDdwQnN6bZeUfBUa3ggMzk16+GHPB3ILT0bGn5hiki763/aouffjtnNrl3WIziS8bUaFnNdMTb+8akRiVmnfdfRJ+vWE32KRR88QWAWVkfI+sFD4OewvYNGsIQk0bPhQNSUz7yj3uR8ht7pzVGXwrbuMPV3DaH2IKnfaeGYx9QS/q1PheXulPgdeTcB79eEGUte9P2EWnS4BFMMywJog8MDDRC4VpXZIK6nVaXblJoVprAI+oEoIGyOy8Qg3VApIdrkMQljKyl/ofXkkFy7AGxaRdd4CBFoh aep@nightbringer").unwrap();
        f.write_all(b"ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIL414g0DkIykmS8s6wC9bJ3jvmFpA3jVOPp6bZIxTUQX aep@u4").unwrap();
    }
}


fn get_identity() -> Option<String> {
    use ed25519_dalek::{SecretKey, PublicKey};
    let f = match File::open("/proc/mtd") {
        Ok(f) => f,
        Err(e) => {println!("cannot read /proc/mtd: {}", e); return None;},
    };
    let parts = match mtdparts::parse_mtd(&f) {
        Ok(v) => v,
        Err(e) => {println!("cannot parse /proc/mtd: {}", e); return None;},
    };
    let i = match parts.get("identity") {
        Some(i) => i,
        None  => {println!("missing mtd partition 'identity'"); return None;},
    };
    let mut f = match File::open(format!("/dev/mtdblock{}", i)) {
        Ok(f) => f,
        Err(e) => {println!("cannot open /dev/mtdblock{}: {}", i, e); return None;},
    };

    let mut buf = [0;4096];
    if let Err(e) = f.read_exact(&mut buf) {
        println!("cannot read /dev/mtdblock{}: {}", i, e);
        return None;
    }

    let sc :SecretKey = match SecretKey::from_bytes(&buf[..32]) {
        Ok(v) => v,
        Err(e) => {println!("cannot load secret data: {}", e); return None;},
    };

    let pk: PublicKey = PublicKey::from_secret::<sha2::Sha512>(&sc);


    Some(bs58::encode(pk.as_bytes())
        .with_alphabet(bs58::alphabet::BITCOIN)
        .into_string())
}


