# Keep perf script records and frames intact unless an explicitly supplied
# kallsyms snapshot can name an unresolved kernel instruction pointer.

function normalize_address(address) {
	address = tolower(address)
	sub(/^0x/, "", address)
	return substr("0000000000000000", 1, 16 - length(address)) address
}

function hex_offset(address, base,    i, a, b, borrow, digit, result) {
	borrow = 0
	result = ""
	for (i = 16; i > 0; i--) {
		a = index("0123456789abcdef", substr(address, i, 1)) - 1
		b = index("0123456789abcdef", substr(base, i, 1)) - 1 + borrow
		borrow = a < b
		digit = a - b + (borrow ? 16 : 0)
		result = substr("0123456789abcdef", digit + 1, 1) result
	}
	sub(/^0+/, "", result)
	return result == "" ? "0" : result
}

function is_sample_header(line) {
	return line ~ /^[[:space:]]*[^[:space:]].*[[:space:]][[:digit:]]+(\/[[:digit:]]+)?([[:space:]]+\[[[:digit:]]+\])?[[:space:]]+[[:digit:]]+\.[[:digit:]]+:[[:space:]]+[[:digit:]]+[[:space:]]+[^[:space:]]+:?[[:space:]]*$/
}

function finish_sample() {
	if (have_sample && !sample_has_frame) {
		print "\t0000000000000000 [unavailable_stack] ([unavailable])"
	}
	have_sample = 0
}

function symbolize(address,    low, high, middle, base, symbol, offset) {
	low = 1
	high = symbol_count
	resolved_index = 0
	while (low <= high) {
		middle = int((low + high) / 2)
		if (sorted_addresses[middle] <= address) {
			resolved_index = middle
			low = middle + 1
		} else {
			high = middle - 1
		}
	}
	if (resolved_index == 0) return ""
	base = sorted_addresses[resolved_index]
	if (!(base in symbols)) return ""
	resolved_base = base
	symbol = symbols[base]
	offset = hex_offset(address, base)
	if (resolved_index == symbol_count) {
		return offset == "0" ? symbol : ""
	}
	if (boundary_modules[base] != boundary_modules[sorted_addresses[resolved_index + 1]]) return ""
	return symbol (offset == "0" ? "" : "+0x" offset)
}

BEGIN {
	if (kallsyms != "") {
		while ((getline line < kallsyms) > 0) {
			count = split(line, fields, /[[:space:]]+/)
			if (count < 3 || fields[1] !~ /^[[:xdigit:]]+$/ || fields[3] == "") continue
			address = normalize_address(fields[1])
			boundaries[address] = 1
			if (count >= 4 && fields[4] ~ /^\[.*\]$/) {
				boundary_modules[address] = fields[4]
			} else if (!(address in boundary_modules)) {
				boundary_modules[address] = ""
			}
			if (fields[2] ~ /^[TtWw]$/ &&
			    fields[3] !~ /^(_etext|etext|__etext|endtext|end_text|__end_text)$/ &&
			    !(address in symbols)) {
				symbols[address] = fields[3]
				if (count >= 4 && fields[4] ~ /^\[.*\]$/) modules[address] = fields[4]
			}
		}
		close(kallsyms)
		symbol_count = asorti(boundaries, sorted_addresses, "@ind_str_asc")
	}
}

{
	if (is_sample_header($0)) {
		if (have_sample) {
			finish_sample()
			print ""
		}
		print
		have_sample = 1
		sample_has_frame = 0
		next
	}
	if ($0 == "") {
		finish_sample()
		print
		next
	}
	if (have_sample && $0 ~ /^[[:space:]]+(0x)?[[:xdigit:]]+[[:space:]]/) sample_has_frame = 1

	if ($1 ~ /^ffffffff[[:xdigit:]]+$/ &&
	    $2 == "[unknown]" && $3 == "([unknown])" && kallsyms != "") {
		match($0, /^[[:space:]]*/)
		indentation = substr($0, 1, RLENGTH)
		address = normalize_address($1)
		name = symbolize(address)
		if (name != "") {
			module = modules[resolved_base]
			if (module == "") {
				module = "([kernel])"
			} else if (module ~ /^\[[^]]+\]$/) {
				module = "(" module ")"
			}
			$2 = name
			$3 = module
			$0 = indentation $1 " " $2 " " $3
		}
	} else if ($1 ~ /^[[:xdigit:]]+$/ && $3 ~ /^\[[^]]+\]$/) {
		match($0, /^[[:space:]]*/)
		indentation = substr($0, 1, RLENGTH)
		$3 = "(" $3 ")"
		$0 = indentation $1 " " $2 " " $3
	}
	print
}

END {
	finish_sample()
}
