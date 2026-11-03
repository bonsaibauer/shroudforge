#!/usr/bin/env bash
set -Eeuo pipefail

archive_directory="build"
report_path="virustotal-release.json"
poll_interval_seconds=30
max_analysis_wait_seconds=1200
minimum_request_interval_seconds=16
maximum_file_size_bytes=$((650 * 1000 * 1000))
upload_url_threshold_bytes=$((32 * 1000 * 1000))

while (($#)); do
  case "$1" in
    --archive-directory)
      archive_directory="${2:?Missing value for --archive-directory}"
      shift 2
      ;;
    --report-path)
      report_path="${2:?Missing value for --report-path}"
      shift 2
      ;;
    *)
      printf 'Unknown argument: %s\n' "$1" >&2
      exit 2
      ;;
  esac
done

mkdir -p "$(dirname -- "$report_path")"

scan_results='[]'
global_errors='[]'
last_api_request_epoch=0
api_http_status='000'
api_error_message=''
temporary_directory=''

cleanup() {
  if [[ -n "$temporary_directory" && -d "$temporary_directory" ]]; then
    rm -rf -- "$temporary_directory"
  fi
}
trap cleanup EXIT

append_global_error() {
  local message="$1"
  global_errors="$(jq -c --arg message "$message" '. + [$message]' <<<"$global_errors")"
}

append_scan_result() {
  local result_json="$1"
  scan_results="$(jq -c --argjson result "$result_json" '. + [$result]' <<<"$scan_results")"
}

api_request() {
  local output_path="$1"
  shift

  local now
  local wait_seconds
  now="$(date +%s)"
  if ((last_api_request_epoch > 0)); then
    wait_seconds=$((minimum_request_interval_seconds - (now - last_api_request_epoch)))
    if ((wait_seconds > 0)); then
      sleep "$wait_seconds"
    fi
  fi
  last_api_request_epoch="$(date +%s)"

  local http_status
  if ! http_status="$(curl \
    --silent \
    --show-error \
    --connect-timeout 15 \
    --max-time 300 \
    --output "$output_path" \
    --write-out '%{http_code}' \
    --header "x-apikey: $VT_API_KEY" \
    --header 'accept: application/json' \
    "$@")"; then
    api_http_status='000'
    api_error_message='The VirusTotal API request could not be completed.'
    return 1
  fi

  api_http_status="$http_status"
  if [[ "$http_status" == 2?? ]]; then
    api_error_message=''
    return 0
  fi

  api_error_message="$(jq -r '(.error.message // .error // .message // "The API returned an HTTP error.") | tostring | .[0:400]' "$output_path" 2>/dev/null || true)"
  if [[ -z "$api_error_message" ]]; then
    api_error_message='The API returned an HTTP error without a readable message.'
  fi
  return 1
}

make_error_result() {
  local archive_name="$1"
  local sha256="$2"
  local message="$3"

  jq -cn \
    --arg archive "$archive_name" \
    --arg sha256 "$sha256" \
    --arg error "$message" \
    '{archive:$archive, sha256:(if $sha256 == "" then null else $sha256 end), status:"error", finding_count:0, stats:null, findings:[], report_url:(if $sha256 == "" then null else "https://www.virustotal.com/gui/file/" + $sha256 end), error:$error}'
}

make_scan_result() {
  local response_path="$1"
  local archive_name="$2"
  local sha256="$3"
  local source="$4"

  jq -ce \
    --arg archive "$archive_name" \
    --arg sha256 "$sha256" \
    --arg source "$source" \
    --arg completed_at "$(date -u +'%Y-%m-%dT%H:%M:%SZ')" \
    --slurpfile api_response "$response_path" \
    '
      .data.attributes as $attributes
      | ($attributes.last_analysis_stats // $attributes.stats // null) as $raw_stats
      | if ($raw_stats | type) != "object" or ($raw_stats | length) == 0 then
          error("VirusTotal completed without last_analysis_stats.")
        else
          ($attributes.last_analysis_results // $attributes.results // {}) as $results
          | {
              archive: $archive,
              sha256: $sha256,
              source: $source,
              status: (if (($raw_stats.malicious // 0) | tonumber? // 0) > 0 then "findings" elif (($raw_stats.suspicious // 0) | tonumber? // 0) > 0 then "warning" else "clean" end),
              finding_count: ((($raw_stats.malicious // 0) | tonumber? // 0) + (($raw_stats.suspicious // 0) | tonumber? // 0)),
              stats: {
                malicious: (($raw_stats.malicious // 0) | tonumber? // 0),
                suspicious: (($raw_stats.suspicious // 0) | tonumber? // 0),
                undetected: (($raw_stats.undetected // 0) | tonumber? // 0),
                harmless: (($raw_stats.harmless // 0) | tonumber? // 0),
                timeout: (($raw_stats.timeout // 0) | tonumber? // 0),
                confirmed_timeout: (($raw_stats["confirmed-timeout"] // 0) | tonumber? // 0),
                failure: (($raw_stats.failure // 0) | tonumber? // 0),
                type_unsupported: (($raw_stats["type-unsupported"] // 0) | tonumber? // 0),
                other: (($raw_stats.other // 0) | tonumber? // 0)
              },
              analysis_date: ($attributes.last_analysis_date // $attributes.date // $completed_at),
              report_url: ("https://www.virustotal.com/gui/file/" + $sha256),
              api_response_type: "analysis",
              api_response: $api_response[0],
              findings: [
                ($results | to_entries[]? | select(.value.category == "malicious" or .value.category == "suspicious"))
                | {
                    level: .value.category,
                    title: (.value.engine_name // .key),
                    message: (.value.result // .value.category),
                    location: $archive
                  }
              ],
              error: null
            }
        end
    ' "$response_path"
}

scan_archive() {
  local archive_path="$1"
  local archive_name
  local sha256
  local archive_size
  local response_path="$temporary_directory/response.json"
  local upload_url_path="$temporary_directory/upload-url.json"
  local analysis_path="$temporary_directory/analysis.json"
  local source
  local upload_url
  local analysis_id
  local encoded_analysis_id
  local analysis_status
  local attempt
  local max_analysis_attempts=$((max_analysis_wait_seconds / poll_interval_seconds))
  local analysis_not_found_count=0
  local result_json

  archive_name="$(basename -- "$archive_path")"
  if ! sha256="$(sha256sum -- "$archive_path" | awk '{print $1}')"; then
    append_scan_result "$(make_error_result "$archive_name" '' 'Could not calculate the release archive SHA-256.')"
    return 1
  fi

  archive_size="$(stat -c '%s' -- "$archive_path")"
  if ((archive_size > maximum_file_size_bytes)); then
    append_scan_result "$(make_error_result "$archive_name" "$sha256" 'The release archive exceeds VirusTotal’s documented 650 MB upload limit.')"
    return 1
  fi

  if ((archive_size > upload_url_threshold_bytes)); then
    if ! api_request "$upload_url_path" --request GET 'https://www.virustotal.com/api/v3/files/upload_url'; then
      append_scan_result "$(make_error_result "$archive_name" "$sha256" "Could not obtain the large-file upload URL. HTTP $api_http_status: $api_error_message")"
      return 1
    fi
    upload_url="$(jq -r '.data // empty' "$upload_url_path")"
    if [[ -z "$upload_url" ]]; then
      append_scan_result "$(make_error_result "$archive_name" "$sha256" 'VirusTotal did not return a large-file upload URL.')"
      return 1
    fi
  else
    upload_url='https://www.virustotal.com/api/v3/files'
  fi

  if ! api_request "$response_path" --request POST --form "file=@$archive_path;type=application/zip" "$upload_url"; then
    append_scan_result "$(make_error_result "$archive_name" "$sha256" "VirusTotal file upload failed with HTTP $api_http_status: $api_error_message")"
    return 1
  fi

  analysis_id="$(jq -r '.data.id // empty' "$response_path")"
  if [[ -z "$analysis_id" ]]; then
    append_scan_result "$(make_error_result "$archive_name" "$sha256" 'VirusTotal accepted the upload without returning an analysis ID.')"
    return 1
  fi
  encoded_analysis_id="$(jq -rn --arg id "$analysis_id" '$id | @uri')"

  source='new_analysis'
  for ((attempt = 1; attempt <= max_analysis_attempts; attempt++)); do
    sleep "$poll_interval_seconds"
    if ! api_request "$analysis_path" --request GET "https://www.virustotal.com/api/v3/analyses/$encoded_analysis_id"; then
      if [[ "$api_http_status" == "404" ]]; then
        analysis_not_found_count=$((analysis_not_found_count + 1))
        if ((attempt < max_analysis_attempts)); then
          printf 'VirusTotal analysis is not available yet (HTTP 404). Retrying in %s seconds (%s/%s).\n' \
            "$poll_interval_seconds" "$attempt" "$max_analysis_attempts" >&2
        fi
        continue
      fi
      append_scan_result "$(make_error_result "$archive_name" "$sha256" "VirusTotal analysis polling failed with HTTP $api_http_status: $api_error_message")"
      return 1
    fi
    analysis_not_found_count=0

    analysis_status="$(jq -r '.data.attributes.status // empty' "$analysis_path")"
    case "$analysis_status" in
      completed)
        if ! result_json="$(make_scan_result "$analysis_path" "$archive_name" "$sha256" "$source")"; then
          append_scan_result "$(make_error_result "$archive_name" "$sha256" 'VirusTotal completed the analysis without usable statistics.')"
          return 1
        fi
        append_scan_result "$result_json"
        return 0
        ;;
      queued|in-progress)
        ;;
      *)
        append_scan_result "$(make_error_result "$archive_name" "$sha256" "VirusTotal returned an unexpected analysis state: ${analysis_status:-missing}.")"
        return 1
        ;;
    esac
  done

  if ((analysis_not_found_count > 0)); then
    append_scan_result "$(make_error_result "$archive_name" "$sha256" "VirusTotal analysis remained unavailable after ${analysis_not_found_count} consecutive HTTP 404 responses over up to ${max_analysis_wait_seconds} seconds: $api_error_message")"
  else
    append_scan_result "$(make_error_result "$archive_name" "$sha256" "VirusTotal analysis did not complete within ${max_analysis_wait_seconds} seconds.")"
  fi
  return 1
}

write_report() {
  local completed_at
  completed_at="$(date -u +'%Y-%m-%dT%H:%M:%SZ')"
  jq -n \
    --argjson files "$scan_results" \
    --argjson global_errors "$global_errors" \
    --arg completed_at "$completed_at" \
    '
      ($files | map(.stats // {})) as $stats
      | ([ $stats[].malicious // 0 ] | add // 0) as $malicious
      | ([ $stats[].suspicious // 0 ] | add // 0) as $suspicious
      | ([ $stats[].undetected // 0 ] | add // 0) as $undetected
      | ([ $stats[].harmless // 0 ] | add // 0) as $harmless
      | ([ $files[] | select(.status == "error") ] | length) as $file_errors
      | {
          scanner: "VirusTotal",
          target: "Release ZIP files",
          api_version: "VirusTotal API v3",
          status: (if (($global_errors | length) + $file_errors) > 0 then "error" elif $malicious > 0 then "findings" elif $suspicious > 0 then "warning" else "clean" end),
          archive_count: ([ $files[] | select(.archive != null) ] | length),
          finding_count: ($malicious + $suspicious),
          malicious_count: $malicious,
          suspicious_count: $suspicious,
          counts: {
            error: (($global_errors | length) + $file_errors),
            warning: $suspicious,
            note: 0,
            other: 0
          },
          undetected_count: $undetected,
          harmless_count: $harmless,
          files: $files,
          findings: [ $files[].findings[]? ],
          errors: ($global_errors + [ $files[] | select(.status == "error") | .error ]),
          error: (if (($global_errors | length) + $file_errors) > 0 then (($global_errors + [ $files[] | select(.status == "error") | .error ]) | join(" ")) else null end),
          scan_completed_at: $completed_at,
          policy: {
            malicious_blocks_publication: ($malicious >= 2),
            malicious_blocking_threshold: 2,
            single_malicious_detection_is_advisory: true,
            suspicious_is_advisory: true
          }
        }
    ' >"$report_path"
}

if ! command -v jq >/dev/null 2>&1; then
  printf 'jq is required to create the VirusTotal report.\n' >&2
  exit 1
fi

temporary_directory="$(mktemp -d)"
if [[ -z "${VT_API_KEY:-}" ]]; then
  append_global_error 'The VT_API_KEY GitHub Actions secret is not configured.'
else
  shopt -s nullglob
  archives=("$archive_directory"/shroudforge-*.zip)
  if ((${#archives[@]} == 0)); then
    append_global_error "No shroudforge release ZIPs were found in '$archive_directory'."
  else
    for archive in "${archives[@]}"; do
      if ! scan_archive "$archive"; then
        printf 'VirusTotal could not produce a clean scan report for %s.\n' "$(basename -- "$archive")" >&2
      fi
    done
  fi
fi

write_report

report_status="$(jq -r '.status' "$report_path")"
malicious_count="$(jq -r '.malicious_count' "$report_path")"
if [[ "$report_status" == 'error' ]] || ((malicious_count >= 2)); then
  exit 1
fi
