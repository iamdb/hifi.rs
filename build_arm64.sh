#!/bin/sh

docker build -f Dockerfile.arm64 -t hifirs .
docker cp $(docker create hifirs:latest):hifi-rs .
