#!/bin/bash

script_dirname="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
ssh -o StrictHostKeychecking=no -p 2222 -i $script_dirname/_output/sshkey ubuntu@127.0.0.1