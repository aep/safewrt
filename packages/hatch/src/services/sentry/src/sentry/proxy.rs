use hyper::{self, header, Client, Headers, Method, Uri};
use hyper::server::Response;
use hyper::client;

use futures::{self, Future};

use tokio_core::reactor::Handle;

use bytes::Bytes;

pub type Result = Box<Future<Item = hyper::server::Response, Error = hyper::Error>>;

fn serve_offline_page() -> Response {
    Response::new()
        .with_status(hyper::StatusCode::GatewayTimeout)
        .with_header(header::Connection::close())
        .with_body(Bytes::from_static(include_bytes!("../../res/offline.html")))
}

fn serve_client_response(resp: client::Response) -> Response {
    let mut headers = resp.headers().clone();
    headers.set(header::Connection::close());

    Response::new()
        .with_status(resp.status())
        .with_headers(headers)
        .with_body(resp.body())
}

pub fn request(
    handle: &Handle,
    inc_uri: &Uri,
    inc_method: &Method,
    headers: &Headers,
    ignore_headers: &[&str],
) -> Result {
    let mut out_req = client::Request::new(inc_method.to_owned(), inc_uri.to_owned());

    for header in headers.iter() {
        if !ignore_headers.contains(&header.name()) {
            out_req
                .headers_mut()
                .append_raw(header.name().to_owned(), header.raw().to_owned());
        }
    }

    Box::new(Client::new(handle).request(out_req).then(|ret| {
        futures::future::ok(if let Ok(resp) = ret {
            serve_client_response(resp)
        } else {
            serve_offline_page()
        })
    }))
}
