apt-get -o Dir::Etc::sourcelist=/dev/null -o Dir::Etc::sourceparts=- -y --no-install-recommends install /inputs/packages/*.deb
cp /inputs/marker /image-marker
