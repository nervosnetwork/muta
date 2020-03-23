import gql from "graphql-tag";
import { mutaClient, delay, client } from "./utils";

async function main() {
  let q = `
        query {
            getBlock {
                header {
                    height
                }
            }
        }
        `;
  for (let i = 0; i < 10; i++) {
    await delay(100);
    const res = await client.query({ query: gql(q) });
    console.log(res.data.getLatestBlock.header);

    const height = await mutaClient.getLatestBlockHeight();
    console.log(height);

    const height2 = await mutaClient.getLatestBlockHeight();
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
