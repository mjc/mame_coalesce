#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
temporary_dir=$(mktemp -d)
trap 'rm -rf -- "$temporary_dir"' EXIT

cat >"$temporary_dir/perf-script.txt" <<'EOF'
catalog_import 7/7 [001] 1.000000: 1 cycles:u:
	ffffffffa6a00080 [unknown] ([unknown])
	fffffffffffff000 [unknown] ([unknown])
	0000000000400000 [unknown] ([unknown])
	ffffffffa6a00000 [unknown] ([unknown])
	ffffffffa6a00040 [known_kernel] ([kernel.kallsyms])
	ffffffffa7000000 [unknown] ([unknown])
	ffffffffa7000001 [unknown] ([unknown])
	ffffffffb0000000 [unknown] ([unknown])
	ffffffffc00481c8 zpl_iter_write+0x40 [zfs]
	ffffffffc0048040 [unknown] ([unknown])
	ffffffffc0048060 [unknown] ([unknown])
	ffffffffc0048180 [unknown] ([unknown])
	ffffffffc0048210 [unknown] ([unknown])
	ffffffffc004f000 [unknown] ([unknown])
	ffffffffc005f000 [unknown] ([unknown])
	ffffffffc0061000 [unknown] ([unknown])
	ffffffffc0070000 [unknown] ([unknown])
	ffffffffc0070001 [unknown] ([unknown])

kernel_only 8/8 [001] 2.000000: 3 cycles:u:
    ffffffffc00481c8 [unknown] ([unknown])
EOF

cat >"$temporary_dir/kallsyms" <<'EOF'
ffffffffa6a00000 T entry_SYSCALL_64
ffffffffa6a00040 T entry_SYSCALL_64_safe_stack
ffffffffa7000000 T _etext
ffffffffc0048000 t zpl_iter_write [zfs]
ffffffffc0048050 D zpl_data [zfs]
ffffffffc0048200 T next_function [zfs]
ffffffffc0050000 t spl_function [spl]
ffffffffc0060000 T kernel_after_module
ffffffffc0070000 T final_function [zfs]
EOF

cat >"$temporary_dir/expected.txt" <<'EOF'
catalog_import 7/7 [001] 1.000000: 1 cycles:u:
	ffffffffa6a00080 [unknown] ([unknown])
	fffffffffffff000 [unknown] ([unknown])
	0000000000400000 [unknown] ([unknown])
	ffffffffa6a00000 [unknown] ([unknown])
	ffffffffa6a00040 [known_kernel] ([kernel.kallsyms])
	ffffffffa7000000 [unknown] ([unknown])
	ffffffffa7000001 [unknown] ([unknown])
	ffffffffb0000000 [unknown] ([unknown])
	ffffffffc00481c8 zpl_iter_write+0x40 ([zfs])
	ffffffffc0048040 [unknown] ([unknown])
	ffffffffc0048060 [unknown] ([unknown])
	ffffffffc0048180 [unknown] ([unknown])
	ffffffffc0048210 [unknown] ([unknown])
	ffffffffc004f000 [unknown] ([unknown])
	ffffffffc005f000 [unknown] ([unknown])
	ffffffffc0061000 [unknown] ([unknown])
	ffffffffc0070000 [unknown] ([unknown])
	ffffffffc0070001 [unknown] ([unknown])

kernel_only 8/8 [001] 2.000000: 3 cycles:u:
    ffffffffc00481c8 [unknown] ([unknown])
EOF
awk -f "$script_dir/filter_perf_user_stacks.awk" \
	"$temporary_dir/perf-script.txt" >"$temporary_dir/preserved.txt"
diff -u "$temporary_dir/expected.txt" "$temporary_dir/preserved.txt"

awk -v kallsyms="$temporary_dir/kallsyms" \
	-f "$script_dir/filter_perf_user_stacks.awk" \
	"$temporary_dir/perf-script.txt" >"$temporary_dir/symbolized.txt"
sed \
	-e 's/ffffffffa6a00080 \[unknown\] (\[unknown\])/ffffffffa6a00080 entry_SYSCALL_64_safe_stack+0x40 ([kernel])/' \
	-e 's/ffffffffa6a00000 \[unknown\] (\[unknown\])/ffffffffa6a00000 entry_SYSCALL_64 ([kernel])/' \
	-e 's/ffffffffc0048040 \[unknown\] (\[unknown\])/ffffffffc0048040 zpl_iter_write+0x40 ([zfs])/' \
	-e 's/ffffffffc0070000 \[unknown\] (\[unknown\])/ffffffffc0070000 final_function ([zfs])/' \
	"$temporary_dir/expected.txt" >"$temporary_dir/expected-symbolized.txt"
diff -u "$temporary_dir/expected-symbolized.txt" "$temporary_dir/symbolized.txt"

awk -f "$script_dir/filter_perf_user_stacks.awk" \
	"$temporary_dir/perf-script.txt" |
	inferno-collapse-perf --addrs >"$temporary_dir/collapsed.txt"
rg -F 'kernel_only;[unknown <ffffffffc00481c8>] 3' \
	"$temporary_dir/collapsed.txt" >/dev/null

cat >"$temporary_dir/header-only.txt" <<'EOF'
framed 1 1.000000: 1 cycles:u:
    0000000000001000 [unknown] (/tmp/app)

    empty_before_blank 2 2.000000: 3 cycles:u:
diagnostic: preserve this non-header line

empty_at_eof 3 3.000000: 5 cycles:u:
EOF
awk -f "$script_dir/filter_perf_user_stacks.awk" \
	"$temporary_dir/header-only.txt" >"$temporary_dir/header-only-filtered.txt"
rg -F 'diagnostic: preserve this non-header line' \
	"$temporary_dir/header-only-filtered.txt" >/dev/null
rg -F '0000000000000000 [unavailable_stack] ([unavailable])' \
	"$temporary_dir/header-only-filtered.txt" >/dev/null
inferno-collapse-perf --addrs \
	"$temporary_dir/header-only-filtered.txt" >"$temporary_dir/header-only.folded" \
	2>"$temporary_dir/inferno.stderr"
period_total=$(awk '{ total += $NF } END { printf "%.0f", total }' \
	"$temporary_dir/header-only.folded")
[[ $period_total == 9 ]]
rg -F 'empty_at_eof;[unavailable_stack] 5' \
	"$temporary_dir/header-only.folded" >/dev/null

cat >"$temporary_dir/consecutive.txt" <<'EOF'
framed 11 1.000000: 1 cycles:u:
    0000000000001000 [unknown] (/tmp/app)
empty_first 12 2.000000: 3 cycles:u:
empty_last 13 3.000000: 5 cycles:u:
EOF
awk -f "$script_dir/filter_perf_user_stacks.awk" \
	"$temporary_dir/consecutive.txt" >"$temporary_dir/consecutive-filtered.txt"
inferno-collapse-perf --addrs \
	"$temporary_dir/consecutive-filtered.txt" >"$temporary_dir/consecutive.folded" \
	2>"$temporary_dir/consecutive.stderr"
consecutive_periods=$(awk '{ total += $NF } END { printf "%.0f", total }' \
	"$temporary_dir/consecutive.folded")
[[ $consecutive_periods == 9 ]]
rg -F 'empty_first;[unavailable_stack] 3' \
	"$temporary_dir/consecutive.folded" >/dev/null
rg -F 'empty_last;[unavailable_stack] 5' \
	"$temporary_dir/consecutive.folded" >/dev/null

printf 'filter_perf_user_stacks: all checks passed\n'
