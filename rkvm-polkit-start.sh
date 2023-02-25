#!/bin/bash

env_file=/tmp/.rkvm-$1.env 
declare -px > $env_file
pkexec bash -c "source $env_file; rm $env_file; $HOME/bin/rkvm-$1 $HOME/.config/rkvm/$1.toml"
