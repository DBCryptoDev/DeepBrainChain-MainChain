use super::super::mock::{TerminatingRental as IRMachine, *};
use crate::{
    tests::test_verify_online::new_test_with_machine_bonding_ext, RentOrderDetail, RentStatus,
};
// use committee::CommitteeStakeInfo;
use dbc_support::{
    live_machine::LiveMachine,
    machine_type::{CommitteeUploadInfo, MachineStatus},
    rental_type::MachineGPUOrder,
    report::{
        MTCommitteeOpsDetail, MTCommitteeOrderList, MTLiveReportList, MTOrderStatus,
        MTReportInfoDetail, MachineFaultType, ReportStatus, ReporterReportList, ReporterStakeInfo,
    },
    verify_online::StashMachine,
    BoxPubkey, ReportHash,
};
use frame_support::assert_ok;
use sp_runtime::Perbill;
use std::convert::TryInto;

pub fn new_test_with_machine_online_ext() -> sp_io::TestExternalities {
    let mut ext = new_test_with_machine_bonding_ext();
    ext.execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        let committee1 = sr25519::Public::from(Sr25519Keyring::Alice);
        let committee2 = sr25519::Public::from(Sr25519Keyring::Charlie);
        let _committee3 = sr25519::Public::from(Sr25519Keyring::Dave);
        let committee4 = sr25519::Public::from(Sr25519Keyring::Eve);

        // 委员会添加机器Hash
        let hash1: [u8; 16] =
            hex::decode("53cf058dfa07ef517b2f28bccff88c2b").unwrap().try_into().unwrap();
        let hash2: [u8; 16] =
            hex::decode("3f775d3f4a144b94d6d551f6091a5126").unwrap().try_into().unwrap();
        let hash3: [u8; 16] =
            hex::decode("4983040157403addac94ca860ddbff7f").unwrap().try_into().unwrap();

        run_to_block(3);

        assert_ok!(IRMachine::submit_confirm_hash(
            Origin::signed(committee1),
            machine_id.clone(),
            hash1
        ));
        assert_ok!(IRMachine::submit_confirm_hash(
            Origin::signed(committee2),
            machine_id.clone(),
            hash2
        ));
        assert_ok!(IRMachine::submit_confirm_hash(
            Origin::signed(committee4),
            machine_id.clone(),
            hash3
        ));

        // 委员会提交原始信息
        let mut upload_info = CommitteeUploadInfo {
            machine_id: machine_id.clone(),
            gpu_type: "GeForceRTX3080".as_bytes().to_vec(),
            gpu_num: 8,
            cuda_core: 8704,
            gpu_mem: 10,
            calc_point: 119780,
            sys_disk: 500,
            data_disk: 3905,
            cpu_type: "Intel(R) Xeon(R) Silver 4214R".as_bytes().to_vec(),
            cpu_core_num: 46,
            cpu_rate: 2400,
            mem_num: 440,

            rand_str: "abcdefg1".as_bytes().to_vec(),
            is_support: true,
        };

        // 委员会添加机器原始值
        assert_ok!(IRMachine::submit_confirm_raw(Origin::signed(committee1), upload_info.clone()));

        upload_info.rand_str = "abcdefg2".as_bytes().to_vec();
        assert_ok!(IRMachine::submit_confirm_raw(Origin::signed(committee2), upload_info.clone()));
        upload_info.rand_str = "abcdefg3".as_bytes().to_vec();
        assert_ok!(IRMachine::submit_confirm_raw(Origin::signed(committee4), upload_info));

        run_to_block(4);
    });
    ext
}

#[test]
fn rent_machine_works() {
    new_test_with_machine_online_ext().execute_with(|| {
        let committee1 = sr25519::Public::from(Sr25519Keyring::Alice);
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let _controller = sr25519::Public::from(Sr25519Keyring::Eve);
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        // 用户租用
        let renter1 = sr25519::Public::from(Sr25519Keyring::Bob);
        // let renter2 = sr25519::Public::from(Sr25519Keyring::Bob);
        assert_ok!(IRMachine::rent_machine(Origin::signed(renter1), machine_id.clone(), 8, 60));
        {
            // - Writes: MachineRentOrder, RentOrder, machine_status, UserRented, PendingRentEnding,
            // PendingConfirming, RenterTotalStake, FreeBalance(少10DBC)
            assert_eq!(IRMachine::user_rented(renter1), vec![0]);
            assert_eq!(
                IRMachine::rent_order(0),
                crate::RentOrderDetail {
                    machine_id: machine_id.clone(),
                    renter: renter1,
                    rent_start: 5,
                    confirm_rent: 0,
                    rent_end: 5 + 60,
                    // 租金: 119780 / 1000 * 5000000 / 12000 * (60 / 2880)
                    stake_amount: 1039756916666666666,
                    rent_status: crate::RentStatus::WaitingVerifying,
                    gpu_num: 8,
                    gpu_index: vec![0, 1, 2, 3, 4, 5, 6, 7]
                }
            );
            assert_eq!(IRMachine::pending_rent_ending(5 + 60), vec![0]);
            assert_eq!(IRMachine::pending_confirming(5 + 30), vec![0]);
            assert_eq!(IRMachine::renter_total_stake(renter1), 1039756916666666666);
            assert_eq!(
                IRMachine::machine_rent_order(&machine_id),
                MachineGPUOrder { rent_order: vec![0], used_gpu: vec![0, 1, 2, 3, 4, 5, 6, 7] }
            );

            let machine_info = IRMachine::machines_info(&machine_id);
            assert_eq!(machine_info.machine_status, MachineStatus::Rented);

            assert_eq!(Balances::reserved_balance(renter1), 1039756916666666666);
            assert_eq!(
                Balances::free_balance(renter1),
                INIT_BALANCE - 10 * ONE_DBC - (1039756916666666666)
            );

            // committee1
            assert_eq!(Balances::reserved_balance(committee1), 20000 * ONE_DBC);
            assert_eq!(Balances::free_balance(committee1), INIT_BALANCE - 20000 * ONE_DBC);
        }

        assert_ok!(IRMachine::confirm_rent(Origin::signed(renter1), 0));
        {
            // - Writes: PendingConfirming, RentOrder, LiveMachine, MachineInfo, StashMachine

            // - Writes: PendingConfirming,
            assert_eq!(
                IRMachine::live_machines(),
                LiveMachine { rented_machine: vec![machine_id.clone()], ..Default::default() }
            );
            let machine_info = IRMachine::machines_info(&machine_id);
            assert_eq!(machine_info.total_rented_times, 1);
            assert_eq!(machine_info.renters, vec![renter1]);

            assert_eq!(
                IRMachine::stash_machines(stash),
                StashMachine {
                    total_machine: vec![machine_id.clone()],
                    online_machine: vec![machine_id.clone()],
                    total_calc_points: 119780,
                    total_rented_gpu: 8,
                    total_gpu_num: 8,
                    total_rent_fee: 0,
                    ..Default::default()
                }
            );

            // TODO: 当为空时，删除
            // assert_eq!(<crate::PendingConfirming<TestRuntime>>::contains_key(35), false);

            assert_eq!(
                IRMachine::rent_order(0),
                RentOrderDetail {
                    machine_id: machine_id.clone(),
                    renter: renter1,
                    rent_start: 5,
                    confirm_rent: 5,
                    rent_end: 65,
                    stake_amount: 1039756916666666666,
                    rent_status: RentStatus::Renting,
                    gpu_num: 8,
                    gpu_index: vec![0, 1, 2, 3, 4, 5, 6, 7],
                }
            );

            assert_eq!(Balances::reserved_balance(renter1), 1039756916666666666);
            assert_eq!(
                Balances::free_balance(renter1),
                INIT_BALANCE - (1039756916666666666 + 10 * ONE_DBC)
            );

            assert_eq!(Balances::reserved_balance(stash), 0);
            assert_eq!(Balances::free_balance(stash), INIT_BALANCE);

            assert_eq!(Balances::reserved_balance(committee1), 20000 * ONE_DBC);
            assert_eq!(Balances::free_balance(committee1), INIT_BALANCE - 20000 * ONE_DBC);
        }

        run_to_block(100);
        {
            // 结束租用: 将租金99%转给stash,1%转给几个委员会

            let rent_fee = 1039756916666666666;

            let reward_to_stash = Perbill::from_rational_approximation(99u32, 100u32) * rent_fee;
            let committee_each_get =
                Perbill::from_rational_approximation(1u32, 3u32) * (rent_fee - reward_to_stash);
            let stash_get = rent_fee - committee_each_get * 3;
            assert_eq!(
                Balances::free_balance(committee1),
                INIT_BALANCE - 20000 * ONE_DBC + committee_each_get
            );
            assert_eq!(Balances::reserved_balance(committee1), 20000 * ONE_DBC);

            // - Writes: MachineRentedGPU, LiveMachines, MachinesInfo, StashMachine
            assert_eq!(Balances::reserved_balance(renter1), 0);
            assert_eq!(
                Balances::free_balance(renter1),
                INIT_BALANCE - 1039756916666666666 - 10 * ONE_DBC
            );

            // 租金被质押
            assert_eq!(Balances::free_balance(stash), INIT_BALANCE);
            assert_eq!(Balances::reserved_balance(stash), stash_get);
        }
        // 这时候质押的金额应该转给stash账户,
        // 如果stash的押金够则转到stash的free，否则转到staked
    })
}

// 用户下线，将按照使用时长付租金
#[test]
fn machine_offline_works() {
    new_test_with_machine_online_ext().execute_with(|| {
        // 用户租用
        let controller = sr25519::Public::from(Sr25519Keyring::Eve);
        let renter1 = sr25519::Public::from(Sr25519Keyring::Bob);
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        // let renter2 = sr25519::Public::from(Sr25519Keyring::Bob);
        assert_ok!(IRMachine::rent_machine(Origin::signed(renter1), machine_id.clone(), 8, 360));
        assert_ok!(IRMachine::confirm_rent(Origin::signed(renter1), 0));

        // NOTE: 使用了130个块（将按一小时收费）
        run_to_block(4 + 130);

        {
            assert_eq!(
                IRMachine::machine_rent_order(&machine_id),
                MachineGPUOrder { rent_order: vec![0], used_gpu: vec![0, 1, 2, 3, 4, 5, 6, 7] }
            );

            assert!(<crate::RentOrder<TestRuntime>>::contains_key(0));
            assert!(<crate::MachineRentOrder<TestRuntime>>::contains_key(&machine_id));
            assert_eq!(
                IRMachine::rent_order(0),
                RentOrderDetail {
                    machine_id: machine_id.clone(),
                    renter: renter1,
                    rent_start: 5,
                    confirm_rent: 5,
                    rent_end: 365,
                    stake_amount: 6238541666666666666,
                    rent_status: RentStatus::Renting,
                    gpu_num: 8,
                    gpu_index: vec![0, 1, 2, 3, 4, 5, 6, 7],
                }
            );
        }
        assert_ok!(IRMachine::machine_offline(Origin::signed(controller), machine_id.clone()));

        // - Write: MachineInfo, OfflineMachines,
        // - Delte: MachineRentOrder, RentOrder
        {
            let machine_info = IRMachine::machines_info(&machine_id);
            assert_eq!(
                machine_info.machine_status,
                MachineStatus::StakerReportOffline(
                    135,
                    Box::new(MachineStatus::AddingCustomizeInfo)
                )
            );
            assert_eq!(IRMachine::offline_machines(135 + 28800), vec![machine_id.clone()]);
            assert!(!<crate::RentOrder<TestRuntime>>::contains_key(0));
            assert!(!<crate::MachineRentOrder<TestRuntime>>::contains_key(&machine_id));
            // 租金： 6238541666666666666 / 3 = 2079513888888888888

            assert_eq!(Balances::free_balance(renter1), 9997910486113190625000);
            assert_eq!(Balances::reserved_balance(renter1), 0);
            assert_eq!(machine_info.stake_amount, 2058718747962076389);
            assert_eq!(Balances::reserved_balance(stash), 2058718747962076389);
        }
    })
}

// TODO: 增加机器有租金的情况
// 机器下线超过10天，将惩罚掉质押币(不允许申诉)
#[test]
fn machine_offline_10more_days_slash_works() {
    new_test_with_machine_online_ext().execute_with(|| {
        let controller = sr25519::Public::from(Sr25519Keyring::Eve);
        let _renter1 = sr25519::Public::from(Sr25519Keyring::Bob);
        let _stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        assert!(!<crate::OfflineMachines<TestRuntime>>::contains_key(&5 + 28800));
        assert_ok!(IRMachine::machine_offline(Origin::signed(controller), machine_id.clone()));
        {
            assert_eq!(IRMachine::offline_machines(5 + 28800), vec![machine_id.clone()])
        };

        run_to_block(6 + 28800);
        {
            assert!(!<crate::OfflineMachines<TestRuntime>>::contains_key(&5 + 28800));
            let machine_info = IRMachine::machines_info(&machine_id);
            assert_eq!(machine_info.stake_amount, 0);
        }
    })
}

// 机器在线无法使用被举报
#[test]
fn machine_online_inaccessible_slash_works() {
    new_test_with_machine_online_ext().execute_with(|| {
        let _controller = sr25519::Public::from(Sr25519Keyring::Eve);
        let renter1 = sr25519::Public::from(Sr25519Keyring::Bob);
        let _stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let _machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        assert_ok!(IRMachine::report_machine_fault(
            Origin::signed(renter1),
            ReportHash::default(),
            BoxPubkey::default(),
        ));

        {
            assert_eq!(
                IRMachine::live_report(),
                MTLiveReportList { bookable_report: vec![0], ..Default::default() }
            );
            assert_eq!(
                IRMachine::reporter_report(&renter1),
                ReporterReportList { processing_report: vec![0], ..Default::default() }
            );
            // assert_eq!(IRMachine::reporter_stake_params(), Default::default());
            assert_eq!(
                IRMachine::reporter_stake(&renter1),
                ReporterStakeInfo {
                    staked_amount: 20000 * ONE_DBC,
                    used_stake: 1000 * ONE_DBC,
                    ..Default::default()
                }
            );
            assert_eq!(
                IRMachine::report_info(0),
                MTReportInfoDetail {
                    reporter: renter1,
                    report_time: 5,
                    reporter_stake: 1000 * ONE_DBC,
                    machine_fault_type: MachineFaultType::RentedHardwareCounterfeit(
                        Default::default(),
                        Default::default()
                    ),
                    ..Default::default()
                }
            );
        }

        let committee1 = sr25519::Public::from(Sr25519Keyring::Alice);
        let _committee2 = sr25519::Public::from(Sr25519Keyring::Charlie);
        let _committee3 = sr25519::Public::from(Sr25519Keyring::Dave);
        let _committee4 = sr25519::Public::from(Sr25519Keyring::Eve);

        assert_ok!(IRMachine::committee_book_report(Origin::signed(committee1), 0));
        {
            assert_eq!(
                IRMachine::report_info(0),
                MTReportInfoDetail {
                    reporter: renter1,
                    report_time: 5,
                    reporter_stake: 1000 * ONE_DBC,
                    first_book_time: 5,
                    report_status: ReportStatus::Verifying,
                    verifying_committee: Some(committee1),
                    booked_committee: vec![committee1],
                    confirm_start: 365,
                    machine_fault_type: MachineFaultType::RentedHardwareCounterfeit(
                        Default::default(),
                        Default::default()
                    ),
                    ..Default::default()
                }
            );
            assert_eq!(
                IRMachine::committee_report_order(committee1),
                MTCommitteeOrderList { booked_report: vec![0], ..Default::default() }
            );
            assert_eq!(
                IRMachine::committee_report_ops(committee1, 0),
                MTCommitteeOpsDetail {
                    booked_time: 5,
                    staked_balance: 1000 * ONE_DBC,
                    order_status: MTOrderStatus::WaitingEncrypt,
                    ..Default::default()
                }
            );
            assert_eq!(
                IRMachine::live_report(),
                MTLiveReportList { verifying_report: vec![0], ..Default::default() }
            );
        }
        assert_ok!(IRMachine::reporter_add_encrypted_error_info(
            Origin::signed(renter1),
            0,
            committee1,
            vec![]
        ));
        {
            assert_eq!(
                IRMachine::report_info(0),
                MTReportInfoDetail {
                    reporter: renter1,
                    report_time: 5,
                    reporter_stake: 1000 * ONE_DBC,
                    first_book_time: 5,
                    report_status: ReportStatus::Verifying,
                    verifying_committee: Some(committee1),
                    booked_committee: vec![committee1],
                    get_encrypted_info_committee: vec![committee1],
                    confirm_start: 365,
                    machine_fault_type: MachineFaultType::RentedHardwareCounterfeit(
                        Default::default(),
                        Default::default()
                    ),

                    ..Default::default()
                }
            );
            assert_eq!(
                IRMachine::committee_report_ops(committee1, 0),
                MTCommitteeOpsDetail {
                    booked_time: 5,
                    encrypted_err_info: Some(vec![]),
                    encrypted_time: 5,
                    staked_balance: 1000 * ONE_DBC,
                    order_status: MTOrderStatus::Verifying,
                    ..Default::default()
                }
            );
        }
        run_to_block(366);
        // assert_ok!(IRMachine::committee_submit_verify_hash(
        //     Origin::signed(committee1),
        //     1,
        //     ReportHash::default()
        // ));
        // assert_ok!(IRMachine::committee_submit_verify_raw(
        //     origin,
        //     report_id,
        //     machine_id,
        //     rent_order_id,
        //     reporter_rand_str,
        //     committee_rand_str,
        //     err_reason,
        //     extra_err_info,
        //     support_report
        // ));
    })
}
