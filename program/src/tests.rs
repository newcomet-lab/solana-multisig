use crate::instruction::ApproveInstruction;
use crate::instruction::MultiSigInstruction;
use crate::instruction::ProposeInstruction;
use crate::instruction::{InitInstruction, ProtectedAccountConfig};
use crate::processor::pda_tag;
use crate::state::{
    AccountType, GroupData, GroupMember, ProposalConfig, ProposalData, ProposedAccountMeta,
    ProposedInstruction,
};

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::system_instruction;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_program_test::ProgramTest;
use solana_sdk::system_program::ID as SYSTEM_PROGRAM_ID;
use solana_sdk::{
    account::Account, signature::Keypair, signature::Signer, transaction::Transaction,
};

fn do_init(
    program_id: Pubkey,
    threshold: u32,
    members: impl IntoIterator<Item = (Pubkey, u32)>,
    payer: Pubkey,
    protected_account_config: Option<ProtectedAccountConfig>,
) -> (Transaction, Pubkey) {
    let group_data = GroupData {
        members: members
            .into_iter()
            .map(|(key, weight)| GroupMember {
                public_key: key,
                weight,
            })
            .collect(),
        threshold,
    };

    let serialized = group_data.try_to_vec().unwrap();
    let group_hash = hash(&serialized);
    let (group_account, _) =
        Pubkey::find_program_address(&[pda_tag::GROUP, group_hash.as_ref()], &program_id);

    let do_init_protected = protected_account_config.is_some();
    let command = MultiSigInstruction::Init(InitInstruction {
        lamports: 1000,
        group_data,
        protected_account_config,
    });

    let mut account_metas = vec![
        AccountMeta::new_readonly(payer, true),
        AccountMeta::new(group_account, false),
        AccountMeta::new(SYSTEM_PROGRAM_ID, false),
    ];
    if do_init_protected {
        let (protected_addr, _) = Pubkey::find_program_address(
            &[pda_tag::PROTECTED, group_account.as_ref()],
            &program_id,
        );

        account_metas.push(AccountMeta::new(protected_addr, false));
    }

    let transaction = Transaction::new_with_payer(
        &[Instruction::new_with_borsh(
            program_id,
            &command,
            account_metas,
        )],
        Some(&payer),
    );
    (transaction, group_account)
}

#[tokio::test]
async fn init_works() {
    let program_id = Pubkey::new_unique();
    let alice_key = Pubkey::new_unique();
    let bob_key = Pubkey::new_unique();
    let chris_key = Pubkey::new_unique();

    let mut program_test = ProgramTest::new(env!("CARGO_PKG_NAME"), program_id, None);
    program_test.prefer_bpf(true);
    program_test.add_program(env!("CARGO_PKG_NAME"), program_id, None);

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
    let users = vec![(alice_key, 2), (bob_key, 1), (chris_key, 1)];
    let threshold = 2;
    const PROTECTED_LAMPORTS: u64 = 1000;
    let init_protected = ProtectedAccountConfig {
        space: 0,
        owner: SYSTEM_PROGRAM_ID,
        lamports: PROTECTED_LAMPORTS,
    };

    let (mut transaction, group_account) = do_init(
        program_id,
        threshold,
        users.clone(),
        payer.pubkey(),
        Some(init_protected),
    );
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
    let data = banks_client
        .get_account(group_account)
        .await
        .unwrap()
        .unwrap()
        .data;
    assert_eq!(data[0], u8::from(AccountType::Group));
    let group_data = GroupData::try_from_slice(&data[1..]).unwrap();
    assert_eq!(group_data.threshold, threshold);
    assert!(group_data
        .members
        .into_iter()
        .zip(users.into_iter())
        .all(|(member, (key, weight))| { member.public_key == key && member.weight == weight }));

    let (protected_addr, _) =
        Pubkey::find_program_address(&[pda_tag::PROTECTED, group_account.as_ref()], &program_id);
    let protected_account = banks_client
        .get_account(protected_addr)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(protected_account.lamports, PROTECTED_LAMPORTS);
}

fn do_propose(
    program_id: Pubkey,
    signer: Pubkey,
    payer: Pubkey,
    group_account: Pubkey,
    proposed_instruction: Instruction,
) -> (Transaction, Pubkey) {
    let (protected_account, _) = Pubkey::find_program_address(
        &[pda_tag::PROTECTED, &group_account.to_bytes()[..]],
        &program_id,
    );

    let proposed_instruction = ProposedInstruction {
        accounts: proposed_instruction
            .accounts
            .into_iter()
            .map(|meta| ProposedAccountMeta {
                pubkey: meta.pubkey,
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            })
            .collect(),
        program_id: proposed_instruction.program_id,
        data: proposed_instruction.data,
    };
    let command = MultiSigInstruction::Propose(ProposeInstruction {
        instructions: vec![proposed_instruction.clone()],
        lamports: 100,
        salt: 1
    });
    let proposal_config = ProposalConfig {
        group: group_account,
        instructions: vec![proposed_instruction.clone()],
        author: signer,
        salt: 1
    };
    let serialized = proposal_config.try_to_vec().unwrap();
    let hash = hash(&serialized);
    let (proposal_key, _) =
        Pubkey::find_program_address(&[pda_tag::PROPOSAL, hash.as_ref()], &program_id);
    let mut accounts = vec![
        AccountMeta::new(signer, true),
        AccountMeta::new(group_account, false),
        AccountMeta::new(proposal_key, false),
        AccountMeta::new_readonly(proposed_instruction.program_id, false),
    ];
    accounts.extend(proposed_instruction.accounts.iter().map(|acc| {
        let is_signer = if acc.pubkey == protected_account {
            false
        } else {
            acc.is_signer
        };
        if acc.is_writable {
            AccountMeta::new(acc.pubkey, is_signer)
        } else {
            AccountMeta::new_readonly(acc.pubkey, is_signer)
        }
    }));
    let transaction = Transaction::new_with_payer(
        &[Instruction::new_with_borsh(program_id, &command, accounts)],
        Some(&payer),
    );
    (transaction, proposal_key)
}

#[tokio::test]
async fn propose_over_threshold() {
    let program_id = Pubkey::new_unique();
    let alice = Keypair::new();
    let alice_key = alice.pubkey();
    let bob_key = Pubkey::new_unique();
    let chris_key = Pubkey::new_unique();

    let mut program_test = ProgramTest::new(env!("CARGO_PKG_NAME"), program_id, None);
    program_test.prefer_bpf(true);
    program_test.add_program(env!("CARGO_PKG_NAME"), program_id, None);
    program_test.add_account(
        alice_key,
        Account {
            lamports: 1000,
            owner: SYSTEM_PROGRAM_ID,
            ..Account::default()
        },
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
    // init first
    let (mut transaction, group_account) = do_init(
        program_id,
        2,
        vec![(alice_key, 2), (bob_key, 1), (chris_key, 1)],
        payer.pubkey(),
        None,
    );
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    let (protected_account, _) = Pubkey::find_program_address(
        &[pda_tag::PROTECTED, &group_account.to_bytes()[..]],
        &program_id,
    );

    // now propose
    let proposed_instruction = system_instruction::create_account(
        &alice_key,
        &protected_account,
        10,
        0,
        &SYSTEM_PROGRAM_ID,
    );

    let (mut transaction, proposal_acc) = do_propose(
        program_id,
        alice_key,
        payer.pubkey(),
        group_account,
        proposed_instruction,
    );

    assert!(banks_client
        .get_account(proposal_acc)
        .await
        .unwrap()
        .is_none());
    transaction.sign(&[&payer, &alice], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
    // doesnt create proposal, executes immediately
    assert!(banks_client
        .get_account(proposal_acc)
        .await
        .unwrap()
        .is_none());
    assert!(banks_client
        .get_account(protected_account)
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn propose_not_over_threshold() {
    let program_id = Pubkey::new_unique();
    let alice_key = Pubkey::new_unique();
    let bob = Keypair::new();
    let bob_key = bob.pubkey();
    let chris_key = Pubkey::new_unique();
    let destination = Pubkey::new_unique();

    let mut program_test = ProgramTest::new(env!("CARGO_PKG_NAME"), program_id, None);
    program_test.prefer_bpf(true);
    program_test.add_program(env!("CARGO_PKG_NAME"), program_id, None);
    program_test.add_account(
        bob_key,
        Account {
            lamports: 1000,
            owner: SYSTEM_PROGRAM_ID,
            ..Account::default()
        },
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
    let users = vec![(alice_key, 2), (bob_key, 1), (chris_key, 1)];
    // init first
    let (mut transaction, group_account) =
        do_init(program_id, 2, users.clone(), payer.pubkey(), None);
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    let (protected_account, _) = Pubkey::find_program_address(
        &[pda_tag::PROTECTED, &group_account.to_bytes()[..]],
        &program_id,
    );

    // now propose
    let proposed_instruction = system_instruction::transfer(&protected_account, &destination, 100);

    let (mut transaction, proposal_acc) = do_propose(
        program_id,
        bob_key,
        payer.pubkey(),
        group_account,
        proposed_instruction.clone(),
    );

    assert!(banks_client
        .get_account(proposal_acc)
        .await
        .unwrap()
        .is_none());
    transaction.sign(&[&payer, &bob], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
    let proposal = banks_client
        .get_account(proposal_acc)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(proposal.data[0], u8::from(AccountType::Proposal));
    let proposal_data = ProposalData::try_from_slice(&proposal.data[1..]).unwrap();
    assert!(proposal_data
        .state
        .is_approved_by(users.iter().position(|(key, _)| *key == bob_key).unwrap()));

    assert!(banks_client
        .get_account(protected_account)
        .await
        .unwrap()
        .is_none());
}

async fn do_transfer(
    client: &mut solana_program_test::BanksClient,
    from: &Keypair,
    to: Pubkey,
    lamport: u64,
) {
    let instruction = system_instruction::transfer(&from.pubkey(), &to, lamport);
    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&from.pubkey()));
    let recent_blockhash = client.get_recent_blockhash().await.unwrap();

    transaction.sign(&[from], recent_blockhash);
    client.process_transaction(transaction).await.unwrap();
}

fn do_approve(
    program_id: Pubkey,
    signer: Pubkey,
    payer: Pubkey,
    proposal_data: ProposalData,
    proposal_acc: Pubkey,
    protected_account: Pubkey,
) -> Transaction {
    let group_acc = proposal_data.config.group;
    let command = crate::instruction::MultiSigInstruction::Approve(ApproveInstruction {});

    let mut accounts = vec![
        AccountMeta::new(signer, true),
        AccountMeta::new(group_acc, false),
        AccountMeta::new(proposal_acc, false),
        AccountMeta::new_readonly(proposal_data.config.instructions[0].program_id, false),
    ];
    accounts.extend(
        proposal_data.config.instructions[0]
            .accounts
            .iter()
            .map(|acc| {
                let is_signer = if acc.pubkey == protected_account {
                    false
                } else {
                    acc.is_signer
                };
                if acc.is_writable {
                    AccountMeta::new(acc.pubkey, is_signer)
                } else {
                    AccountMeta::new_readonly(acc.pubkey, is_signer)
                }
            }),
    );
    Transaction::new_with_payer(
        &[Instruction::new_with_borsh(program_id, &command, accounts)],
        Some(&payer),
    )
}

#[tokio::test]
async fn approve_over_threshold() {
    const PROPOSED_LAMPORTS: u64 = 50;

    let program_id = Pubkey::new_unique();
    let alice_key = Pubkey::new_unique();
    let bob = Keypair::new();
    let bob_key = bob.pubkey();
    let chris = Keypair::new();
    let chris_key = chris.pubkey();
    let destination_acc = Pubkey::new_unique();

    let mut program_test = ProgramTest::new(env!("CARGO_PKG_NAME"), program_id, None);
    program_test.prefer_bpf(true);
    program_test.add_program(env!("CARGO_PKG_NAME"), program_id, None);
    program_test.add_account(
        bob_key,
        Account {
            lamports: 1000,
            owner: SYSTEM_PROGRAM_ID,
            ..Account::default()
        },
    );
    program_test.add_account(
        chris_key,
        Account {
            lamports: 1000,
            owner: SYSTEM_PROGRAM_ID,
            ..Account::default()
        },
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
    // init first
    let (mut transaction, group_account) = do_init(
        program_id,
        2,
        vec![(alice_key, 2), (bob_key, 1), (chris_key, 1)],
        payer.pubkey(),
        None,
    );
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    let (protected_account, _) = Pubkey::find_program_address(
        &[pda_tag::PROTECTED, &group_account.to_bytes()[..]],
        &program_id,
    );

    // now propose
    let proposed_instruction =
        system_instruction::transfer(&protected_account, &destination_acc, PROPOSED_LAMPORTS);

    let (mut transaction, proposal_acc) = do_propose(
        program_id,
        bob_key,
        payer.pubkey(),
        group_account,
        proposed_instruction,
    );

    assert!(banks_client
        .get_account(proposal_acc)
        .await
        .unwrap()
        .is_none());
    assert!(banks_client
        .get_account(protected_account)
        .await
        .unwrap()
        .is_none());
    transaction.sign(&[&payer, &bob], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
    assert!(banks_client
        .get_account(proposal_acc)
        .await
        .unwrap()
        .is_some());
    assert!(banks_client
        .get_account(protected_account)
        .await
        .unwrap()
        .is_none());

    // transfer funds to protected acc
    do_transfer(&mut banks_client, &payer, protected_account, 100).await;

    // approve
    let proposal = banks_client
        .get_account(proposal_acc)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(proposal.data[0], u8::from(AccountType::Proposal));
    let proposal_data = ProposalData::try_from_slice(&proposal.data[1..]).unwrap();
    let mut transaction = do_approve(
        program_id,
        chris.pubkey(),
        payer.pubkey(),
        proposal_data,
        proposal_acc,
        protected_account,
    );
    transaction.sign(&[&chris, &payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    let destination = banks_client
        .get_account(destination_acc)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(destination.lamports, PROPOSED_LAMPORTS);
    let proposal = banks_client
        .get_account(proposal_acc)
        .await
        .unwrap()
        .unwrap();
    assert!(proposal.data.iter().all(|b| *b == 0));
}

#[tokio::test]
async fn approve_overflow() {
    const PROPOSED_LAMPORTS: u64 = 50;

    let program_id = Pubkey::new_unique();
    let alice_key = Pubkey::new_unique();
    let bob = Keypair::new();
    let bob_key = bob.pubkey();
    let chris = Keypair::new();
    let chris_key = chris.pubkey();
    let destination_acc = Pubkey::new_unique();

    let mut program_test = ProgramTest::new(env!("CARGO_PKG_NAME"), program_id, None);
    program_test.prefer_bpf(true);
    program_test.add_program(env!("CARGO_PKG_NAME"), program_id, None);
    program_test.add_account(
        bob_key,
        Account {
            lamports: 1000,
            owner: SYSTEM_PROGRAM_ID,
            ..Account::default()
        },
    );
    program_test.add_account(
        chris_key,
        Account {
            lamports: 1000,
            owner: SYSTEM_PROGRAM_ID,
            ..Account::default()
        },
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
    // init first
    let (mut transaction, group_account) = do_init(
        program_id,
        2,
        vec![(alice_key, 2), (bob_key, 1), (chris_key, u32::max_value())],
        payer.pubkey(),
        None,
    );
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    let (protected_account, _) = Pubkey::find_program_address(
        &[pda_tag::PROTECTED, &group_account.to_bytes()[..]],
        &program_id,
    );

    // now propose
    let proposed_instruction =
        system_instruction::transfer(&protected_account, &destination_acc, PROPOSED_LAMPORTS);

    let (mut transaction, proposal_acc) = do_propose(
        program_id,
        bob_key,
        payer.pubkey(),
        group_account,
        proposed_instruction,
    );

    assert!(banks_client
        .get_account(proposal_acc)
        .await
        .unwrap()
        .is_none());
    assert!(banks_client
        .get_account(protected_account)
        .await
        .unwrap()
        .is_none());
    transaction.sign(&[&payer, &bob], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
    assert!(banks_client
        .get_account(proposal_acc)
        .await
        .unwrap()
        .is_some());
    assert!(banks_client
        .get_account(protected_account)
        .await
        .unwrap()
        .is_none());

    // transfer funds to protected acc
    do_transfer(&mut banks_client, &payer, protected_account, 100).await;

    // approve
    let proposal = banks_client
        .get_account(proposal_acc)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(proposal.data[0], u8::from(AccountType::Proposal));
    let proposal_data = ProposalData::try_from_slice(&proposal.data[1..]).unwrap();
    let mut transaction = do_approve(
        program_id,
        chris.pubkey(),
        payer.pubkey(),
        proposal_data,
        proposal_acc,
        protected_account,
    );
    transaction.sign(&[&chris, &payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    let destination = banks_client
        .get_account(destination_acc)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(destination.lamports, PROPOSED_LAMPORTS);
}

#[tokio::test]
async fn approve_twice_fails() {
    const PROPOSED_LAMPORTS: u64 = 50;

    let program_id = Pubkey::new_unique();
    let alice_key = Pubkey::new_unique();
    let bob = Keypair::new();
    let bob_key = bob.pubkey();
    let chris = Keypair::new();
    let chris_key = chris.pubkey();
    let destination_acc = Pubkey::new_unique();

    let mut program_test = ProgramTest::new(env!("CARGO_PKG_NAME"), program_id, None);
    program_test.prefer_bpf(true);
    program_test.add_program(env!("CARGO_PKG_NAME"), program_id, None);
    program_test.add_account(
        bob_key,
        Account {
            lamports: 1000,
            owner: SYSTEM_PROGRAM_ID,
            ..Account::default()
        },
    );
    program_test.add_account(
        chris_key,
        Account {
            lamports: 1000,
            owner: SYSTEM_PROGRAM_ID,
            ..Account::default()
        },
    );

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;
    // init first
    let (mut transaction, group_account) = do_init(
        program_id,
        2,
        vec![(alice_key, 2), (bob_key, 1), (chris_key, 1)],
        payer.pubkey(),
        None,
    );
    transaction.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    let (protected_account, _) = Pubkey::find_program_address(
        &[pda_tag::PROTECTED, &group_account.to_bytes()[..]],
        &program_id,
    );

    // now propose
    let proposed_instruction =
        system_instruction::transfer(&protected_account, &destination_acc, PROPOSED_LAMPORTS);

    let (mut transaction, proposal_acc) = do_propose(
        program_id,
        bob_key,
        payer.pubkey(),
        group_account,
        proposed_instruction,
    );

    assert!(banks_client
        .get_account(proposal_acc)
        .await
        .unwrap()
        .is_none());
    assert!(banks_client
        .get_account(protected_account)
        .await
        .unwrap()
        .is_none());
    transaction.sign(&[&payer, &bob], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();
    assert!(banks_client
        .get_account(proposal_acc)
        .await
        .unwrap()
        .is_some());
    assert!(banks_client
        .get_account(protected_account)
        .await
        .unwrap()
        .is_none());

    // transfer funds to protected acc
    do_transfer(&mut banks_client, &payer, protected_account, 100).await;

    // approve
    let proposal = banks_client
        .get_account(proposal_acc)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(proposal.data[0], u8::from(AccountType::Proposal));
    let proposal_data = ProposalData::try_from_slice(&proposal.data[1..]).unwrap();
    let mut transaction = do_approve(
        program_id,
        bob.pubkey(),
        payer.pubkey(),
        proposal_data,
        proposal_acc,
        protected_account,
    );
    transaction.sign(&[&bob, &payer], recent_blockhash);
    assert!(banks_client.process_transaction(transaction).await.is_err());
}
