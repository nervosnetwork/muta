import fetch from "node-fetch";
import { createHttpLink } from "apollo-link-http";
import { InMemoryCache } from "apollo-cache-inmemory";
import ApolloClient from "apollo-client";
import { readFileSync } from "fs";
import { AssetService, Muta } from "muta-sdk";
const toml = require("toml");

export const CHAIN_ID =
  "0xb6a4d7da21443f5e816e8700eea87610e6d769657d6b8ec73028457bf2ca4036";
export const API_URL = process.env.API_URL || "http://localhost:8000/graphql";
export const client = new ApolloClient({
  link: createHttpLink({
    uri: API_URL,
    fetch: fetch
  }),
  cache: new InMemoryCache(),
  defaultOptions: { query: { fetchPolicy: "no-cache" } }
});
export const muta = new Muta({
  endpoint: API_URL,
  chainId: CHAIN_ID
});
export const mutaClient = muta.client();

export function makeid(length: number) {
  var result = "";
  var characters = "abcdef0123456789";
  var charactersLength = characters.length;
  for (var i = 0; i < length; i++) {
    result += characters.charAt(Math.floor(Math.random() * charactersLength));
  }
  return result;
}

export function getNonce() {
  return makeid(64);
}

export function delay(ms: number) {
  return new Promise(resolve => setTimeout(resolve, ms));
}
