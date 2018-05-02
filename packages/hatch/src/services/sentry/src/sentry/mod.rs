mod sentry;
mod ip;
mod portal;
mod proxy;
mod service;
mod ubus;

use errors::*;
use sentry::sentry::Sentry;
use sentry::service::Service;

use std::fs::File;
use std::io::Read;
use std::str::FromStr;

use tokio_core::reactor::Core;
use tokio_core::net::TcpListener;

use hyper;
use hyper::server::Http;

use futures::Stream;

use rand::{self, Rng};

const DEFAULT_PATH_TO_REDIRECT_URL: &'static str = "/etc/sentry.url";
const DEFAULT_REDIRECT_URL: &'static str = "http://portal.captif.io/?origin=";
const DEFAULT_LISTEN_PORT: u16 = 8444;
const SECRET_LENGTH: usize = 16;

fn get_redirect_url(path_opt: Option<&str>) -> String {
    let path = path_opt.unwrap_or_else(|| DEFAULT_PATH_TO_REDIRECT_URL);

    if let Ok(mut file) = File::open(path) {
        let mut url = String::new();

        if file.read_to_string(&mut url).is_ok() && hyper::Uri::from_str(&url).is_ok() {
            return url;
        }
    }

    DEFAULT_REDIRECT_URL.to_owned()
}

fn get_redirect_host(redirect_url: &str) -> Result<String> {
    let uri = hyper::Uri::from_str(redirect_url)
        .chain_err(|| "unable to convert redirect url to an uri")?;

    uri.host()
        .map(|s| s.to_owned())
        .ok_or_else(|| "unable to extract the host from the redirect url".into())
}

fn create_secret() -> String {
    rand::thread_rng()
        .gen_ascii_chars()
        .take(SECRET_LENGTH)
        .collect::<String>()
}

pub fn sentry_main(
    pylon_name: String,
    path_to_redirect_url: Option<&str>,
    listen_port: Option<u16>,
) -> Result<()> {


    let redirect_url = get_redirect_url(path_to_redirect_url);
    let redirect_host =
        get_redirect_host(&redirect_url).chain_err(|| "Error extracting redirect host!")?;
    let secret = create_secret();

    let listen_address_string = format!(
        "0.0.0.0:{}",
        listen_port.unwrap_or_else(|| DEFAULT_LISTEN_PORT)
    );
    let listen_address = listen_address_string
        .parse()
        .chain_err(|| "Error parsing listen address!")?;

    let mut evt_loop = Core::new().chain_err(|| "Could not initialize event loop")?;
    let evt_loop_handle = evt_loop.handle();

    let listener =
        TcpListener::bind(&listen_address, &evt_loop_handle).chain_err(|| "unable to listen")?;
    let mut http = Http::new();

    let sentry = Sentry::new(secret.clone(), pylon_name.clone(), evt_loop_handle.clone());

    // listen for all incoming requests
    let server = listener.incoming().for_each(move |(socket, addr)| {
        let sentry_service =
            Service::new(redirect_url.clone(), redirect_host.clone(), sentry.clone());
        http.keep_alive(false)
            .bind_connection(&evt_loop_handle, socket, addr, sentry_service);
        Ok(())
    });

    evt_loop
        .run(server)
        .chain_err(|| "error running the event loop")
}
