#!/usr/bin/env bash
set -euo pipefail

readonly ARCHIVE_METADATA_URL='https://archive.org/metadata'
readonly ARCHIVE_DOWNLOAD_URL='https://archive.org/download'
readonly DEFAULT_WORK_DIR='tmp/public-domain-rom-test'
readonly DEFAULT_MAX_ROMS=128
readonly DEFAULT_CATALOG_TIER='curated'
readonly DEFAULT_MODE='parent-bundles'
readonly DEFAULT_TOOL_CMD='cargo run --quiet --'

work_dir="$DEFAULT_WORK_DIR"
max_roms="$DEFAULT_MAX_ROMS"
catalog_tier="$DEFAULT_CATALOG_TIER"
jobs="$(nproc 2>/dev/null || printf '1')"
mode="$DEFAULT_MODE"
dry_run=0
skip_run=0
refresh=0
tool_cmd="${MAME_COALESCE_CMD:-$DEFAULT_TOOL_CMD}"

rom_count=0
declare -A game_name_counts=()
declare -a manifest_sources=()
declare -a manifest_roms=()
last_game_name=''

usage() {
  cat <<'USAGE'
Fetch public-domain ROM-like test data, generate a focused Logiqx DAT, and run mame_coalesce.

Options:
  --work-dir DIR          Directory for downloads, generated DAT, database, and output.
  --max-roms N           Maximum ROM entries in the generated DAT. Use 0 for no cap.
  --catalog-tier TIER    metadata or curated. Default: curated.
  --jobs N               Scan worker count passed to mame_coalesce.
  --mode MODE            parent-bundles or per-game.
  --dry-run              Pass --dry-run to mame_coalesce.
  --skip-run             Download and generate the DAT without running mame_coalesce.
  --refresh              Redownload and re-extract sources.
  --tool-cmd CMD         Command prefix for the mame_coalesce CLI.
  -h, --help             Show this help.
USAGE
}

while (($#)); do
  case "$1" in
    --work-dir)
      work_dir="$2"
      shift 2
      ;;
    --max-roms)
      max_roms="$2"
      shift 2
      ;;
    --catalog-tier)
      catalog_tier="$2"
      shift 2
      ;;
    --jobs)
      jobs="$2"
      shift 2
      ;;
    --mode)
      mode="$2"
      shift 2
      ;;
    --dry-run)
      dry_run=1
      shift
      ;;
    --skip-run)
      skip_run=1
      shift
      ;;
    --refresh)
      refresh=1
      shift
      ;;
    --tool-cmd)
      tool_cmd="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'unknown argument: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ "$catalog_tier" != metadata && "$catalog_tier" != curated ]]; then
  printf 'catalog tier must be "metadata" or "curated"\n' >&2
  exit 2
fi

if [[ "$mode" != parent-bundles && "$mode" != per-game ]]; then
  printf 'mode must be "parent-bundles" or "per-game"\n' >&2
  exit 2
fi

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf 'missing required command: %s\n' "$1" >&2
    exit 127
  fi
}

for command in bsdtar cargo curl find git jq md5sum sed sha1sum sort stat; do
  require_command "$command"
done

readonly downloads_dir="$work_dir/archive-downloads"
readonly source_dir="$work_dir/source-roms"
readonly dat_path="$work_dir/dats/public-domain-roms.dat"
readonly output_dir="$work_dir/out"
readonly db_path="$work_dir/coalesce.db"
readonly tmp_dir="$work_dir/tmp"

mkdir -p "$downloads_dir" "$source_dir" "$(dirname "$dat_path")" "$output_dir" "$tmp_dir"

url_encode_segment() {
  local input="$1"
  local output=''
  local char hex
  local i
  for ((i = 0; i < ${#input}; i++)); do
    char="${input:i:1}"
    case "$char" in
      [a-zA-Z0-9.~_-])
        output+="$char"
        ;;
      *)
        printf -v hex '%02X' "'$char"
        output+="%$hex"
        ;;
    esac
  done
  printf '%s' "$output"
}

safe_name() {
  printf '%s' "$1" | sed -E 's/[^A-Za-z0-9_.-]+/_/g; s/^[._-]+//; s/[._-]+$//'
}

file_stem() {
  local name
  name="$(basename "$1")"
  printf '%s' "${name%.*}"
}

file_ext() {
  local name
  name="$(basename "$1")"
  if [[ "$name" == *.* ]]; then
    printf '%s' ".${name##*.}"
  fi
}

json_escape() {
  jq -Rn --arg value "$1" '$value'
}

xml_escape_attr() {
  printf '%s' "$1" |
    sed -e 's/&/\&amp;/g' -e 's/"/\&quot;/g' -e "s/'/\&apos;/g" -e 's/</\&lt;/g' -e 's/>/\&gt;/g'
}

xml_escape_text() {
  printf '%s' "$1" |
    sed -e 's/&/\&amp;/g' -e 's/</\&lt;/g' -e 's/>/\&gt;/g'
}

name_is_disallowed() {
  local normalized
  normalized="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
  case "$normalized" in
    *'donkey kong'*|*goldeneye*|*nintendo*|*quake*|*sonic*|*'spice girls'*|*'super bomberman'*|*tetris*|*topgun*|*'virtual springfield'*)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_rom_name() {
  local name lower ext
  name="$(basename "$1")"
  lower="$(printf '%s' "$name" | tr '[:upper:]' '[:lower:]')"
  ext=".${lower##*.}"
  case "$ext" in
    .bin|.ch8|.chip8|.gb|.gba|.gbc|.n64|.nes|.rom|.sfc|.smc|.v64|.z64)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_archive_name() {
  local lower ext
  lower="$(printf '%s' "$(basename "$1")" | tr '[:upper:]' '[:lower:]')"
  ext=".${lower##*.}"
  case "$ext" in
    .zip|.7z|.rar)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

archive_download_url() {
  printf '%s/%s/%s' \
    "$ARCHIVE_DOWNLOAD_URL" \
    "$(url_encode_segment "$1")" \
    "$(url_encode_segment "$2")"
}

metadata_path_for() {
  printf '%s/%s.metadata.json' "$tmp_dir" "$(safe_name "$1")"
}

download_metadata() {
  local identifier="$1"
  local path
  path="$(metadata_path_for "$identifier")"
  if [[ ! -f "$path" || "$refresh" -eq 1 ]]; then
    curl -fsSL --retry 4 --retry-delay 2 "$ARCHIVE_METADATA_URL/$(url_encode_segment "$identifier")" -o "$path"
  fi
  printf '%s' "$path"
}

metadata_has_public_domain_license() {
  local metadata_path="$1"
  local license_url
  license_url="$(jq -r '.metadata.licenseurl // "" | ascii_downcase' "$metadata_path")"
  [[ "$license_url" == *creativecommons.org/publicdomain/mark* || "$license_url" == *creativecommons.org/publicdomain/zero* ]]
}

metadata_has_curated_signal() {
  local metadata_path="$1"
  local text
  text="$(jq -r '[.metadata.title, .metadata.description, .metadata.subject] | map(. // "") | join(" ") | ascii_downcase' "$metadata_path")"
  [[ "$text" == *'public domain'* || "$text" == *'pd rom'* || "$text" == *pdrom* || "$text" == *homebrew* ]]
}

metadata_file_sha1() {
  local metadata_path="$1"
  local file_name="$2"
  jq -r --arg name "$file_name" '.files[] | select(.name == $name) | .sha1 // ""' "$metadata_path"
}

metadata_lists_file() {
  local metadata_path="$1"
  local file_name="$2"
  jq -e --arg name "$file_name" '.files[] | select(.name == $name)' "$metadata_path" >/dev/null
}

download_file() {
  local url="$1"
  local destination="$2"

  printf 'downloading: %s\n' "$url"
  curl -fsSL --retry 4 --retry-delay 2 -o "$destination.partial" "$url"
  mv "$destination.partial" "$destination"
}

verify_sha1() {
  local path="$1"
  local expected="$2"
  local actual

  if [[ -z "$expected" ]]; then
    return
  fi

  actual="$(sha1sum "$path" | awk '{print $1}')"
  if [[ "$actual" != "$expected" ]]; then
    rm -f "$path"
    printf 'sha1 mismatch for %s: expected %s, got %s\n' "$path" "$expected" "$actual" >&2
    exit 1
  fi
}

source_catalog() {
  cat <<'CATALOG'
metadata|archive|rs32_20200909|NES PD.zip|0|0|archive.org metadata licenseurl is Public Domain Mark; description says it includes PD ROMS
metadata|archive|rs32_20200909|SNES PD.zip|0|0|archive.org metadata licenseurl is Public Domain Mark; description says it includes PD ROMS
metadata|archive|rs32_20200909|GBA PD.rar|0|0|archive.org metadata licenseurl is Public Domain Mark; description says it includes PD ROMS
curated|archive|Chip-8RomsThatAreInThePublicDomain|c8games.zip|1|0|archive.org title says CHIP-8 ROMs are in the public domain; Zophar page calls the pack public domain
curated|archive|pdrc2_5-submissions|pdrc2_5-submissions.zip|0|0|archive.org description identifies PDRoms Coding Competition 2.5 and links pdroms.de source
curated|git_bundle|github.com-DerekTurtleRoe-N64-PD-ROMS_-_2023-10-31_17-21-10|DerekTurtleRoe-N64-PD-ROMS_-_2023-10-31_17-21-10.bundle|0|1|upstream repository README/LICENSE say ROMs are public domain unless otherwise noted
CATALOG
}

selected_catalog() {
  if [[ "$catalog_tier" == metadata ]]; then
    source_catalog | awk -F'|' '$1 == "metadata"'
  else
    source_catalog
  fi
}

write_dat_header() {
  cat >"$dat_path" <<'XML'
<?xml version="1.0"?>
<datafile>
  <header>
    <name>Public Domain Archive.org Smoke Test</name>
    <description>Generated from archive.org public-domain ROM sources</description>
    <version>1</version>
    <author>mame_coalesce test data script</author>
    <url>https://archive.org/</url>
  </header>
XML
}

write_dat_footer() {
  printf '</datafile>\n' >>"$dat_path"
}

unique_game_name() {
  local base="$1"
  local count suffix prefix

  base="$(safe_name "$base")"
  base="${base:0:80}"
  if [[ -z "$base" ]]; then
    base='rom'
  fi

  count="${game_name_counts[$base]:-0}"
  count=$((count + 1))
  game_name_counts[$base]="$count"

  if ((count == 1)); then
    last_game_name="$base"
  else
    suffix="_$count"
    prefix="${base:0:$((80 - ${#suffix}))}"
    last_game_name="$prefix$suffix"
  fi
}

game_name_for_entry() {
  local source="$1"
  local rom_name="$2"
  local source_stem rom_stem

  source_stem="$(safe_name "$(file_stem "$source")")"
  rom_stem="$(safe_name "$(file_stem "$rom_name")")"
  if [[ "$source_stem" == "$rom_stem" ]]; then
    unique_game_name "$rom_stem"
  else
    unique_game_name "${source_stem}_${rom_stem}"
  fi
}

hash_file_to_dat() {
  local data_path="$1"
  local source="$2"
  local rom_name="$3"
  local evidence_tier="$4"
  local evidence="$5"
  local size sha1 md5 game_name

  if ((max_roms > 0 && rom_count >= max_roms)); then
    return 1
  fi

  size="$(stat -c '%s' "$data_path")"
  sha1="$(sha1sum "$data_path" | awk '{print $1}')"
  md5="$(md5sum "$data_path" | awk '{print $1}')"
  game_name_for_entry "$source" "$rom_name"
  game_name="$last_game_name"

  {
    printf '  <game name="%s" sourcefile="%s">\n' "$(xml_escape_attr "$game_name")" "$(xml_escape_attr "$source")"
    printf '    <description>%s</description>\n' "$(xml_escape_text "$game_name")"
    printf '    <year>unknown</year>\n'
    printf '    <manufacturer>%s</manufacturer>\n' "$(xml_escape_text "$evidence_tier")"
    printf '    <rom name="%s" size="%s" sha1="%s" md5="%s" crc="00000000"/>\n' "$(xml_escape_attr "$rom_name")" "$size" "$sha1" "$md5"
    printf '  </game>\n'
  } >>"$dat_path"

  manifest_roms+=("$(jq -cn \
    --arg game_name "$game_name" \
    --arg rom_name "$rom_name" \
    --argjson size "$size" \
    --arg sha1 "$sha1" \
    --arg md5 "$md5" \
    --arg source "$source" \
    --arg evidence_tier "$evidence_tier" \
    --arg evidence "$evidence" \
    '{game_name:$game_name, rom_name:$rom_name, size:$size, crc:"00000000", md5:$md5, sha1:$sha1, source:$source, evidence_tier:$evidence_tier, evidence:$evidence}')")
  rom_count=$((rom_count + 1))
}

hash_zip_entries_to_dat() {
  local archive_path="$1"
  local source_root="$2"
  local exclude_disallowed="$3"
  local evidence_tier="$4"
  local evidence="$5"
  local relative_archive entry tmp_entry

  relative_archive="${archive_path#"$source_root"/}"
  while IFS= read -r entry; do
    if ((max_roms > 0 && rom_count >= max_roms)); then
      return
    fi
    if ! is_rom_name "$entry"; then
      continue
    fi
    if [[ "$evidence_tier" == curated || "$exclude_disallowed" == 1 ]] && name_is_disallowed "$entry"; then
      continue
    fi

    tmp_entry="$tmp_dir/entry-$rom_count.bin"
    if ! bsdtar -xOf "$archive_path" "$entry" >"$tmp_entry"; then
      printf 'skipping unreadable ZIP entry: %s:%s\n' "$archive_path" "$entry" >&2
      rm -f "$tmp_entry"
      continue
    fi
    hash_file_to_dat "$tmp_entry" "$relative_archive" "$entry" "$evidence_tier" "$evidence" || true
    rm -f "$tmp_entry"
  done < <(bsdtar -tf "$archive_path" 2>/dev/null || {
    printf 'skipping unreadable ZIP source archive: %s\n' "$archive_path" >&2
    true
  })
}

hash_source_tree_to_dat() {
  local source_root="$1"
  local allow_extensionless="$2"
  local exclude_disallowed="$3"
  local evidence_tier="$4"
  local evidence="$5"
  local path relative ext

  while IFS= read -r -d '' path; do
    if ((max_roms > 0 && rom_count >= max_roms)); then
      return
    fi
    relative="${path#"$source_root"/}"

    if [[ "$relative" == .git/* ]]; then
      continue
    fi
    if [[ "$evidence_tier" == curated || "$exclude_disallowed" == 1 ]] && name_is_disallowed "$relative"; then
      continue
    fi

    ext="$(file_ext "$path" | tr '[:upper:]' '[:lower:]')"
    if [[ "$ext" == .zip ]]; then
      hash_zip_entries_to_dat "$path" "$source_root" "$exclude_disallowed" "$evidence_tier" "$evidence"
    elif is_rom_name "$path"; then
      hash_file_to_dat "$path" "$relative" "$(basename "$path")" "$evidence_tier" "$evidence" || true
    elif [[ "$allow_extensionless" == 1 && -z "$ext" ]]; then
      hash_file_to_dat "$path" "$relative" "$(basename "$path")" "$evidence_tier" "$evidence" || true
    fi
  done < <(find "$source_root" -type f -print0 | sort -z)
}

prepare_source() {
  local kind="$1"
  local downloaded="$2"
  local prepared_root="$3"

  if [[ "$refresh" -eq 1 && -e "$prepared_root" ]]; then
    rm -rf "$prepared_root"
  fi

  case "$kind" in
    archive)
      mkdir -p "$prepared_root"
      if [[ ! -f "$prepared_root/.extracted" ]]; then
        bsdtar -xf "$downloaded" -C "$prepared_root"
        sha1sum "$downloaded" | awk '{print $1}' >"$prepared_root/.extracted"
      fi
      ;;
    git_bundle)
      if [[ ! -d "$prepared_root/.git" ]]; then
        git clone "$downloaded" "$prepared_root" >/dev/null
      fi
      ;;
    direct)
      mkdir -p "$prepared_root"
      cp -p "$downloaded" "$prepared_root/"
      ;;
    *)
      printf 'unknown source kind: %s\n' "$kind" >&2
      exit 2
      ;;
  esac
}

write_dat_header

while IFS='|' read -r tier kind identifier file_name allow_extensionless exclude_disallowed evidence; do
  metadata_path="$(download_metadata "$identifier")"

  if ! metadata_lists_file "$metadata_path" "$file_name"; then
    printf 'archive.org metadata did not list expected file: %s/%s\n' "$identifier" "$file_name" >&2
    exit 1
  fi

  if ! metadata_has_public_domain_license "$metadata_path"; then
    if [[ "$tier" != curated ]] || ! metadata_has_curated_signal "$metadata_path"; then
      printf 'refusing to download %s: metadata lacks accepted public-domain evidence\n' "$identifier" >&2
      exit 1
    fi
  fi

  safe_source_name="$(safe_name "$identifier")__$(safe_name "$(file_stem "$file_name")")"
  downloaded="$downloads_dir/$safe_source_name$(file_ext "$file_name")"
  expected_sha1="$(metadata_file_sha1 "$metadata_path" "$file_name")"

  if [[ ! -f "$downloaded" || "$refresh" -eq 1 ]]; then
    download_file "$(archive_download_url "$identifier" "$file_name")" "$downloaded"
  else
    printf 'using cached: %s\n' "$downloaded"
  fi
  verify_sha1 "$downloaded" "$expected_sha1"

  prepared_root="$source_dir/$safe_source_name"
  prepare_source "$kind" "$downloaded" "$prepared_root"
  hash_source_tree_to_dat "$prepared_root" "$allow_extensionless" "$exclude_disallowed" "$tier" "$evidence"

  manifest_sources+=("$(jq -cn \
    --arg identifier "$identifier" \
    --arg file_name "$file_name" \
    --arg kind "$kind" \
    --arg evidence_tier "$tier" \
    --arg evidence "$evidence" \
    --arg downloaded_file "$downloaded" \
    --arg downloaded_sha1 "$(sha1sum "$downloaded" | awk '{print $1}')" \
    --arg prepared_root "$prepared_root" \
    '{identifier:$identifier, file_name:$file_name, kind:$kind, evidence_tier:$evidence_tier, evidence:$evidence, downloaded_file:$downloaded_file, downloaded_sha1:$downloaded_sha1, prepared_root:$prepared_root}')")
done < <(selected_catalog)

if ((rom_count == 0)); then
  printf 'no ROM entries found in downloaded public-domain sources\n' >&2
  exit 1
fi

write_dat_footer

{
  printf '{\n'
  printf '  "policy": {\n'
  printf '    "metadata": "archive.org item licenseurl must be Public Domain Mark or CC0",\n'
  printf '    "curated": "item/upstream text must explicitly claim public-domain/PD ROM status",\n'
  printf '    "excluded": ["abandonware", "commercial ROM-set mirrors", "translations/patches of commercial games", "obvious derivative demos using commercial game properties"]\n'
  printf '  },\n'
  printf '  "sources": [\n'
  for i in "${!manifest_sources[@]}"; do
    [[ "$i" == 0 ]] || printf ',\n'
    printf '    %s' "${manifest_sources[$i]}"
  done
  printf '\n  ],\n'
  printf '  "roms": [\n'
  for i in "${!manifest_roms[@]}"; do
    [[ "$i" == 0 ]] || printf ',\n'
    printf '    %s' "${manifest_roms[$i]}"
  done
  printf '\n  ]\n'
  printf '}\n'
} >"$work_dir/manifest.json"

printf 'generated DAT: %s\n' "$dat_path"
printf 'archive.org downloads: %s\n' "$downloads_dir"
printf 'source archives/files: %s\n' "$source_dir"
printf 'catalog tier: %s\n' "$catalog_tier"
printf 'ROM entries in DAT: %s\n' "$rom_count"

if [[ "$skip_run" -eq 1 ]]; then
  exit 0
fi

rm -f "$db_path"

read -r -a tool_cmd_parts <<<"$tool_cmd"
cmd=("${tool_cmd_parts[@]}" --database-path "$db_path" run --dat "$dat_path" --source "$source_dir" --out "$output_dir" --jobs "$jobs" --mode "$mode" --strict)
if [[ "$dry_run" -eq 1 ]]; then
  cmd+=(--dry-run)
fi

printf 'running:'
printf ' %q' "${cmd[@]}"
printf '\n'
"${cmd[@]}"
printf 'output: %s\n' "$output_dir"
