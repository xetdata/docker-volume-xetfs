#!/bin/bash

set -x


if [ ! -f "git-xet" ] || [ ! -f "volume-xethub" ]; then
  pushd builder
    docker-compose build rust_build
    docker-compose run  --rm --name builder rust_build
  popd
  cp builder/rust_target/release/git-xet .
  cp builder/rust_target/release/volume-xethub .
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

IMAGE_NAME=xethub-volume
PLUGIN_NAME=xethub/xetfs
TAG=latest

set -x

# remove existing plugin
docker plugin disable -f $PLUGIN_NAME
docker plugin rm -f $PLUGIN_NAME

set -e

# build docker image
docker build -t $IMAGE_NAME:$TAG -f Dockerfile.release .

# update rootfs
id=$(docker create $IMAGE_NAME:$TAG unix)

UNAME=$( uname )
mkdir -p rootfs
if [[ $UNAME == "Linux" ]]; then
  docker export "$id" | sudo tar -x -C rootfs
else
  docker export "$id" | tar -x -C rootfs
fi
docker rm -vf "$id"
docker rmi $IMAGE_NAME:$TAG

# create/enable the plugin
if [[ $UNAME == "Linux" ]]; then
  sudo docker plugin create $PLUGIN_NAME .
else
  docker plugin create $PLUGIN_NAME .
fi
docker plugin enable $PLUGIN_NAME

