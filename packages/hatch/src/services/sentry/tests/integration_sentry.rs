extern crate bytes;
#[macro_use]
extern crate derive_new;
extern crate futures;
extern crate hyper;
extern crate iptables;
#[macro_use]
extern crate lazy_static;
extern crate regex;
extern crate sentry;
#[cfg(test)]
extern crate tempdir;
extern crate tokio_core;

use std::path::{Path, PathBuf};
use std::fs::{remove_file, File, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::env;
use std::process::Command;
use std::io::{Read, Write};
use std::thread;
use std::net::{SocketAddr, TcpStream};
use std::str::{self, FromStr};
use std::sync::mpsc;
use std::time;

use tokio_core::reactor::Core;
use tokio_core::net::TcpListener;

use hyper::server;
use hyper::Headers;
use hyper::header;

use futures::future::Future;
use futures::Stream;

use regex::Regex;

const TEST_DEVICE_IP: &'static str = "127.0.0.1";
const TEST_DEVICE_MAC: &'static str = "DE:AD:BE:EF:00:11";
const TEST_DEVICE_HOSTNAME: &'static str = "testmachine";
const TEST_PYLON_NAME: &'static str = "pylonpylon";
const TEST_PORTAL_UPPER_BODY: &'static str = "portal-content-yeah-cake!";
const IP_COMMAND_OUTPUT: &'static str = "127.0.0.1 dev enp0s20u1 lladdr DE:AD:BE:EF:00:11 \
                                         REACHABLE";
const UBUS_COMMAND_OUTPUT: &'static str = r#"
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
                                        "hostname": "testmachine",
                                        "ip": "127.0.0.1",
                                        "valid": -42406
                                }
                        ]
                }
        }
   }"#;
const PORTAL_HEADER_CONNECTED_IP: &'static str = "X-SC-Sentry-Connected-Ip";
const PORTAL_HEADER_CONNECTED_MAC: &'static str = "X-SC-Sentry-Connected-Mac";
const PORTAL_HEADER_CONNECTED_HOSTNAME: &'static str = "X-SC-Sentry-Connected-Hostname";
const PORTAL_HEADER_SECRET: &'static str = "X-SC-Sentry-Secret";
const PORTAL_HEADER_PYLON: &'static str = "X-SC-Sentry-Pylon";
const REFERER_SERVICE_BODY: &'static str = "RefererBody";

lazy_static! {
    static ref PATH_VAR_ORIGINAL_VALUE: String = env::var("PATH").unwrap();
}

fn create_command(path: &Path, name: &str, output: &str, remove_new_lines: bool) {
    let file_name = path.join(name);
    let mut file = File::create(&file_name).unwrap();

    let new_output = if remove_new_lines {
        output.replace("\\n", "")
    } else {
        output.to_owned()
    }.replace("\"", "\\\"");

    let sh_cmd =
        String::from_utf8(Command::new("which").arg("sh").output().unwrap().stdout).unwrap();

    let src = format!(
        "#!{} \necho -n $@ >> {}_input\necho \"{}\"\nexit 0\n",
        sh_cmd,
        file_name.to_str().unwrap(),
        new_output
    );

    file.write_all(src.as_bytes()).unwrap();
    file.set_permissions(Permissions::from_mode(0o755)).unwrap();
}

fn check_command_input(path: &Path, expected_input: &str, name: &str) {
    let file_name = path.join(format!("{}_input", name));
    let mut file = File::open(&file_name).unwrap();
    let mut file_content = String::new();

    file.read_to_string(&mut file_content).unwrap();
    assert_eq!(expected_input, file_content);

    remove_file(file_name).unwrap();
}

fn check_command_input_contains(path: &Path, expected_inputs: &[&str], name: &str) {
    let file_name = path.join(format!("{}_input", name));
    let mut file = File::open(&file_name).unwrap();
    let mut file_content = String::new();

    file.read_to_string(&mut file_content).unwrap();

    for expected in expected_inputs {
        assert!(file_content.contains(expected));
    }

    remove_file(file_name).unwrap();
}

fn create_ip_command(path: &Path) {
    create_command(path, "ip", IP_COMMAND_OUTPUT, false)
}

fn check_ip_command_input(path: &Path, expected_input: &str) {
    check_command_input(path, expected_input, "ip")
}

fn create_ubus_command(path: &Path) {
    create_command(path, "ubus", UBUS_COMMAND_OUTPUT, true)
}

fn check_ubus_command_input(path: &Path, expected_input: &str) {
    check_command_input(path, expected_input, "ubus")
}

fn check_ubus_command_input_contains(path: &Path, expected_inputs: &[&str]) {
    check_command_input_contains(path, expected_inputs, "ubus")
}

fn check_iptables_rule() {
    let ipt = iptables::new(false).unwrap();
    let rules = ipt.list("nat", "prerouting_public_rule")
        .expect("Error getting iptables rules");
    let mut found = false;

    let expected_rule_regex = Regex::new(&format!(
        "-A prerouting_public_rule -m mac \
         --mac-source {} -m \
         comment --comment \"timestamp=\\d+\" -j ACCEPT",
        TEST_DEVICE_MAC
    )).unwrap();

    for rule in rules {
        found |= expected_rule_regex.is_match(&rule);
    }

    assert!(found);
}

fn create_iptables_command(path: &Path) {
    let iptables_cmd = String::from_utf8(
        Command::new("which")
            .arg("iptables")
            .output()
            .unwrap()
            .stdout,
    ).unwrap()
        .replace("\n", "");

    Command::new("ln")
        .args(&[
            "-s",
            &iptables_cmd,
            path.join("iptables").to_str().unwrap(),
        ])
        .output()
        .expect("Error creating iptables command!");

    let ipt = iptables::new(false).unwrap();
    ipt.flush_table("nat").expect("Could not flush the table");
    ipt.new_chain("nat", "prerouting_public_rule")
        .expect("Could not create new chain");
}

fn create_all_commands(path: &Path) {
    create_ip_command(path);
    create_ubus_command(path);
    create_iptables_command(path);
}

fn check_header_value(headers: &Headers, name: &str, expect_val: &str) {
    assert_eq!(
        str::from_utf8(headers.get_raw(name).unwrap().one().unwrap()).unwrap(),
        expect_val
    );
}

fn check_header_exist(headers: &Headers, name: &str, should_exist: bool) {
    assert_eq!(headers.get_raw(name).is_some(), should_exist)
}

#[derive(Clone, Copy, new)]
struct PortalService {
    hostname_should_exist: bool,
}

impl server::Service for PortalService {
    type Request = server::Request;
    type Response = server::Response;
    type Error = hyper::Error;
    type Future = futures::Finished<Self::Response, hyper::Error>;
    fn call(&self, req: Self::Request) -> Self::Future {
        let headers = req.headers();
        check_header_value(headers, PORTAL_HEADER_CONNECTED_IP, TEST_DEVICE_IP);
        check_header_value(headers, PORTAL_HEADER_CONNECTED_MAC, TEST_DEVICE_MAC);

        if self.hostname_should_exist {
            check_header_value(
                headers,
                PORTAL_HEADER_CONNECTED_HOSTNAME,
                TEST_DEVICE_HOSTNAME,
            );
        } else {
            check_header_exist(headers, PORTAL_HEADER_CONNECTED_HOSTNAME, false);
        }

        check_header_value(headers, PORTAL_HEADER_PYLON, TEST_PYLON_NAME);

        let secret = str::from_utf8(
            headers
                .get_raw(PORTAL_HEADER_SECRET)
                .unwrap()
                .one()
                .unwrap(),
        ).unwrap();
        let body = format!("{}\n{}", TEST_PORTAL_UPPER_BODY, secret);

        futures::finished(
            server::Response::new()
                .with_header(header::ContentLength(body.len() as u64))
                .with_header(header::ContentType::plaintext())
                .with_body(body),
        )
    }
}

#[derive(Clone, Copy, new)]
struct RefererService {}

impl server::Service for RefererService {
    type Request = server::Request;
    type Response = server::Response;
    type Error = hyper::Error;
    type Future = futures::Finished<Self::Response, hyper::Error>;
    fn call(&self, req: Self::Request) -> Self::Future {
        let headers = req.headers();
        check_header_exist(headers, PORTAL_HEADER_CONNECTED_IP, false);
        check_header_exist(headers, PORTAL_HEADER_CONNECTED_MAC, false);
        check_header_exist(headers, PORTAL_HEADER_CONNECTED_HOSTNAME, false);
        check_header_exist(headers, PORTAL_HEADER_PYLON, false);
        check_header_exist(headers, PORTAL_HEADER_SECRET, false);

        futures::finished(
            server::Response::new()
                .with_header(header::ContentLength(REFERER_SERVICE_BODY.len() as u64))
                .with_header(header::ContentType::plaintext())
                .with_body(REFERER_SERVICE_BODY),
        )
    }
}

fn spawn_portal(hostname_should_exist: bool) -> SocketAddr {
    let (tx, rx): (mpsc::Sender<SocketAddr>, mpsc::Receiver<SocketAddr>) = mpsc::channel();

    thread::spawn(move || {
        let addr = "127.0.0.1:0".parse().unwrap();

        let mut event_loop = Core::new().unwrap();
        let handle = event_loop.handle();
        let listener = TcpListener::bind(&addr, &handle).unwrap();

        tx.send(listener.local_addr().unwrap()).unwrap();

        let http = hyper::server::Http::new();

        // run until the end of the universe!
        event_loop
            .run(listener.incoming().for_each(move |(socket, addr)| {
                http.bind_connection(
                    &handle,
                    socket,
                    addr,
                    PortalService::new(hostname_should_exist),
                );
                Ok(())
            }))
            .unwrap()
    });

    rx.recv().unwrap()
}

fn spawn_referer() -> SocketAddr {
    let (tx, rx): (mpsc::Sender<SocketAddr>, mpsc::Receiver<SocketAddr>) = mpsc::channel();

    thread::spawn(move || {
        let addr = "127.0.0.1:0".parse().unwrap();

        let mut event_loop = Core::new().unwrap();
        let handle = event_loop.handle();
        let listener = TcpListener::bind(&addr, &handle).unwrap();

        tx.send(listener.local_addr().unwrap()).unwrap();

        let http = hyper::server::Http::new();

        // run until the end of the universe!
        event_loop
            .run(listener.incoming().for_each(move |(socket, addr)| {
                http.bind_connection(&handle, socket, addr, RefererService::new());
                Ok(())
            }))
            .unwrap()
    });

    rx.recv().unwrap()
}

fn portal_address_to_redirect_url(addr: &SocketAddr) -> String {
    format!("http://{}:{}/", addr.ip(), addr.port())
}

fn create_pylon_and_url_file(path: &Path, addr: &SocketAddr) -> (String, String) {
    let pylon_path = path.join("pylon");
    let mut pylon_file = File::create(&pylon_path).unwrap();

    pylon_file.write_all(TEST_PYLON_NAME.as_bytes()).unwrap();

    let url_file_path = path.join("url");
    let mut url_file = File::create(&url_file_path).unwrap();

    url_file
        .write_all(portal_address_to_redirect_url(addr).as_bytes())
        .unwrap();

    (
        pylon_path.to_str().unwrap().to_owned(),
        url_file_path.to_str().unwrap().to_owned(),
    )
}

fn spawn_sentry(pylon_file: String, redirect_url_file: String, port: u16) {
    thread::spawn(move || {
        sentry::sentry_main(
            Some(pylon_file.as_str()),
            Some(redirect_url_file.as_str()),
            Some(port),
        ).unwrap()
    });
}

fn wait_for_sentry(
    client: &mut hyper::Client<hyper::client::HttpConnector>,
    evt_loop: &mut Core,
    port: u16,
) {
    for _ in 0..10 {
        let uri = hyper::Uri::from_str(&format!("http://127.0.0.1:{}/test?", port)).unwrap();
        let mut req = hyper::client::Request::new(hyper::Method::Get, uri);

        let test_host = "test.test";
        req.headers_mut().set(header::Host::new(test_host, None));

        if evt_loop.run(client.request(req)).is_ok() {
            return;
        }

        thread::sleep(time::Duration::from_millis(10));
    }

    assert!(false);
}

fn test_sentry_phase_one(
    client: &mut hyper::Client<hyper::client::HttpConnector>,
    evt_loop: &mut Core,
    portal_address: &SocketAddr,
    port: u16,
) {
    let redirect_url = portal_address_to_redirect_url(portal_address);
    let uri = hyper::Uri::from_str(&format!("http://127.0.0.1:{}/test?", port)).unwrap();
    let mut req = hyper::client::Request::new(hyper::Method::Get, uri);

    let test_host = "test.test";
    req.headers_mut().set(header::Host::new(test_host, None));

    let resp = evt_loop.run(client.request(req)).unwrap();

    assert_eq!(resp.status(), hyper::StatusCode::Found);
    assert_eq!(
        resp.headers().get::<header::Location>().unwrap(),
        &header::Location::new(format!("{}http://{}/test?", redirect_url, test_host))
    );
    assert_eq!(
        resp.headers().get::<header::Connection>(),
        Some(&header::Connection::close())
    );
}

fn resolve_body(resp: hyper::client::Response, evt_loop: &mut Core) -> String {
    let work = resp.body()
        .map_err(|_| ())
        .fold(vec![], |mut acc, chunk| {
            acc.extend_from_slice(&chunk);
            Ok(acc)
        })
        .and_then(|v| String::from_utf8(v).map_err(|_| ()));

    evt_loop.run(work).unwrap()
}

fn test_sentry_phase_two(
    client: &mut hyper::Client<hyper::client::HttpConnector>,
    evt_loop: &mut Core,
    portal_address: &SocketAddr,
    path: &Path,
    port: u16,
) -> String {
    let uri = hyper::Uri::from_str(&format!("http://127.0.0.1:{}/", port)).unwrap();
    let mut req = hyper::client::Request::new(hyper::Method::Get, uri);

    req.headers_mut().set(header::Host::new(
        format!("{}", portal_address.ip()),
        portal_address.port(),
    ));

    let resp = evt_loop.run(client.request(req)).unwrap();

    check_ip_command_input(path, "n");
    check_ubus_command_input(path, "call dhcp ipv4leases");

    assert_eq!(
        resp.headers().get::<header::Connection>(),
        Some(&header::Connection::close())
    );

    let body = resolve_body(resp, evt_loop);

    assert_eq!(body.lines().count(), 2);
    assert!(body.contains(TEST_PORTAL_UPPER_BODY));

    // get the secret
    body.lines().nth(1).unwrap().to_owned()
}

fn test_sentry_phase_three(
    client: &mut hyper::Client<hyper::client::HttpConnector>,
    evt_loop: &mut Core,
    portal_address: &SocketAddr,
    path: &Path,
    secret: &str,
    port: u16,
) {
    let uri =
        hyper::Uri::from_str(&format!("http://127.0.0.1:{}/?secret={}", port, secret)).unwrap();
    let mut req = hyper::client::Request::new(hyper::Method::Get, uri);

    req.headers_mut().set(header::Host::new(
        format!("{}", portal_address.ip()),
        portal_address.port(),
    ));

    let resp = evt_loop.run(client.request(req)).unwrap();

    check_ip_command_input(path, "nn");
    check_ubus_command_input_contains(
        path,
        &[
            "call dhcp ipv4leases",
            "timestamp",
            TEST_DEVICE_IP,
            TEST_DEVICE_MAC,
            "/sentry/accept",
        ],
    );
    check_iptables_rule();

    assert_eq!(
        resp.headers().get::<header::Connection>(),
        Some(&header::Connection::close())
    );

    let body = resolve_body(resp, evt_loop);

    assert_eq!(body.lines().count(), 2);
    assert!(body.contains(TEST_PORTAL_UPPER_BODY));
}

fn spawn_client(portal_address: SocketAddr, path: PathBuf, port: u16) {
    thread::spawn(move || {
        let mut evt_loop = Core::new().unwrap();
        let handle = evt_loop.handle();

        let mut client = hyper::Client::new(&handle);

        wait_for_sentry(&mut client, &mut evt_loop, port);
        test_sentry_phase_two(
            &mut client,
            &mut evt_loop,
            &portal_address,
            path.as_path(),
            port,
        );
    });
}

#[test]
fn test_sentry_main() {
    env::set_var("PATH", PATH_VAR_ORIGINAL_VALUE.clone());
    let port = 8444;

    let fake_path = tempdir::TempDir::new("fake_path").unwrap();
    create_all_commands(fake_path.path());

    // we want that our fake programs are called by sentry
    env::set_var("PATH", fake_path.path());

    let portal_address = spawn_portal(true);
    let (pylon_file, redirect_url_file) =
        create_pylon_and_url_file(fake_path.path(), &portal_address);

    spawn_sentry(pylon_file.clone(), redirect_url_file.clone(), port);

    let mut evt_loop = Core::new().unwrap();
    let handle = evt_loop.handle();

    let mut client = hyper::Client::new(&handle);

    wait_for_sentry(&mut client, &mut evt_loop, port);
    test_sentry_phase_one(&mut client, &mut evt_loop, &portal_address, port);
    let secret = test_sentry_phase_two(
        &mut client,
        &mut evt_loop,
        &portal_address,
        fake_path.path(),
        port,
    );
    test_sentry_phase_three(
        &mut client,
        &mut evt_loop,
        &portal_address,
        fake_path.path(),
        &secret,
        port,
    );
}

#[test]
#[should_panic(expected = "Could not get mac address for the following ip address: 127.0.0.1")]
fn test_sentry_does_not_find_mac_address() {
    env::set_var("PATH", PATH_VAR_ORIGINAL_VALUE.clone());
    let port = 8445;

    let fake_path = tempdir::TempDir::new("fake_path").unwrap();
    create_command(fake_path.path(), "ip", "", false);

    // we want that our fake programs are called by sentry
    env::set_var("PATH", fake_path.path());

    let portal_address = spawn_portal(true);
    let (pylon_file, redirect_url_file) =
        create_pylon_and_url_file(fake_path.path(), &portal_address);
    spawn_client(portal_address, fake_path.path().to_path_buf(), port);

    sentry::sentry_main(
        Some(pylon_file.as_str()),
        Some(redirect_url_file.as_str()),
        Some(port),
    ).unwrap()
}

fn test_sentry_fetch_offline_page(
    client: &mut hyper::Client<hyper::client::HttpConnector>,
    evt_loop: &mut Core,
    portal_address: &SocketAddr,
    path: &Path,
    port: u16,
) {
    let uri = hyper::Uri::from_str(&format!("http://127.0.0.1:{}/", port)).unwrap();
    let mut req = hyper::client::Request::new(hyper::Method::Get, uri);

    req.headers_mut().set(header::Host::new(
        format!("{}", portal_address.ip()),
        portal_address.port(),
    ));

    let resp = evt_loop.run(client.request(req)).unwrap();

    check_ip_command_input(path, "n");
    check_ubus_command_input(path, "call dhcp ipv4leases");

    assert_eq!(
        resp.headers().get::<header::Connection>(),
        Some(&header::Connection::close())
    );

    let body = resolve_body(resp, evt_loop);

    let offline_page = bytes::Bytes::from_static(include_bytes!("../res/offline.html"));

    assert_eq!(body, offline_page);
}

#[test]
fn test_sentry_serve_offline_page() {
    env::set_var("PATH", PATH_VAR_ORIGINAL_VALUE.clone());
    let port = 8446;

    let fake_path = tempdir::TempDir::new("fake_path").unwrap();
    create_all_commands(fake_path.path());

    // we want that our fake programs are called by sentry
    env::set_var("PATH", fake_path.path());

    let portal_address: SocketAddr = "0.0.0.0:0".parse().unwrap();
    let (pylon_file, redirect_url_file) =
        create_pylon_and_url_file(fake_path.path(), &portal_address);
    spawn_sentry(pylon_file, redirect_url_file, port);

    let mut evt_loop = Core::new().unwrap();
    let handle = evt_loop.handle();

    let mut client = hyper::Client::new(&handle);

    wait_for_sentry(&mut client, &mut evt_loop, port);
    test_sentry_fetch_offline_page(
        &mut client,
        &mut evt_loop,
        &portal_address,
        fake_path.path(),
        port,
    );
}

#[test]
fn test_sentry_connection_closed() {
    env::set_var("PATH", PATH_VAR_ORIGINAL_VALUE.clone());
    let port = 8447;
    let fake_path = tempdir::TempDir::new("fake_path").unwrap();

    create_all_commands(fake_path.path());

    // we want that our fake programs are called by sentry
    env::set_var("PATH", fake_path.path());

    let portal_address: SocketAddr = "0.0.0.0:0".parse().unwrap();
    let (pylon_file, redirect_url_file) =
        create_pylon_and_url_file(fake_path.path(), &portal_address);
    spawn_sentry(pylon_file, redirect_url_file, port);

    let mut evt_loop = Core::new().unwrap();
    let handle = evt_loop.handle();
    let mut hyper_client = hyper::Client::new(&handle);

    wait_for_sentry(&mut hyper_client, &mut evt_loop, port);

    let sentry_address: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();

    let mut client = TcpStream::connect(sentry_address).unwrap();
    client
        .write_all(
            b"GET / HTTP/1.1\r\n\
                      Host: example.domain\r\n\
                      \r\n\
                      ",
        )
        .expect("request page");

    let mut buf = [0; 1024 * 8];
    let header_end = b"\r\n\r\n";
    loop {
        let n = client.read(&mut buf[..]).expect("receive redirect");
        if n < buf.len() && &buf[n - header_end.len()..n] == header_end {
            break;
        }
    }

    client
        .write_all(
            b"GET / HTTP/1.1\r\n\
                       Host: example.domain\r\n\
                       \r\n\
                       ",
        )
        .expect("request page second time");

    let mut buf = [0; 1024 * 8];
    match client.read(&mut buf[..]) {
        Ok(0) | Err(_) => {}
        Ok(n) => panic!("read {} bytes from a closed connection!", n),
    };
}

fn test_sentry_referer_proxy(
    client: &mut hyper::Client<hyper::client::HttpConnector>,
    evt_loop: &mut Core,
    portal_address: &SocketAddr,
    referer_address: &SocketAddr,
    port: u16,
) {
    let uri = hyper::Uri::from_str(&format!("http://localhost:{}/", port)).unwrap();
    let mut req = hyper::client::Request::new(hyper::Method::Get, uri);

    req.headers_mut()
        .set(header::Host::new("localhost", referer_address.port()));
    req.headers_mut()
        .set(header::Referer::new(format!("http://{}/", portal_address)));

    let resp = evt_loop.run(client.request(req)).unwrap();

    assert_eq!(
        resp.headers().get::<header::Connection>(),
        Some(&header::Connection::close())
    );

    let body = resolve_body(resp, evt_loop);

    assert_eq!(&body, REFERER_SERVICE_BODY);
}

#[test]
fn test_sentry_referer() {
    env::set_var("PATH", PATH_VAR_ORIGINAL_VALUE.clone());
    let port = 8448;
    let fake_path = tempdir::TempDir::new("fake_path").unwrap();

    create_all_commands(fake_path.path());

    // we want that our fake programs are called by sentry
    env::set_var("PATH", fake_path.path());


    let portal_address = spawn_portal(true);
    let (pylon_file, redirect_url_file) =
        create_pylon_and_url_file(fake_path.path(), &portal_address);

    spawn_sentry(pylon_file.clone(), redirect_url_file.clone(), port);

    let referer_address = spawn_referer();

    let mut evt_loop = Core::new().unwrap();
    let handle = evt_loop.handle();

    let mut client = hyper::Client::new(&handle);

    wait_for_sentry(&mut client, &mut evt_loop, port);
    test_sentry_referer_proxy(
        &mut client,
        &mut evt_loop,
        &portal_address,
        &referer_address,
        port,
    );
}
