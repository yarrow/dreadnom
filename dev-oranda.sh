set -e
diff -q oranda.json oranda.json.release
perl -i -lnE 'print unless /"path_prefix":/' oranda.json
trap 'cp oranda.json.release oranda.json' SIGINT
oranda dev
