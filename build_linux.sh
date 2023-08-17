#!/bin/sh

docker build -t hifirs .
docker cp $(docker create hifirs:latest):hifi-rs .
