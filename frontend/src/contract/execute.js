import { LCDClient, MsgExecuteContract, Fee, Coin } from "@terra-money/terra.js";
import { contractAddress } from "./address";

// ==== utils ====

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const until = Date.now() + 1000 * 60 * 60;
const untilInterval = Date.now() + 1000 * 60;

const _exec =
  (msg, cost, fee = new Fee(1000000, { uusd: 600000 })) =>
  async (wallet) => {
    const lcd = new LCDClient({
      URL: wallet.network.lcd,
      chainID: wallet.network.chainID,
    });
    const { result } = await wallet.post({
      fee,
      msgs: [
        new MsgExecuteContract(
          wallet.walletAddress,
          contractAddress(wallet),
          msg,
          { uluna: cost }
        ),
        new MsgExecuteContract(
          wallet.walletAddress,
          contractAddress(wallet),
          msg,
          { uluna: cost }
        ),
      ],
    });

    while (true) {
      try {
        return await lcd.tx.txInfo(result.txhash);
      } catch (e) {
        if (Date.now() < untilInterval) {
          await sleep(500);
        } else if (Date.now() < until) {
          await sleep(1000 * 10);
        } else {
          throw new Error(
            `Transaction queued. To verify the status, please check the transaction hash: ${result.txhash}`
          );
        }
      }
    }
  };

// ==== execute contract ====

export const mint = async (wallet, token_id, owner_address, nft_name, image_url, cost) => {
  
  await _exec(
    {
      mint: {
        owner: owner_address,
        token_num: token_id,
        extension: {
          name: nft_name,
          image: image_url,
        },
      }
    }, cost)(wallet);
}