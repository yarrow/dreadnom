set -e
if ! diff -rq latest_output/dir/lair  latest_output/zip/lair >/dev/null; then
    echo latest_output/dir/lair and latest_output/zip/lair differ
    exit 1
fi
if ! diff -rq latest_output/dir/thing  latest_output/zip/thing >/dev/null; then
    echo latest_output/dir/thing and latest_output/zip/thing differ
    exit 1
fi
rm -r baseline_output/lair
cp -r latest_output/dir/lair baseline_output/lair
rm -r baseline_output/thing
cp -r latest_output/dir/thing baseline_output/thing
