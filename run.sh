#!/bin/bash

# Case script to run phantom daemon or phantom-gui based on argument

case "$1" in
    d)
        echo "Running phantom daemon..."
        sudo ./target/release/phantom --daemon
        ;;
    g)
        echo "Running phantom-gui..."
        ./target/release/phantom-gui
        ;;
    *)
        echo "Usage: $0 {d|g}"
        echo "  d: Run phantom daemon with sudo"
        echo "  g: Run phantom-gui"
        exit 1
        ;;
esac