#!/bin/sh

{
version=`curl -sSI https://github.com/rust-disk-partition-management/gptman/releases/latest/download/ | grep -Po 'releases/download/\K\S+'`
curl -L -o gptman "https://github.com/rust-disk-partition-management/gptman/releases/latest/download/gptman-$version-linux-x86_64"
chmod +x gptman
}
