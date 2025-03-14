use color_eyre::eyre::Context;
use inquire::CustomType;
use serde_json::{json, Value};

use crate::common::CallResultExt;
use crate::common::JsonRpcClientExt;

use super::super::view_ft_balance::get_ft_balance;

#[derive(Debug, Clone, interactive_clap::InteractiveClap)]
#[interactive_clap(input_context = super::amount_ft::AmountFtContext)]
#[interactive_clap(output_context = PrepaidGasContext)]
pub struct PrepaidGas {
    #[interactive_clap(skip_default_input_arg)]
    /// Enter gas for function call:
    gas: crate::common::NearGas,
    #[interactive_clap(named_arg)]
    /// Enter deposit for a function call
    attached_deposit: Deposit,
}

#[derive(Debug, Clone)]
pub struct PrepaidGasContext {
    global_context: crate::GlobalContext,
    signer_account_id: near_primitives::types::AccountId,
    ft_contract_account_id: near_primitives::types::AccountId,
    receiver_account_id: near_primitives::types::AccountId,
    ft_transfer_amount: crate::types::ft_properties::FungibleTokenTransferAmount,
    gas: crate::common::NearGas,
    memo: Option<String>,
}

impl PrepaidGasContext {
    pub fn from_previous_context(
        previous_context: super::amount_ft::AmountFtContext,
        scope: &<PrepaidGas as interactive_clap::ToInteractiveClapContextScope>::InteractiveClapContextScope,
    ) -> color_eyre::eyre::Result<Self> {
        Ok(Self {
            global_context: previous_context.global_context,
            signer_account_id: previous_context.signer_account_id,
            ft_contract_account_id: previous_context.ft_contract_account_id,
            receiver_account_id: previous_context.receiver_account_id,
            ft_transfer_amount: previous_context.ft_transfer_amount,
            gas: scope.gas,
            memo: previous_context.memo,
        })
    }
}

impl PrepaidGas {
    fn input_gas(
        _context: &super::amount_ft::AmountFtContext,
    ) -> color_eyre::eyre::Result<Option<crate::common::NearGas>> {
        eprintln!();
        Ok(Some(
            CustomType::new("Enter gas for function call:")
                .with_starting_input("100 TeraGas")
                .with_validator(move |gas: &crate::common::NearGas| {
                    if gas > &near_gas::NearGas::from_tgas(300) {
                        Ok(inquire::validator::Validation::Invalid(
                            inquire::validator::ErrorMessage::Custom(
                                "You need to enter a value of no more than 300 TeraGas".to_string(),
                            ),
                        ))
                    } else {
                        Ok(inquire::validator::Validation::Valid)
                    }
                })
                .prompt()?,
        ))
    }
}

#[derive(Debug, Clone, interactive_clap::InteractiveClap)]
#[interactive_clap(input_context = PrepaidGasContext)]
#[interactive_clap(output_context = DepositContext)]
pub struct Deposit {
    #[interactive_clap(skip_default_input_arg)]
    /// Enter deposit for a function call:
    deposit: crate::types::near_token::NearToken,
    #[interactive_clap(named_arg)]
    /// Select network
    network_config: crate::network_for_transaction::NetworkForTransactionArgs,
}

#[derive(Clone)]
pub struct DepositContext(crate::commands::ActionContext);

impl DepositContext {
    pub fn from_previous_context(
        previous_context: PrepaidGasContext,
        scope: &<Deposit as interactive_clap::ToInteractiveClapContextScope>::InteractiveClapContextScope,
    ) -> color_eyre::eyre::Result<Self> {
        let get_prepopulated_transaction_after_getting_network_callback: crate::commands::GetPrepopulatedTransactionAfterGettingNetworkCallback =
            std::sync::Arc::new({
                let signer_account_id = previous_context.signer_account_id.clone();
                let ft_contract_account_id = previous_context.ft_contract_account_id.clone();
                let receiver_account_id = previous_context.receiver_account_id.clone();
                let deposit = scope.deposit;
                let ft_transfer_amount = previous_context.ft_transfer_amount.clone();
                let memo = previous_context.memo.clone();

                move |network_config| {
                    let amount_ft = get_amount_ft(
                        &ft_transfer_amount,
                        network_config,
                        &signer_account_id,
                        &ft_contract_account_id
                    )?;

                    get_prepopulated_transaction(
                        network_config,
                        &ft_contract_account_id,
                        &receiver_account_id,
                        &signer_account_id,
                        &amount_ft,
                        &memo,
                        &deposit,
                        &previous_context.gas
                    )
                }
            });

        let on_after_sending_transaction_callback: crate::transaction_signature_options::OnAfterSendingTransactionCallback = std::sync::Arc::new({
            let signer_account_id = previous_context.signer_account_id.clone();
            let ft_contract_account_id = previous_context.ft_contract_account_id.clone();
            let receiver_account_id = previous_context.receiver_account_id.clone();
            let ft_transfer_amount = previous_context.ft_transfer_amount.clone();

            move |outcome_view, network_config| {
                let amount_ft = get_amount_ft(
                    &ft_transfer_amount,
                    network_config,
                    &signer_account_id,
                    &ft_contract_account_id
                )?;

                if let near_primitives::views::FinalExecutionStatus::SuccessValue(_) = outcome_view.status {
                    eprintln!(
                        "<{signer_account_id}> has successfully transferred {amount_ft} (FT-contract: {ft_contract_account_id}) to <{receiver_account_id}>.",
                    );
                }
                Ok(())
            }
        });

        Ok(Self(crate::commands::ActionContext {
            global_context: previous_context.global_context,
            interacting_with_account_ids: vec![
                previous_context.ft_contract_account_id,
                previous_context.signer_account_id,
                previous_context.receiver_account_id,
            ],
            get_prepopulated_transaction_after_getting_network_callback,
            on_before_signing_callback: std::sync::Arc::new(
                |_prepolulated_unsinged_transaction, _network_config| Ok(()),
            ),
            on_before_sending_transaction_callback: std::sync::Arc::new(
                |_signed_transaction, _network_config| Ok(String::new()),
            ),
            on_after_sending_transaction_callback,
        }))
    }
}

impl From<DepositContext> for crate::commands::ActionContext {
    fn from(item: DepositContext) -> Self {
        item.0
    }
}

impl Deposit {
    fn input_deposit(
        _context: &PrepaidGasContext,
    ) -> color_eyre::eyre::Result<Option<crate::types::near_token::NearToken>> {
        eprintln!();
        Ok(Some(
            CustomType::new("Enter deposit for a function call (example: 10 NEAR or 0.5 near or 10000 yoctonear):")
                .with_starting_input("1 yoctoNEAR")
                .prompt()?
        ))
    }
}

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(
    name = "Creating a pre-populated transaction for signature ...",
    skip_all
)]
fn get_prepopulated_transaction(
    network_config: &crate::config::NetworkConfig,
    ft_contract_account_id: &near_primitives::types::AccountId,
    receiver_account_id: &near_primitives::types::AccountId,
    signer_id: &near_primitives::types::AccountId,
    amount_ft: &crate::types::ft_properties::FungibleToken,
    memo: &Option<String>,
    deposit: &crate::types::near_token::NearToken,
    gas: &crate::common::NearGas,
) -> color_eyre::eyre::Result<crate::commands::PrepopulatedTransaction> {
    let mut transfer_args = serde_json::Map::new();
    transfer_args.insert(
        "receiver_id".to_string(),
        json!(receiver_account_id.to_string()),
    );
    transfer_args.insert("amount".to_string(), json!(amount_ft.amount().to_string()));
    if let Some(m) = memo {
        if !m.trim().is_empty() {
            transfer_args.insert("memo".to_string(), json!(m));
        }
    }
    let args_ft_transfer = serde_json::to_vec(&transfer_args)?;
    let action_ft_transfer = near_primitives::transaction::Action::FunctionCall(Box::new(
        near_primitives::transaction::FunctionCallAction {
            method_name: "ft_transfer".to_string(),
            args: args_ft_transfer,
            gas: gas.as_gas(),
            deposit: deposit.as_yoctonear(),
        },
    ));

    let args = serde_json::to_vec(&json!({"account_id": receiver_account_id.to_string()}))?;

    let call_result = network_config
        .json_rpc_client()
        .blocking_call_view_function(
            ft_contract_account_id,
            "storage_balance_of",
            args.clone(),
            near_primitives::types::Finality::Final.into(),
        )
        .wrap_err_with(||{
            format!("Failed to fetch query for view method: 'storage_balance_of' (contract <{}> on network <{}>)",
                ft_contract_account_id,
                network_config.network_name
            )
        })?;

    if call_result.parse_result_from_json::<Value>()?.is_null() {
        let action_storage_deposit = near_primitives::transaction::Action::FunctionCall(Box::new(
            near_primitives::transaction::FunctionCallAction {
                method_name: "storage_deposit".to_string(),
                args,
                gas: gas.as_gas(),
                deposit: near_token::NearToken::from_millinear(100).as_yoctonear(),
            },
        ));
        return Ok(crate::commands::PrepopulatedTransaction {
            signer_id: signer_id.clone(),
            receiver_id: ft_contract_account_id.clone(),
            actions: vec![action_storage_deposit, action_ft_transfer.clone()],
        });
    }

    Ok(crate::commands::PrepopulatedTransaction {
        signer_id: signer_id.clone(),
        receiver_id: ft_contract_account_id.clone(),
        actions: vec![action_ft_transfer.clone()],
    })
}

fn get_amount_ft(
    ft_transfer_amount: &crate::types::ft_properties::FungibleTokenTransferAmount,
    network_config: &crate::config::NetworkConfig,
    signer_account_id: &near_primitives::types::AccountId,
    ft_contract_account_id: &near_primitives::types::AccountId,
) -> color_eyre::eyre::Result<crate::types::ft_properties::FungibleToken> {
    match ft_transfer_amount {
        crate::types::ft_properties::FungibleTokenTransferAmount::ExactAmount(ft) => Ok(ft.clone()),
        crate::types::ft_properties::FungibleTokenTransferAmount::MaxAmount => {
            let function_args =
                serde_json::to_vec(&json!({"account_id": signer_account_id.to_string()}))?;
            let amount = get_ft_balance(
                network_config,
                ft_contract_account_id,
                function_args,
                near_primitives::types::Finality::Final.into(),
            )?
            .parse_result_from_json::<String>()?;
            let crate::types::ft_properties::FtMetadata { decimals, symbol } =
                crate::types::ft_properties::params_ft_metadata(
                    ft_contract_account_id.clone(),
                    network_config,
                    near_primitives::types::Finality::Final.into(),
                )?;
            Ok(crate::types::ft_properties::FungibleToken::from_params_ft(
                amount.parse::<u128>()?,
                decimals,
                symbol,
            ))
        }
    }
}
