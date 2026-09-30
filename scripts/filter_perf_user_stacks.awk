# A user-space cycles event can still carry unresolved high kernel-address
# frames in its call chain. Remove those frames, retaining every sample and
# every user-space frame for the flamegraph.
$1 ~ /^ffffffff[[:xdigit:]]+$/ && $2 == "[unknown]" && $3 == "([unknown])" { next }
{ print }
