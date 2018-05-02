[![CircleCI](https://circleci.com/gh/superscale/sentry-rs.svg?style=svg&circle-token=e5230043fe2c70dcafbeb2dcd2c5f485ed5ad357)](https://circleci.com/gh/superscale/sentry-rs)

# Sentry
A captive portal service for wifi routers.


Sentry intercepts port 80 requests and redirects everything to a login page.
Once a user is logged in (a user is a mac address), the interception is stopped for that mac.

# Usage

```
echo "device.host.name"    > targetsystem/etc/superscale/name
echo "http://yoursite.com" > targetsystem/etc/sentry.url

```

each initial unauthorized user will be redirecteds to yoursite.com
with an http header "X-SC-Sentry-Secret" set to a generated random value.
Once the user is authorized, the site must redirect the user to a page that contains the secret in the url parameters.

for example this is what the site will receive:

```
GET / HTTP/1.1
host: mysite.com
X-SC-Sentry-Secret: abc123
Origin: the-actual-site-the-user-wanted.com

```

any number of redirects can ocure and any resource can be loaded from that site,
as long as the origin header contains 'yoursite.com'.
this is very different from most captive portal software, which requires all resources to be on the same host.

eventually when the user is authorized and granted internet access, the last page load must contain the secret like:

```
/authorized.html?bla=abc123
```

Sentry will detect the secret as "known to the user" and add an exception for that mac address to iptables.

# Services

The project provides two further services: `access_control` and `time_control`.

When Sentry authorizes a user, it creates an iptable rule. This iptable rule contains a timestamp, this is the 
timestamp when the user was authorized. The `access_control` service can read these timestamps and remove them if they are expired.
The valid duration of an access is configurable.

The `time_control` service can be used to configure up and down times of the public wifi.
