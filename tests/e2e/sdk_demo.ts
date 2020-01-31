import gql from "graphql-tag";
import { muta, CHAIN_CONFIG, delay, client } from "./utils";

async function main() {
  let q = `
        query {
            getLatestBlock {
                header {
                    height
                }
            }
        }
        `;
  const muta_client = muta.client;
  for (let i = 0; i < 10; i++) {
    await delay(100);
    const res = await client.query({ query: gql(q) });
    console.log(res.data.getLatestBlock.header);

    const height = await muta_client.getBlockHeight();
    console.log(height);

    const height2 = await muta.client.getBlockHeight();
    console.log(height2);
  }
}

main()
  .then(
    () => {
      console.log("---- exit ----");
    },
    err => {
      console.log(err);
    }
  )
  .then(() => {
    process.exit();
  });
