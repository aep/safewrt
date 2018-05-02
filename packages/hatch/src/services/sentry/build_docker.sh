#!/usr/bin/env bash

cat Dockerfile | docker build -t sentry -
