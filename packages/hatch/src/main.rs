extern crate failure;
extern crate futures;
#[macro_use]  extern crate tokio_core;
extern crate tokio_io;
extern crate bytes;
extern crate trust_dns_resolver;
#[macro_use] extern crate log;
extern crate env_logger;
extern crate tokio_timer;
extern crate nix;
extern crate mtdparts;
extern crate ed25519_dalek;
extern crate bs58;
extern crate sha2;
extern crate libc;

extern crate sentry;

mod services;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use ed25519_dalek::{SecretKey, PublicKey};
use log::{Record, Metadata};
use std::env;
use std::ffi::CString;

fn main() {
    log::set_boxed_logger(Box::new(Logger::new())).unwrap();
    log::set_max_level(log::LevelFilter::Info);

    let identity = match getidentity() {
        Some(id) => id,
        None => {
            use nix::unistd;
            let mut buf = [0u8; 64];
            let hostname_cstr = unistd::gethostname(&mut buf).unwrap();
            hostname_cstr.to_str().unwrap().to_string()
        }
    };
    info!("using identity: {}", identity);


    let arg = env::args().nth(0).unwrap();
    let program_os = Path::new(&arg)
        .file_name()
        .expect("Could not get program name from argument 0!");
    let program = program_os.to_str().unwrap();

    match program {
        "lifeline" => services::lifeline1::main(identity),
        "sentry" => sentry::sentry_main(identity, None, None).unwrap(),
        _ => panic!("program \"${}\" not built in", program),
    }
}

fn getidentity() -> Option<String> {
    let f = match File::open("/proc/mtd") {
        Ok(f) => f,
        Err(e) => {warn!("cannot read /proc/mtd: {}", e); return None;},
    };
    let parts = match mtdparts::parse_mtd(&f) {
        Ok(v) => v,
        Err(e) => {warn!("cannot parse /proc/mtd: {}", e); return None;},
    };
    let i = match parts.get("identity") {
        Some(i) => i,
        None  => {warn!("missing mtd partition 'identity'"); return None;},
    };
    let mut f = match File::open(format!("/dev/mtdblock{}", i)) {
        Ok(f) => f,
        Err(e) => {warn!("cannot open /dev/mtdblock{}: {}", i, e); return None;},
    };

    let mut buf = [0;4096];
    if let Err(e) = f.read_exact(&mut buf) {
        warn!("cannot read /dev/mtdblock{}: {}", i, e);
        return None;
    }

    let sc :SecretKey = match SecretKey::from_bytes(&buf[..32]) {
        Ok(v) => v,
        Err(e) => {warn!("cannot load secret data: {}", e); return None;},
    };

    let pk: PublicKey = PublicKey::from_secret::<sha2::Sha512>(&sc);

    Some(bs58::encode(pk.as_bytes())
        .with_alphabet(bs58::alphabet::BITCOIN)
        .into_string())

}



struct Logger {}

impl Logger {
    fn new() -> Self {
        Self {
        }
    }
}

impl log::Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {

        let message = format!("{}", record.args());
        eprintln!("{} - {}", record.level(), message);

        let sysloglvl = match record.level() {
            log::Level::Error => Some(libc::LOG_ERR),
            log::Level::Warn  => Some(libc::LOG_WARNING),
            log::Level::Info  => Some(libc::LOG_INFO),
            log::Level::Debug => Some(libc::LOG_DEBUG),
            log::Level::Trace => None,
        };

        if let Some(level) = sysloglvl {
            let cmsg = CString::new(message).unwrap();
            unsafe{libc::syslog(level, cmsg.as_ptr());};
        }
    }

    fn flush(&self) {}
}
