#!/usr/bin/env bash

./build_docker.sh

docker run --cap-add=NET_ADMIN --cap-add=NET_RAW --rm -v $(pwd):/repo -w /code sentry /bin/sh -c "cp -R /repo/* . && cargo build && ./test.sh"
