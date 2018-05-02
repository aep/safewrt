FROM ubuntu
MAINTAINER Bastian Köcher <git@kchr.de>

ENV DEBIAN_FRONTEND noninteractive

ENV LANG C.UTF-8

RUN \
  apt-get update && \
  apt-get -y install \
          git \
          curl \
          gcc \
          libcurl4-openssl-dev \
          libelf-dev \
          libdw-dev \
          binutils-dev \
          cmake \
          libjson-c-dev \
          libjson-c2 \
          libssl-dev \
          openssl \
          pkg-config \
          wget \
          unzip \
          python \
          libiberty-dev \
          iptables

RUN curl https://sh.rustup.rs -sSf | \
    sh -s -- --default-toolchain stable -y

ENV PATH=/root/.cargo/bin:$PATH

RUN mkdir -p /code