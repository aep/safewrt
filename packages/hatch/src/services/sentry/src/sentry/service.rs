use sentry::Sentry;
use sentry::proxy;

use std::net::SocketAddr;
use std::str::FromStr;

use hyper;
use hyper::server::{self, Request, Response};
use hyper::header::{Connection, Host, Location, Referer};

use futures::future::{Either, Future};
use futures;

#[derive(Clone, new, Debug)]
pub struct Service {
    redirect_url: String,
    redirect_host: String,
    sentry: Sentry,
}

/// The service that handles the http requests.
///
/// When a client connects to the router, its http traffic will be redirected to this service.
/// The flow through the service is as follows:
///
/// 1. The initial http connection will be redirected to our portal by returning
///    http status code 302.
/// 2. The client will request the portal and will get the portal served.
/// 3. The client presses the accept button in the portal. The button redirects to a
///    new page in the portal. This redirect contains our secret, so the service will
///    authorize the client.
/// 4. After authorization, the service should not see any new requests from the client.
impl Service {
    fn remote_addr_to_ip(&self, remote_addr: &SocketAddr) -> String {
        format!("{}", remote_addr.ip())
    }

    /// Checks the request for the service secret. If the secret is present,
    /// the client is authorized.
    fn handle_authorized(&self, req: &Request) {
        if let Some(query) = req.uri().query() {
            if self.sentry.contains_secret(query) {
                if let Some(address) = req.remote_addr() {
                    self.sentry
                        .authorize_client(&self.remote_addr_to_ip(&address));
                }
            }
        }
    }

    /// Fetches the portal if the host header is equal to the `redirect_host`
    fn handle_portal(&self, req: &Request) -> Option<proxy::Result> {
        if let Some(host) = req.headers().get::<Host>() {
            if host.hostname() == self.redirect_host {
                let address = req.remote_addr()
                    .expect("Could not extract the remote address");
                let uri = hyper::Uri::from_str(&format!("http://{}{}", host, req.uri().as_ref()))
                    .expect("Error at building the portal url!");

                return Some(self.sentry.fetch_portal(
                    &self.remote_addr_to_ip(&address),
                    &uri,
                    req.method(),
                    req.headers(),
                ));
            }
        }

        None
    }

    /// Proxies requests, if the referer header is equal to the `redirect_host`
    fn handle_referer(&self, req: &Request) -> Option<proxy::Result> {
        let host = if let Some(host) = req.headers().get::<Host>() {
            host
        } else {
            return None;
        };

        let referer = if let Some(referer) = req.headers().get::<Referer>() {
            referer
        } else {
            return None;
        };

        if let Ok(ref_uri) = hyper::Uri::from_str(referer.chars().as_str()) {
            if ref_uri.host() == Some(self.redirect_host.as_str()) {
                let uri = hyper::Uri::from_str(&format!("http://{}{}", host, req.uri().as_ref()))
                    .expect("Error at building the referer url!");

                return Some(self.sentry.proxy_request(&uri, req.method(), req.headers()));
            }
        }

        None
    }

    /// Redirects each request to the portal
    fn handle_redirect(&self, req: &Request) -> Response {
        let host = if let Some(host) = req.headers().get::<Host>() {
            host.hostname()
        } else {
            ""
        };

        let mut resp = Response::new();
        resp.set_status(hyper::StatusCode::Found);

        let location = format!(
            "{}http://{}{}",
            self.redirect_url.to_owned(),
            host,
            req.uri().as_ref()
        );

        resp.headers_mut().set(Location::new(location));
        resp.headers_mut().set(Connection::close());

        resp
    }
}

impl server::Service for Service {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = Either<
        futures::future::Ok<Response, hyper::Error>,
        Box<Future<Item = Response, Error = hyper::Error>>,
    >;

    fn call(&self, req: Request) -> Self::Future {
        self.handle_authorized(&req);

        if let Some(resp) = self.handle_portal(&req) {
            Either::B(resp)
        } else if let Some(resp) = self.handle_referer(&req) {
            Either::B(resp)
        } else {
            Either::A(futures::future::ok(self.handle_redirect(&req)))
        }
    }
}
