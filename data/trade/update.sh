#!/bin/sh
# Refreshes the trade site data Price Check builds its searches from; run it
# when a league adds mods or bases, then commit the changed files.
#   data/trade/stats.tsv  stat id<TAB>text ("\n" for line breaks)
#   data/trade/bases.txt  one base type per line
# Explicit stats are searched one by one, and pseudo ones where the trade site
# has no explicit stat (item rarity); implicit and crafted mods only count
# toward the pseudo totals.
# The trade site answers non-browser clients with 403, hence the User-Agent.
set -eu
cd "$(dirname "$0")"
agent='Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36'
api=https://www.pathofexile.com/api/trade/data
curl -sSf -A "$agent" "$api/stats" | jq -r '
  .result[] | select(.id == "explicit" or .id == "pseudo") | .entries[] | select(.option == null)
  | [.id, .text] | @tsv' > stats.tsv
curl -sSf -A "$agent" "$api/items" | jq -r '[.result[].entries[] | select(.name == null) | .type] | unique[]' > bases.txt
date -u +%Y-%m-%d > fetched
