use sentry::proxy;

use hyper::{Headers, Method, Uri};

use tokio_core::reactor::Handle;

const HEADER_CONNECTED_IP: &'static str = "X-SC-Sentry-Connected-Ip";
const HEADER_CONNECTED_MAC: &'static str = "X-SC-Sentry-Connected-Mac";
const HEADER_CONNECTED_HOSTNAME: &'static str = "X-SC-Sentry-Connected-Hostname";
const HEADER_SECRET: &'static str = "X-SC-Sentry-Secret";
const HEADER_PYLON: &'static str = "X-SC-Sentry-Pylon";

fn add_extra_headers(
    headers: &mut Headers,
    secret: &str,
    pylon_name: &str,
    address: &str,
    mac_address: &str,
    hostname: &Option<String>,
) {
    headers.append_raw(HEADER_CONNECTED_IP, address);
    headers.append_raw(HEADER_CONNECTED_MAC, mac_address);

    if let Some(ref hostname) = *hostname {
        headers.append_raw(HEADER_CONNECTED_HOSTNAME, hostname.as_str());
    }

    headers.append_raw(HEADER_SECRET, secret);
    headers.append_raw(HEADER_PYLON, pylon_name);
}

pub fn fetch(
    handle: &Handle,
    inc_uri: &Uri,
    inc_method: &Method,
    mut inc_headers: Headers,
    secret: &str,
    pylon_name: &str,
    address: &str,
    mac_address: &str,
    hostname: &Option<String>,
) -> proxy::Result {
    add_extra_headers(
        &mut inc_headers,
        secret,
        pylon_name,
        address,
        mac_address,
        hostname,
    );

    proxy::request(handle, inc_uri, inc_method, &inc_headers, &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    use hyper::{self, header, server};

    use futures;
    use futures::Stream;
    use futures::Future;

    use std::net::SocketAddr;
    use std::str::{self, FromStr};

    use tokio_core::net::TcpListener;
    use tokio_proto::streaming::Message;
    use tokio_core::reactor::Core;

    const TEST_ADDRESS: &'static str = "127.0.0.1";
    const TEST_MAC_ADDRESS: &'static str = "DE:AD:BE:EF:DE:AD";
    const TEST_HOSTNAME: &'static str = "testmachine";
    const TEST_BODY: &'static str = "portaltest";
    const TEST_SECRET: &'static str = "secret";
    const TEST_PYLON_NAME: &'static str = "pylon!";

    fn check_header_value(headers: &Headers, name: &str, expect_val: &str) {
        assert_eq!(
            str::from_utf8(headers.get_raw(name).unwrap().one().unwrap()).unwrap(),
            expect_val
        );
    }

    #[derive(Clone, Copy)]
    struct PortalService;

    impl server::Service for PortalService {
        type Request = server::Request;
        type Response = server::Response;
        type Error = hyper::Error;
        type Future = futures::Finished<Self::Response, hyper::Error>;
        fn call(&self, req: Self::Request) -> Self::Future {
            let headers = req.headers();
            check_header_value(&headers, HEADER_CONNECTED_IP, TEST_ADDRESS);
            check_header_value(&headers, HEADER_CONNECTED_MAC, TEST_MAC_ADDRESS);
            check_header_value(&headers, HEADER_CONNECTED_HOSTNAME, TEST_HOSTNAME);
            check_header_value(&headers, HEADER_PYLON, TEST_PYLON_NAME);
            check_header_value(&headers, HEADER_SECRET, TEST_SECRET);

            futures::finished(
                server::Response::new()
                    .with_header(header::ContentLength(TEST_BODY.len() as u64))
                    .with_header(header::ContentType::plaintext())
                    .with_body(TEST_BODY),
            )
        }
    }

    fn spawn_portal(handle: &Handle) -> SocketAddr {
        let addr = "127.0.0.1:0".parse().unwrap();
        let listener = TcpListener::bind(&addr, handle).unwrap();
        let addr = listener.local_addr().unwrap();

        let handle2 = handle.clone();
        let http = hyper::server::Http::new();
        handle.spawn(
            listener
                .incoming()
                .for_each(move |(socket, addr)| {
                    http.bind_connection(&handle2, socket, addr, PortalService);
                    Ok(())
                })
                .then(|_| Ok(())),
        );

        return addr;
    }

    #[test]
    fn test_portal() {
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        let portal_addr = spawn_portal(&handle);
        let portal_uri = hyper::Uri::from_str(format!("http://{}/", portal_addr).as_str()).unwrap();

        let mut headers = header::Headers::default();
        headers.set(header::Host::new(
            format!("{}", portal_addr.ip()),
            portal_addr.port(),
        ));

        let resp = core.run(fetch(
            &handle,
            &portal_uri,
            &hyper::Method::Get,
            headers,
            &TEST_SECRET,
            &TEST_PYLON_NAME,
            TEST_ADDRESS,
            TEST_MAC_ADDRESS,
            &Some(TEST_HOSTNAME.to_owned()),
        )).unwrap();

        assert_eq!(
            resp.headers().get::<header::Connection>(),
            Some(&header::Connection::close())
        );

        let mut message: Message<server::__ProtoResponse, hyper::Body> = resp.into();

        // SRC: https://github.com/hyperium/hyper/issues/1098
        let work = message
            .take_body()
            .unwrap()
            .map_err(|_| ())
            .fold(vec![], |mut acc, chunk| {
                acc.extend_from_slice(&chunk);
                Ok(acc)
            })
            .and_then(|v| String::from_utf8(v).map_err(|_| ()));

        let body = core.run(work).unwrap();

        assert_eq!(body, TEST_BODY);
    }
}
