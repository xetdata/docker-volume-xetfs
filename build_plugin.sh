#!/bin/bash

set -x


if [ ! -f "git-xet" ] || [ ! -f "volume-xethub" ]; then
  cd builder
    docker-compose build rust_build
    docker-compose up rust_build
    cp rust_target/release/git-xet ..
    cp rust_target/release/volume-xethub ..
  cd ..
fi

# check we have git-xet
if [ ! -f "git-xet" ]; then
  echo "You're missing git-xet!"
  exit 1
fi

# check we have volume-xethub
if [ ! -f "volume-xethub" ]; then
  echo "You're missing volume-xethub!"
  exit 1
fi

# ensure volume will be able to execute git-xet
chmod +x git-xet

set -x
PLUGIN_NAME=xethub/xetfs

# remove existing plugin
docker plugin disable -f $PLUGIN_NAME
docker plugin rm -f $PLUGIN_NAME

set -e

# build docker image
docker build -t xethub-volume:latest -f Dockerfile.release .
# update rootfs (might need to use sudo on linux)
id=$(docker create xethub-volume:latest unix)
mkdir -p rootfs
docker export "$id" | tar -x -C rootfs
docker rm -vf "$id"
docker rmi xethub-volume:latest

# create/enable the plugin
docker plugin create $PLUGIN_NAME .
docker plugin enable $PLUGIN_NAME
