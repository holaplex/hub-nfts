#!/bin/bash
set -eux
ARCHITECTURE=$(uname -m)

if [[ $ARCHITECTURE == "x86_64" ]]; then
    ARCHITECTURE="x86_64"
elif [[ $ARCHITECTURE == *'aarch'* ]]; then
    ARCHITECTURE="aarch_64"
elif [[ $ARCHITECTURE == *'arm'* ]]; then
    ARCHITECTURE="aarch_64"
else
    echo "Unsupported architecture"
    exit 1
fi

OS_TYPE=$(uname | tr '[:upper:]' '[:lower:]')

PROTOC_VERSION="23.2"

URL="https://github.com/protocolbuffers/protobuf/releases/download/v${PROTOC_VERSION}/protoc-${PROTOC_VERSION}-${OS_TYPE}-${ARCHITECTURE}.zip"

wget -q "$URL" -O protoc.zip

unzip protoc.zip
cp ./bin/protoc /usr/bin/protoc
protoc --version
rm -rf bin protoc.zip
