# Write 20 directories of 100 one-line files each into "$1".
set -eu
for directory in $(seq 1 20); do
    mkdir -p "$1/d$directory"
    for file in $(seq 1 100); do
        echo "$file" > "$1/d$directory/f$file"
    done
done
