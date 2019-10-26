#!/usr/bin/env bash

BASEDIR=$(dirname "$0")

function check() {
  if ! type "$1" > /dev/null; then
  echo "$1 is required, install first $2"
  echo $2
  exit 1
fi
}

check node
check graphql-markdown "run npm install graphql-markdown --global"

endpoint="http://127.0.0.1:8000/graphql"
if [ ! -z "$1" ]; then
  endpoint=$1
fi

res_code=$(curl --write-out %{http_code} --silent --output /dev/null \
            -X POST -d '{"query":"query { getLatestEpoch { header { epochId }}}"}' \
            $endpoint)

if [ $res_code -ne 200 ]; then
  echo "$endpoint GraphQL endpoint request failed"
  echo "start API server at first or use the custom endpoint make doc-api http://x.x.x.x:8000/graphql"
  exit 1;
fi

prologue="
>[GraphQL](https://graphql.org) is a query language for APIs and a runtime for fulfilling those queries with your existing data.
GraphQL provides a complete and understandable description of the data in your API,
gives clients the power to ask for exactly what they need and nothing more,
makes it easier to evolve APIs over time, and enables powerful developer tools.

Muta has embeded a [Graph*i*QL](https://github.com/graphql/graphiql) for checking and calling API. Started a the Muta
node, and then try open http://127.0.0.1:8000/graphiql in the browser.
"

graphql-markdown $endpoint --title "Muta GraphQL API" --prologue "$prologue" > $BASEDIR/../graphql_api.md
