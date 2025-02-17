#!/bin/bash
set -e

rm -rf obsidianized/*
target/debug/dreadnom dread/thing/ obsidianized/thing 
target/debug/dreadnom dread/lair/ obsidianized/lair 
diff -rq obsidianized baseline
