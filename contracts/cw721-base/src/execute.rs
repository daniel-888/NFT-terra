use serde::de::DeserializeOwned;
use serde::Serialize;

use cosmwasm_std::{
    Addr, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128,
};

use cw2::set_contract_version;
use cw721::{
    ContractInfoResponse, CustomMsg, Cw721Execute, Cw721ReceiveMsg, Expiration, NumTokensResponse,
};

use crate::constants::*;
use crate::error::ContractError;
use crate::msg::*;
use crate::state::*;

impl<'a, T, C> Cw721Contract<'a, T, C>
where
    T: Serialize + DeserializeOwned + Clone + Default,
    C: CustomMsg,
{
    pub fn instantiate(
        &self,
        deps: DepsMut,
        env: Env,
        _info: MessageInfo,
        msg: InstantiateMsg,
    ) -> StdResult<Response<C>> {
        set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

        let info = ContractInfoResponse {
            name: msg.name,
            symbol: msg.symbol,
        };
        self.contract_info.save(deps.storage, &info)?;
        let minter = deps.api.addr_validate(&msg.minter)?;

        self.minter.save(deps.storage, &minter)?;
        self.time_deployed.save(deps.storage, &env.block.time)?;
        Ok(Response::default())
    }

    pub fn _execute(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: ExecuteMsg<T>,
    ) -> Result<Response<C>, ContractError> {
        match msg {
            ExecuteMsg::Mint(msg) => self.mint(deps, env, info, msg),
            ExecuteMsg::Approve {
                spender,
                token_id,
                expires,
            } => self.approve(deps, env, info, spender, token_id, expires),
            ExecuteMsg::Revoke { spender, token_id } => {
                self.revoke(deps, env, info, spender, token_id)
            }
            ExecuteMsg::ApproveAll { operator, expires } => {
                self.approve_all(deps, env, info, operator, expires)
            }
            ExecuteMsg::RevokeAll { operator } => self.revoke_all(deps, env, info, operator),
            ExecuteMsg::TransferNft {
                recipient,
                token_id,
            } => self.transfer_nft(deps, env, info, recipient, token_id),
            ExecuteMsg::SendNft {
                contract,
                token_id,
                msg,
            } => self.send_nft(deps, env, info, contract, token_id, msg),
            _ => Err(ContractError::CannotExecuteMsg {}),
        }
    }
}

// TODO pull this into some sort of trait extension??
impl<'a, T, C> Cw721Contract<'a, T, C>
where
    T: Serialize + DeserializeOwned + Clone + Default,
    C: CustomMsg,
{
    pub fn mint<'b>(
        &'b self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: MintMsg<T>,
    ) -> Result<Response<C>, ContractError> {
        let mut fund = Coin {
            amount: Uint128::from(0u128),
            denom: String::from("luna"),
        };

        for coin in info.clone().funds {
            if coin.denom == "uluna" {
                fund = Coin {
                    amount: fund.amount + coin.amount,
                    denom: coin.denom,
                };
            }
        }
        let token_minted: NumTokensResponse = deps
            .querier
            .query_wasm_smart(env.contract.address.clone(), &QueryMsg::NumTokens {})?;

        let balance_response: GetBalanceResponse = deps.querier.query_wasm_smart(
            env.contract.address.clone(),
            &QueryMsg::GetBalance {
                owner: info.sender.clone().into_string(),
            },
        )?;
        let balance = balance_response.balance;

        let _minter = self.minter.load(deps.storage)?;

        let get_whitelist: IsOnWhitelistResponse = deps.querier.query_wasm_smart(
            env.contract.address.clone(),
            &QueryMsg::IsOnWhitelist {
                member: info.sender.to_string(),
            },
        )?;

        let get_presale: IsOnPresaleResponse = deps
            .querier
            .query_wasm_smart(env.contract.address.clone(), &QueryMsg::IsOnPresale {})?;

        let can_mint = if get_presale.flag
            && token_minted.count < 2
            && balance < 1
            && get_whitelist.is_on_whitelist
        {
            match fund.amount.u128() {
                130000 => msg.token_num == String::from("a"),
                125000 => msg.token_num == String::from("b"),
                _ => false,
            }
        } else if !get_presale.flag && balance < 2 && token_minted.count < 4 {
            match fund.amount.u128() {
                150000 => msg.token_num == String::from("a"),
                145000 => msg.token_num == String::from("b"),
                140000 => msg.token_num == String::from("c"),
                135000 => msg.token_num == String::from("d"),
                130000 => msg.token_num == String::from("e"),
                _ => false,
            }
        } else {
            false
        };

        if !can_mint {
            if get_presale.flag && token_minted.count >= 2 {
                return Err(ContractError::PresaleLimitExceeded {});
            } else if get_presale.flag && balance >= 1 {
                return Err(ContractError::WalletLimitExceeded {});
            } else if get_presale.flag && !get_whitelist.is_on_whitelist {
                return Err(ContractError::NotWhitelist {});
            } else if get_presale.flag {
                return Err(ContractError::FundMismatch {});
            } else if !get_presale.flag && token_minted.count >= 4 {
                return Err(ContractError::SoldOut {});
            } else if !get_presale.flag && balance >= 2 {
                return Err(ContractError::WalletLimitExceeded {});
            } else if !get_presale.flag {
                return Err(ContractError::FundMismatch {});
            } else {
                return Err(ContractError::Unauthorized {});
            }
        };

        // if info.sender != minter {
        //     return Err(ContractError::Unauthorized {});
        // }

        let token_id: &str = &(token_minted.count + 1).to_string()[..];

        let extension_response: GetExtensionResponse<T> = deps.querier.query_wasm_smart(
            env.contract.address.clone(),
            &QueryMsg::GetExtension {
                token_id: String::from(token_id),
            },
        )?;
        // create the token
        let token = TokenInfo {
            owner: deps.api.addr_validate(&msg.owner)?,
            approvals: vec![],
            token_uri: Some(format!("{}{}.json", BASE_URI, token_id)),
            extension: extension_response.extension,
        };

        self.tokens
            .update(deps.storage, token_id, |old| match old {
                Some(pre_token) => match pre_token.owner == "not_yet_set" {
                    false => Err(ContractError::Claimed {}),
                    true => Ok(token),
                },
                None => Err(ContractError::CannotGetExtension {}),
            })?;

        self.wallet_balance.save(
            deps.storage,
            &Addr::unchecked(msg.owner.clone()),
            &(balance + 1),
        )?;
        self.increment_tokens(deps.storage)?;

        Ok(Response::new()
            .add_attribute("action", "mint")
            .add_attribute("minter", info.sender)
            .add_attribute("owner", msg.owner)
            .add_attribute("token_id", token_id))
    }
}

impl<'a, T, C> Cw721Execute<T, C> for Cw721Contract<'a, T, C>
where
    T: Serialize + DeserializeOwned + Clone + Default,
    C: CustomMsg,
{
    type Err = ContractError;

    fn transfer_nft(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        recipient: String,
        token_id: String,
    ) -> Result<Response<C>, ContractError> {
        self._transfer_nft(deps, &env, &info, &recipient, &token_id)?;

        Ok(Response::new()
            .add_attribute("action", "transfer_nft")
            .add_attribute("sender", info.sender)
            .add_attribute("recipient", recipient)
            .add_attribute("token_id", token_id))
    }

    fn send_nft(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        contract: String,
        token_id: String,
        msg: Binary,
    ) -> Result<Response<C>, ContractError> {
        // Transfer token
        self._transfer_nft(deps, &env, &info, &contract, &token_id)?;

        let send = Cw721ReceiveMsg {
            sender: info.sender.to_string(),
            token_id: token_id.clone(),
            msg,
        };

        // Send message
        Ok(Response::new()
            .add_message(send.into_cosmos_msg(contract.clone())?)
            .add_attribute("action", "send_nft")
            .add_attribute("sender", info.sender)
            .add_attribute("recipient", contract)
            .add_attribute("token_id", token_id))
    }

    fn approve(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        spender: String,
        token_id: String,
        expires: Option<Expiration>,
    ) -> Result<Response<C>, ContractError> {
        self._update_approvals(deps, &env, &info, &spender, &token_id, true, expires)?;

        Ok(Response::new()
            .add_attribute("action", "approve")
            .add_attribute("sender", info.sender)
            .add_attribute("spender", spender)
            .add_attribute("token_id", token_id))
    }

    fn revoke(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        spender: String,
        token_id: String,
    ) -> Result<Response<C>, ContractError> {
        self._update_approvals(deps, &env, &info, &spender, &token_id, false, None)?;

        Ok(Response::new()
            .add_attribute("action", "revoke")
            .add_attribute("sender", info.sender)
            .add_attribute("spender", spender)
            .add_attribute("token_id", token_id))
    }

    fn approve_all(
        &self,
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        operator: String,
        expires: Option<Expiration>,
    ) -> Result<Response<C>, ContractError> {
        // reject expired data as invalid
        let expires = expires.unwrap_or_default();
        if expires.is_expired(&env.block) {
            return Err(ContractError::Expired {});
        }

        // set the operator for us
        let operator_addr = deps.api.addr_validate(&operator)?;
        self.operators
            .save(deps.storage, (&info.sender, &operator_addr), &expires)?;

        Ok(Response::new()
            .add_attribute("action", "approve_all")
            .add_attribute("sender", info.sender)
            .add_attribute("operator", operator))
    }

    fn revoke_all(
        &self,
        deps: DepsMut,
        _env: Env,
        info: MessageInfo,
        operator: String,
    ) -> Result<Response<C>, ContractError> {
        let operator_addr = deps.api.addr_validate(&operator)?;
        self.operators
            .remove(deps.storage, (&info.sender, &operator_addr));

        Ok(Response::new()
            .add_attribute("action", "revoke_all")
            .add_attribute("sender", info.sender)
            .add_attribute("operator", operator))
    }
}

// helpers
impl<'a, T, C> Cw721Contract<'a, T, C>
where
    T: Serialize + DeserializeOwned + Clone + Default,
    C: CustomMsg,
{
    pub fn _transfer_nft(
        &self,
        deps: DepsMut,
        env: &Env,
        info: &MessageInfo,
        recipient: &str,
        token_id: &str,
    ) -> Result<TokenInfo<T>, ContractError> {
        let mut token = self.tokens.load(deps.storage, &token_id)?;
        // ensure we have permissions
        self.check_can_send(deps.as_ref(), env, info, &token)?;
        // set owner and remove existing approvals
        token.owner = deps.api.addr_validate(recipient)?;
        token.approvals = vec![];
        self.tokens.save(deps.storage, &token_id, &token)?;
        Ok(token)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn _update_approvals(
        &self,
        deps: DepsMut,
        env: &Env,
        info: &MessageInfo,
        spender: &str,
        token_id: &str,
        // if add == false, remove. if add == true, remove then set with this expiration
        add: bool,
        expires: Option<Expiration>,
    ) -> Result<TokenInfo<T>, ContractError> {
        let mut token = self.tokens.load(deps.storage, &token_id)?;
        // ensure we have permissions
        self.check_can_approve(deps.as_ref(), env, info, &token)?;

        // update the approval list (remove any for the same spender before adding)
        let spender_addr = deps.api.addr_validate(spender)?;
        token.approvals = token
            .approvals
            .into_iter()
            .filter(|apr| apr.spender != spender_addr)
            .collect();

        // only difference between approve and revoke
        if add {
            // reject expired data as invalid
            let expires = expires.unwrap_or_default();
            if expires.is_expired(&env.block) {
                return Err(ContractError::Expired {});
            }
            let approval = Approval {
                spender: spender_addr,
                expires,
            };
            token.approvals.push(approval);
        }

        self.tokens.save(deps.storage, &token_id, &token)?;

        Ok(token)
    }

    /// returns true iff the sender can execute approve or reject on the contract
    pub fn check_can_approve(
        &self,
        deps: Deps,
        env: &Env,
        info: &MessageInfo,
        token: &TokenInfo<T>,
    ) -> Result<(), ContractError> {
        // owner can approve
        if token.owner == info.sender {
            return Ok(());
        }
        // operator can approve
        let op = self
            .operators
            .may_load(deps.storage, (&token.owner, &info.sender))?;
        match op {
            Some(ex) => {
                if ex.is_expired(&env.block) {
                    Err(ContractError::Unauthorized {})
                } else {
                    Ok(())
                }
            }
            None => Err(ContractError::Unauthorized {}),
        }
    }

    /// returns true iff the sender can transfer ownership of the token
    pub fn check_can_send(
        &self,
        deps: Deps,
        env: &Env,
        info: &MessageInfo,
        token: &TokenInfo<T>,
    ) -> Result<(), ContractError> {
        // owner can send
        if token.owner == info.sender {
            return Ok(());
        }

        // any non-expired token approval can send
        if token
            .approvals
            .iter()
            .any(|apr| apr.spender == info.sender && !apr.is_expired(&env.block))
        {
            return Ok(());
        }

        // operator can send
        let op = self
            .operators
            .may_load(deps.storage, (&token.owner, &info.sender))?;
        match op {
            Some(ex) => {
                if ex.is_expired(&env.block) {
                    Err(ContractError::Unauthorized {})
                } else {
                    Ok(())
                }
            }
            None => Err(ContractError::Unauthorized {}),
        }
    }
}
