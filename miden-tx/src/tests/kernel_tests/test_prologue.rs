use alloc::collections::BTreeMap;

use miden_lib::{
    accounts::wallets::BasicWallet,
    errors::tx_kernel_errors::{
        ERR_ACCOUNT_SEED_DIGEST_MISMATCH,
        ERR_PROLOGUE_NEW_FUNGIBLE_FAUCET_RESERVED_SLOT_MUST_BE_EMPTY,
        ERR_PROLOGUE_NEW_NON_FUNGIBLE_FAUCET_RESERVED_SLOT_MUST_BE_VALID_EMPY_SMT,
    },
    transaction::{
        memory::{
            MemoryOffset, ACCT_DB_ROOT_PTR, ACCT_ID_PTR, BLK_HASH_PTR, BLOCK_METADATA_PTR,
            BLOCK_NUMBER_IDX, CHAIN_MMR_NUM_LEAVES_PTR, CHAIN_MMR_PEAKS_PTR, CHAIN_ROOT_PTR,
            INIT_ACCT_HASH_PTR, INIT_NONCE_PTR, INPUT_NOTES_COMMITMENT_PTR, INPUT_NOTE_ARGS_OFFSET,
            INPUT_NOTE_ASSETS_HASH_OFFSET, INPUT_NOTE_ASSETS_OFFSET, INPUT_NOTE_ID_OFFSET,
            INPUT_NOTE_INPUTS_HASH_OFFSET, INPUT_NOTE_METADATA_OFFSET,
            INPUT_NOTE_NUM_ASSETS_OFFSET, INPUT_NOTE_SCRIPT_ROOT_OFFSET, INPUT_NOTE_SECTION_OFFSET,
            INPUT_NOTE_SERIAL_NUM_OFFSET, KERNEL_ROOT_PTR, NATIVE_ACCT_CODE_COMMITMENT_PTR,
            NATIVE_ACCT_ID_AND_NONCE_PTR, NATIVE_ACCT_PROCEDURES_SECTION_PTR,
            NATIVE_ACCT_STORAGE_COMMITMENT_PTR, NATIVE_ACCT_STORAGE_SLOTS_SECTION_PTR,
            NATIVE_ACCT_VAULT_ROOT_PTR, NATIVE_NUM_ACCT_PROCEDURES_PTR,
            NATIVE_NUM_ACCT_STORAGE_SLOTS_PTR, NOTE_ROOT_PTR, NULLIFIER_DB_ROOT_PTR,
            PREV_BLOCK_HASH_PTR, PROOF_HASH_PTR, PROTOCOL_VERSION_IDX, TIMESTAMP_IDX, TX_HASH_PTR,
            TX_SCRIPT_ROOT_PTR,
        },
        TransactionKernel,
    },
};
use miden_objects::{
    accounts::{
        AccountBuilder, AccountComponent, AccountProcedureInfo, AccountStorage, AccountType,
        StorageSlot,
    },
    testing::{
        account_component::BASIC_WALLET_CODE,
        constants::FUNGIBLE_FAUCET_INITIAL_BALANCE,
        storage::{generate_account_seed, AccountSeedType},
    },
    transaction::{TransactionArgs, TransactionScript},
    Digest, FieldElement,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use vm_processor::{AdviceInputs, ONE};

use super::{Felt, Process, Word, ZERO};
use crate::{
    assert_execution_error,
    testing::{
        utils::input_note_data_ptr, MockHost, TransactionContext, TransactionContextBuilder,
    },
    tests::kernel_tests::read_root_mem_value,
};

#[test]
fn test_transaction_prologue() {
    let mut tx_context = TransactionContextBuilder::with_standard_account(ONE)
        .with_mock_notes_preserved()
        .build();

    let code = "
        use.kernel::prologue

        begin
            exec.prologue::prepare_transaction
        end
        ";

    let mock_tx_script_code = "
        begin
            push.1.2.3.4 dropw
        end
        ";

    let mock_tx_script_program = TransactionKernel::assembler()
        .with_debug_mode(true)
        .assemble_program(mock_tx_script_code)
        .unwrap();

    let tx_script = TransactionScript::new(mock_tx_script_program, vec![]);

    let note_args = [
        [Felt::new(91), Felt::new(91), Felt::new(91), Felt::new(91)],
        [Felt::new(92), Felt::new(92), Felt::new(92), Felt::new(92)],
    ];

    let note_args_map = BTreeMap::from([
        (tx_context.input_notes().get_note(0).note().id(), note_args[0]),
        (tx_context.input_notes().get_note(1).note().id(), note_args[1]),
    ]);

    let tx_args = TransactionArgs::new(
        Some(tx_script),
        Some(note_args_map),
        tx_context.tx_args().advice_inputs().clone().map,
    );

    tx_context.set_tx_args(tx_args);
    let process = tx_context.execute_code(code).unwrap();

    global_input_memory_assertions(&process, &tx_context);
    block_data_memory_assertions(&process, &tx_context);
    chain_mmr_memory_assertions(&process, &tx_context);
    account_data_memory_assertions(&process, &tx_context);
    input_notes_memory_assertions(&process, &tx_context, &note_args);
}

fn global_input_memory_assertions(process: &Process<MockHost>, inputs: &TransactionContext) {
    assert_eq!(
        read_root_mem_value(process, BLK_HASH_PTR),
        inputs.tx_inputs().block_header().hash().as_elements(),
        "The block hash should be stored at the BLK_HASH_PTR"
    );

    assert_eq!(
        read_root_mem_value(process, ACCT_ID_PTR)[0],
        inputs.account().id().into(),
        "The account ID should be stored at the ACCT_ID_PTR"
    );

    assert_eq!(
        read_root_mem_value(process, INIT_ACCT_HASH_PTR),
        inputs.account().hash().as_elements(),
        "The account commitment should be stored at the ACCT_HASH_PTR"
    );

    assert_eq!(
        read_root_mem_value(process, INPUT_NOTES_COMMITMENT_PTR),
        inputs.input_notes().commitment().as_elements(),
        "The nullifier commitment should be stored at the INPUT_NOTES_COMMITMENT_PTR"
    );

    assert_eq!(
        read_root_mem_value(process, INIT_NONCE_PTR)[0],
        inputs.account().nonce(),
        "The initial nonce should be stored at the INIT_NONCE_PTR"
    );

    assert_eq!(
        read_root_mem_value(process, TX_SCRIPT_ROOT_PTR),
        *inputs.tx_args().tx_script().as_ref().unwrap().hash(),
        "The transaction script root should be stored at the TX_SCRIPT_ROOT_PTR"
    );
}

fn block_data_memory_assertions(process: &Process<MockHost>, inputs: &TransactionContext) {
    assert_eq!(
        read_root_mem_value(process, BLK_HASH_PTR),
        inputs.tx_inputs().block_header().hash().as_elements(),
        "The block hash should be stored at the BLK_HASH_PTR"
    );

    assert_eq!(
        read_root_mem_value(process, PREV_BLOCK_HASH_PTR),
        inputs.tx_inputs().block_header().prev_hash().as_elements(),
        "The previous block hash should be stored at the PREV_BLK_HASH_PTR"
    );

    assert_eq!(
        read_root_mem_value(process, CHAIN_ROOT_PTR),
        inputs.tx_inputs().block_header().chain_root().as_elements(),
        "The chain root should be stored at the CHAIN_ROOT_PTR"
    );

    assert_eq!(
        read_root_mem_value(process, ACCT_DB_ROOT_PTR),
        inputs.tx_inputs().block_header().account_root().as_elements(),
        "The account db root should be stored at the ACCT_DB_ROOT_PRT"
    );

    assert_eq!(
        read_root_mem_value(process, NULLIFIER_DB_ROOT_PTR),
        inputs.tx_inputs().block_header().nullifier_root().as_elements(),
        "The nullifier db root should be stored at the NULLIFIER_DB_ROOT_PTR"
    );

    assert_eq!(
        read_root_mem_value(process, TX_HASH_PTR),
        inputs.tx_inputs().block_header().tx_hash().as_elements(),
        "The TX hash should be stored at the TX_HASH_PTR"
    );

    assert_eq!(
        read_root_mem_value(process, KERNEL_ROOT_PTR),
        inputs.tx_inputs().block_header().kernel_root().as_elements(),
        "The kernel root should be stored at the KERNEL_ROOT_PTR"
    );

    assert_eq!(
        read_root_mem_value(process, PROOF_HASH_PTR),
        inputs.tx_inputs().block_header().proof_hash().as_elements(),
        "The proof hash should be stored at the PROOF_HASH_PTR"
    );

    assert_eq!(
        read_root_mem_value(process, BLOCK_METADATA_PTR)[BLOCK_NUMBER_IDX],
        inputs.tx_inputs().block_header().block_num().into(),
        "The block number should be stored at BLOCK_METADATA_PTR[BLOCK_NUMBER_IDX]"
    );

    assert_eq!(
        read_root_mem_value(process, BLOCK_METADATA_PTR)[PROTOCOL_VERSION_IDX],
        inputs.tx_inputs().block_header().version().into(),
        "The protocol version should be stored at BLOCK_METADATA_PTR[PROTOCOL_VERSION_IDX]"
    );

    assert_eq!(
        read_root_mem_value(process, BLOCK_METADATA_PTR)[TIMESTAMP_IDX],
        inputs.tx_inputs().block_header().timestamp().into(),
        "The timestamp should be stored at BLOCK_METADATA_PTR[TIMESTAMP_IDX]"
    );

    assert_eq!(
        read_root_mem_value(process, NOTE_ROOT_PTR),
        inputs.tx_inputs().block_header().note_root().as_elements(),
        "The note root should be stored at the NOTE_ROOT_PTR"
    );
}

fn chain_mmr_memory_assertions(process: &Process<MockHost>, prepared_tx: &TransactionContext) {
    // update the chain MMR to point to the block against which this transaction is being executed
    let mut chain_mmr = prepared_tx.tx_inputs().block_chain().clone();
    chain_mmr.add_block(*prepared_tx.tx_inputs().block_header(), true);

    assert_eq!(
        read_root_mem_value(process, CHAIN_MMR_NUM_LEAVES_PTR)[0],
        Felt::new(chain_mmr.chain_length() as u64),
        "The number of leaves should be stored at the CHAIN_MMR_NUM_LEAVES_PTR"
    );

    for (i, peak) in chain_mmr.peaks().peaks().iter().enumerate() {
        // The peaks should be stored at the CHAIN_MMR_PEAKS_PTR
        let i: u32 = i.try_into().expect(
            "Number of peaks is log2(number_of_leaves), this value won't be larger than 2**32",
        );
        assert_eq!(read_root_mem_value(process, CHAIN_MMR_PEAKS_PTR + i), Word::from(peak));
    }
}

fn account_data_memory_assertions(process: &Process<MockHost>, inputs: &TransactionContext) {
    assert_eq!(
        read_root_mem_value(process, NATIVE_ACCT_ID_AND_NONCE_PTR),
        [inputs.account().id().into(), ZERO, ZERO, inputs.account().nonce()],
        "The account id should be stored at NATIVE_ACCT_ID_AND_NONCE_PTR[0]"
    );

    assert_eq!(
        read_root_mem_value(process, NATIVE_ACCT_VAULT_ROOT_PTR),
        inputs.account().vault().commitment().as_elements(),
        "The account vault root commitment should be stored at NATIVE_ACCT_VAULT_ROOT_PTR"
    );

    assert_eq!(
        read_root_mem_value(process, NATIVE_ACCT_STORAGE_COMMITMENT_PTR),
        Word::from(inputs.account().storage().commitment()),
        "The account storage commitment should be stored at NATIVE_ACCT_STORAGE_COMMITMENT_PTR"
    );

    assert_eq!(
        read_root_mem_value(process, NATIVE_ACCT_CODE_COMMITMENT_PTR),
        inputs.account().code().commitment().as_elements(),
        "account code commitment should be stored at NATIVE_ACCT_CODE_COMMITMENT_PTR"
    );

    assert_eq!(
        read_root_mem_value(process, NATIVE_NUM_ACCT_STORAGE_SLOTS_PTR),
        [
            u16::try_from(inputs.account().storage().slots().len()).unwrap().into(),
            ZERO,
            ZERO,
            ZERO
        ],
        "The number of initialised storage slots should be stored at NATIVE_NUM_ACCT_STORAGE_SLOTS_PTR"
    );

    for (i, elements) in inputs
        .account()
        .storage()
        .as_elements()
        .chunks(StorageSlot::NUM_ELEMENTS_PER_STORAGE_SLOT / 2)
        .enumerate()
    {
        assert_eq!(
            read_root_mem_value(process, NATIVE_ACCT_STORAGE_SLOTS_SECTION_PTR + i as u32),
            Word::try_from(elements).unwrap(),
            "The account storage slots should be stored starting at NATIVE_ACCT_STORAGE_SLOTS_SECTION_PTR"
        )
    }

    assert_eq!(
        read_root_mem_value(process, NATIVE_NUM_ACCT_PROCEDURES_PTR),
        [
            u16::try_from(inputs.account().code().procedures().len()).unwrap().into(),
            ZERO,
            ZERO,
            ZERO
        ],
        "The number of procedures should be stored at NATIVE_NUM_ACCT_PROCEDURES_PTR"
    );

    for (i, elements) in inputs
        .account()
        .code()
        .as_elements()
        .chunks(AccountProcedureInfo::NUM_ELEMENTS_PER_PROC / 2)
        .enumerate()
    {
        assert_eq!(
            read_root_mem_value(process, NATIVE_ACCT_PROCEDURES_SECTION_PTR + i as u32),
            Word::try_from(elements).unwrap(),
            "The account procedures and storage offsets should be stored starting at NATIVE_ACCT_PROCEDURES_SECTION_PTR"
        );
    }
}

fn input_notes_memory_assertions(
    process: &Process<MockHost>,
    inputs: &TransactionContext,
    note_args: &[[Felt; 4]],
) {
    assert_eq!(
        read_root_mem_value(process, INPUT_NOTE_SECTION_OFFSET),
        [Felt::new(inputs.input_notes().num_notes() as u64), ZERO, ZERO, ZERO],
        "number of input notes should be stored at the INPUT_NOTES_OFFSET"
    );

    for (input_note, note_idx) in inputs.input_notes().iter().zip(0_u32..) {
        let note = input_note.note();

        assert_eq!(
            read_root_mem_value(process, INPUT_NOTE_SECTION_OFFSET + 1 + note_idx),
            note.nullifier().as_elements(),
            "note nullifier should be computer and stored at the correct offset"
        );

        assert_eq!(
            read_note_element(process, note_idx, INPUT_NOTE_ID_OFFSET),
            note.id().as_elements(),
            "ID hash should be computed and stored at the correct offset"
        );

        assert_eq!(
            read_note_element(process, note_idx, INPUT_NOTE_SERIAL_NUM_OFFSET),
            note.serial_num(),
            "note serial num should be stored at the correct offset"
        );

        assert_eq!(
            read_note_element(process, note_idx, INPUT_NOTE_SCRIPT_ROOT_OFFSET),
            note.script().hash().as_elements(),
            "note script hash should be stored at the correct offset"
        );

        assert_eq!(
            read_note_element(process, note_idx, INPUT_NOTE_INPUTS_HASH_OFFSET),
            note.inputs().commitment().as_elements(),
            "note input hash should be stored at the correct offset"
        );

        assert_eq!(
            read_note_element(process, note_idx, INPUT_NOTE_ASSETS_HASH_OFFSET),
            note.assets().commitment().as_elements(),
            "note asset hash should be stored at the correct offset"
        );

        assert_eq!(
            read_note_element(process, note_idx, INPUT_NOTE_METADATA_OFFSET),
            Word::from(note.metadata()),
            "note metadata should be stored at the correct offset"
        );

        assert_eq!(
            read_note_element(process, note_idx, INPUT_NOTE_ARGS_OFFSET),
            Word::from(note_args[note_idx as usize]),
            "note args should be stored at the correct offset"
        );

        assert_eq!(
            read_note_element(process, note_idx, INPUT_NOTE_NUM_ASSETS_OFFSET),
            [Felt::from(note.assets().num_assets() as u32), ZERO, ZERO, ZERO],
            "number of assets should be stored at the correct offset"
        );

        for (asset, asset_idx) in note.assets().iter().cloned().zip(0_u32..) {
            let word: Word = asset.into();
            assert_eq!(
                read_note_element(process, note_idx, INPUT_NOTE_ASSETS_OFFSET + asset_idx),
                word,
                "assets should be stored at (INPUT_NOTES_OFFSET + (note_index + 1) * 1024 + 7..)"
            );
        }
    }
}

#[cfg_attr(not(feature = "testing"), ignore)]
#[test]
pub fn test_prologue_create_account() {
    let (account, seed) = AccountBuilder::new()
        .init_seed(ChaCha20Rng::from_entropy().gen())
        .with_component(
            AccountComponent::compile(
                BASIC_WALLET_CODE,
                TransactionKernel::testing_assembler(),
                AccountStorage::mock_storage_slots(),
            )
            .unwrap()
            .with_supported_type(AccountType::RegularAccountUpdatableCode),
        )
        .build_testing()
        .unwrap();
    let tx_context = TransactionContextBuilder::new(account).account_seed(seed).build();

    let code = "
    use.kernel::prologue

    begin
        exec.prologue::prepare_transaction
    end
    ";

    tx_context.execute_code(code).unwrap();
}

#[cfg_attr(not(feature = "testing"), ignore)]
#[test]
pub fn test_prologue_create_account_valid_fungible_faucet_reserved_slot() {
    let (acct_id, account_seed) = generate_account_seed(
        AccountSeedType::FungibleFaucetValidInitialBalance,
        TransactionKernel::assembler().with_debug_mode(true),
    );

    let tx_context =
        TransactionContextBuilder::with_fungible_faucet(acct_id.into(), Felt::ZERO, ZERO)
            .account_seed(Some(account_seed))
            .build();

    let code = "
    use.kernel::prologue

    begin
        exec.prologue::prepare_transaction
    end
    ";

    tx_context.execute_code(code).unwrap();
}

#[cfg_attr(not(feature = "testing"), ignore)]
#[test]
pub fn test_prologue_create_account_invalid_fungible_faucet_reserved_slot() {
    let (acct_id, account_seed) = generate_account_seed(
        AccountSeedType::FungibleFaucetInvalidInitialBalance,
        TransactionKernel::assembler(),
    );

    let tx_context = TransactionContextBuilder::with_fungible_faucet(
        acct_id.into(),
        Felt::ZERO,
        Felt::new(FUNGIBLE_FAUCET_INITIAL_BALANCE),
    )
    .account_seed(Some(account_seed))
    .build();

    let code = "
    use.kernel::prologue

    begin
        exec.prologue::prepare_transaction
    end
    ";

    let process = tx_context.execute_code(code);
    assert_execution_error!(process, ERR_PROLOGUE_NEW_FUNGIBLE_FAUCET_RESERVED_SLOT_MUST_BE_EMPTY);
}

#[cfg_attr(not(feature = "testing"), ignore)]
#[test]
pub fn test_prologue_create_account_valid_non_fungible_faucet_reserved_slot() {
    let (acct_id, account_seed) = generate_account_seed(
        AccountSeedType::NonFungibleFaucetValidReservedSlot,
        TransactionKernel::assembler().with_debug_mode(true),
    );

    let tx_context =
        TransactionContextBuilder::with_non_fungible_faucet(acct_id.into(), Felt::ZERO, true)
            .account_seed(Some(account_seed))
            .build();

    let code = "
    use.kernel::prologue

    begin
        exec.prologue::prepare_transaction
    end
    ";

    let process = tx_context.execute_code(code);

    assert!(process.is_ok())
}

#[cfg_attr(not(feature = "testing"), ignore)]
#[test]
pub fn test_prologue_create_account_invalid_non_fungible_faucet_reserved_slot() {
    let (acct_id, account_seed) = generate_account_seed(
        AccountSeedType::NonFungibleFaucetInvalidReservedSlot,
        TransactionKernel::assembler().with_debug_mode(true),
    );

    let tx_context =
        TransactionContextBuilder::with_non_fungible_faucet(acct_id.into(), Felt::ZERO, false)
            .account_seed(Some(account_seed))
            .build();

    let code = "
    use.kernel::prologue

    begin
        exec.prologue::prepare_transaction
    end
    ";

    let process = tx_context.execute_code(code);

    assert_execution_error!(
        process,
        ERR_PROLOGUE_NEW_NON_FUNGIBLE_FAUCET_RESERVED_SLOT_MUST_BE_VALID_EMPY_SMT
    );
}

#[cfg_attr(not(feature = "testing"), ignore)]
#[test]
pub fn test_prologue_create_account_invalid_seed() {
    let (acct, account_seed) = AccountBuilder::new()
        .init_seed(ChaCha20Rng::from_entropy().gen())
        .account_type(AccountType::RegularAccountUpdatableCode)
        .with_component(BasicWallet)
        .build_testing()
        .unwrap();

    let code = "
    use.kernel::prologue

    begin
        exec.prologue::prepare_transaction
    end
    ";

    // override the seed with an invalid seed to ensure the kernel fails
    let account_seed_key = [acct.id().into(), ZERO, ZERO, ZERO];
    let adv_inputs =
        AdviceInputs::default().with_map([(Digest::from(account_seed_key), vec![ZERO; 4])]);

    let tx_context = TransactionContextBuilder::new(acct)
        .account_seed(account_seed)
        .advice_inputs(adv_inputs)
        .build();
    let process = tx_context.execute_code(code);

    assert_execution_error!(process, ERR_ACCOUNT_SEED_DIGEST_MISMATCH)
}

#[test]
fn test_get_blk_version() {
    let tx_context = TransactionContextBuilder::with_standard_account(ONE).build();
    let code = "
    use.kernel::memory
    use.kernel::prologue

    begin
        exec.prologue::prepare_transaction
        exec.memory::get_blk_version

        # truncate the stack 
        swap drop
    end
    ";

    let process = tx_context.execute_code(code).unwrap();

    assert_eq!(process.stack.get(0), tx_context.tx_inputs().block_header().version().into());
}

#[test]
fn test_get_blk_timestamp() {
    let tx_context = TransactionContextBuilder::with_standard_account(ONE).build();
    let code = "
    use.kernel::memory
    use.kernel::prologue

    begin
        exec.prologue::prepare_transaction
        exec.memory::get_blk_timestamp

        # truncate the stack 
        swap drop
    end
    ";

    let process = tx_context.execute_code(code).unwrap();

    assert_eq!(process.stack.get(0), tx_context.tx_inputs().block_header().timestamp().into());
}

// HELPER FUNCTIONS
// ================================================================================================

fn read_note_element(process: &Process<MockHost>, note_idx: u32, offset: MemoryOffset) -> Word {
    read_root_mem_value(process, input_note_data_ptr(note_idx) + offset)
}
